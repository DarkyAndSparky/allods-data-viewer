//! Ordinary picture files: `.ico`, `.cur`, `.tga`, `.dds` and the dimensions of JPEG, GIF and BMP.
//!
//! The game ships a few pictures outside its own texture containers (the launcher icon,
//! UI textures as DDS). They are decoded to RGBA here so that the viewer can show them as PNG;
//! JPEG, GIF and BMP are shown by the web view itself and only measured here.

use crate::error::{Error, Result};
use crate::texture::{Dxt, decode_dxt};

/// Largest picture (in pixels) that is decoded.
const MAX_PIXELS: u64 = 16 * 1024 * 1024;

/// A decoded picture.
pub enum Picture {
    /// Straight RGBA pixels, row by row from the top.
    Rgba {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// `width * height * 4` bytes.
        pixels: Vec<u8>,
    },
    /// A PNG file stored inside the container (Vista style icons).
    Png(Vec<u8>),
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u16_be(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        d.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn pixel_count(w: u32, h: u32) -> Result<usize> {
    let n = u64::from(w) * u64::from(h);
    if w == 0 || h == 0 || n > MAX_PIXELS {
        return Err(Error::limit(format!(
            "picture size {w}x{h} is not supported"
        )));
    }
    usize::try_from(n).map_err(|_| Error::limit("picture is too large"))
}

/// Decodes the largest image of a Windows icon (`.ico`) or cursor (`.cur`) file. A cursor has
/// the same layout; only the type word and the meaning of two directory fields (hot spot
/// instead of planes and bit depth) differ.
///
/// # Errors
///
/// [`Error::Format`] for a damaged or unsupported icon, [`Error::Limit`] for huge sizes.
pub fn decode_ico(d: &[u8]) -> Result<Picture> {
    let kind = u16_at(d, 2);
    if u16_at(d, 0) != Some(0) || !matches!(kind, Some(1 | 2)) {
        return Err(Error::format("not an icon or cursor file"));
    }
    let cursor = kind == Some(2);
    let count = usize::from(u16_at(d, 4).ok_or_else(|| Error::format("icon header is cut off"))?);
    if count == 0 || count > 256 {
        return Err(Error::format("icon image count is out of range"));
    }
    // Choose the entry with the most pixels, then the most bits.
    let mut best: Option<(u32, u16, usize, usize)> = None;
    for i in 0..count {
        let e = 6 + i * 16;
        let w = u32::from(
            *d.get(e)
                .ok_or_else(|| Error::format("icon directory is cut off"))?,
        );
        let h = u32::from(
            *d.get(e + 1)
                .ok_or_else(|| Error::format("icon directory is cut off"))?,
        );
        let (w, h) = (if w == 0 { 256 } else { w }, if h == 0 { 256 } else { h });
        let bits = if cursor {
            0
        } else {
            u16_at(d, e + 6).unwrap_or(0)
        };
        let size =
            u32_at(d, e + 8).ok_or_else(|| Error::format("icon directory is cut off"))? as usize;
        let off =
            u32_at(d, e + 12).ok_or_else(|| Error::format("icon directory is cut off"))? as usize;
        let key = w * h;
        if best.is_none_or(|(k, b, _, _)| (key, bits) > (k, b)) {
            best = Some((key, bits, off, size));
        }
    }
    let (_, _, off, size) = best.ok_or_else(|| Error::format("icon has no images"))?;
    let end = off
        .checked_add(size)
        .filter(|&e| e <= d.len())
        .ok_or_else(|| Error::format("icon image is outside the file"))?;
    let img = &d[off..end];
    if img.starts_with(b"\x89PNG") {
        return Ok(Picture::Png(img.to_vec()));
    }
    decode_dib(img)
}

/// Reads one colour of a TGA pixel or palette entry (`bits` is 8 for gray, 15, 16, 24 or 32).
/// `alpha_bits` is the attribute bit count from the image descriptor; zero means opaque.
fn tga_color(px: &[u8], bits: u8, gray: bool, alpha_bits: bool) -> Option<[u8; 4]> {
    let expand = |v: u16| u8::try_from((v << 3) | (v >> 2)).unwrap_or(255);
    Some(match (bits, gray) {
        (8, true) => {
            let g = *px.first()?;
            [g, g, g, 255]
        }
        (16, true) => {
            let g = *px.first()?;
            [g, g, g, if alpha_bits { *px.get(1)? } else { 255 }]
        }
        (15 | 16, false) => {
            let v = u16::from_le_bytes([*px.first()?, *px.get(1)?]);
            let a = if alpha_bits && v & 0x8000 == 0 {
                0
            } else {
                255
            };
            [
                expand((v >> 10) & 31),
                expand((v >> 5) & 31),
                expand(v & 31),
                a,
            ]
        }
        (24, false) => [*px.get(2)?, *px.get(1)?, *px.first()?, 255],
        (32, false) => [
            *px.get(2)?,
            *px.get(1)?,
            *px.first()?,
            if alpha_bits { *px.get(3)? } else { 255 },
        ],
        _ => return None,
    })
}

/// Decodes a Truevision TGA file: colour-mapped, true colour and gray images, plain or
/// run-length encoded, 8 to 32 bits per pixel, any origin.
///
/// # Errors
///
/// [`Error::Format`] for a damaged or unsupported file, [`Error::Limit`] for huge sizes.
pub fn decode_tga(d: &[u8]) -> Result<Picture> {
    let bad = |m: &str| Error::format(format!("TGA: {m}"));
    let head = d.get(..18).ok_or_else(|| bad("header is cut off"))?;
    let (id_len, cmap_type, img_type) = (usize::from(head[0]), head[1], head[2]);
    let cmap_first = usize::from(u16::from_le_bytes([head[3], head[4]]));
    let cmap_len = usize::from(u16::from_le_bytes([head[5], head[6]]));
    let cmap_bits = head[7];
    let w = u32::from(u16::from_le_bytes([head[12], head[13]]));
    let h = u32::from(u16::from_le_bytes([head[14], head[15]]));
    let bpp = head[16];
    let desc = head[17];
    let alpha_bits = desc & 0x0f != 0;
    let rle = matches!(img_type, 9..=11);
    let base = img_type & 3; // 1 mapped, 2 true colour, 3 gray
    if !matches!(img_type, 1..=3 | 9..=11) {
        return Err(bad("unsupported image type"));
    }
    let n = pixel_count(w, h)?;
    let gray = base == 3;
    let mapped = base == 1;
    let step = match (bpp, mapped, gray) {
        (8, _, _) | (16, _, _) | (24, false, false) | (32, false, false) => usize::from(bpp / 8),
        (15, false, false) => 2,
        _ => return Err(bad("unsupported pixel size")),
    };
    let mut pos = 18usize
        .checked_add(id_len)
        .ok_or_else(|| bad("header is cut off"))?;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    if cmap_type == 1 {
        if !matches!(cmap_bits, 15 | 16 | 24 | 32) {
            return Err(bad("unsupported palette entry size"));
        }
        let entry = usize::from(cmap_bits.div_ceil(8));
        let size = cmap_len
            .checked_mul(entry)
            .ok_or_else(|| bad("palette is too large"))?;
        let raw = d
            .get(
                pos..pos
                    .checked_add(size)
                    .ok_or_else(|| bad("palette is cut off"))?,
            )
            .ok_or_else(|| bad("palette is cut off"))?;
        for e in raw.chunks_exact(entry) {
            palette.push(tga_color(e, cmap_bits, false, alpha_bits).ok_or_else(|| bad("palette"))?);
        }
        pos += size;
    } else if mapped {
        return Err(bad("colour-mapped image without a palette"));
    }
    let data = d.get(pos..).ok_or_else(|| bad("pixel data is cut off"))?;
    let pixel = |px: &[u8]| -> Result<[u8; 4]> {
        if mapped {
            let idx = if step == 1 {
                usize::from(*px.first().ok_or_else(|| bad("pixel data is cut off"))?)
            } else {
                usize::from(u16::from_le_bytes([
                    *px.first().ok_or_else(|| bad("pixel data is cut off"))?,
                    *px.get(1).ok_or_else(|| bad("pixel data is cut off"))?,
                ]))
            };
            palette
                .get(idx.wrapping_sub(cmap_first))
                .copied()
                .ok_or_else(|| bad("palette index is out of range"))
        } else {
            tga_color(px, bpp, gray, alpha_bits).ok_or_else(|| bad("pixel data is cut off"))
        }
    };
    let mut rgba = Vec::with_capacity(n * 4);
    if rle {
        let mut at = 0usize;
        while rgba.len() < n * 4 {
            let packet = *data.get(at).ok_or_else(|| bad("pixel data is cut off"))?;
            at += 1;
            let count = usize::from(packet & 0x7f) + 1;
            if packet & 0x80 != 0 {
                let px = data
                    .get(at..at + step)
                    .ok_or_else(|| bad("pixel data is cut off"))?;
                at += step;
                let c = pixel(px)?;
                for _ in 0..count {
                    if rgba.len() < n * 4 {
                        rgba.extend_from_slice(&c);
                    }
                }
            } else {
                for _ in 0..count {
                    let px = data
                        .get(at..at + step)
                        .ok_or_else(|| bad("pixel data is cut off"))?;
                    at += step;
                    if rgba.len() < n * 4 {
                        rgba.extend_from_slice(&pixel(px)?);
                    }
                }
            }
        }
    } else {
        let need = n
            .checked_mul(step)
            .ok_or_else(|| bad("image is too large"))?;
        let raw = data
            .get(..need)
            .ok_or_else(|| bad("pixel data is cut off"))?;
        for px in raw.chunks_exact(step) {
            rgba.extend_from_slice(&pixel(px)?);
        }
    }
    let (wu, hu) = (w as usize, h as usize);
    let top_first = desc & 0x20 != 0;
    let right_first = desc & 0x10 != 0;
    let mut out = vec![0u8; n * 4];
    for y in 0..hu {
        let sy = if top_first { y } else { hu - 1 - y };
        for x in 0..wu {
            let sx = if right_first { wu - 1 - x } else { x };
            let from = (sy * wu + sx) * 4;
            let to = (y * wu + x) * 4;
            if let (Some(src), Some(dst)) = (rgba.get(from..from + 4), out.get_mut(to..to + 4)) {
                dst.copy_from_slice(src);
            }
        }
    }
    Ok(Picture::Rgba {
        width: w,
        height: h,
        pixels: out,
    })
}

/// Decodes a bitmap stored as an icon image: a BITMAPINFOHEADER, a palette, the colour rows
/// (bottom row first) and a 1 bit transparency mask.
fn decode_dib(d: &[u8]) -> Result<Picture> {
    let bad = || Error::format("icon bitmap is damaged");
    if u32_at(d, 0) != Some(40) {
        return Err(Error::format("unsupported icon bitmap header"));
    }
    let w = u32_at(d, 4).ok_or_else(bad)?;
    let h2 = u32_at(d, 8).ok_or_else(bad)?;
    let bpp = u32::from(u16_at(d, 14).ok_or_else(bad)?);
    let compression = u32_at(d, 16).ok_or_else(bad)?;
    let used = u32_at(d, 32).ok_or_else(bad)? as usize;
    let h = h2 / 2;
    if compression != 0 || !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return Err(Error::format("unsupported icon bitmap format"));
    }
    let n = pixel_count(w, h)?;
    let (w, h) = (w as usize, h as usize);
    let palette_len = if bpp <= 8 {
        if used == 0 {
            1usize << bpp
        } else {
            used.min(1usize << bpp)
        }
    } else {
        0
    };
    let palette_at = 40usize;
    let rows_at = palette_at + palette_len * 4;
    let row_bytes = (w * bpp as usize).div_ceil(32) * 4;
    let mask_at = rows_at
        .checked_add(row_bytes.checked_mul(h).ok_or_else(bad)?)
        .ok_or_else(bad)?;
    let mask_row = w.div_ceil(32) * 4;
    let palette = d.get(palette_at..rows_at).ok_or_else(bad)?;
    let rows = d.get(rows_at..mask_at).ok_or_else(bad)?;
    // Some writers leave the mask out; treat that as fully opaque.
    let mask = d.get(mask_at..mask_at.saturating_add(mask_row * h));
    let mut out = vec![0u8; n * 4];
    let mut any_alpha = false;
    for y in 0..h {
        let src = &rows[(h - 1 - y) * row_bytes..][..row_bytes];
        for x in 0..w {
            let (b, g, r, a) = match bpp {
                32 => (src[x * 4], src[x * 4 + 1], src[x * 4 + 2], src[x * 4 + 3]),
                24 => (src[x * 3], src[x * 3 + 1], src[x * 3 + 2], 255),
                _ => {
                    let per = 8 / bpp as usize;
                    let byte = src[x / per];
                    let shift = 8 - bpp as usize * (x % per + 1);
                    let idx = usize::from(byte >> shift) & ((1 << bpp) - 1);
                    let p = palette.get(idx * 4..idx * 4 + 4).unwrap_or(&[0, 0, 0, 0]);
                    (p[0], p[1], p[2], 255)
                }
            };
            any_alpha |= a != 0 && a != 255;
            let o = (y * w + x) * 4;
            out[o..o + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    let alpha_rich = bpp == 32 && out.chunks(4).any(|p| p[3] != 0) && any_alpha;
    if !alpha_rich {
        if let Some(mask) = mask {
            for y in 0..h {
                let row = &mask[(h - 1 - y) * mask_row..][..mask_row];
                for x in 0..w {
                    let hidden = row[x / 8] >> (7 - x % 8) & 1 == 1;
                    let o = (y * w + x) * 4 + 3;
                    out[o] = if hidden { 0 } else { 255 };
                }
            }
        } else if bpp == 32 {
            for p in out.chunks_mut(4) {
                p[3] = 255;
            }
        }
    }
    Ok(Picture::Rgba {
        width: w as u32,
        height: h as u32,
        pixels: out,
    })
}

/// Decodes the top level of a DDS file (DXT1, DXT3, DXT5 or 24/32 bit uncompressed).
///
/// # Errors
///
/// [`Error::Format`] for a damaged or unsupported file.
pub fn decode_dds(d: &[u8]) -> Result<Picture> {
    let bad = || Error::format("DDS header is damaged");
    if d.get(..4) != Some(b"DDS ") || u32_at(d, 4) != Some(124) {
        return Err(Error::format("not a DDS file"));
    }
    let h = u32_at(d, 12).ok_or_else(bad)?;
    let w = u32_at(d, 16).ok_or_else(bad)?;
    let pf_flags = u32_at(d, 80).ok_or_else(bad)?;
    let fourcc = d.get(84..88).ok_or_else(bad)?;
    let n = pixel_count(w, h)?;
    let body = d.get(128..).ok_or_else(bad)?;
    if pf_flags & 4 != 0 {
        let fmt = match fourcc {
            b"DXT1" => Dxt::Dxt1,
            b"DXT3" => Dxt::Dxt3,
            b"DXT5" => Dxt::Dxt5,
            _ => return Err(Error::format("unsupported DDS compression")),
        };
        let blocks = (w as usize).div_ceil(4) * (h as usize).div_ceil(4);
        let need = blocks * fmt.block_bytes();
        let top = body
            .get(..need)
            .ok_or_else(|| Error::format("DDS pixel data is cut off"))?;
        let pixels = decode_dxt(top, w, h, fmt)?;
        return Ok(Picture::Rgba {
            width: w,
            height: h,
            pixels,
        });
    }
    let bits = u32_at(d, 88).ok_or_else(bad)?;
    if pf_flags & 0x40 == 0 || !matches!(bits, 24 | 32) {
        return Err(Error::format("unsupported DDS pixel format"));
    }
    let masks = [92, 96, 100, 104].map(|o| u32_at(d, o).unwrap_or(0));
    let bytes = bits as usize / 8;
    let need = n.checked_mul(bytes).ok_or_else(bad)?;
    let src = body
        .get(..need)
        .ok_or_else(|| Error::format("DDS pixel data is cut off"))?;
    let take = |px: u32, mask: u32| -> u8 {
        if mask == 0 {
            return 255;
        }
        let shift = mask.trailing_zeros();
        let max = mask >> shift;
        // Scale the channel to 8 bits.
        u8::try_from(((px & mask) >> shift) * 255 / max).unwrap_or(255)
    };
    let mut out = Vec::with_capacity(n * 4);
    for p in src.chunks_exact(bytes) {
        let mut raw = [0u8; 4];
        raw[..bytes].copy_from_slice(p);
        let px = u32::from_le_bytes(raw);
        let a = if pf_flags & 1 != 0 {
            take(px, masks[3])
        } else {
            255
        };
        out.extend_from_slice(&[
            take(px, masks[0]),
            take(px, masks[1]),
            take(px, masks[2]),
            a,
        ]);
    }
    Ok(Picture::Rgba {
        width: w,
        height: h,
        pixels: out,
    })
}

/// MIME type of a picture the web view can show on its own.
#[must_use]
pub fn raster_mime(d: &[u8]) -> Option<&'static str> {
    if d.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if d.starts_with(b"GIF87a") || d.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if d.starts_with(b"BM") && d.len() > 26 {
        Some("image/bmp")
    } else {
        None
    }
}

/// Width and height of a JPEG, GIF or BMP, read from its header.
#[must_use]
pub fn raster_size(d: &[u8]) -> Option<(u32, u32)> {
    match raster_mime(d)? {
        "image/gif" => Some((u32::from(u16_at(d, 6)?), u32::from(u16_at(d, 8)?))),
        "image/bmp" => {
            let w = i32::from_le_bytes(d.get(18..22)?.try_into().ok()?);
            let h = i32::from_le_bytes(d.get(22..26)?.try_into().ok()?);
            Some((w.unsigned_abs(), h.unsigned_abs()))
        }
        _ => {
            // Walk the marker segments until a start-of-frame marker.
            let mut p = 2usize;
            while p + 4 <= d.len() {
                if d[p] != 0xff {
                    return None;
                }
                let m = d[p + 1];
                if m == 0xff {
                    p += 1;
                    continue;
                }
                let len = usize::from(u16_be(d, p + 2)?);
                if (0xc0..=0xcf).contains(&m) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
                    let h = u32::from(u16_be(d, p + 5)?);
                    let w = u32::from(u16_be(d, p + 7)?);
                    return Some((w, h));
                }
                p = p.checked_add(2)?.checked_add(len)?;
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 icon with 4 bit palette: red, green / blue, white; the last pixel is masked out.
    fn ico4() -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend(40u32.to_le_bytes());
        dib.extend(2u32.to_le_bytes());
        dib.extend(4u32.to_le_bytes()); // height doubled
        dib.extend(1u16.to_le_bytes());
        dib.extend(4u16.to_le_bytes());
        dib.extend([0; 24]); // compression..important
        let pal: [[u8; 4]; 4] = [
            [0, 0, 255, 0],
            [0, 255, 0, 0],
            [255, 0, 0, 0],
            [255, 255, 255, 0],
        ];
        let mut full = vec![[0u8; 4]; 16];
        full[..4].copy_from_slice(&pal);
        for p in full {
            dib.extend(p);
        }
        // Rows bottom first, 4 bytes each: bottom row = blue, white; top row = red, green.
        dib.extend([0x23, 0, 0, 0]);
        dib.extend([0x01, 0, 0, 0]);
        // Mask: bottom row second pixel hidden.
        dib.extend([0x40, 0, 0, 0]);
        dib.extend([0x00, 0, 0, 0]);
        let mut f = vec![0, 0, 1, 0, 1, 0, 2, 2, 16, 0, 1, 0, 4, 0];
        f.extend((dib.len() as u32).to_le_bytes());
        f.extend(22u32.to_le_bytes());
        f.extend(dib);
        f
    }

    fn tga(
        img_type: u8,
        w: u16,
        h: u16,
        bpp: u8,
        desc: u8,
        cmap: (u16, u8, &[u8]),
        body: &[u8],
    ) -> Vec<u8> {
        let mut f = vec![0, u8::from(!cmap.2.is_empty()), img_type];
        f.extend(0u16.to_le_bytes());
        f.extend(cmap.0.to_le_bytes());
        f.push(cmap.1);
        f.extend([0, 0, 0, 0]);
        f.extend(w.to_le_bytes());
        f.extend(h.to_le_bytes());
        f.extend([bpp, desc]);
        f.extend(cmap.2);
        f.extend(body);
        f
    }

    fn rgba(p: Picture) -> (u32, u32, Vec<u8>) {
        match p {
            Picture::Rgba {
                width,
                height,
                pixels,
            } => (width, height, pixels),
            Picture::Png(_) => panic!("expected pixels"),
        }
    }

    #[test]
    fn tga_true_colour_bottom_left_and_top_left() {
        // 2x2, 24 bit BGR; file rows are bottom first: row0 = blue, white; row1 = red, green.
        let body = [255, 0, 0, 255, 255, 255, 0, 0, 255, 0, 255, 0];
        let (w, h, px) = rgba(decode_tga(&tga(2, 2, 2, 24, 0, (0, 0, &[]), &body)).unwrap());
        assert_eq!((w, h), (2, 2));
        // Top row of the picture is the last file row: red, green.
        assert_eq!(&px[..8], &[255, 0, 0, 255, 0, 255, 0, 255]);
        assert_eq!(&px[8..], &[0, 0, 255, 255, 255, 255, 255, 255]);
        // Top-left origin: rows keep file order.
        let (_, _, px) = rgba(decode_tga(&tga(2, 2, 2, 24, 0x20, (0, 0, &[]), &body)).unwrap());
        assert_eq!(&px[..8], &[0, 0, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn tga_32_bit_alpha_only_when_declared() {
        let body = [1, 2, 3, 100];
        let with = rgba(decode_tga(&tga(2, 1, 1, 32, 8, (0, 0, &[]), &body)).unwrap()).2;
        assert_eq!(with, [3, 2, 1, 100]);
        let without = rgba(decode_tga(&tga(2, 1, 1, 32, 0, (0, 0, &[]), &body)).unwrap()).2;
        assert_eq!(without, [3, 2, 1, 255]);
    }

    #[test]
    fn tga_rle_palette_and_gray() {
        // RLE true colour 4x1: a run of 3 red pixels and one raw green.
        let body = [0x82, 0, 0, 255, 0x00, 0, 255, 0];
        let (_, _, px) = rgba(decode_tga(&tga(10, 4, 1, 24, 0x20, (0, 0, &[]), &body)).unwrap());
        assert_eq!(&px[..12], &[255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]);
        assert_eq!(&px[12..], &[0, 255, 0, 255]);
        // Colour mapped 8 bit: two entries (24 bit BGR), pixels index them.
        let pal = [0, 0, 255, 255, 0, 0];
        let (_, _, px) = rgba(decode_tga(&tga(1, 2, 1, 8, 0x20, (2, 24, &pal), &[1, 0])).unwrap());
        assert_eq!(px, [0, 0, 255, 255, 255, 0, 0, 255]);
        // Gray 8 bit.
        let (_, _, px) = rgba(decode_tga(&tga(3, 1, 1, 8, 0x20, (0, 0, &[]), &[77])).unwrap());
        assert_eq!(px, [77, 77, 77, 255]);
        // 16 bit A1R5G5B5 white.
        let (_, _, px) =
            rgba(decode_tga(&tga(2, 1, 1, 16, 0x20, (0, 0, &[]), &[0xff, 0x7f])).unwrap());
        assert_eq!(px, [255, 255, 255, 255]);
    }

    #[test]
    fn tga_rejects_damaged_files() {
        let good = tga(2, 2, 2, 24, 0, (0, 0, &[]), &[0; 12]);
        assert!(decode_tga(&[]).is_err());
        assert!(decode_tga(&good[..10]).is_err());
        assert!(decode_tga(&good[..good.len() - 1]).is_err());
        assert!(decode_tga(&tga(5, 1, 1, 24, 0, (0, 0, &[]), &[0; 3])).is_err());
        assert!(decode_tga(&tga(2, 0, 4, 24, 0, (0, 0, &[]), &[])).is_err());
        assert!(decode_tga(&tga(2, 65535, 65535, 24, 0, (0, 0, &[]), &[])).is_err());
        assert!(decode_tga(&tga(2, 1, 1, 12, 0, (0, 0, &[]), &[0; 2])).is_err());
        // Palette index outside the table, and a cut-off RLE run.
        assert!(decode_tga(&tga(1, 1, 1, 8, 0, (1, 24, &[0, 0, 0]), &[5])).is_err());
        assert!(decode_tga(&tga(10, 4, 1, 24, 0, (0, 0, &[]), &[0x83, 1, 2])).is_err());
    }

    #[test]
    fn cursor_decodes_like_an_icon() {
        let mut ico = ico4();
        ico[2] = 2; // a cursor: the type word is 2 and the planes/bits fields hold the hot spot
        assert!(decode_ico(&ico).is_ok());
        ico[2] = 3;
        assert!(decode_ico(&ico).is_err());
    }

    #[test]
    fn icon_palette_and_mask_are_decoded() {
        let Picture::Rgba {
            width,
            height,
            pixels,
        } = decode_ico(&ico4()).unwrap()
        else {
            panic!("expected pixels");
        };
        assert_eq!((width, height), (2, 2));
        assert_eq!(&pixels[..4], &[255, 0, 0, 255]); // red, top left
        assert_eq!(&pixels[4..8], &[0, 255, 0, 255]); // green
        assert_eq!(&pixels[8..12], &[0, 0, 255, 255]); // blue
        assert_eq!(&pixels[12..16], &[255, 255, 255, 0]); // white, masked
    }

    #[test]
    fn damaged_icons_are_rejected() {
        let f = ico4();
        assert!(decode_ico(&[]).is_err());
        assert!(decode_ico(&f[..10]).is_err());
        assert!(decode_ico(&f[..f.len() - 30]).is_err());
        let mut bad = f.clone();
        bad[18..22].copy_from_slice(&u32::MAX.to_le_bytes()); // image size
        assert!(decode_ico(&bad).is_err());
        let mut zero = f;
        zero[4] = 0;
        assert!(decode_ico(&zero).is_err());
    }

    fn dds(w: u32, h: u32, flags: u32, fourcc: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[..4].copy_from_slice(b"DDS ");
        d[4..8].copy_from_slice(&124u32.to_le_bytes());
        d[12..16].copy_from_slice(&h.to_le_bytes());
        d[16..20].copy_from_slice(&w.to_le_bytes());
        d[76..80].copy_from_slice(&32u32.to_le_bytes());
        d[80..84].copy_from_slice(&flags.to_le_bytes());
        d[84..88].copy_from_slice(fourcc);
        d.extend_from_slice(body);
        d
    }

    #[test]
    fn dds_dxt1_and_rgba_are_decoded() {
        // One DXT1 block: both colours white, all indices 0.
        let block = [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];
        let d = dds(4, 4, 4, b"DXT1", &block);
        let Picture::Rgba { width, pixels, .. } = decode_dds(&d).unwrap() else {
            panic!()
        };
        assert_eq!(width, 4);
        assert_eq!(&pixels[..4], &[255, 255, 255, 255]);

        let mut raw = dds(1, 1, 0x41, b"\0\0\0\0", &[10, 20, 30, 40]);
        raw[88..92].copy_from_slice(&32u32.to_le_bytes());
        raw[92..96].copy_from_slice(&0x00ff_0000u32.to_le_bytes());
        raw[96..100].copy_from_slice(&0x0000_ff00u32.to_le_bytes());
        raw[100..104].copy_from_slice(&0x0000_00ffu32.to_le_bytes());
        raw[104..108].copy_from_slice(&0xff00_0000u32.to_le_bytes());
        let Picture::Rgba { pixels, .. } = decode_dds(&raw).unwrap() else {
            panic!()
        };
        assert_eq!(pixels, vec![30, 20, 10, 40]); // BGRA in the file
    }

    #[test]
    fn damaged_dds_is_rejected() {
        let block = [0u8; 8];
        let d = dds(4, 4, 4, b"DXT1", &block);
        assert!(decode_dds(&d[..d.len() - 1]).is_err());
        assert!(decode_dds(&d[..60]).is_err());
        assert!(decode_dds(&dds(4, 4, 4, b"ATI2", &block)).is_err());
        assert!(decode_dds(&dds(0xffff, 0xffff, 4, b"DXT1", &block)).is_err());
        assert!(decode_dds(b"DDS xxxx").is_err());
    }

    #[test]
    fn raster_sizes_are_read() {
        let mut gif = b"GIF89a".to_vec();
        gif.extend([3, 0, 5, 0]);
        assert_eq!(raster_size(&gif), Some((3, 5)));
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0, 7, 0, 9, 3,
        ];
        assert_eq!(raster_size(&jpeg), Some((9, 7)));
        assert_eq!(raster_mime(b"BM"), None);
        assert_eq!(raster_size(&[0xff, 0xd8, 0xff]), None);
    }
}
