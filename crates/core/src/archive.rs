//! Archives other than the game's ZIP-based `.pak`: `.tar`, `.tar.gz` / `.tgz`, `.tar.xz` / `.txz`,
//! a single `.gz` or `.xz`, and `.7z`.
//!
//! One front type, [`Archive`], hides the difference from the session. Everything inside is
//! untrusted: header sizes are checked against the real length, paths go through [`VPath`],
//! inflating has an output cap and a ratio guard, and a damaged header ends the listing
//! (earlier entries stay readable and the reason is kept in [`Archive::skipped`]).

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::{Entry, Layer, PakArchive, Skipped};
use crate::vpath::VPath;
use flate2::read::MultiGzDecoder;
use lzma_rust2::XzReader;
use sevenz_rust2::{ArchiveReader, Password};
use std::collections::HashSet;
use std::io::{Cursor, Read, Seek, SeekFrom};

const BLOCK: usize = 512;
/// Longest GNU long name or pax header that is read.
const MAX_META: u64 = 64 * 1024;

/// The kind of archive, from the file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `.pak` or `.zip`.
    Zip,
    /// `.tar`.
    Tar,
    /// `.tar.gz` or `.tgz`.
    TarGz,
    /// `.gz` (a tar inside is detected by content).
    Gz,
    /// `.tar.xz` or `.txz`.
    TarXz,
    /// `.xz` (a tar inside is detected by content).
    Xz,
    /// `.7z`.
    SevenZ,
}

impl Kind {
    /// Tells the kind by the file name; `None` for anything else.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let l = name.to_ascii_lowercase();
        if l.ends_with(".pak") || l.ends_with(".zip") {
            Some(Self::Zip)
        } else if l.ends_with(".tar.gz") || l.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if l.ends_with(".tar.xz") || l.ends_with(".txz") {
            Some(Self::TarXz)
        } else if l.ends_with(".xz") {
            Some(Self::Xz)
        } else if l.ends_with(".7z") {
            Some(Self::SevenZ)
        } else if l.ends_with(".tar") {
            Some(Self::Tar)
        } else if l.ends_with(".gz") {
            Some(Self::Gz)
        } else {
            None
        }
    }
}

/// An opened archive of any supported kind.
pub enum Archive<R: Read + Seek> {
    /// A ZIP-based archive.
    Zip(PakArchive<R>),
    /// A tar file read in place.
    Tar(TarArchive<R>),
    /// A tar that was inflated from gzip into memory.
    TarMem(TarArchive<Cursor<Vec<u8>>>),
    /// One compressed file (not a tar).
    Single(SingleFile),
    /// A 7-Zip archive.
    SevenZ(Box<SevenZArchive<R>>),
}

/// Memory limit of the 7z decoder for header buffers and dictionaries, in KiB.
const SEVENZ_MEMORY_KB: usize = 256 * 1024;

/// A 7z archive: the listing comes from the header; an entry is decoded on demand.
///
/// Solid blocks are decoded from their start up to the entry, so reading the last file of a
/// large solid block is as slow as reading all of it.
pub struct SevenZArchive<R: Read + Seek> {
    reader: ArchiveReader<R>,
    entries: Vec<Entry>,
    /// Stored name of every entry (`Entry::index` points here).
    names: Vec<String>,
    skipped: Vec<Skipped>,
    limits: Limits,
}

impl<R: Read + Seek> SevenZArchive<R> {
    fn open(mut reader: R, limits: Limits) -> Result<Self> {
        let packed = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;
        let sz = ArchiveReader::new_with_memory_limit(reader, Password::empty(), SEVENZ_MEMORY_KB)
            .map_err(|e| Error::format(format!("7z: {e}")))?;
        let mut entries = Vec::new();
        let mut names = Vec::new();
        let mut skipped = Vec::new();
        let mut seen = HashSet::new();
        for f in sz.archive().files.iter() {
            if f.is_directory() || f.is_anti_item() {
                continue;
            }
            if entries.len() as u64 >= limits.max_entries {
                return Err(Error::limit(format!(
                    "7z: more than {} entries",
                    limits.max_entries
                )));
            }
            let name = f.name().to_owned();
            if f.size() > limits.max_inflated_size {
                skipped.push(Skipped {
                    name,
                    reason: format!("7z: size above {} bytes", limits.max_inflated_size),
                });
                continue;
            }
            match VPath::parse(&name.replace('\\', "/"), limits.max_path_len) {
                Ok(path) => {
                    if seen.insert(path.clone()) {
                        let index = names.len();
                        names.push(name);
                        entries.push(Entry::external(path, f.size(), packed, index));
                    } else {
                        skipped.push(Skipped {
                            name,
                            reason: "7z: the name is used twice".into(),
                        });
                    }
                }
                Err(e) => skipped.push(Skipped {
                    name,
                    reason: e.to_string(),
                }),
            }
        }
        Ok(Self {
            reader: sz,
            entries,
            names,
            skipped,
            limits,
        })
    }

    fn read(&mut self, entry: &Entry) -> Result<Vec<u8>> {
        let name = self
            .names
            .get(entry.index())
            .ok_or_else(|| Error::format("no such entry"))?
            .clone();
        let cap = entry.size.min(self.limits.max_inflated_size);
        let mut out = Vec::new();
        let mut found = false;
        self.reader
            .for_each_entries_with_decision(|e, r| {
                if e.name() != name {
                    return Ok(sevenz_rust2::EntryDecision::Continue);
                }
                found = true;
                r.take(cap.saturating_add(1)).read_to_end(&mut out)?;
                Ok(sevenz_rust2::EntryDecision::Stop)
            })
            .map_err(|e| Error::format(format!("7z: {name}: {e}")))?;
        if !found {
            return Err(Error::format("7z: entry not found"));
        }
        if out.len() as u64 > cap {
            return Err(Error::limit(format!(
                "7z: {name} is larger than its header says"
            )));
        }
        Ok(out)
    }
}

/// The content of a `.gz` that is not a tar.
pub struct SingleFile {
    entries: Vec<Entry>,
    data: Vec<u8>,
}

impl<R: Read + Seek> Archive<R> {
    /// Opens `reader`; `name` (the file name) decides the kind.
    ///
    /// # Errors
    /// Fails for an unknown extension, a damaged archive, or when a limit is exceeded.
    pub fn open(mut reader: R, name: &str, limits: Limits) -> Result<Self> {
        match Kind::from_name(name) {
            Some(Kind::Zip) => Ok(Self::Zip(PakArchive::open(reader, limits)?)),
            Some(Kind::Tar) => Ok(Self::Tar(TarArchive::open(reader, limits)?)),
            Some(kind @ (Kind::TarGz | Kind::Gz | Kind::TarXz | Kind::Xz)) => {
                let packed = reader.seek(SeekFrom::End(0))?;
                reader.seek(SeekFrom::Start(0))?;
                let cap = limits.max_inflated_size;
                let (label, mut data) = (
                    if matches!(kind, Kind::TarXz | Kind::Xz) {
                        "xz"
                    } else {
                        "gzip"
                    },
                    Vec::new(),
                );
                let res = if label == "xz" {
                    XzReader::new_mem_limit(reader, true, SEVENZ_MEMORY_KB as u32)
                        .take(cap.saturating_add(1))
                        .read_to_end(&mut data)
                } else {
                    MultiGzDecoder::new(reader)
                        .take(cap.saturating_add(1))
                        .read_to_end(&mut data)
                };
                res.map_err(|e| Error::format(format!("{label}: {e}")))?;
                let size = data.len() as u64;
                if size > cap {
                    return Err(Error::limit(format!("{label} inflates beyond {cap} bytes")));
                }
                if size > limits.ratio_floor && size / packed.max(1) > limits.max_ratio {
                    return Err(Error::limit(format!(
                        "{label} compression ratio above {}:1",
                        limits.max_ratio
                    )));
                }
                if looks_like_tar(&data) {
                    return Ok(Self::TarMem(TarArchive::open(Cursor::new(data), limits)?));
                }
                let stem = name.rsplit('/').next().unwrap_or(name);
                let lower = stem.to_ascii_lowercase();
                let inner = lower
                    .strip_suffix(".gz")
                    .or_else(|| lower.strip_suffix(".xz"))
                    .or_else(|| lower.strip_suffix(".tgz"))
                    .or_else(|| lower.strip_suffix(".txz"))
                    .map_or(stem, |s| &stem[..s.len()]);
                let path = VPath::parse(
                    if inner.is_empty() { "data" } else { inner },
                    limits.max_path_len,
                )?;
                let entries = vec![Entry::external(path, size, packed, 0)];
                Ok(Self::Single(SingleFile { entries, data }))
            }
            Some(Kind::SevenZ) => Ok(Self::SevenZ(Box::new(SevenZArchive::open(reader, limits)?))),
            None => Err(Error::unsupported(format!(
                "{name}: not a supported archive"
            ))),
        }
    }

    /// Accepted entries in archive order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Zip(a) => a.entries(),
            Self::Tar(a) => a.entries(),
            Self::TarMem(a) => a.entries(),
            Self::Single(s) => &s.entries,
            Self::SevenZ(a) => &a.entries,
        }
    }

    /// Entries that were not accepted, with the reason.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        match self {
            Self::Zip(a) => a.skipped(),
            Self::Tar(a) => a.skipped(),
            Self::TarMem(a) => a.skipped(),
            Self::Single(_) => &[],
            Self::SevenZ(a) => &a.skipped,
        }
    }

    /// Reads an entry (see [`PakArchive::read`] for `layer`; only ZIP has an extra zlib layer).
    ///
    /// # Errors
    /// Fails on I/O errors or when a size limit is exceeded.
    pub fn read(&mut self, entry: &Entry, layer: Layer) -> Result<(Vec<u8>, Layer)> {
        match self {
            Self::Zip(a) => a.read(entry, layer),
            Self::Tar(a) => a.read(entry),
            Self::TarMem(a) => a.read(entry),
            Self::SevenZ(a) => Ok((a.read(entry)?, Layer::Raw)),
            Self::Single(s) => {
                if entry.index() == 0 {
                    Ok((s.data.clone(), Layer::Raw))
                } else {
                    Err(Error::format("no such entry"))
                }
            }
        }
    }
}

/// A tar archive: the headers are indexed once, data is read on demand.
pub struct TarArchive<R: Read + Seek> {
    reader: R,
    entries: Vec<Entry>,
    /// Data offset and size of every entry (`Entry::index` points here).
    spans: Vec<(u64, u64)>,
    skipped: Vec<Skipped>,
    limits: Limits,
}

fn looks_like_tar(data: &[u8]) -> bool {
    data.get(..BLOCK)
        .is_some_and(|h| h.get(257..262) == Some(b"ustar") && header_checksum_ok(h))
}

fn header_checksum_ok(h: &[u8]) -> bool {
    let Some(stored) = h.get(148..156).and_then(parse_octal) else {
        return false;
    };
    let sum: u64 = h
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if (148..156).contains(&i) {
                32
            } else {
                u64::from(b)
            }
        })
        .sum();
    sum == stored
}

/// Octal number padded with NULs or spaces.
fn parse_octal(field: &[u8]) -> Option<u64> {
    let text: Vec<u8> = field
        .iter()
        .copied()
        .skip_while(|&b| b == b' ')
        .take_while(|&b| b != 0 && b != b' ')
        .collect();
    if text.is_empty() {
        return Some(0);
    }
    let mut v = 0u64;
    for b in text {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        v = v.checked_mul(8)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(v)
}

/// Size field: octal, or base-256 when the top bit of the first byte is set.
fn parse_size(field: &[u8]) -> Option<u64> {
    if field.first().is_some_and(|b| b & 0x80 != 0) {
        let mut v = u64::from(field[0] & 0x7f);
        for &b in &field[1..] {
            v = v.checked_mul(256)?.checked_add(u64::from(b))?;
        }
        return Some(v);
    }
    parse_octal(field)
}

fn c_string(field: &[u8]) -> &[u8] {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    &field[..end]
}

/// `path` and `size` records of a pax extended header.
fn parse_pax(data: &[u8]) -> (Option<String>, Option<u64>) {
    let (mut path, mut size) = (None, None);
    let mut rest = data;
    while !rest.is_empty() {
        let Some(sp) = rest.iter().position(|&b| b == b' ') else {
            break;
        };
        let Some(len) = std::str::from_utf8(&rest[..sp])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            break;
        };
        if len <= sp + 1 || len > rest.len() {
            break;
        }
        let record = &rest[sp + 1..len];
        let record = record.strip_suffix(b"\n").unwrap_or(record);
        if let Some(eq) = record.iter().position(|&b| b == b'=') {
            let (k, v) = (&record[..eq], &record[eq + 1..]);
            match k {
                b"path" => path = String::from_utf8(v.to_vec()).ok(),
                b"size" => size = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok()),
                _ => {}
            }
        }
        rest = &rest[len..];
    }
    (path, size)
}

impl<R: Read + Seek> TarArchive<R> {
    /// Indexes the headers of a tar file.
    ///
    /// # Errors
    /// Fails when the first header is not a tar header or a limit is exceeded. A damaged later
    /// header ends the listing and is recorded in [`TarArchive::skipped`].
    pub fn open(mut reader: R, limits: Limits) -> Result<Self> {
        let len = reader.seek(SeekFrom::End(0))?;
        let mut entries: Vec<Entry> = Vec::new();
        let mut spans = Vec::new();
        let mut skipped = Vec::new();
        let mut seen = HashSet::new();
        let mut pos = 0u64;
        let mut long_name: Option<String> = None;
        let mut pax_size: Option<u64> = None;
        let mut header = [0u8; BLOCK];
        let mut first = true;
        while pos.checked_add(BLOCK as u64).is_some_and(|e| e <= len) {
            reader.seek(SeekFrom::Start(pos))?;
            reader.read_exact(&mut header)?;
            if header.iter().all(|&b| b == 0) {
                break;
            }
            if !header_checksum_ok(&header) {
                if first {
                    return Err(Error::format(
                        "tar: the first header is damaged or this is not a tar file",
                    ));
                }
                skipped.push(Skipped {
                    name: String::new(),
                    reason: format!("tar: damaged header at offset {pos}; the rest is not listed"),
                });
                break;
            }
            first = false;
            let size = pax_size
                .take()
                .or_else(|| parse_size(&header[124..136]))
                .ok_or_else(|| Error::format(format!("tar: bad size at offset {pos}")))?;
            let data_start = pos + BLOCK as u64;
            let padded = size.div_ceil(BLOCK as u64).checked_mul(BLOCK as u64);
            let next = padded.and_then(|p| data_start.checked_add(p));
            let type_flag = header[156];
            let ustar = header.get(257..262) == Some(b"ustar");
            let mut name = String::from_utf8_lossy(c_string(&header[0..100])).into_owned();
            if ustar {
                let prefix = c_string(&header[345..500]);
                if !prefix.is_empty() {
                    name = format!("{}/{name}", String::from_utf8_lossy(prefix));
                }
            }
            // The data must be inside the file (the last block may lack padding).
            let end = data_start.checked_add(size);
            if end.is_none_or(|e| e > len) {
                skipped.push(Skipped {
                    name,
                    reason: format!("tar: data is cut off at offset {data_start}"),
                });
                break;
            }
            let take_meta = |reader: &mut R| -> Result<Vec<u8>> {
                if size > MAX_META {
                    return Err(Error::limit("tar: a name or extended header is too long"));
                }
                reader.seek(SeekFrom::Start(data_start))?;
                let mut buf = vec![0u8; size as usize];
                reader.read_exact(&mut buf)?;
                Ok(buf)
            };
            match type_flag {
                b'L' => {
                    let buf = take_meta(&mut reader)?;
                    long_name = Some(String::from_utf8_lossy(c_string(&buf)).into_owned());
                }
                b'x' => {
                    let buf = take_meta(&mut reader)?;
                    let (p, s) = parse_pax(&buf);
                    if p.is_some() {
                        long_name = p;
                    }
                    pax_size = s;
                }
                b'g' | b'K' | b'5' => {}
                b'0' | 0 | b'7' => {
                    let name = long_name.take().unwrap_or(name);
                    if name.ends_with('/') {
                        // an old-style directory entry
                    } else if let Some(reason) =
                        Self::reject(&name, size, &mut seen, &entries, &limits)
                    {
                        skipped.push(Skipped { name, reason });
                    } else {
                        match VPath::parse(&name, limits.max_path_len) {
                            Ok(path) => {
                                spans.push((data_start, size));
                                entries.push(Entry::external(path, size, size, spans.len() - 1));
                            }
                            Err(e) => skipped.push(Skipped {
                                name,
                                reason: e.to_string(),
                            }),
                        }
                    }
                }
                other => {
                    let name = long_name.take().unwrap_or(name);
                    skipped.push(Skipped {
                        name,
                        reason: format!(
                            "tar: entry type {:?} is not a plain file",
                            char::from(other)
                        ),
                    });
                }
            }
            let Some(next) = next else {
                break;
            };
            pos = next;
        }
        Ok(Self {
            reader,
            entries,
            spans,
            skipped,
            limits,
        })
    }

    fn reject(
        name: &str,
        size: u64,
        seen: &mut HashSet<String>,
        entries: &[Entry],
        limits: &Limits,
    ) -> Option<String> {
        if entries.len() as u64 >= limits.max_entries {
            return Some(format!("more than {} entries", limits.max_entries));
        }
        if size > limits.max_entry_size {
            return Some(format!("size {size} exceeds {}", limits.max_entry_size));
        }
        if !seen.insert(name.to_owned()) {
            return Some("duplicate path".to_owned());
        }
        None
    }

    /// Accepted entries in archive order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entries that were not accepted, with the reason.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Reads the data of an entry.
    ///
    /// # Errors
    /// Fails on I/O errors or when the entry is larger than the limit.
    pub fn read(&mut self, entry: &Entry) -> Result<(Vec<u8>, Layer)> {
        let &(offset, size) = self
            .spans
            .get(entry.index())
            .ok_or_else(|| Error::format("no such entry"))?;
        if size > self.limits.max_entry_size {
            return Err(Error::limit(format!(
                "{} exceeds {} bytes",
                entry.path, self.limits.max_entry_size
            )));
        }
        self.reader.seek(SeekFrom::Start(offset))?;
        let mut out = Vec::new();
        (&mut self.reader).take(size).read_to_end(&mut out)?;
        if out.len() as u64 != size {
            return Err(Error::format(format!("{}: data is cut off", entry.path)));
        }
        Ok((out, Layer::Raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn header(name: &str, size: usize, flag: u8) -> [u8; BLOCK] {
        let mut h = [0u8; BLOCK];
        h[..name.len().min(100)].copy_from_slice(&name.as_bytes()[..name.len().min(100)]);
        h[100..107].copy_from_slice(b"0000644");
        h[124..135].copy_from_slice(format!("{size:011o}").as_bytes());
        h[156] = flag;
        h[257..262].copy_from_slice(b"ustar");
        let sum: u64 = h
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(b)
                }
            })
            .sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        h[155] = b' ';
        h
    }

    fn push(out: &mut Vec<u8>, name: &str, data: &[u8], flag: u8) {
        out.extend_from_slice(&header(name, data.len(), flag));
        out.extend_from_slice(data);
        out.resize(out.len().div_ceil(BLOCK) * BLOCK, 0);
    }

    fn finish(mut out: Vec<u8>) -> Vec<u8> {
        out.extend_from_slice(&[0u8; BLOCK * 2]);
        out
    }

    fn sample() -> Vec<u8> {
        let mut t = Vec::new();
        push(&mut t, "dir/", b"", b'5');
        push(&mut t, "dir/a.txt", b"hello", b'0');
        push(&mut t, "b.bin", &[7u8; 700], 0);
        finish(t)
    }

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).expect("write");
        e.finish().expect("finish")
    }

    fn read_all<R: Read + Seek>(a: &mut Archive<R>) -> Vec<(String, Vec<u8>)> {
        let entries: Vec<Entry> = a.entries().to_vec();
        entries
            .iter()
            .map(|e| {
                (
                    e.path.as_str().to_owned(),
                    a.read(e, Layer::Decoded).expect("read").0,
                )
            })
            .collect()
    }

    #[test]
    fn tar_lists_and_reads_files() {
        let mut a = Archive::open(Cursor::new(sample()), "x.tar", Limits::default()).expect("open");
        let all = read_all(&mut a);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], ("dir/a.txt".to_owned(), b"hello".to_vec()));
        assert_eq!(all[1].1, vec![7u8; 700]);
        assert!(a.skipped().is_empty());
    }

    #[test]
    fn empty_tar_is_an_empty_archive() {
        let a =
            Archive::open(Cursor::new(vec![0u8; 1024]), "x.tar", Limits::default()).expect("open");
        assert!(a.entries().is_empty());
        let a = Archive::open(Cursor::new(Vec::new()), "x.tar", Limits::default()).expect("open");
        assert!(a.entries().is_empty());
    }

    #[test]
    fn tar_gz_and_tgz_are_inflated() {
        for name in ["x.tar.gz", "x.TGZ", "x.gz"] {
            let mut a =
                Archive::open(Cursor::new(gz(&sample())), name, Limits::default()).expect("open");
            assert!(matches!(a, Archive::TarMem(_)), "{name}");
            assert_eq!(read_all(&mut a).len(), 2);
        }
    }

    #[test]
    fn single_gz_is_one_file_named_without_the_extension() {
        let mut a = Archive::open(
            Cursor::new(gz(b"just text")),
            "dir/notes.txt.gz",
            Limits::default(),
        )
        .expect("open");
        let all = read_all(&mut a);
        assert_eq!(all, vec![("notes.txt".to_owned(), b"just text".to_vec())]);
    }

    #[test]
    fn long_names_pax_and_prefix_are_understood() {
        let long = format!("{}/file.txt", "d".repeat(150));
        let mut t = Vec::new();
        let mut name_data = long.clone().into_bytes();
        name_data.push(0);
        push(&mut t, "././@LongLink", &name_data, b'L');
        push(&mut t, "short", b"1", b'0');
        let rec = "30 path=pax/dir/name.txt\n";
        let rec = format!("{} path=pax/dir/name.txt\n", rec.len());
        push(&mut t, "PaxHeader", rec.as_bytes(), b'x');
        push(&mut t, "x", b"22", b'0');
        let mut h = header("tail.txt", 1, b'0');
        h[345..350].copy_from_slice(b"pre/f");
        let sum: u64 = h
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(b)
                }
            })
            .sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        t.extend_from_slice(&h);
        t.extend_from_slice(&[b'z'; BLOCK]);
        let mut a =
            Archive::open(Cursor::new(finish(t)), "x.tar", Limits::default()).expect("open");
        let names: Vec<String> = read_all(&mut a).into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names,
            vec![
                long,
                "pax/dir/name.txt".to_owned(),
                "pre/f/tail.txt".to_owned()
            ]
        );
    }

    #[test]
    fn unsafe_duplicate_and_odd_entries_are_skipped_with_a_reason() {
        let mut t = Vec::new();
        push(&mut t, "../evil", b"x", b'0');
        push(&mut t, "/abs", b"x", b'0');
        push(&mut t, "ok", b"x", b'0');
        push(&mut t, "ok", b"y", b'0');
        push(&mut t, "link", b"", b'2');
        let a = Archive::open(Cursor::new(finish(t)), "x.tar", Limits::default()).expect("open");
        assert_eq!(a.entries().len(), 1);
        assert_eq!(a.skipped().len(), 4);
    }

    #[test]
    fn oversized_entries_are_skipped() {
        let limits = Limits {
            max_entry_size: 10,
            ..Limits::default()
        };
        let mut t = Vec::new();
        push(&mut t, "big", &[1u8; 20], b'0');
        push(&mut t, "small", &[1u8; 5], b'0');
        let a = Archive::open(Cursor::new(finish(t)), "x.tar", limits).expect("open");
        assert_eq!(a.entries().len(), 1);
        assert_eq!(a.skipped().len(), 1);
    }

    #[test]
    fn base_256_sizes_are_read_and_checked() {
        let mut h = header("big", 0, b'0');
        h[124..136].fill(0);
        h[124] = 0x80;
        h[135] = 0x20; // 32 bytes, in base-256
        let sum: u64 = h
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(b)
                }
            })
            .sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        let mut t = h.to_vec();
        t.extend_from_slice(&[9u8; BLOCK]);
        let mut a =
            Archive::open(Cursor::new(finish(t)), "x.tar", Limits::default()).expect("open");
        assert_eq!(read_all(&mut a)[0].1, vec![9u8; 32]);
        let mut h2 = h;
        h2[124] = 0xff; // enormous
        let sum: u64 = h2
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(b)
                }
            })
            .sum();
        h2[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        assert!(Archive::open(Cursor::new(h2.to_vec()), "x.tar", Limits::default()).is_err());
    }

    #[test]
    fn truncated_tar_keeps_what_was_complete() {
        let t = sample();
        // cut inside the data of the last file
        let a = Archive::open(
            Cursor::new(t[..BLOCK * 4 + 300].to_vec()),
            "x.tar",
            Limits::default(),
        )
        .expect("open");
        assert_eq!(a.entries().len(), 1);
        assert!(a.skipped().iter().any(|s| s.reason.contains("cut off")));
    }

    #[test]
    fn corrupted_headers_are_handled() {
        assert!(
            Archive::open(
                Cursor::new(b"not a tar file at all".repeat(60)),
                "x.tar",
                Limits::default()
            )
            .is_err()
        );
        let mut t = sample();
        t[BLOCK + 5] ^= 0xff; // header of the first file
        let a = Archive::open(Cursor::new(t), "x.tar", Limits::default()).expect("open");
        assert_eq!(a.entries().len(), 0);
        let mut t = sample();
        let at = BLOCK * 3; // header of the second file
        t[at + 10] ^= 0xff;
        let a = Archive::open(Cursor::new(t), "x.tar", Limits::default()).expect("open");
        assert_eq!(a.entries().len(), 1);
        assert!(!a.skipped().is_empty());
    }

    #[test]
    fn gzip_bombs_and_damage_are_refused() {
        let zeros = vec![0u8; 4 * 1024 * 1024];
        let limits = Limits {
            ratio_floor: 1024,
            max_ratio: 100,
            ..Limits::default()
        };
        assert!(Archive::open(Cursor::new(gz(&zeros)), "x.gz", limits).is_err());
        let limits = Limits {
            max_inflated_size: 1000,
            ..Limits::default()
        };
        assert!(Archive::open(Cursor::new(gz(&zeros)), "x.gz", limits).is_err());
        let g = gz(&sample());
        assert!(
            Archive::open(
                Cursor::new(g[..g.len() / 2].to_vec()),
                "x.tar.gz",
                Limits::default()
            )
            .is_err()
        );
        assert!(Archive::open(Cursor::new(b"plain".to_vec()), "x.gz", Limits::default()).is_err());
        assert!(Archive::open(Cursor::new(Vec::new()), "x.rar", Limits::default()).is_err());
    }

    #[test]
    fn every_cut_and_random_damage_is_handled_without_panic() {
        let t = sample();
        for cut in (0..t.len()).step_by(7) {
            let _ = Archive::open(Cursor::new(t[..cut].to_vec()), "x.tar", Limits::default());
        }
        let mut seed = 11u32;
        for _ in 0..300 {
            let mut d = t.clone();
            for _ in 0..6 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let i = (seed >> 8) as usize % d.len();
                d[i] = (seed >> 24) as u8;
            }
            if let Ok(mut a) = Archive::open(Cursor::new(d), "x.tar", Limits::default()) {
                let es: Vec<Entry> = a.entries().to_vec();
                for e in &es {
                    let _ = a.read(e, Layer::Raw);
                }
            }
        }
    }

    #[test]
    fn kinds_follow_the_name() {
        assert_eq!(Kind::from_name("A.PAK"), Some(Kind::Zip));
        assert_eq!(Kind::from_name("a.tar.gz"), Some(Kind::TarGz));
        assert_eq!(Kind::from_name("a.tgz"), Some(Kind::TarGz));
        assert_eq!(Kind::from_name("a.tar"), Some(Kind::Tar));
        assert_eq!(Kind::from_name("a.gz"), Some(Kind::Gz));
        assert_eq!(Kind::from_name("a.7z"), Some(Kind::SevenZ));
        assert_eq!(Kind::from_name("a.tar.xz"), Some(Kind::TarXz));
        assert_eq!(Kind::from_name("a.TXZ"), Some(Kind::TarXz));
        assert_eq!(Kind::from_name("a.xz"), Some(Kind::Xz));
        assert_eq!(Kind::from_name("a.rar"), None);
    }

    fn xz(data: &[u8]) -> Vec<u8> {
        let mut w = lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1))
            .expect("xz writer");
        w.write_all(data).expect("write");
        w.finish().expect("finish")
    }

    fn seven(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = sevenz_rust2::ArchiveWriter::new(Cursor::new(Vec::new())).expect("7z writer");
        for (name, data) in files {
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file(name),
                Some(Cursor::new(data.to_vec())),
            )
            .expect("push");
        }
        w.finish().expect("finish").into_inner()
    }

    fn read_one(a: &mut Archive<Cursor<Vec<u8>>>, name: &str) -> Vec<u8> {
        let e = a
            .entries()
            .iter()
            .find(|e| e.path.as_str() == name)
            .expect("entry")
            .clone();
        a.read(&e, Layer::Raw).expect("read").0
    }

    #[test]
    fn tar_xz_and_xz_are_browsed() {
        let mut a = Archive::open(Cursor::new(xz(&sample())), "a.tar.xz", Limits::default())
            .expect("tar.xz");
        assert_eq!(read_one(&mut a, "dir/a.txt"), b"hello");
        assert_eq!(read_one(&mut a, "b.bin"), vec![7u8; 700]);
        let mut a =
            Archive::open(Cursor::new(xz(b"plain")), "note.txt.xz", Limits::default()).expect("xz");
        assert_eq!(a.entries().len(), 1);
        assert_eq!(read_one(&mut a, "note.txt"), b"plain");
    }

    #[test]
    fn xz_truncated_and_corrupted_are_errors() {
        let full = xz(&sample());
        for cut in [0, 5, 12, full.len() / 2, full.len() - 1] {
            let r = Archive::open(
                Cursor::new(full[..cut].to_vec()),
                "a.tar.xz",
                Limits::default(),
            );
            assert!(r.is_err(), "cut {cut}");
        }
        let mut bad = full.clone();
        let mid = bad.len() / 2;
        bad[mid] ^= 0xff;
        assert!(Archive::open(Cursor::new(bad), "a.tar.xz", Limits::default()).is_err());
    }

    #[test]
    fn xz_inflate_cap_is_enforced() {
        let limits = Limits {
            max_inflated_size: 1000,
            ..Limits::default()
        };
        let r = Archive::open(Cursor::new(xz(&vec![0u8; 5000])), "a.xz", limits);
        assert!(r.is_err());
    }

    #[test]
    fn sevenz_lists_and_reads() {
        let data = seven(&[
            ("dir/a.txt", b"hello"),
            ("b.bin", &[9u8; 4000]),
            ("../evil", b"x"),
        ]);
        let mut a = Archive::open(Cursor::new(data), "a.7z", Limits::default()).expect("7z");
        assert_eq!(read_one(&mut a, "dir/a.txt"), b"hello");
        assert_eq!(read_one(&mut a, "b.bin"), vec![9u8; 4000]);
        assert_eq!(a.entries().len(), 2);
        assert_eq!(a.skipped().len(), 1);
    }

    #[test]
    fn sevenz_truncated_and_corrupted_never_panic() {
        let full = seven(&[("a.txt", b"hello"), ("b.bin", &[1u8; 2000])]);
        for cut in 0..full.len() {
            let _ = Archive::open(Cursor::new(full[..cut].to_vec()), "a.7z", Limits::default());
        }
        for i in 0..full.len() {
            let mut bad = full.clone();
            bad[i] ^= 0xa5;
            if let Ok(mut a) = Archive::open(Cursor::new(bad), "a.7z", Limits::default()) {
                for e in a.entries().to_vec() {
                    let _ = a.read(&e, Layer::Raw);
                }
            }
        }
        assert!(
            Archive::open(Cursor::new(full[..10].to_vec()), "a.7z", Limits::default()).is_err()
        );
    }
}
