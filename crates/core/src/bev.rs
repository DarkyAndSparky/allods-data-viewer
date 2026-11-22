//! `.bev` files: FMOD Designer event projects (`FEV `) as shipped with the game.
//!
//! The file is a zlib stream. The inflated data starts with a block that is not understood
//! (128 bytes in the sample), then a RIFF `FEV ` container follows. Only the parts that were
//! checked against a real file are read: the format version, the project name, the languages
//! and the string table with the event and wave paths. The event data itself (chunk `LGCY`)
//! is listed by size and not decoded. See `docs/formats/fsb4.md`.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::looks_like_zlib;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// How far into the file the RIFF header is searched for.
const RIFF_SEARCH: usize = 1024;
/// Most strings that are read from the string table.
const MAX_STRINGS: usize = 1_000_000;

/// What was read from a `.bev`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bev {
    /// `FMT ` chunk value (`0x00450000` in the sample).
    pub version: u32,
    /// Project name (chunk `PROP`).
    pub project: String,
    /// Language names (chunk `LANG`).
    pub languages: Vec<String>,
    /// String table (chunk `STRR`): suffixes, event names and `/`-paths of events.
    pub strings: Vec<String>,
    /// Every chunk of the project list with its size, in order.
    pub chunks: Vec<(String, u32)>,
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o.checked_add(4)?)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// Reads a `.bev` (zlib-compressed) or an already inflated one.
///
/// # Errors
/// Fails when the data is not a FEV container, or is damaged in a way that makes a chunk
/// run past the end. Chunks that are not understood are only listed.
pub fn parse(data: &[u8], limits: &Limits) -> Result<Bev> {
    let inflated;
    let d: &[u8] = if looks_like_zlib(data) {
        let cap = limits.max_inflated_size;
        let mut out = Vec::new();
        ZlibDecoder::new(data)
            .take(cap + 1)
            .read_to_end(&mut out)
            .map_err(|e| Error::format(format!("bev: {e}")))?;
        if out.len() as u64 > cap {
            return Err(Error::limit("bev inflates beyond the limit"));
        }
        inflated = out;
        &inflated
    } else {
        data
    };
    let head = &d[..d.len().min(RIFF_SEARCH)];
    let start = head
        .windows(12)
        .position(|w| &w[..4] == b"RIFF" && &w[8..12] == b"FEV ")
        .ok_or_else(|| Error::format("bev: no FEV container found"))?;
    let riff_size = u32_at(d, start + 4).ok_or_else(|| Error::format("bev: cut off"))? as usize;
    let end = start
        .checked_add(8)
        .and_then(|s| s.checked_add(riff_size))
        .map_or(d.len(), |e| e.min(d.len()));
    let mut bev = Bev::default();
    let mut pos = start + 12;
    while pos + 8 <= end {
        let tag = &d[pos..pos + 4];
        let size = u32_at(d, pos + 4).ok_or_else(|| Error::format("bev: cut off"))? as usize;
        let body = pos + 8;
        let body_end = body
            .checked_add(size)
            .filter(|e| *e <= end)
            .ok_or_else(|| {
                Error::format(format!("bev: chunk at offset {pos} runs past the end"))
            })?;
        match tag {
            b"FMT " => bev.version = u32_at(d, body).unwrap_or(0),
            b"LIST" if size >= 4 && d.get(body..body + 4) == Some(b"PROJ") => {
                read_project(&d[body + 4..body_end], &mut bev)?;
            }
            _ => bev
                .chunks
                .push((String::from_utf8_lossy(tag).into_owned(), size as u32)),
        }
        pos = body_end + (size & 1);
    }
    Ok(bev)
}

fn read_project(d: &[u8], bev: &mut Bev) -> Result<()> {
    let mut pos = 0usize;
    while pos + 8 <= d.len() {
        let tag = &d[pos..pos + 4];
        let size = u32_at(d, pos + 4).unwrap_or(0) as usize;
        let body = pos + 8;
        let end = body
            .checked_add(size)
            .filter(|e| *e <= d.len())
            .ok_or_else(|| Error::format("bev: project chunk runs past the end"))?;
        let b = &d[body..end];
        match tag {
            b"PROP" => {
                let n = u32_at(b, 0).unwrap_or(0) as usize;
                bev.project = text(
                    b.get(4..4usize.saturating_add(n).min(b.len()))
                        .unwrap_or(&[]),
                );
            }
            b"LANG" => bev.languages = read_languages(b),
            b"STRR" => bev.strings = read_strings(b)?,
            _ => {}
        }
        bev.chunks
            .push((String::from_utf8_lossy(tag).into_owned(), size as u32));
        pos = end + (size & 1);
    }
    Ok(())
}

/// `u32` count, then (`u32` length, text with a NUL) for each.
fn read_languages(b: &[u8]) -> Vec<String> {
    let count = u32_at(b, 0).unwrap_or(0) as usize;
    let mut out = Vec::new();
    let mut pos = 4usize;
    for _ in 0..count.min(64) {
        let Some(n) = u32_at(b, pos) else { break };
        let n = n as usize;
        let Some(s) = b.get(pos + 4..pos.saturating_add(4).saturating_add(n)) else {
            break;
        };
        out.push(text(s));
        pos += 4 + n;
    }
    out
}

/// `u32` count, `count` offsets into the text that follows, then NUL-terminated strings.
fn read_strings(b: &[u8]) -> Result<Vec<String>> {
    let count = u32_at(b, 0).ok_or_else(|| Error::format("bev: string table is cut off"))? as usize;
    if count > MAX_STRINGS {
        return Err(Error::limit("bev: too many strings"));
    }
    let base = count
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .filter(|b0| *b0 <= b.len())
        .ok_or_else(|| Error::format("bev: string table is cut off"))?;
    let blob = &b[base..];
    let mut out = Vec::with_capacity(count.min(blob.len()));
    for i in 0..count {
        let off = u32_at(b, 4 + i * 4).unwrap_or(u32::MAX) as usize;
        let s = blob
            .get(off..)
            .ok_or_else(|| Error::format(format!("bev: string {i} points outside the table")))?;
        out.push(text(s));
    }
    Ok(out)
}

impl Bev {
    /// Strings that are paths (start with `/`) and the other names, in file order.
    #[must_use]
    pub fn split_strings(&self) -> (Vec<String>, Vec<String>) {
        let (paths, names): (Vec<_>, Vec<_>) = self
            .strings
            .iter()
            .cloned()
            .partition(|x| x.starts_with('/'));
        (names, paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut c = tag.to_vec();
        c.extend_from_slice(&(body.len() as u32).to_le_bytes());
        c.extend_from_slice(body);
        if body.len() % 2 == 1 {
            c.push(0);
        }
        c
    }

    fn strr(items: &[&str]) -> Vec<u8> {
        let mut offs = Vec::new();
        let mut blob = Vec::new();
        for s in items {
            offs.push(blob.len() as u32);
            blob.extend_from_slice(s.as_bytes());
            blob.push(0);
        }
        let mut b = (items.len() as u32).to_le_bytes().to_vec();
        for o in offs {
            b.extend_from_slice(&o.to_le_bytes());
        }
        b.extend_from_slice(&blob);
        b
    }

    fn sample() -> Vec<u8> {
        let mut prop = 5u32.to_le_bytes().to_vec();
        prop.extend_from_slice(b"Demo\0");
        let mut lang = 2u32.to_le_bytes().to_vec();
        for l in ["english", "russian"] {
            lang.extend_from_slice(&(l.len() as u32 + 1).to_le_bytes());
            lang.extend_from_slice(l.as_bytes());
            lang.push(0);
        }
        let mut list = b"PROJ".to_vec();
        list.extend(chunk(b"PROP", &prop));
        list.extend(chunk(b"LGCY", &[1, 2, 3]));
        list.extend(chunk(b"STRR", &strr(&["_rus", "Boss1", "/zones/A/Say1"])));
        list.extend(chunk(b"LANG", &lang));
        let mut inner = b"FEV ".to_vec();
        inner.extend(chunk(b"FMT ", &0x0045_0000u32.to_le_bytes()));
        inner.extend(chunk(b"LIST", &list));
        let mut d = vec![0xAAu8; 128];
        d.extend_from_slice(b"RIFF");
        d.extend_from_slice(&(inner.len() as u32).to_le_bytes());
        d.extend(inner);
        d
    }

    fn zlib(d: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(d).expect("write");
        e.finish().expect("finish")
    }

    #[test]
    fn reads_project_languages_and_strings() {
        for data in [sample(), zlib(&sample())] {
            let b = parse(&data, &Limits::default()).expect("parse");
            assert_eq!(b.version, 0x0045_0000);
            assert_eq!(b.project, "Demo");
            assert_eq!(b.languages, ["english", "russian"]);
            assert_eq!(b.strings, ["_rus", "Boss1", "/zones/A/Say1"]);
            assert!(b.chunks.iter().any(|(t, s)| t == "LGCY" && *s == 3));
            let (names, paths) = b.split_strings();
            assert_eq!(names, ["_rus", "Boss1"]);
            assert_eq!(paths, ["/zones/A/Say1"]);
        }
    }

    #[test]
    fn data_without_a_container_is_an_error() {
        assert!(parse(b"", &Limits::default()).is_err());
        assert!(parse(&[7u8; 300], &Limits::default()).is_err());
        assert!(parse(&zlib(&[7u8; 300]), &Limits::default()).is_err());
    }

    #[test]
    fn truncated_files_are_errors_or_partial_never_a_panic() {
        let d = sample();
        for cut in 0..d.len() {
            let _ = parse(&d[..cut], &Limits::default());
        }
        let z = zlib(&d);
        for cut in 0..z.len() {
            let _ = parse(&z[..cut], &Limits::default());
        }
    }

    #[test]
    fn damaged_string_table_is_refused() {
        let mut d = sample();
        let at = d.windows(4).position(|w| w == b"STRR").expect("strr") + 8;
        let mut bad = d.clone();
        bad[at..at + 4].copy_from_slice(&0x00ff_ffffu32.to_le_bytes()); // count too large for the chunk
        assert!(parse(&bad, &Limits::default()).is_err());
        d[at + 4..at + 8].copy_from_slice(&0x7fff_0000u32.to_le_bytes()); // first offset outside
        assert!(parse(&d, &Limits::default()).is_err());
        let mut huge = sample();
        let at = huge.windows(4).position(|w| w == b"STRR").expect("strr") + 8;
        huge[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge, &Limits::default()).is_err());
    }

    #[test]
    fn chunk_past_the_end_is_refused() {
        let mut d = sample();
        let at = d.windows(4).position(|w| w == b"LGCY").expect("lgcy") + 4;
        d[at..at + 4].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
        assert!(parse(&d, &Limits::default()).is_err());
    }

    #[test]
    fn random_damage_never_panics() {
        let base = sample();
        let mut seed = 5u32;
        for _ in 0..2000 {
            let mut d = base.clone();
            for _ in 0..5 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let i = (seed >> 8) as usize % d.len();
                d[i] = (seed >> 24) as u8;
            }
            let _ = parse(&d, &Limits::default());
        }
    }

    #[test]
    fn inflate_has_an_output_cap() {
        let z = zlib(&vec![0u8; 100_000]);
        let limits = Limits {
            max_inflated_size: 1000,
            ..Limits::default()
        };
        assert!(parse(&z, &limits).is_err());
    }
}
