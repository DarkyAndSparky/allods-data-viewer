//! Reader for the community size list `textures_database.bin` ("TDB1" / "TDB2").
//!
//! The list belongs to the texture viewer described in `docs/formats/texture-list-tdb2.md`.
//! A record names a texture path and the candidate index the author chose for it (see
//! [`crate::texheur`]), the block format and the transparent padding.

use crate::error::{Error, Result};
use crate::texture::Dxt;
use std::collections::HashMap;

/// One record of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TdbEntry {
    /// Index into the candidate list of [`crate::texheur::candidate_sizes`].
    pub index: u16,
    /// Block format, `None` when the record says unknown.
    pub format: Option<Dxt>,
    /// Transparent columns on the right.
    pub pad_w: u16,
    /// Transparent rows at the bottom.
    pub pad_h: u16,
}

/// The parsed list; keys are lower-case, `/` separated, with a leading `/`.
#[derive(Debug, Default, Clone)]
pub struct Tdb {
    entries: HashMap<String, TdbEntry>,
}

struct Cur<'a> {
    d: &'a [u8],
    p: usize,
}

impl Cur<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let e = self
            .p
            .checked_add(n)
            .filter(|&e| e <= self.d.len())
            .ok_or_else(|| Error::format("texture list is truncated"))?;
        let s = &self.d[self.p..e];
        self.p = e;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Result<String> {
        // .NET BinaryReader.ReadString: 7-bit encoded length, then UTF-8.
        let (mut len, mut shift) = (0usize, 0u32);
        loop {
            let b = self.take(1)?[0];
            len |= usize::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                return Err(Error::format("bad string length in texture list"));
            }
        }
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}

fn norm(path: &str) -> String {
    let mut s = path.replace('\\', "/").to_lowercase();
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    s
}

fn fmt(code: u8) -> Option<Dxt> {
    match code {
        1 => Some(Dxt::Dxt1),
        2 => Some(Dxt::Dxt3),
        3 => Some(Dxt::Dxt5),
        _ => None,
    }
}

impl Tdb {
    /// Parses an inflated list.
    ///
    /// # Errors
    /// Returns a format error when the data is not a TDB1/TDB2 list.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur { d: data, p: 0 };
        let magic = c.u32()?;
        let v2 = match magic {
            0x3242_4454 => true,
            0x2042_4454 => false,
            _ => return Err(Error::format("not a texture list (bad magic)")),
        };
        let count = c.u32()? as usize;
        let mut entries = HashMap::new();
        for _ in 0..count.min(1_000_000) {
            let name = c.string()?;
            let index = c.u16()?;
            let format = fmt(c.take(1)?[0]);
            let (pad_w, pad_h) = if v2 { (c.u16()?, c.u16()?) } else { (0, 0) };
            entries.insert(
                norm(&name),
                TdbEntry {
                    index,
                    format,
                    pad_w,
                    pad_h,
                },
            );
        }
        Ok(Self { entries })
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Finds the record whose path is a suffix of `path` (at a folder boundary).
    #[must_use]
    pub fn find(&self, path: &str) -> Option<TdbEntry> {
        let p = norm(path);
        let alt = p.replace(".hi.bin", ".bin");
        for cand in [&p, &alt] {
            for (i, _) in cand.match_indices('/') {
                if let Some(e) = self.entries.get(&cand[i..]) {
                    return Some(*e);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, i: u16, f: u8, pw: u16, ph: u16) -> Vec<u8> {
        let mut v = vec![name.len() as u8];
        v.extend_from_slice(name.as_bytes());
        v.extend_from_slice(&i.to_le_bytes());
        v.push(f);
        v.extend_from_slice(&pw.to_le_bytes());
        v.extend_from_slice(&ph.to_le_bytes());
        v
    }

    #[test]
    fn parses_v2_and_finds_by_suffix() {
        let mut d = b"TDB2".to_vec();
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend(rec("/Interface/A/B.(UITexture).bin", 3, 3, 5, 7));
        let t = Tdb::parse(&d).expect("parse");
        let e = t
            .find(r"C:\x\Interface\A\b.(UITexture).bin")
            .expect("found");
        assert_eq!(
            (e.index, e.format, e.pad_w, e.pad_h),
            (3, Some(Dxt::Dxt5), 5, 7)
        );
        assert!(t.find("Other/B.(UITexture).bin").is_none());
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        assert!(Tdb::parse(b"XXXX\0\0\0\0").is_err());
        let mut d = b"TDB2".to_vec();
        d.extend_from_slice(&5u32.to_le_bytes());
        assert!(Tdb::parse(&d).is_err());
    }
}
