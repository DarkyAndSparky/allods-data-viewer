//! Reading `.pak` archives (ZIP files) with safety limits.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::vpath::VPath;
use flate2::read::ZlibDecoder;
use std::io::{Read, Seek, Write};
use zip::{CompressionMethod, ZipArchive};

/// Compression method of an entry at the ZIP layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Stored without compression.
    Stored,
    /// Deflate.
    Deflate,
    /// Anything else; such entries cannot be read.
    Other,
}

/// Which layer of an entry's data is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Data as stored in the ZIP, without removing the extra zlib layer.
    Raw,
    /// Data with the extra zlib layer removed when the entry has one.
    Decoded,
}

/// One accepted entry of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Validated path inside the archive.
    pub path: VPath,
    /// Size after the ZIP layer.
    pub size: u64,
    /// Size as stored in the archive.
    pub packed_size: u64,
    /// CRC32 stored in the archive.
    pub crc32: u32,
    /// ZIP compression method.
    pub method: Method,
    index: usize,
}

/// An entry that was not accepted, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Name as stored (lossy for display).
    pub name: String,
    /// Why the entry was rejected.
    pub reason: String,
}

/// An opened `.pak`.
pub struct PakArchive<R: Read + Seek> {
    zip: ZipArchive<R>,
    entries: Vec<Entry>,
    skipped: Vec<Skipped>,
    limits: Limits,
}

/// Returns true when `head` starts like a zlib stream.
#[must_use]
pub fn looks_like_zlib(head: &[u8]) -> bool {
    head.len() >= 2 && head[0] == 0x78 && matches!(head[1], 0x01 | 0x5E | 0x9C | 0xDA)
}

fn map_io(err: std::io::Error) -> Error {
    if err.kind() == std::io::ErrorKind::InvalidData
        && err.to_string().to_ascii_lowercase().contains("checksum")
    {
        Error::crc(err.to_string())
    } else {
        Error::Io(err)
    }
}

fn map_zip(err: zip::result::ZipError) -> Error {
    match err {
        zip::result::ZipError::Io(e) => map_io(e),
        zip::result::ZipError::UnsupportedArchive(m) => Error::unsupported(m.to_string()),
        other => Error::format(other.to_string()),
    }
}

impl<R: Read + Seek> PakArchive<R> {
    /// Opens an archive and validates every entry name and size.
    ///
    /// Entries that break a rule are listed in [`PakArchive::skipped`] and cannot be
    /// extracted; the rest of the archive stays usable.
    ///
    /// # Errors
    ///
    /// Fails when the file is not a ZIP, or has more entries than the limit.
    pub fn open(reader: R, limits: Limits) -> Result<Self> {
        let mut zip = ZipArchive::new(reader).map_err(map_zip)?;
        let count = zip.len();
        if count as u64 > limits.max_entries {
            return Err(Error::limit(format!(
                "{count} entries, limit is {}",
                limits.max_entries
            )));
        }
        let mut entries = Vec::with_capacity(count);
        let mut skipped = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for index in 0..count {
            let file = zip.by_index_raw(index).map_err(map_zip)?;
            if file.is_dir() {
                continue;
            }
            let name = match file.name() {
                Ok(n) => n.into_owned(),
                Err(e) => {
                    skipped.push(Skipped {
                        name: String::from_utf8_lossy(file.name_raw()).into_owned(),
                        reason: format!("bad name: {e}"),
                    });
                    continue;
                }
            };
            let size = file.size();
            let packed = file.compressed_size();
            let method = match file.compression() {
                CompressionMethod::Stored => Method::Stored,
                CompressionMethod::Deflated => Method::Deflate,
                _ => Method::Other,
            };
            let mut reject = |reason: String| {
                skipped.push(Skipped {
                    name: name.clone(),
                    reason,
                });
            };
            let path = match VPath::parse(&name, limits.max_path_len) {
                Ok(p) => p,
                Err(e) => {
                    reject(e.to_string());
                    continue;
                }
            };
            if method == Method::Other {
                reject("unsupported compression method".to_owned());
                continue;
            }
            if size > limits.max_entry_size {
                reject(format!("size {size} exceeds {}", limits.max_entry_size));
                continue;
            }
            if size > limits.ratio_floor && size / packed.max(1) > limits.max_ratio {
                reject(format!("compression ratio above {}:1", limits.max_ratio));
                continue;
            }
            if !seen.insert(path.as_str().to_owned()) {
                reject("duplicate path".to_owned());
                continue;
            }
            entries.push(Entry {
                path,
                size,
                packed_size: packed,
                crc32: file.crc32(),
                method,
                index,
            });
        }
        Ok(Self {
            zip,
            entries,
            skipped,
            limits,
        })
    }

    /// Accepted entries in archive order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entries rejected while opening.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Reads an entry into memory and returns the data and the layer actually returned.
    ///
    /// With [`Layer::Decoded`] a zlib layer is removed when the data starts like zlib and
    /// inflates cleanly; otherwise the data is returned as is (and reported as
    /// [`Layer::Raw`]). The ZIP CRC is checked in both cases.
    ///
    /// # Errors
    ///
    /// Fails on I/O errors, a CRC mismatch, or when a size limit is exceeded.
    pub fn read(&mut self, entry: &Entry, layer: Layer) -> Result<(Vec<u8>, Layer)> {
        let mut file = self.zip.by_index(entry.index).map_err(map_zip)?;
        let mut raw = Vec::new();
        let cap = self.limits.max_entry_size;
        (&mut file)
            .take(cap + 1)
            .read_to_end(&mut raw)
            .map_err(map_io)?;
        if raw.len() as u64 > cap {
            return Err(Error::limit(format!("{} exceeds {cap} bytes", entry.path)));
        }
        if layer == Layer::Decoded && looks_like_zlib(&raw) {
            let cap = self.limits.max_inflated_size;
            let mut out = Vec::new();
            let result = ZlibDecoder::new(raw.as_slice())
                .take(cap + 1)
                .read_to_end(&mut out);
            if result.is_ok() {
                if out.len() as u64 > cap {
                    return Err(Error::limit(format!(
                        "{} inflates beyond {cap} bytes",
                        entry.path
                    )));
                }
                return Ok((out, Layer::Decoded));
            }
        }
        Ok((raw, Layer::Raw))
    }

    /// Reads an entry and writes it to `out`.
    ///
    /// # Errors
    ///
    /// Same as [`PakArchive::read`], plus write errors.
    pub fn read_to(&mut self, entry: &Entry, layer: Layer, out: &mut impl Write) -> Result<Layer> {
        let (data, used) = self.read(entry, layer)?;
        out.write_all(&data)?;
        Ok(used)
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    /// Builds an in-memory ZIP from `(name, data, deflate)` triples.
    pub fn make_zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, deflate) in files {
            let method = if *deflate {
                CompressionMethod::Deflated
            } else {
                CompressionMethod::Stored
            };
            zw.start_file(
                *name,
                SimpleFileOptions::default().compression_method(method),
            )
            .expect("start file");
            zw.write_all(data).expect("write");
        }
        zw.finish().expect("finish").into_inner()
    }

    /// Wraps data in a zlib stream.
    pub fn zlib(data: &[u8]) -> Vec<u8> {
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data).expect("zlib write");
        enc.finish().expect("zlib finish")
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use std::io::Cursor;

    fn open(bytes: Vec<u8>) -> PakArchive<Cursor<Vec<u8>>> {
        PakArchive::open(Cursor::new(bytes), Limits::default()).expect("open")
    }

    #[test]
    fn lists_and_reads_stored_and_deflate() {
        let payload = vec![7u8; 5000];
        let mut pak = open(make_zip(&[
            ("a.txt", b"hello", false),
            ("d/b.bin", &payload, true),
        ]));
        assert_eq!(pak.entries().len(), 2);
        let e = pak.entries()[1].clone();
        assert_eq!(e.method, Method::Deflate);
        assert_eq!(e.size, 5000);
        assert!(e.packed_size < 5000);
        let (data, layer) = pak.read(&e, Layer::Decoded).expect("read");
        assert_eq!(data, payload);
        assert_eq!(layer, Layer::Raw);
    }

    #[test]
    fn removes_zlib_layer_only_when_asked() {
        let inner = b"inner text inner text inner text".to_vec();
        let mut pak = open(make_zip(&[("x.bin", &zlib(&inner), false)]));
        let e = pak.entries()[0].clone();
        let (decoded, l) = pak.read(&e, Layer::Decoded).expect("decoded");
        assert_eq!((decoded, l), (inner, Layer::Decoded));
        let (raw, l) = pak.read(&e, Layer::Raw).expect("raw");
        assert!(looks_like_zlib(&raw));
        assert_eq!(l, Layer::Raw);
    }

    #[test]
    fn broken_zlib_is_treated_as_plain_data() {
        let mut fake = vec![0x78, 0x9C];
        fake.extend_from_slice(b"not really zlib data at all");
        let mut pak = open(make_zip(&[("x.bin", &fake, false)]));
        let e = pak.entries()[0].clone();
        let (data, l) = pak.read(&e, Layer::Decoded).expect("read");
        assert_eq!((data, l), (fake, Layer::Raw));
    }

    #[test]
    fn hostile_names_are_skipped() {
        let pak = open(make_zip(&[
            ("ok.txt", b"1", false),
            ("../evil.txt", b"2", false),
            ("/abs.txt", b"3", false),
        ]));
        assert_eq!(pak.entries().len(), 1);
        assert_eq!(pak.skipped().len(), 2);
    }

    #[test]
    fn not_a_zip_is_a_format_error() {
        let err = PakArchive::open(
            Cursor::new(b"hello world, not a zip".to_vec()),
            Limits::default(),
        );
        assert!(matches!(err, Err(Error::Format(_))));
    }

    #[test]
    fn truncated_archive_fails_cleanly() {
        let mut bytes = make_zip(&[("a.txt", b"hello world", false)]);
        bytes.truncate(bytes.len() / 2);
        assert!(PakArchive::open(Cursor::new(bytes), Limits::default()).is_err());
    }

    #[test]
    fn corrupted_data_is_a_crc_error() {
        let mut bytes = make_zip(&[("a.txt", b"hello world", false)]);
        let pos = bytes
            .windows(11)
            .position(|w| w == b"hello world")
            .expect("find");
        bytes[pos] ^= 0xFF;
        let mut pak = open(bytes);
        let e = pak.entries()[0].clone();
        assert!(matches!(pak.read(&e, Layer::Raw), Err(Error::Crc(_))));
    }

    #[test]
    fn entry_count_limit_is_enforced() {
        let limits = Limits {
            max_entries: 1,
            ..Limits::default()
        };
        let bytes = make_zip(&[("a", b"1", false), ("b", b"2", false)]);
        assert!(matches!(
            PakArchive::open(Cursor::new(bytes), limits),
            Err(Error::Limit(_))
        ));
    }

    #[test]
    fn ratio_bomb_is_skipped() {
        let big = vec![0u8; 3 * 1024 * 1024];
        let limits = Limits {
            max_ratio: 100,
            ..Limits::default()
        };
        let pak = PakArchive::open(Cursor::new(make_zip(&[("bomb.bin", &big, true)])), limits)
            .expect("open");
        assert!(pak.entries().is_empty());
        assert!(pak.skipped()[0].reason.contains("ratio"));
    }

    #[test]
    fn inflated_size_limit_is_enforced() {
        let limits = Limits {
            max_inflated_size: 10,
            ..Limits::default()
        };
        let bytes = make_zip(&[("x.bin", &zlib(&[1u8; 100]), false)]);
        let mut pak = PakArchive::open(Cursor::new(bytes), limits).expect("open");
        let e = pak.entries()[0].clone();
        assert!(matches!(pak.read(&e, Layer::Decoded), Err(Error::Limit(_))));
    }
}
