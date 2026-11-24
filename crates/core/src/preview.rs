//! Previews for a file browser: turns the bytes of one file into something a UI can show.
//!
//! The functions here take bytes, not paths, so they work the same for files inside a `.pak`
//! and for loose files. Nothing here prints or touches the disk.

use crate::browse::{FileKind, classify};
use crate::error::{Error, Result};
use crate::fsb::{parse_bank, unwrap_bank};
use crate::limits::Limits;
use crate::texheur;
use crate::texture::{Raw, decode_dxt, decode_raw, level_dims, read_levels, write_png};
use std::io::Cursor;

/// Most bytes of text returned by a preview.
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// Bytes shown in a hex dump.
pub const HEX_BYTES: usize = 512;

/// One stream of a sound bank, for display.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    /// Stream name (may be empty).
    pub name: String,
    /// Sample rate in Hz when known.
    pub frequency: Option<u32>,
    /// Channel count.
    pub channels: u32,
    /// Duration in seconds when known.
    pub seconds: Option<f64>,
}

/// What a UI can show for a file.
#[derive(Debug, Clone, PartialEq)]
pub enum Preview {
    /// Text, cut at [`MAX_TEXT_BYTES`].
    Text {
        /// The text.
        text: String,
        /// True when the file was longer than the part shown.
        truncated: bool,
    },
    /// A `.bev` FMOD Designer project (see [`crate::bev`]).
    Bev {
        /// Format version.
        version: u32,
        /// Project name.
        project: String,
        /// Language names.
        languages: Vec<String>,
        /// Chunks of the project list with sizes.
        chunks: Vec<(String, u32)>,
        /// Plain names (sound banks, events, suffixes).
        names: Vec<String>,
        /// Paths of events and waves.
        paths: Vec<String>,
    },
    /// An SVG picture that went through [`crate::svg::sanitize`]; only `svg` may be drawn.
    Svg {
        /// The sanitised document.
        svg: String,
        /// The original text (cut at [`MAX_TEXT_BYTES`]) for the "source" view.
        source: String,
        /// How many elements, attributes and values were removed.
        removed: usize,
        /// Names of the first removed things.
        removed_names: Vec<String>,
    },
    /// A `.loc` localization file; read its items page by page with the session.
    Loc {
        /// Number of items.
        count: usize,
        /// True when the layout stores keys (layout 1).
        keys: bool,
    },
    /// A picture as PNG.
    Image {
        /// Width of the picture in pixels.
        width: u32,
        /// Height of the picture in pixels.
        height: u32,
        /// PNG file bytes.
        png: Vec<u8>,
        /// Short description of the source format (for example `DXT5`).
        format: String,
        /// True when the size was guessed, not read from a database.
        guessed: bool,
        /// All plausible sizes (a guess can be corrected by choosing another one).
        candidates: Vec<(u32, u32)>,
        /// Index of the size used in `candidates`.
        index: usize,
        /// Pixel format of each candidate (for example `DXT1`, `RGB565`).
        labels: Vec<String>,
    },
    /// A 3D model (geometry `.bin`); fetch the buffers with [`mesh_bytes`].
    Model {
        /// Number of vertices.
        vertices: usize,
        /// Number of triangles.
        triangles: usize,
        /// Guessed vertex size in bytes.
        stride: usize,
    },
    /// A skeletal animation clip (header only; key frames are not decoded).
    Animation {
        /// Frames per second.
        fps: u16,
        /// Number of frames.
        frames: u16,
        /// Length in seconds.
        seconds: f64,
        /// Number of bones.
        bones: u32,
    },
    /// A Theora video (header only; frames are not decoded).
    Video {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Frames per second.
        fps: f64,
        /// Length in seconds, when known.
        seconds: Option<f64>,
        /// Number of frames, when known.
        frames: Option<u64>,
    },
    /// A sound bank.
    Audio {
        /// Streams in bank order.
        streams: Vec<AudioStream>,
    },
    /// Anything else: size and the start of the data as a hex dump.
    Binary {
        /// File length.
        len: usize,
        /// Classic hex dump of the first bytes.
        hex: String,
    },
}

/// Settings for [`preview`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Choose this texture size candidate instead of the default guess.
    pub texture_index: Option<usize>,
}

/// Builds a classic `offset  hex  ascii` dump of the first [`HEX_BYTES`] bytes.
#[must_use]
pub fn hex_dump(data: &[u8]) -> String {
    let mut out = String::new();
    for (row, chunk) in data
        .iter()
        .take(HEX_BYTES)
        .copied()
        .collect::<Vec<u8>>()
        .chunks(16)
        .enumerate()
    {
        out.push_str(&format!("{:06x}  ", row * 16));
        for i in 0..16 {
            match chunk.get(i) {
                Some(b) => out.push_str(&format!("{b:02x} ")),
                None => out.push_str("   "),
            }
        }
        out.push(' ');
        out.extend(chunk.iter().map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            }
        }));
        out.push('\n');
    }
    out
}

/// Windows-1251 characters of bytes 0x80..=0xBF (0xC0..=0xFF map to U+0410..=U+044F).
const CP1251_HIGH: [char; 64] = [
    '\u{402}', '\u{403}', '\u{201A}', '\u{453}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{20AC}', '\u{2030}', '\u{409}', '\u{2039}', '\u{40A}', '\u{40C}', '\u{40B}', '\u{40F}',
    '\u{452}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{FFFD}', '\u{2122}', '\u{459}', '\u{203A}', '\u{45A}', '\u{45C}', '\u{45B}', '\u{45F}',
    '\u{A0}', '\u{40E}', '\u{45E}', '\u{408}', '\u{A4}', '\u{490}', '\u{A6}', '\u{A7}', '\u{401}',
    '\u{A9}', '\u{404}', '\u{AB}', '\u{AC}', '\u{AD}', '\u{AE}', '\u{407}', '\u{B0}', '\u{B1}',
    '\u{406}', '\u{456}', '\u{491}', '\u{B5}', '\u{B6}', '\u{B7}', '\u{451}', '\u{2116}',
    '\u{454}', '\u{BB}', '\u{458}', '\u{405}', '\u{455}', '\u{457}',
];

fn decode_cp1251(data: &[u8]) -> String {
    data.iter()
        .map(|&b| match b {
            0..=0x7f => char::from(b),
            0x80..=0xbf => CP1251_HIGH[usize::from(b - 0x80)],
            _ => char::from_u32(0x410 + u32::from(b - 0xc0)).unwrap_or('\u{FFFD}'),
        })
        .collect()
}

/// Decodes text with a UTF-16 (either byte order) or UTF-8 byte order mark. Without a mark the
/// bytes are UTF-8 when they are valid UTF-8, else Windows-1251 (the encoding of old Russian
/// client files).
#[must_use]
pub fn decode_text(data: &[u8]) -> String {
    if let Some(rest) = data.strip_prefix(&[0xff, 0xfe]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = data.strip_prefix(&[0xfe, 0xff]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let body = data.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(data);
    match std::str::from_utf8(body) {
        Ok(text) => text.to_string(),
        Err(_) => decode_cp1251(body),
    }
}

/// Tries to show `data` as a block-compressed texture `.bin` (with an optional `.hi.bin`).
fn texture_preview(
    name: &str,
    known: Option<(u32, u32)>,
    data: &[u8],
    hi: Option<&[u8]>,
    opts: Options,
    limits: &Limits,
) -> Result<Preview> {
    let mut levels = read_levels(Cursor::new(data), limits)?;
    if let Some(hi) = hi {
        levels.extend(read_levels(Cursor::new(hi), limits)?);
    }
    levels.sort_by_key(|l| l.id);
    levels.dedup_by_key(|l| l.id);
    let g = texheur::guess(&levels, None).ok_or_else(|| Error::format("not a texture"))?;
    let top = levels.first().ok_or_else(|| Error::format("no levels"))?;
    // Options: the guessed block format first, then the uncompressed formats.
    let mut options: Vec<(Option<Raw>, u32, u32)> =
        g.candidates.iter().map(|&(w, h)| (None, w, h)).collect();
    let dxt_count = options.len();
    let shift = 2 * top.id.clamp(0, 15) as u32;
    for raw in Raw::ALL {
        let bytes = raw.bytes();
        let len = top.data.len() as u64;
        if len.is_multiple_of(bytes) {
            for (w, h) in texheur::sizes_for_pixels((len / bytes) << shift) {
                options.push((Some(raw), w, h));
            }
        }
    }
    options.dedup();
    let index = opts.texture_index.map_or_else(
        || {
            // A size from the resource database beats every guess.
            if let Some(k) = known
                && let Some(i) = options.iter().position(|&(_, w, h)| (w, h) == k)
            {
                return i;
            }
            // Terrain splat maps are 16 bit (RGB565) 256x256 maps (`docs/formats/bin-db.md`).
            if name.contains("SplatMap")
                && let Some(i) = options
                    .iter()
                    .position(|&(r, w, h)| r == Some(Raw::Rgb565) && w == h)
            {
                return i;
            }
            let (w, h) = g.size();
            options
                .iter()
                .position(|&(r, ow, oh)| r.is_none() && (ow, oh) == (w, h))
                .unwrap_or(0)
        },
        |i| i.min(options.len().saturating_sub(1)),
    );
    let (raw, w, h) = *options
        .get(index)
        .ok_or_else(|| Error::format("no texture size candidates"))?;
    let (lw, lh) = level_dims(w, h, top.id);
    let (rgba, format) = match raw {
        None => (
            decode_dxt(&top.data, lw, lh, g.format)?,
            format!("{:?}", g.format).to_ascii_uppercase(),
        ),
        Some(r) => (decode_raw(&top.data, lw, lh, r)?, r.label().to_owned()),
    };
    let labels = options
        .iter()
        .enumerate()
        .map(|(i, (r, _, _))| match r {
            Some(r) => r.label().to_owned(),
            None if i < dxt_count => format!("{:?}", g.format).to_ascii_uppercase(),
            None => String::new(),
        })
        .collect();
    let png = write_png(lw, lh, &rgba)?;
    Ok(Preview::Image {
        width: lw,
        height: lh,
        png,
        format,
        guessed: true,
        candidates: options.iter().map(|&(_, a, b)| (a, b)).collect(),
        index,
        labels,
    })
}

/// Builds a preview of a file from its name and (already unwrapped) bytes.
///
/// `hi` is the sibling `.hi.bin` of a texture when the caller has it.
/// Failures never escape: a file that cannot be shown in its kind falls back to a hex dump.
#[must_use]
pub fn preview(
    name: &str,
    data: &[u8],
    hi: Option<&[u8]>,
    opts: Options,
    limits: &Limits,
) -> Preview {
    preview_sized(name, data, hi, opts, None, limits)
}

/// Like [`preview`], with the texture size `known` from the resource database (width,
/// height of mip 0). When it matches one of the candidates it becomes the default.
#[must_use]
pub fn preview_sized(
    name: &str,
    data: &[u8],
    hi: Option<&[u8]>,
    opts: Options,
    known: Option<(u32, u32)>,
    limits: &Limits,
) -> Preview {
    let head = &data[..data.len().min(64)];
    let lower = name.to_ascii_lowercase();
    let svg = if lower.ends_with(".svg") {
        crate::svg::sanitize(data).ok()
    } else {
        None
    };
    let bev = if lower.ends_with(".bev") {
        crate::bev::parse(data, limits).ok()
    } else {
        None
    };
    let attempt =
        match classify(name, head) {
            _ if bev.is_some() => bev.map(|b| {
                let (names, paths) = b.split_strings();
                Preview::Bev {
                    version: b.version,
                    project: b.project,
                    languages: b.languages,
                    chunks: b.chunks,
                    names,
                    paths,
                }
            }),
            FileKind::Text | FileKind::Other if svg.is_some() => svg.map(|c| Preview::Svg {
                svg: c.svg,
                source: decode_text(&data[..data.len().min(MAX_TEXT_BYTES)]),
                removed: c.removed,
                removed_names: c.removed_names,
            }),
            FileKind::Text if lower.ends_with(".loc") => crate::loc::read_loc(data, limits)
                .ok()
                .map(|f| Preview::Loc {
                    count: f.items.len(),
                    keys: f.layout == crate::loc::LocLayout::V1,
                }),
            FileKind::Text => {
                let cut = data.len().min(MAX_TEXT_BYTES);
                Some(Preview::Text {
                    text: decode_text(&data[..cut]),
                    truncated: data.len() > cut,
                })
            }
            FileKind::Animation => animation_preview(data, limits).ok(),
            FileKind::Video => crate::ogv::parse(data).ok().map(|v| Preview::Video {
                width: v.width,
                height: v.height,
                fps: v.fps(),
                seconds: v.seconds(),
                frames: v.frames,
            }),
            FileKind::Audio => audio_streams(data, limits)
                .ok()
                .map(|streams| Preview::Audio { streams }),
            FileKind::Texture if lower.ends_with(".ico") => crate::picture::decode_ico(data)
                .ok()
                .and_then(|p| picture_preview(p, "ICO")),
            FileKind::Texture if lower.ends_with(".cur") => crate::picture::decode_ico(data)
                .ok()
                .and_then(|p| picture_preview(p, "CUR")),
            FileKind::Texture if lower.ends_with(".tga") => crate::picture::decode_tga(data)
                .ok()
                .and_then(|p| picture_preview(p, "TGA")),
            FileKind::Texture if data.starts_with(b"DDS ") => crate::picture::decode_dds(data)
                .ok()
                .and_then(|p| picture_preview(p, "DDS")),
            FileKind::Texture if crate::picture::raster_mime(data).is_some() => {
                crate::picture::raster_size(data).map(|(width, height)| Preview::Image {
                    width,
                    height,
                    png: data.to_vec(),
                    format: crate::picture::raster_mime(data)
                        .and_then(|m| m.strip_prefix("image/"))
                        .unwrap_or("raster")
                        .to_ascii_uppercase(),
                    guessed: false,
                    candidates: Vec::new(),
                    index: 0,
                    labels: Vec::new(),
                })
            }
            FileKind::Texture if data.starts_with(b"\x89PNG") => {
                png_size(data).map(|(w, h)| Preview::Image {
                    width: w,
                    height: h,
                    png: data.to_vec(),
                    format: "PNG".into(),
                    guessed: false,
                    candidates: Vec::new(),
                    index: 0,
                    labels: Vec::new(),
                })
            }
            FileKind::Other | FileKind::Texture | FileKind::Model if lower.ends_with(".bin") => {
                // Geometry files are chunk lists too, so a texture parse would accept them; the
                // geometry check is much stricter (indices, positions, vertex size), so it goes first.
                model_preview(data, limits)
                    .ok()
                    .or_else(|| texture_preview(name, known, data, hi, opts, limits).ok())
            }
            FileKind::Other if looks_like_text(data) => {
                let cut = data.len().min(MAX_TEXT_BYTES);
                Some(Preview::Text {
                    text: decode_text(&data[..cut]),
                    truncated: data.len() > cut,
                })
            }
            _ => None,
        };
    attempt.unwrap_or_else(|| Preview::Binary {
        len: data.len(),
        hex: hex_dump(data),
    })
}

/// Turns a decoded picture into an image preview (PNG bytes).
fn picture_preview(p: crate::picture::Picture, format: &str) -> Option<Preview> {
    let (width, height, png) = match p {
        crate::picture::Picture::Rgba {
            width,
            height,
            pixels,
        } => (
            width,
            height,
            crate::texture::write_png(width, height, &pixels).ok()?,
        ),
        crate::picture::Picture::Png(png) => {
            let (w, h) = png_size(&png)?;
            (w, h, png)
        }
    };
    Some(Preview::Image {
        width,
        height,
        png,
        format: format.into(),
        guessed: false,
        candidates: Vec::new(),
        index: 0,
        labels: Vec::new(),
    })
}

/// True for data that is most likely text: a UTF-16 byte order mark, or the first 8 KiB has no
/// NUL and no control characters other than tab, line breaks and escape.
fn looks_like_text(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    if data.starts_with(&[0xff, 0xfe]) || data.starts_with(&[0xfe, 0xff]) {
        return true;
    }
    let head = &data[..data.len().min(8192)];
    head.iter()
        .all(|&b| b >= 0x20 || matches!(b, b'\t' | b'\n' | b'\r' | 0x1b))
        && head.iter().any(|b| b.is_ascii_graphic())
}

fn animation_preview(data: &[u8], limits: &Limits) -> Result<Preview> {
    let inflated = crate::geometry::inflate(data, limits)?;
    let info = crate::animation::parse(&inflated)?;
    Ok(Preview::Animation {
        fps: info.fps,
        frames: info.frames,
        seconds: info.seconds(),
        bones: info.bones,
    })
}

fn load_geometry(data: &[u8], limits: &Limits) -> Result<crate::geometry::Geometry> {
    let inflated = crate::geometry::inflate(data, limits)?;
    let g = crate::geometry::parse(&inflated, None)?;
    let count = g.vertex_data.len() / g.stride;
    if count == 0 || g.indices.is_empty() {
        return Err(Error::format("geometry has no vertices or no triangles"));
    }
    if g.indices.iter().any(|&i| usize::from(i) >= count) {
        return Err(Error::format("geometry index is outside the vertex block"));
    }
    if g.vertices()
        .iter()
        .any(|v| v.pos.iter().any(|c| !c.is_finite()))
    {
        return Err(Error::format("geometry has non-finite positions"));
    }
    Ok(g)
}

fn model_preview(data: &[u8], limits: &Limits) -> Result<Preview> {
    let g = load_geometry(data, limits)?;
    Ok(Preview::Model {
        vertices: g.vertex_data.len() / g.stride,
        triangles: g.indices.len() / 3,
        stride: g.stride,
    })
}

/// Packs a geometry `.bin` for a viewer: `u32 vertices`, `u32 indices`, then `f32` positions
/// (3 per vertex), `f32` texture coordinates (2 per vertex) and `u32` indices, all little endian.
///
/// # Errors
///
/// Fails when the data is not a consistent geometry file.
pub fn mesh_bytes(data: &[u8], limits: &Limits) -> Result<Vec<u8>> {
    let g = load_geometry(data, limits)?;
    let verts = g.vertices();
    let mut out = Vec::with_capacity(8 + verts.len() * 20 + g.indices.len() * 4);
    out.extend((verts.len() as u32).to_le_bytes());
    out.extend((g.indices.len() as u32).to_le_bytes());
    for v in &verts {
        for c in v.pos {
            out.extend(c.to_le_bytes());
        }
    }
    for v in &verts {
        for c in v.uv {
            out.extend(c.to_le_bytes());
        }
    }
    for &i in &g.indices {
        out.extend(u32::from(i).to_le_bytes());
    }
    Ok(out)
}

/// Looks into a file to tell what it is. Unlike [`classify`] it can tell a texture `.bin` from
/// a model `.bin`, at the price of parsing the data (no pictures are decoded).
#[must_use]
pub fn sniff(name: &str, data: &[u8], limits: &Limits) -> FileKind {
    let by_name = classify(name, &data[..data.len().min(64)]);
    if by_name != FileKind::Other || !name.to_ascii_lowercase().ends_with(".bin") {
        return by_name;
    }
    if load_geometry(data, limits).is_ok() {
        return FileKind::Model;
    }
    let is_texture = read_levels(Cursor::new(data), limits)
        .ok()
        .is_some_and(|levels| texheur::guess(&levels, None).is_some());
    if is_texture {
        FileKind::Texture
    } else {
        FileKind::Other
    }
}

/// A binary glTF (`.glb`) of a geometry `.bin`.
///
/// # Errors
///
/// Fails when the data is not a consistent geometry file.
pub fn mesh_glb(data: &[u8], limits: &Limits) -> Result<Vec<u8>> {
    load_geometry(data, limits)?.to_glb()
}

/// A binary glTF file of a geometry `.bin` with one skeletal animation clip (`clip` is the content
/// of an `X.<Clip>.(SkeletalAnimation).bin`, zlib compressed or inflated).
///
/// # Errors
///
/// Fails when the model or the clip is not consistent, or the clip does not animate the model's
/// bones.
pub fn mesh_glb_animated(data: &[u8], clip: &[u8], name: &str, limits: &Limits) -> Result<Vec<u8>> {
    let g = load_geometry(data, limits)?;
    let inflated = crate::geometry::inflate(clip, limits)?;
    let clip = crate::animation::decode(&inflated)?;
    g.to_glb_animated(&clip, name)
}

/// Binary glTF of a geometry `.bin` split into textured parts, optionally with one clip
/// (`clip` is the clip file and its name).
///
/// # Errors
///
/// Fails when the model or the clip is not consistent.
pub fn mesh_glb_textured(
    data: &[u8],
    parts: &[crate::geometry::MeshPart],
    clip: Option<(&[u8], &str)>,
    limits: &Limits,
) -> Result<Vec<u8>> {
    let g = load_geometry(data, limits)?;
    let decoded = match clip {
        Some((bytes, name)) => {
            let inflated = crate::geometry::inflate(bytes, limits)?;
            Some((crate::animation::decode(&inflated)?, name))
        }
        None => None,
    };
    g.to_glb_textured(parts, decoded.as_ref().map(|(c, n)| (c, *n)))
}

/// Wavefront OBJ text of a geometry `.bin` (positions, UVs, triangles).
///
/// # Errors
///
/// Fails when the data is not a consistent geometry file.
pub fn mesh_obj(data: &[u8], name: &str, limits: &Limits) -> Result<String> {
    Ok(load_geometry(data, limits)?.to_obj(name))
}

fn png_size(d: &[u8]) -> Option<(u32, u32)> {
    let w = d.get(16..20)?;
    let h = d.get(20..24)?;
    Some((
        u32::from_be_bytes([w[0], w[1], w[2], w[3]]),
        u32::from_be_bytes([h[0], h[1], h[2], h[3]]),
    ))
}

fn audio_streams(data: &[u8], limits: &Limits) -> Result<Vec<AudioStream>> {
    let bank = parse_bank(&unwrap_bank(Cursor::new(data), limits)?, limits)?;
    Ok(bank
        .streams
        .iter()
        .map(|s| AudioStream {
            name: s.name.clone(),
            frequency: s.frequency,
            channels: s.channels,
            seconds: s.seconds(),
        })
        .collect())
}

/// Decodes stream `index` of a `.fsb` or `.bsb` to a WAV file.
///
/// # Errors
///
/// Fails when the bank is damaged, the index is out of range or the stream cannot be decoded.
pub fn audio_wav(data: &[u8], index: usize, limits: &Limits) -> Result<Vec<u8>> {
    let raw = unwrap_bank(Cursor::new(data), limits)?;
    let bank = parse_bank(&raw, limits)?;
    Ok(bank.decode(&raw, index)?.to_wav())
}

/// The soundtrack of an `.ogv` as a WAV file; empty when the video has none.
///
/// # Errors
///
/// Fails when the file is not an Ogg video or its Vorbis stream is damaged.
pub fn video_audio_wav(data: &[u8], limits: &Limits) -> Result<Vec<u8>> {
    Ok(crate::video::ogg_audio(data, limits)?.map_or_else(Vec::new, |p| p.to_wav()))
}

/// Decodes stream `index` of a `.fsb` or `.bsb` to an MP3 file (320 kbps, see `mp3.rs`).
///
/// # Errors
///
/// Fails like [`audio_wav`], and for sample rates or channel counts MP3 cannot carry.
pub fn audio_mp3(data: &[u8], index: usize, limits: &Limits) -> Result<Vec<u8>> {
    let raw = unwrap_bank(Cursor::new(data), limits)?;
    let bank = parse_bank(&raw, limits)?;
    bank.decode(&raw, index)?.to_mp3()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::Dxt;

    fn dxt1_chunk(id: i32, blocks: usize) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(id.to_le_bytes());
        v.extend(((blocks * 8) as i32).to_le_bytes());
        v.extend(std::iter::repeat_n(0u8, blocks * 8));
        v
    }

    #[test]
    fn svg_is_sanitised_and_keeps_the_source() {
        let src = br#"<svg><script>x()</script><rect width="2"/></svg>"#;
        match preview("a.svg", src, None, Options::default(), &Limits::default()) {
            Preview::Svg {
                svg,
                source,
                removed,
                removed_names,
            } => {
                assert!(!svg.contains("script"));
                assert!(svg.contains("<rect width=\"2\"/>"));
                assert!(source.contains("<script>"));
                assert_eq!(removed, 1);
                assert_eq!(removed_names, vec!["<script>".to_string()]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bev_container_is_previewed() {
        let mut d = b"RIFF".to_vec();
        d.extend_from_slice(&16u32.to_le_bytes());
        d.extend_from_slice(b"FEV FMT ");
        d.extend_from_slice(&4u32.to_le_bytes());
        d.extend_from_slice(&0x0045_0000u32.to_le_bytes());
        match preview("x.bev", &d, None, Options::default(), &Limits::default()) {
            Preview::Bev {
                version, project, ..
            } => {
                assert_eq!(version, 0x0045_0000);
                assert!(project.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bev_is_listed_and_a_broken_one_is_not_a_bev() {
        let p = preview(
            "a.bev",
            b"junk junk junk",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(!matches!(p, Preview::Bev { .. }), "{p:?}");
    }

    #[test]
    fn broken_svg_is_shown_as_text() {
        let p = preview(
            "a.svg",
            b"<svg><g></svg>",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Text { .. }), "{p:?}");
    }

    #[test]
    fn text_is_shown_and_cut() {
        let big = vec![b'a'; MAX_TEXT_BYTES + 10];
        match preview("a.txt", &big, None, Options::default(), &Limits::default()) {
            Preview::Text { text, truncated } => {
                assert_eq!(text.len(), MAX_TEXT_BYTES);
                assert!(truncated);
            }
            other => panic!("{other:?}"),
        }
        match preview(
            "a.xml",
            b"\xef\xbb\xbf<a/>",
            None,
            Options::default(),
            &Limits::default(),
        ) {
            Preview::Text { text, truncated } => {
                assert_eq!((text.as_str(), truncated), ("<a/>", false))
            }
            other => panic!("{other:?}"),
        }
        let utf16 = [0xff, 0xfe, b'h', 0, b'i', 0];
        match preview(
            "a.txt",
            &utf16,
            None,
            Options::default(),
            &Limits::default(),
        ) {
            Preview::Text { text, .. } => assert_eq!(text, "hi"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn texture_bin_gets_a_guessed_picture_and_candidates() {
        // 16x16 DXT1 = 4x4 blocks of 8 bytes = 128 bytes.
        let data = dxt1_chunk(0, 16);
        let p = preview("t.bin", &data, None, Options::default(), &Limits::default());
        let Preview::Image {
            png,
            candidates,
            index,
            guessed,
            ..
        } = p
        else {
            panic!("expected an image");
        };
        assert!(guessed);
        assert!(png.starts_with(b"\x89PNG"));
        assert!(!candidates.is_empty() && index < candidates.len());
        let other = preview(
            "t.bin",
            &data,
            None,
            Options {
                texture_index: Some(usize::MAX),
            },
            &Limits::default(),
        );
        assert!(matches!(other, Preview::Image { .. }));
        let _ = Dxt::Dxt1;
    }

    #[test]
    fn splat_map_defaults_to_square_rgb565() {
        // 16x16 pixels of 16 bit data = 512 bytes (also valid as 32x32 DXT1).
        let data = dxt1_chunk(0, 64);
        assert_eq!(data.len() - 8, 512);
        let p = preview(
            "2_6_SplatMap_0.(Texture).bin",
            &data,
            None,
            Options::default(),
            &Limits::default(),
        );
        let Preview::Image {
            width,
            height,
            format,
            ..
        } = p
        else {
            panic!("expected an image");
        };
        assert_eq!((width, height, format.as_str()), (16, 16, "RGB565"));
        let q = preview("t.bin", &data, None, Options::default(), &Limits::default());
        let Preview::Image { format, .. } = q else {
            panic!("expected an image");
        };
        assert_eq!(format, "DXT1");
    }

    #[test]
    fn unknown_and_damaged_data_fall_back_to_hex() {
        let p = preview(
            "x.bin",
            b"\x01\x02zz",
            None,
            Options::default(),
            &Limits::default(),
        );
        match p {
            Preview::Binary { len, hex } => {
                assert_eq!(len, 4);
                assert!(hex.starts_with("000000  01 02 7a 7a"), "{hex}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            preview(
                "x.fsb",
                b"FSB5 broken",
                None,
                Options::default(),
                &Limits::default()
            ),
            Preview::Binary { .. }
        ));
        assert!(audio_wav(b"nope", 0, &Limits::default()).is_err());
        assert!(matches!(
            preview("", &[], None, Options::default(), &Limits::default()),
            Preview::Binary { len: 0, .. }
        ));
    }

    /// Builds a tiny geometry file: two triangles on four vertices of `stride` bytes.
    fn tiny_geometry(stride: usize) -> Vec<u8> {
        let mut vb = Vec::new();
        for (i, p) in [
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
        .iter()
        .enumerate()
        {
            let mut v = Vec::new();
            for c in p {
                v.extend(c.to_le_bytes());
            }
            v.extend((i as f32 * 0.25).to_le_bytes());
            v.extend(0.5f32.to_le_bytes());
            v.resize(stride, 0xff);
            vb.extend(v);
        }
        let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let mut d = Vec::new();
        d.extend(0u32.to_le_bytes());
        d.extend((vb.len() as u32).to_le_bytes());
        d.extend(&vb);
        d.extend(4u32.to_le_bytes());
        d.extend(12u32.to_le_bytes());
        for i in idx {
            d.extend(i.to_le_bytes());
        }
        d.extend(2u32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d
    }

    #[test]
    fn geometry_bin_becomes_a_model_and_packs_buffers() {
        let d = tiny_geometry(32);
        match preview("m.bin", &d, None, Options::default(), &Limits::default()) {
            Preview::Model {
                vertices,
                triangles,
                ..
            } => assert_eq!((vertices, triangles), (4, 2)),
            other => panic!("{other:?}"),
        }
        let b = mesh_bytes(&d, &Limits::default()).expect("mesh");
        assert_eq!(&b[0..4], 4u32.to_le_bytes());
        assert_eq!(&b[4..8], 6u32.to_le_bytes());
        assert_eq!(b.len(), 8 + 4 * 12 + 4 * 8 + 6 * 4);
        // Damaged: cut in the middle, and an index past the vertex block.
        assert!(mesh_bytes(&d[..40], &Limits::default()).is_err());
        let mut bad = d.clone();
        let at = bad.len() - 8 - 12; // first index
        bad[at..at + 2].copy_from_slice(&99u16.to_le_bytes());
        assert!(mesh_bytes(&bad, &Limits::default()).is_err());
    }

    #[test]
    fn type_tag_in_the_name_selects_the_preview() {
        let l = Limits::default();
        let model = preview(
            "Rock.(Geometry).bin",
            &tiny_geometry(32),
            None,
            Options::default(),
            &l,
        );
        assert!(matches!(model, Preview::Model { .. }));
        let tex = preview(
            "Rock.(Texture).bin",
            &dxt1_chunk(0, 16),
            None,
            Options::default(),
            &l,
        );
        assert!(matches!(tex, Preview::Image { .. }));
    }

    #[test]
    fn animated_glb_rejects_a_clip_that_is_not_a_clip() {
        let d = tiny_geometry(32);
        let r = mesh_glb_animated(&d, b"not a clip", "x", &Limits::default());
        assert!(r.is_err());
    }

    #[test]
    fn animation_clip_gets_a_header_preview() {
        let mut d = vec![0u8; 8];
        d.extend(30u16.to_le_bytes());
        d.extend(61u16.to_le_bytes());
        d.extend(408u32.to_le_bytes());
        d.extend(46u32.to_le_bytes());
        d.resize(64, 0);
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
        let p = preview(
            "A.Idle.(SkeletalAnimation).bin",
            &d,
            None,
            Options::default(),
            &Limits::default(),
        );
        match p {
            Preview::Animation {
                fps,
                frames,
                seconds,
                bones,
            } => {
                assert_eq!((fps, frames, bones), (30, 61, 46));
                assert!((seconds - 2.0).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
        let junk = preview(
            "A.Idle.(SkeletalAnimation).bin",
            b"junk",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(junk, Preview::Binary { .. }));
    }

    #[test]
    fn decode_text_falls_back_to_windows_1251() {
        assert_eq!(decode_text(&[0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2]), "Привет");
        assert_eq!(decode_text(&[0xa8, 0xb8, 0xb9, 0x85]), "Ёё№…");
        assert_eq!(decode_text("Привет".as_bytes()), "Привет");
        assert_eq!(decode_text(b""), "");
    }

    #[test]
    fn sniff_tells_models_from_textures() {
        let l = Limits::default();
        assert_eq!(sniff("m.bin", &tiny_geometry(32), &l), FileKind::Model);
        assert_eq!(sniff("t.bin", &dxt1_chunk(0, 16), &l), FileKind::Texture);
        assert_eq!(sniff("x.bin", b"junk", &l), FileKind::Other);
        assert_eq!(sniff("a.txt", b"hi", &l), FileKind::Text);
        assert_eq!(sniff("b.bsb", b"", &l), FileKind::Audio);
    }

    #[test]
    fn video_header_becomes_a_preview() {
        let data = crate::ogv::tests::sample();
        let p = preview(
            "Intro.ogv",
            &data,
            None,
            Options::default(),
            &Limits::default(),
        );
        assert_eq!(
            p,
            Preview::Video {
                width: 640,
                height: 480,
                fps: 25.0,
                seconds: Some(12.0 / 25.0),
                frames: Some(12),
            }
        );
    }

    #[test]
    fn png_passes_through() {
        let png = write_png(2, 1, &[0; 8]).expect("png");
        match preview("a.png", &png, None, Options::default(), &Limits::default()) {
            Preview::Image { width, height, .. } => assert_eq!((width, height), (2, 1)),
            other => panic!("{other:?}"),
        }
    }
}

#[cfg(test)]
mod text_sniff_tests {
    use super::*;

    #[test]
    fn plain_text_and_binary_are_told_apart() {
        assert!(looks_like_text(b"Message: ok\r\n"));
        assert!(looks_like_text(&[0xff, 0xfe, b'a', 0]));
        assert!(!looks_like_text(&[]));
        assert!(!looks_like_text(&[1, 2, 3, 0, 0, 65]));
        let p = preview(
            "x.log",
            b"line\n",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Text { .. }));
        let p = preview(
            "x.dat",
            b"hello",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Text { .. }));
        let p = preview(
            "x.dat",
            &[0, 1, 2],
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Binary { .. }));
        let p = preview(
            "a.cfg.bak",
            b"k=v",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Text { .. }));
    }

    #[test]
    fn broken_pictures_fall_back_to_hex() {
        let p = preview(
            "a.ico",
            &[0, 0, 1, 0, 9],
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Binary { .. }));
        let p = preview(
            "a.dds",
            b"DDS xx",
            None,
            Options::default(),
            &Limits::default(),
        );
        assert!(matches!(p, Preview::Binary { .. }));
    }
}
