//! Readers for `.loc` localization files (both layouts).
//!
//! The format notes are in `docs/formats/loc.md`. Input is the payload after the zlib
//! layer; [`read_loc`] removes that layer when present.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::looks_like_zlib;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Which layout a file uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocLayout {
    /// 32 bit layout with keys.
    V1,
    /// 64 bit layout without keys.
    V2,
}

/// One text, with its key when the layout stores keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocItem {
    /// Item index.
    pub id: u32,
    /// Resource name; `None` for layout 2.
    pub key: Option<String>,
    /// The text.
    pub text: String,
}

/// A parsed `.loc` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocFile {
    /// Detected layout.
    pub layout: LocLayout,
    /// All items in file order.
    pub items: Vec<LocItem>,
}

fn u32_at(d: &[u8], pos: usize) -> Result<u32> {
    let b = d
        .get(
            pos..pos
                .checked_add(4)
                .ok_or_else(|| Error::format("offset overflow"))?,
        )
        .ok_or_else(|| Error::format(format!("read past end at {pos}")))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_at(d: &[u8], pos: usize) -> Result<u64> {
    let b = d
        .get(
            pos..pos
                .checked_add(8)
                .ok_or_else(|| Error::format("offset overflow"))?,
        )
        .ok_or_else(|| Error::format(format!("read past end at {pos}")))?;
    let mut a = [0u8; 8];
    a.copy_from_slice(b);
    Ok(u64::from_le_bytes(a))
}

fn to_usize(v: u64) -> Result<usize> {
    usize::try_from(v).map_err(|_| Error::format("value does not fit in memory"))
}

/// Decodes `units` UTF-16LE code units at `pos` and checks the NUL terminator.
fn utf16_at(d: &[u8], pos: usize, units: usize) -> Result<String> {
    let bytes = units
        .checked_mul(2)
        .ok_or_else(|| Error::format("text length overflow"))?;
    let end = pos
        .checked_add(bytes)
        .and_then(|e| e.checked_add(2))
        .ok_or_else(|| Error::format("text offset overflow"))?;
    let raw = d
        .get(pos..end)
        .ok_or_else(|| Error::format(format!("text at {pos} runs past end of file")))?;
    if raw[bytes] != 0 || raw[bytes + 1] != 0 {
        return Err(Error::format(format!("text at {pos} lacks NUL terminator")));
    }
    let words: Vec<u16> = raw[..bytes]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok(String::from_utf16_lossy(&words))
}

/// Parses an already inflated payload.
///
/// # Errors
///
/// Returns [`Error::Format`] when the data does not match either layout, and
/// [`Error::Limit`] for absurd item counts.
pub fn parse_loc(d: &[u8], limits: &Limits) -> Result<LocFile> {
    if u32_at(d, 0)? == 0 {
        parse_v1(d, limits)
    } else {
        parse_v2(d, limits)
    }
}

fn parse_v1(d: &[u8], limits: &Limits) -> Result<LocFile> {
    if u32_at(d, 8)? != 8 {
        return Err(Error::format("not a .loc: header word at 8 is not 8"));
    }
    let sizes_pos = to_usize(u64::from(u32_at(d, 4)?))?;
    let count = to_usize(u64::from(u32_at(d, 12)?))?;
    if count as u64 > limits.max_entries || count.saturating_mul(12) > d.len() {
        return Err(Error::limit(format!("{count} items does not fit the file")));
    }
    let sizes = 16usize
        .checked_add(sizes_pos)
        .ok_or_else(|| Error::format("sizes_pos overflow"))?;
    let sizes_end = count
        .checked_mul(8)
        .and_then(|n| n.checked_add(sizes))
        .ok_or_else(|| Error::format("size table overflow"))?;
    if sizes_end > d.len() {
        return Err(Error::format("size table runs past end of file"));
    }
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let rec = 16 + i * 12;
        let pos = to_usize(u64::from(u32_at(d, rec)?))?;
        let str_size = to_usize(u64::from(u32_at(d, rec + 4)?))?;
        let id = u32_at(d, rec + 8)?;
        let kstart = rec
            .checked_add(pos)
            .ok_or_else(|| Error::format("key offset overflow"))?;
        let kend = kstart
            .checked_add(str_size)
            .ok_or_else(|| Error::format("key size overflow"))?;
        let kraw = d
            .get(kstart..kend)
            .ok_or_else(|| Error::format(format!("key of item {i} runs past end")))?;
        let kraw = kraw.strip_suffix(&[0]).unwrap_or(kraw);
        let key = String::from_utf8_lossy(kraw).into_owned();
        let srec = sizes + i * 8;
        let length = to_usize(u64::from(u32_at(d, srec)?))?;
        let tpos = to_usize(u64::from(u32_at(d, srec + 4)?))?;
        let tstart = sizes_end
            .checked_add(8)
            .and_then(|s| s.checked_add(tpos))
            .ok_or_else(|| Error::format("text offset overflow"))?;
        let text = utf16_at(d, tstart, length)?;
        items.push(LocItem {
            id,
            key: Some(key),
            text,
        });
    }
    Ok(LocFile {
        layout: LocLayout::V1,
        items,
    })
}

fn parse_v2(d: &[u8], limits: &Limits) -> Result<LocFile> {
    let words = to_usize(u64_at(d, 8)?)?;
    if !words.is_multiple_of(2) {
        return Err(Error::format("descriptor word count is odd"));
    }
    let count = words / 2;
    if count as u64 > limits.max_entries || count.saturating_mul(16) > d.len() {
        return Err(Error::limit(format!("{count} items does not fit the file")));
    }
    let base = count
        .checked_mul(16)
        .and_then(|n| n.checked_add(16 + 12))
        .ok_or_else(|| Error::format("descriptor table overflow"))?;
    if base > d.len() {
        return Err(Error::format("descriptor table runs past end of file"));
    }
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let length = to_usize(u64_at(d, 16 + i * 16)?)?;
        let offset = to_usize(u64_at(d, 16 + i * 16 + 8)?)?;
        let pos = base
            .checked_add(offset)
            .ok_or_else(|| Error::format("text offset overflow"))?;
        let text = utf16_at(d, pos, length)?;
        items.push(LocItem {
            id: u32::try_from(i).map_err(|_| Error::limit("too many items"))?,
            key: None,
            text,
        });
    }
    Ok(LocFile {
        layout: LocLayout::V2,
        items,
    })
}

/// Reads a `.loc` file as stored on disk or in a `.pak`: removes the zlib layer when
/// present, then parses.
///
/// # Errors
///
/// Fails on I/O errors, size limits, or malformed data.
pub fn read_loc(mut input: impl Read, limits: &Limits) -> Result<LocFile> {
    let cap = limits.max_inflated_size;
    let mut raw = Vec::new();
    (&mut input).take(cap + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > cap {
        return Err(Error::limit("file too large"));
    }
    if looks_like_zlib(&raw) {
        let mut out = Vec::new();
        ZlibDecoder::new(raw.as_slice())
            .take(cap + 1)
            .read_to_end(&mut out)
            .map_err(|e| Error::format(format!("zlib: {e}")))?;
        if out.len() as u64 > cap {
            return Err(Error::limit("inflated .loc too large"));
        }
        return parse_loc(&out, limits);
    }
    parse_loc(&raw, limits)
}

#[cfg(test)]
pub(crate) mod testutil {
    fn utf16(s: &str) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
        v.extend_from_slice(&[0, 0]);
        while !v.len().is_multiple_of(4) {
            v.push(0);
        }
        v
    }

    /// Builds a layout 1 payload following `docs/formats/loc.md`.
    pub fn build_v1(items: &[(&str, &str)]) -> Vec<u8> {
        let count = items.len();
        let mut keys = Vec::new();
        let mut key_pos = Vec::new();
        let table_len = count * 12;
        for (i, (k, _)) in items.iter().enumerate() {
            // key address is relative to the record itself
            let abs = table_len + keys.len();
            key_pos.push((abs - i * 12, k.len() + 1));
            keys.extend_from_slice(k.as_bytes());
            keys.push(0);
        }
        while !keys.len().is_multiple_of(4) {
            keys.push(0);
        }
        let sizes_pos = table_len + keys.len();
        let mut texts = Vec::new();
        let mut sizes = Vec::new();
        for (_, t) in items {
            sizes.push((t.encode_utf16().count(), texts.len()));
            texts.extend_from_slice(&utf16(t));
        }
        let mut out = Vec::new();
        for v in [0u32, sizes_pos as u32, 8, count as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for (i, (pos, size)) in key_pos.iter().enumerate() {
            for v in [*pos as u32, *size as u32, i as u32] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&keys);
        for (len, pos) in sizes {
            out.extend_from_slice(&(len as u32).to_le_bytes());
            out.extend_from_slice(&(pos as u32).to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 8]);
        out.extend_from_slice(&texts);
        out
    }

    /// Builds a layout 2 payload.
    pub fn build_v2(texts: &[&str]) -> Vec<u8> {
        let mut area = Vec::new();
        let mut desc = Vec::new();
        for t in texts {
            let len = t.encode_utf16().count() as u64;
            desc.push((len, area.len() as u64));
            let mut s: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
            s.extend_from_slice(&[0, 0]);
            while !s.len().is_multiple_of(8) {
                s.push(0);
            }
            area.extend_from_slice(&s);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&0x2689_119Fu64.to_le_bytes());
        out.extend_from_slice(&((texts.len() * 2) as u64).to_le_bytes());
        for (l, o) in desc {
            out.extend_from_slice(&l.to_le_bytes());
            out.extend_from_slice(&o.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(&area);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    #[test]
    fn v1_round_trip() {
        let data = build_v1(&[("A/b.txt", "Привет"), ("C.txt", "x\r\ny")]);
        let f = parse_loc(&data, &Limits::default()).expect("parse");
        assert_eq!(f.layout, LocLayout::V1);
        assert_eq!(f.items.len(), 2);
        assert_eq!(f.items[0].key.as_deref(), Some("A/b.txt"));
        assert_eq!(f.items[0].text, "Привет");
        assert_eq!(f.items[1].text, "x\r\ny");
    }

    #[test]
    fn v2_round_trip() {
        let data = build_v2(&["one", "два", ""]);
        let f = parse_loc(&data, &Limits::default()).expect("parse");
        assert_eq!(f.layout, LocLayout::V2);
        let texts: Vec<_> = f.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, ["one", "два", ""]);
        assert!(f.items[0].key.is_none());
    }

    #[test]
    fn truncated_files_never_panic() {
        for data in [build_v1(&[("k", "text")]), build_v2(&["text", "more"])] {
            for cut in 0..data.len() {
                let _ = parse_loc(&data[..cut], &Limits::default());
            }
        }
    }

    #[test]
    fn corrupted_bytes_never_panic() {
        let base = build_v2(&["alpha", "beta"]);
        for i in 0..base.len() {
            let mut d = base.clone();
            d[i] ^= 0xFF;
            let _ = parse_loc(&d, &Limits::default());
        }
        let base = build_v1(&[("k1", "alpha"), ("k2", "beta")]);
        for i in 0..base.len() {
            let mut d = base.clone();
            d[i] ^= 0xFF;
            let _ = parse_loc(&d, &Limits::default());
        }
    }

    #[test]
    fn huge_counts_are_rejected() {
        let mut d = vec![0u8; 16];
        d[8] = 8;
        d[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            parse_loc(&d, &Limits::default()),
            Err(Error::Limit(_))
        ));
        let mut d = vec![0u8; 32];
        d[0] = 1;
        d[8..16].copy_from_slice(&(u64::MAX - 1).to_le_bytes());
        assert!(parse_loc(&d, &Limits::default()).is_err());
    }

    #[test]
    fn read_loc_removes_zlib_layer() {
        let payload = build_v2(&["zipped"]);
        let packed = crate::pak::testutil::zlib(&payload);
        let f = read_loc(packed.as_slice(), &Limits::default()).expect("read");
        assert_eq!(f.items[0].text, "zipped");
        let f = read_loc(payload.as_slice(), &Limits::default()).expect("plain");
        assert_eq!(f.items[0].text, "zipped");
    }

    #[test]
    fn missing_terminator_is_an_error() {
        let mut d = build_v2(&["abc"]);
        let pos = d.len() - 8 + 6; // terminator of "abc" (3 units = 6 bytes)
        d[pos] = 1;
        assert!(parse_loc(&d, &Limits::default()).is_err());
    }
}
