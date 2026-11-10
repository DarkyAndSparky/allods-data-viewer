//! Texture `.bin` containers, block-compressed (DXT) decoding and DDS/PNG writers.
//!
//! The container is described in `docs/formats/texture-bin.md`. The file does not store
//! width, height or pixel format, so the caller supplies them.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::looks_like_zlib;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{Read, Write};

/// Block compression formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dxt {
    /// DXT1 (BC1), 8 bytes per 4x4 block.
    Dxt1,
    /// DXT3 (BC2), 16 bytes per block.
    Dxt3,
    /// DXT5 (BC3), 16 bytes per block.
    Dxt5,
}

impl Dxt {
    /// Bytes per 4x4 block.
    #[must_use]
    pub fn block_bytes(self) -> usize {
        match self {
            Self::Dxt1 => 8,
            Self::Dxt3 | Self::Dxt5 => 16,
        }
    }

    /// FourCC code for DDS.
    #[must_use]
    pub fn fourcc(self) -> [u8; 4] {
        match self {
            Self::Dxt1 => *b"DXT1",
            Self::Dxt3 => *b"DXT3",
            Self::Dxt5 => *b"DXT5",
        }
    }

    /// Size in bytes of a level with the given dimensions.
    #[must_use]
    pub fn level_size(self, width: u32, height: u32) -> u64 {
        u64::from(width.max(1).div_ceil(4))
            * u64::from(height.max(1).div_ceil(4))
            * self.block_bytes() as u64
    }
}

/// One mip level from a `.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    /// Mip index, 0 is the largest.
    pub id: i32,
    /// Raw level data.
    pub data: Vec<u8>,
}

/// Parses the chunks of an inflated texture `.bin`.
///
/// # Errors
///
/// [`Error::Format`] when a chunk header or size is invalid or the chunks do not end
/// exactly at the end of the data.
pub fn parse_levels(d: &[u8]) -> Result<Vec<Level>> {
    let mut pos = 0usize;
    let mut out = Vec::new();
    while pos < d.len() {
        let head = d
            .get(pos..pos + 8)
            .ok_or_else(|| Error::format("truncated chunk header"))?;
        let id = i32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let size = i32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        if id < 0 || size < 0 {
            return Err(Error::format("negative chunk id or size"));
        }
        let start = pos + 8;
        let end = start
            .checked_add(size as usize)
            .ok_or_else(|| Error::format("chunk size overflow"))?;
        let data = d
            .get(start..end)
            .ok_or_else(|| Error::format(format!("chunk {id} runs past end")))?;
        out.push(Level {
            id,
            data: data.to_vec(),
        });
        pos = end;
    }
    Ok(out)
}

/// Reads a texture `.bin` (as stored, or already inflated) and parses its levels.
///
/// # Errors
///
/// Fails on I/O, zlib, size-limit or format errors.
pub fn read_levels(mut input: impl Read, limits: &Limits) -> Result<Vec<Level>> {
    let cap = limits.max_inflated_size;
    let mut raw = Vec::new();
    (&mut input).take(cap + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > cap {
        return Err(Error::limit("texture file too large"));
    }
    if !looks_like_zlib(&raw) {
        // Already inflated, for example extracted by `adv extract` without `--raw`.
        return parse_levels(&raw);
    }
    let mut out = Vec::new();
    ZlibDecoder::new(raw.as_slice())
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::format(format!("zlib: {e}")))?;
    if out.len() as u64 > cap {
        return Err(Error::limit("inflated texture too large"));
    }
    parse_levels(&out)
}

/// Width and height of mip `id` for a texture whose level 0 is `width` x `height`.
#[must_use]
pub fn level_dims(width: u32, height: u32, id: i32) -> (u32, u32) {
    let s = id.clamp(0, 31) as u32;
    ((width >> s).max(1), (height >> s).max(1))
}

fn rgb565(c: u16) -> [u8; 3] {
    let r = u32::from((c >> 11) & 31);
    let g = u32::from((c >> 5) & 63);
    let b = u32::from(c & 31);
    [
        ((r * 255 + 15) / 31) as u8,
        ((g * 255 + 31) / 63) as u8,
        ((b * 255 + 15) / 31) as u8,
    ]
}

fn color_block(b: &[u8], allow_alpha: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |a: u8, b: u8, wa: u32, wb: u32, div: u32| {
        ((u32::from(a) * wa + u32::from(b) * wb) / div) as u8
    };
    let mut pal = [[0u8; 4]; 4];
    pal[0] = [p0[0], p0[1], p0[2], 255];
    pal[1] = [p1[0], p1[1], p1[2], 255];
    if c0 > c1 || !allow_alpha {
        for k in 0..3 {
            pal[2][k] = mix(p0[k], p1[k], 2, 1, 3);
            pal[3][k] = mix(p0[k], p1[k], 1, 2, 3);
        }
        pal[2][3] = 255;
        pal[3][3] = 255;
    } else {
        for k in 0..3 {
            pal[2][k] = mix(p0[k], p1[k], 1, 1, 2);
        }
        pal[2][3] = 255;
        pal[3] = [0, 0, 0, 0];
    }
    let bits = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let mut out = [[0u8; 4]; 16];
    for (i, px) in out.iter_mut().enumerate() {
        *px = pal[((bits >> (2 * i)) & 3) as usize];
    }
    out
}

fn alpha5(b: &[u8]) -> [u8; 16] {
    let (a0, a1) = (u32::from(b[0]), u32::from(b[1]));
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7u32 {
            pal[(i + 1) as usize] = (((7 - i) * a0 + i * a1) / 7) as u8;
        }
    } else {
        for i in 1..5u32 {
            pal[(i + 1) as usize] = (((5 - i) * a0 + i * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits = 0u64;
    for (i, v) in b[2..8].iter().enumerate() {
        bits |= u64::from(*v) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = pal[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

/// Decodes one level to RGBA8 (row-major, 4 bytes per pixel).
///
/// # Errors
///
/// [`Error::Format`] when `data` does not have exactly the size the dimensions and
/// format require, and [`Error::Limit`] for dimensions above 16384.
pub fn decode_dxt(data: &[u8], width: u32, height: u32, format: Dxt) -> Result<Vec<u8>> {
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(Error::limit(format!("unsupported size {width}x{height}")));
    }
    let expected = format.level_size(width, height);
    if data.len() as u64 != expected {
        return Err(Error::format(format!(
            "level has {} bytes, {width}x{height} {format:?} needs {expected}",
            data.len()
        )));
    }
    let (w, h) = (width as usize, height as usize);
    let bw = w.div_ceil(4);
    let mut out = vec![0u8; w * h * 4];
    let bb = format.block_bytes();
    for (bi, block) in data.chunks_exact(bb).enumerate() {
        let (bx, by) = (bi % bw, bi / bw);
        let (colors, alphas): ([[u8; 4]; 16], Option<[u8; 16]>) = match format {
            Dxt::Dxt1 => (color_block(block, true), None),
            Dxt::Dxt3 => {
                let mut a = [0u8; 16];
                for (i, v) in a.iter_mut().enumerate() {
                    let nib = (block[i / 2] >> (4 * (i % 2))) & 15;
                    *v = nib * 17;
                }
                (color_block(&block[8..], false), Some(a))
            }
            Dxt::Dxt5 => (color_block(&block[8..], false), Some(alpha5(&block[..8]))),
        };
        for i in 0..16 {
            let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
            if x < w && y < h {
                let o = (y * w + x) * 4;
                out[o..o + 4].copy_from_slice(&colors[i]);
                if let Some(a) = alphas {
                    out[o + 3] = a[i];
                }
            }
        }
    }
    Ok(out)
}

/// Builds a DDS file. `levels` are ordered largest first; `width`/`height` are the
/// dimensions of the first one. Each level must match its expected size.
///
/// # Errors
///
/// [`Error::Format`] when `levels` is empty or a level has the wrong size.
pub fn write_dds(width: u32, height: u32, format: Dxt, levels: &[&[u8]]) -> Result<Vec<u8>> {
    if levels.is_empty() {
        return Err(Error::format("no levels"));
    }
    for (k, l) in levels.iter().enumerate() {
        let (w, h) = level_dims(width, height, k as i32);
        if l.len() as u64 != format.level_size(w, h) {
            return Err(Error::format(format!("level {k} has the wrong size")));
        }
    }
    let mut out = Vec::new();
    let mut put = |v: u32| out.extend_from_slice(&v.to_le_bytes());
    out_magic(&mut put);
    put(124);
    put(0x1 | 0x2 | 0x4 | 0x1000 | 0x2_0000 | 0x8_0000);
    put(height);
    put(width);
    put(levels[0].len() as u32);
    put(0);
    put(levels.len() as u32);
    for _ in 0..11 {
        put(0);
    }
    put(32);
    put(4);
    put(u32::from_le_bytes(format.fourcc()));
    for _ in 0..5 {
        put(0);
    }
    put(0x1000 | 0x40_0000 | 0x8);
    for _ in 0..4 {
        put(0);
    }
    for l in levels {
        out.extend_from_slice(l);
    }
    Ok(out)
}

fn out_magic(put: &mut impl FnMut(u32)) {
    put(u32::from_le_bytes(*b"DDS "));
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Encodes RGBA8 pixels as a PNG (8 bit, RGBA, no interlace).
///
/// # Errors
///
/// [`Error::Format`] when `rgba` does not hold `width * height * 4` bytes.
pub fn write_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let row = width as usize * 4;
    if width == 0 || height == 0 || rgba.len() != row * height as usize {
        return Err(Error::format("pixel buffer does not match the size"));
    }
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw)?;
    let idat = enc.finish()?;
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin(levels: &[(i32, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        for (id, d) in levels {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&(d.len() as i32).to_le_bytes());
            v.extend_from_slice(d);
        }
        v
    }

    #[test]
    fn chunks_parse_and_reject_garbage() {
        let d = bin(&[(2, &[1; 8]), (1, &[2; 32])]);
        let l = parse_levels(&d).expect("parse");
        assert_eq!((l[0].id, l[0].data.len(), l[1].id), (2, 8, 1));
        assert!(parse_levels(&d[..d.len() - 1]).is_err());
        assert!(parse_levels(&d[..5]).is_err());
        assert!(parse_levels(&bin(&[(-1, &[])])).is_err());
        let mut bad = d.clone();
        bad[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(parse_levels(&bad).is_err());
    }

    #[test]
    fn read_levels_accepts_zlib_and_inflated() {
        let d = bin(&[(1, &[5; 8])]);
        let z = crate::pak::testutil::zlib(&d);
        let a = read_levels(z.as_slice(), &Limits::default()).expect("zlib");
        let b = read_levels(d.as_slice(), &Limits::default()).expect("plain");
        assert_eq!(a, b);
        assert!(read_levels(&b"junk junk junk"[..], &Limits::default()).is_err());
    }

    #[test]
    fn level_sizes() {
        assert_eq!(Dxt::Dxt1.level_size(512, 512), 131_072);
        assert_eq!(Dxt::Dxt5.level_size(4, 4), 16);
        assert_eq!(Dxt::Dxt1.level_size(1, 1), 8);
        assert_eq!(level_dims(512, 256, 2), (128, 64));
        assert_eq!(level_dims(4, 4, 5), (1, 1));
    }

    #[test]
    fn dxt1_solid_and_transparent_blocks() {
        // c0 = pure red (0xF800), c1 = 0, all indices 0 -> red
        let red = [0x00, 0xF8, 0, 0, 0, 0, 0, 0];
        let px = decode_dxt(&red, 4, 4, Dxt::Dxt1).expect("decode");
        assert!(px.chunks(4).all(|p| p == [255, 0, 0, 255]));
        // c0 <= c1 and index 3 -> transparent black
        let tr = [0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        let px = decode_dxt(&tr, 4, 4, Dxt::Dxt1).expect("decode");
        assert!(px.chunks(4).all(|p| p == [0, 0, 0, 0]));
    }

    #[test]
    fn dxt3_and_dxt5_alpha() {
        let mut b3 = [0xFFu8; 8].to_vec();
        b3.extend_from_slice(&[0x00, 0xF8, 0, 0, 0, 0, 0, 0]);
        let px = decode_dxt(&b3, 4, 4, Dxt::Dxt3).expect("decode");
        assert!(px.chunks(4).all(|p| p == [255, 0, 0, 255]));
        let mut b5 = vec![200, 100, 0, 0, 0, 0, 0, 0];
        b5.extend_from_slice(&[0x00, 0xF8, 0, 0, 0, 0, 0, 0]);
        let px = decode_dxt(&b5, 4, 4, Dxt::Dxt5).expect("decode");
        assert!(px.chunks(4).all(|p| p == [255, 0, 0, 200]));
    }

    #[test]
    fn non_multiple_of_four_is_cropped() {
        let px = decode_dxt(&[0u8; 8], 3, 2, Dxt::Dxt1).expect("decode");
        assert_eq!(px.len(), 3 * 2 * 4);
    }

    #[test]
    fn wrong_size_and_huge_dimensions_fail() {
        assert!(decode_dxt(&[0u8; 7], 4, 4, Dxt::Dxt1).is_err());
        assert!(decode_dxt(&[], 20000, 20000, Dxt::Dxt1).is_err());
        assert!(decode_dxt(&[0u8; 8], 0, 4, Dxt::Dxt1).is_err());
    }

    #[test]
    fn dds_header_is_well_formed() {
        let l0 = [0u8; 32];
        let l1 = [0u8; 8];
        let dds = write_dds(8, 8, Dxt::Dxt1, &[&l0, &l1]).expect("dds");
        assert_eq!(&dds[..4], b"DDS ");
        assert_eq!(dds.len(), 128 + 32 + 8);
        assert_eq!(
            u32::from_le_bytes(dds[84..88].try_into().unwrap()),
            u32::from_le_bytes(*b"DXT1")
        );
        assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 2);
        assert!(write_dds(8, 8, Dxt::Dxt1, &[&l1]).is_err());
        assert!(write_dds(8, 8, Dxt::Dxt1, &[]).is_err());
    }

    #[test]
    fn png_is_valid_and_round_trips_through_zlib() {
        let rgba = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let png = write_png(2, 1, &rgba).expect("png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        // known CRC32 check value
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let idat_len = u32::from_be_bytes(png[33..37].try_into().unwrap()) as usize;
        assert_eq!(&png[37..41], b"IDAT");
        let mut raw = Vec::new();
        ZlibDecoder::new(&png[41..41 + idat_len])
            .read_to_end(&mut raw)
            .expect("inflate");
        assert_eq!(raw, [&[0u8][..], &rgba[..]].concat());
        assert!(write_png(2, 2, &rgba).is_err());
    }
}
