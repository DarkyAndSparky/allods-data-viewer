//! `Profiles/game.version`: the version text and the file manifest of an installed client.
//!
//! Layout (see `docs/formats/game-version.md`): a 4 byte number, the tag `ver5`, the length and
//! text of the version, a 4 byte number, then a zlib stream with the manifest.

use crate::error::{Error, Result};
use crate::limits::Limits;
use std::io::Read;

const MAX_DIRS: u32 = 100_000;
const MAX_FILES: u32 = 1_000_000;
const MAX_VERSION_LEN: usize = 256;

/// One file of the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFile {
    /// File name inside its directory.
    pub name: String,
    /// Size in bytes.
    pub size: u32,
    /// Two 16 byte values stored after the size (meaning not confirmed).
    pub hashes: [u8; 32],
    /// One byte after the hashes (meaning not confirmed).
    pub flag: u8,
}

/// A directory of the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDir {
    /// Directory path such as `/data/Mods/Docs/`.
    pub path: String,
    /// Files in it.
    pub files: Vec<ManifestFile>,
}

/// Parsed `game.version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    /// Tag, for example `ver5`.
    pub tag: String,
    /// Version text, for example `18.0.00.05`.
    pub version: String,
    /// Manifest directories.
    pub dirs: Vec<ManifestDir>,
}

struct Reader<'a> {
    d: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .o
            .checked_add(n)
            .filter(|&e| e <= self.d.len())
            .ok_or_else(|| Error::format(format!("game.version is cut off at {}", self.o)))?;
        let s = self.d.get(self.o..end).unwrap_or_default();
        self.o = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32> {
        let b: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| Error::format("game.version is cut off"))?;
        Ok(u32::from_le_bytes(b))
    }

    fn text(&mut self, max: usize) -> Result<String> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(Error::limit("game.version text is too long"));
        }
        String::from_utf8(self.take(n)?.to_vec())
            .map_err(|_| Error::format("game.version text is not UTF-8"))
    }
}

/// Reads only the version text (no inflation).
///
/// # Errors
///
/// [`Error::Format`] when the header does not fit.
pub fn read_version_text(data: &[u8]) -> Result<(String, String)> {
    let mut r = Reader { d: data, o: 4 };
    let tag = String::from_utf8_lossy(r.take(4)?).into_owned();
    if !tag.starts_with("ver") {
        return Err(Error::format("game.version tag is not `ver*`"));
    }
    Ok((tag, r.text(MAX_VERSION_LEN)?))
}

/// Parses the whole file including the manifest.
///
/// # Errors
///
/// [`Error::Format`] / [`Error::Limit`] for damaged or oversized data.
pub fn parse(data: &[u8], limits: &Limits) -> Result<GameVersion> {
    let (tag, version) = read_version_text(data)?;
    let mut r = Reader {
        d: data,
        o: 12 + version.len(),
    };
    let _unknown = r.u32()?;
    let body = data.get(r.o..).unwrap_or_default();
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(body)
        .take(limits.max_inflated_size + 1)
        .read_to_end(&mut raw)
        .map_err(|e| Error::format(format!("game.version zlib error: {e}")))?;
    if raw.len() as u64 > limits.max_inflated_size {
        return Err(Error::limit(
            "game.version is larger than the inflated size limit",
        ));
    }
    let mut r = Reader { d: &raw, o: 0 };
    let dir_count = r.u32()?;
    if dir_count > MAX_DIRS {
        return Err(Error::limit("game.version has too many directories"));
    }
    let mut dirs = Vec::new();
    for _ in 0..dir_count {
        let _record_size = r.u32()?;
        let path = r.text(limits.max_path_len)?;
        let count = r.u32()?;
        if count > MAX_FILES {
            return Err(Error::limit("game.version directory has too many files"));
        }
        let mut files = Vec::new();
        for _ in 0..count {
            let name = r.text(limits.max_path_len)?;
            let size = r.u32()?;
            let hashes: [u8; 32] = r
                .take(32)?
                .try_into()
                .map_err(|_| Error::format("game.version is cut off"))?;
            let flag = *r.take(1)?.first().unwrap_or(&0);
            files.push(ManifestFile {
                name,
                size,
                hashes,
                flag,
            });
        }
        dirs.push(ManifestDir { path, files });
    }
    Ok(GameVersion { tag, version, dirs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    fn text(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u32).to_le_bytes().to_vec();
        v.extend(s.as_bytes());
        v
    }

    fn sample() -> Vec<u8> {
        let mut m = 1u32.to_le_bytes().to_vec();
        m.extend(20u32.to_le_bytes());
        m.extend(text("/bin/"));
        m.extend(1u32.to_le_bytes());
        m.extend(text("a.dll"));
        m.extend(7u32.to_le_bytes());
        m.extend([9u8; 32]);
        m.push(2);
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&m).unwrap();
        let z = z.finish().unwrap();
        let mut f = 0u32.to_le_bytes().to_vec();
        f.extend(b"ver5");
        f.extend(text("1.2.3"));
        f.extend((z.len() as u32).to_le_bytes());
        f.extend(z);
        f
    }

    #[test]
    fn manifest_is_read() {
        let g = parse(&sample(), &Limits::default()).unwrap();
        assert_eq!((g.tag.as_str(), g.version.as_str()), ("ver5", "1.2.3"));
        assert_eq!(g.dirs[0].path, "/bin/");
        let f = &g.dirs[0].files[0];
        assert_eq!((f.name.as_str(), f.size, f.flag), ("a.dll", 7, 2));
    }

    #[test]
    fn truncated_and_corrupted_data_is_rejected() {
        let s = sample();
        for cut in [0, 5, 10, 14, s.len() - 3] {
            assert!(parse(&s[..cut], &Limits::default()).is_err(), "cut {cut}");
        }
        let mut bad = s.clone();
        bad[4] = b'x';
        assert!(parse(&bad, &Limits::default()).is_err());
        let mut huge = s;
        huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge, &Limits::default()).is_err());
        assert!(read_version_text(&[0; 5]).is_err());
    }
}
