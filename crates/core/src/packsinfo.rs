//! `Version/DataPacksInfo.bin` from `Version.pak`: a text list of every pak and the files in it.
//!
//! Lines are tab separated. A pak starts with `name`, `file count`, `pak size`; each following
//! line is `path`, `index`, `size`, `attribute`, `stamp`; an empty line ends the pak.

use crate::error::{Error, Result};

const MAX_PAKS: usize = 100_000;
const MAX_FILES_TOTAL: usize = 20_000_000;

/// One listed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// Path inside the pak.
    pub path: String,
    /// Position in the pak list.
    pub index: u32,
    /// Size in bytes.
    pub size: u64,
    /// Attribute column (32, 128 or 0 seen; meaning not confirmed).
    pub attr: u32,
    /// Last column (same value for most files; meaning not confirmed).
    pub stamp: u64,
}

/// One listed pak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackInfo {
    /// Pak file name.
    pub name: String,
    /// Declared number of files.
    pub declared: u32,
    /// Size of the pak file in bytes.
    pub size: u64,
    /// Files.
    pub files: Vec<PackFile>,
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> Result<T> {
    s.trim()
        .parse()
        .map_err(|_| Error::format(format!("DataPacksInfo: bad {what} `{s}`")))
}

/// Parses the list.
///
/// # Errors
///
/// [`Error::Format`] for a line that fits neither shape, [`Error::Limit`] for oversized lists.
pub fn parse(data: &[u8]) -> Result<Vec<PackInfo>> {
    let text =
        std::str::from_utf8(data).map_err(|_| Error::format("DataPacksInfo is not UTF-8"))?;
    let mut paks: Vec<PackInfo> = Vec::new();
    let mut total = 0usize;
    let mut open = false;
    for line in text.lines() {
        if line.is_empty() {
            open = false;
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        match (cols.len(), open) {
            (3, false) => {
                if paks.len() >= MAX_PAKS {
                    return Err(Error::limit("DataPacksInfo lists too many paks"));
                }
                paks.push(PackInfo {
                    name: cols[0].to_owned(),
                    declared: num(cols[1], "file count")?,
                    size: num(cols[2], "pak size")?,
                    files: Vec::new(),
                });
                open = true;
            }
            (5, true) => {
                total += 1;
                if total > MAX_FILES_TOTAL {
                    return Err(Error::limit("DataPacksInfo lists too many files"));
                }
                let f = PackFile {
                    path: cols[0].to_owned(),
                    index: num(cols[1], "index")?,
                    size: num(cols[2], "size")?,
                    attr: num(cols[3], "attribute")?,
                    stamp: num(cols[4], "stamp")?,
                };
                if let Some(p) = paks.last_mut() {
                    p.files.push(f);
                }
            }
            _ => {
                return Err(Error::format(format!(
                    "DataPacksInfo: unexpected line `{line}`"
                )));
            }
        }
    }
    Ok(paks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paks_and_files_are_read() {
        let t = b"A.pak\t2\t100\nx/a.bin\t0\t10\t32\t7\nx/b.bin\t1\t20\t128\t7\n\nB.pak\t0\t5\n";
        let p = parse(t).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].files[1].path, "x/b.bin");
        assert_eq!((p[0].declared, p[0].size, p[1].files.len()), (2, 100, 0));
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(parse(b"A.pak\tx\t1\n").is_err());
        assert!(parse(b"a\tb\n").is_err());
        assert!(parse(b"x\t0\t1\t2\t3\n").is_err());
        assert!(parse(&[0xff, 0xfe]).is_err());
        assert!(parse(b"A.pak\t1\t1\np\t0\t1\t32\n").is_err());
    }
}
