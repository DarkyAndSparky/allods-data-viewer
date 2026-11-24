//! Resource databases (`.bin`), layout V2 for 64-bit clients.
//!
//! Only the container is read: version buffer, the metadata tables (resource names,
//! struct names), the position of the data image and the pointer-fix table. The data
//! image itself (a raw memory dump of C++ objects) is not decoded. The layout is
//! documented in `docs/formats/bin-db.md`.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::looks_like_zlib;
use flate2::read::ZlibDecoder;
use std::io::Read;

const MAX_BUCKETS: usize = 1 << 20;

/// One named resource (a `.xdb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbFile {
    /// Resource name, for example `Maps/Domination02/MapResource.xdb`.
    pub name: String,
    /// Database id; it is the byte offset of the root object in the data image.
    pub db_id: u64,
}

/// A parsed database (metadata only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database {
    /// The 12 byte version buffer.
    pub version: [u8; 12],
    /// Struct names without the `struct NDb::` prefix.
    pub structs: Vec<String>,
    /// Named resources, sorted by database id.
    pub files: Vec<DbFile>,
    /// Range of the data image inside the inflated payload.
    pub data: std::ops::Range<usize>,
    /// Number of 16 byte pointer-fix entries.
    pub fix_count: usize,
    /// Position of the first fix entry inside the payload.
    pub fixes_at: usize,
    /// Names of the referenced `.pak` files (chunk 6), when present.
    pub packs: Vec<String>,
}

fn i32_at(d: &[u8], pos: usize) -> Result<i32> {
    d.get(
        pos..pos
            .checked_add(4)
            .ok_or_else(|| Error::format("offset overflow"))?,
    )
    .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    .ok_or_else(|| Error::format(format!("read past end at {pos}")))
}

fn i64_at(d: &[u8], pos: usize) -> Result<i64> {
    d.get(
        pos..pos
            .checked_add(8)
            .ok_or_else(|| Error::format("offset overflow"))?,
    )
    .map(|b| {
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        i64::from_le_bytes(a)
    })
    .ok_or_else(|| Error::format(format!("read past end at {pos}")))
}

/// Resolves a field-relative offset: the target is `field + rel`.
fn target(field: usize, rel: i32) -> Result<usize> {
    usize::try_from(field as i64 + i64::from(rel)).map_err(|_| Error::format("negative target"))
}

pub(crate) fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn count(value: i32, what: &str, limits: &Limits) -> Result<usize> {
    let n = usize::try_from(value).map_err(|_| Error::format(format!("negative {what} count")))?;
    if n > MAX_BUCKETS.max(usize::try_from(limits.max_entries).unwrap_or(usize::MAX)) {
        return Err(Error::limit(format!("{what} count {n} too large")));
    }
    Ok(n)
}

/// Inflates a database file (a zlib stream) with the size limit applied.
///
/// # Errors
///
/// Fails on I/O errors, zlib errors, or when the inflated size exceeds the limit.
pub fn inflate_db(mut input: impl Read, limits: &Limits) -> Result<Vec<u8>> {
    let cap = limits.max_entry_size;
    let mut raw = Vec::new();
    (&mut input).take(cap + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > cap {
        return Err(Error::limit("database file too large"));
    }
    if !looks_like_zlib(&raw) {
        return Err(Error::format("database is not a zlib stream"));
    }
    let mut out = Vec::new();
    ZlibDecoder::new(raw.as_slice())
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::format(format!("zlib: {e}")))?;
    if out.len() as u64 > cap {
        return Err(Error::limit("inflated database too large"));
    }
    Ok(out)
}

/// Layout of a resource database, told apart from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// 32-bit chunks starting at offset 0 (Cloud Pirates, client 10.0); see [`crate::db32`].
    Bits32,
    /// 64-bit V2 layout read by [`parse_db`] (client 17.0).
    V2,
    /// Anything else, including the newer 64-bit revision of client 18.0.
    Unsupported,
}

/// Classifies a database from its first bytes without inflating all of it.
///
/// `raw` is the file as stored (zlib stream or plain). Only the first 48 bytes are inflated,
/// so this is cheap even for a file that inflates to hundreds of megabytes.
#[must_use]
pub fn probe_layout(raw: &[u8]) -> Layout {
    let mut head = Vec::with_capacity(48);
    if looks_like_zlib(raw) {
        if ZlibDecoder::new(raw)
            .take(48)
            .read_to_end(&mut head)
            .is_err()
        {
            return Layout::Unsupported;
        }
    } else {
        head.extend_from_slice(raw.get(..48.min(raw.len())).unwrap_or_default());
    }
    let word = |at: usize| -> Option<i32> {
        let b = head.get(at..at + 4)?;
        Some(i32::from_le_bytes([
            *b.first()?,
            *b.get(1)?,
            *b.get(2)?,
            *b.get(3)?,
        ]))
    };
    if word(0) == Some(0) && word(4) == Some(8) {
        return Layout::Bits32;
    }
    // V2: 12 version bytes, chunk id 2; the struct-name directory (pair 2) is small there.
    // The client 18.0 revision has a 65 521-bucket table in that slot.
    if word(12) == Some(2)
        && let Some(structs) = word(24 + 2 * 8 + 4)
        && (1..=5000).contains(&structs)
    {
        return Layout::V2;
    }
    Layout::Unsupported
}

/// Recognises a resource database by its 12-byte version buffer, in a zlib stream or plain.
///
/// Known buffers share the bytes `xx 02 2e 59 73 04 00 00` (see `docs/formats/bin-db.md`).
/// Returns the 12 version bytes.
#[must_use]
pub fn db_signature(data: &[u8]) -> Option<[u8; 12]> {
    let mut head = [0u8; 12];
    if looks_like_zlib(data) {
        ZlibDecoder::new(data).read_exact(&mut head).ok()?;
    } else {
        head.copy_from_slice(data.get(..12)?);
    }
    (head[1..8] == [0x02, 0x2e, 0x59, 0x73, 0x04, 0x00, 0x00]).then_some(head)
}

/// True when every entry of the table is a `struct NDb::` name.
fn structs_are_names(meta: &[u8], (at, n): (usize, usize)) -> bool {
    (0..n).all(|i| {
        let e = at + i * 16;
        let (Some(rel), Some(size)) = (i32_at(meta, e).ok(), i32_at(meta, e + 4).ok()) else {
            return false;
        };
        let (Ok(t), Ok(size)) = (target(e, rel), usize::try_from(size)) else {
            return false;
        };
        t.checked_add(size)
            .and_then(|end| meta.get(t..end))
            .is_some_and(|b| b.starts_with(b"struct NDb::"))
    })
}

/// Reads the struct names and the bucketed resource names from the two directory tables.
fn read_tables(
    meta: &[u8],
    (files_at, files_n): (usize, usize),
    (structs_at, structs_n): (usize, usize),
    limits: &Limits,
) -> Result<(Vec<String>, Vec<DbFile>)> {
    // Struct names.
    let mut structs = Vec::with_capacity(structs_n);
    for i in 0..structs_n {
        let e = structs_at + i * 16;
        let rel = i32_at(meta, e)?;
        let size =
            usize::try_from(i32_at(meta, e + 4)?).map_err(|_| Error::format("bad name size"))?;
        let at = target(e, rel)?;
        let raw = meta
            .get(
                at..at
                    .checked_add(size)
                    .ok_or_else(|| Error::format("name overflow"))?,
            )
            .ok_or_else(|| Error::format("struct name runs past end"))?;
        let raw = raw.split(|&b| b == 0).next().unwrap_or(raw);
        let name = String::from_utf8_lossy(raw);
        structs.push(
            name.strip_prefix("struct NDb::")
                .unwrap_or(&name)
                .to_owned(),
        );
    }

    // Resource names, bucketed.
    let mut files = Vec::new();
    for b in 0..files_n {
        let bucket = files_at + b * 8;
        let rel = i32_at(meta, bucket)?;
        let n = count(i32_at(meta, bucket + 4)?, "bucket", limits)?;
        if n == 0 {
            continue;
        }
        let recs = target(bucket, rel)?;
        for r in 0..n {
            if files.len() as u64 >= limits.max_entries {
                return Err(Error::limit("too many resources"));
            }
            let rec = recs + r * 16;
            let rel = i32_at(meta, rec)?;
            let size = usize::try_from(i32_at(meta, rec + 4)?)
                .ok()
                .filter(|s| *s >= 9)
                .ok_or_else(|| Error::format("bad name block size"))?;
            let db_id =
                u64::try_from(i64_at(meta, rec + 8)?).map_err(|_| Error::format("bad db id"))?;
            let blk = target(rec, rel)?;
            let stored = i32_at(meta, blk + 4)? as u32;
            let name_at = blk + 8;
            let name = meta
                .get(
                    name_at
                        ..name_at
                            .checked_add(size - 9)
                            .ok_or_else(|| Error::format("name overflow"))?,
                )
                .ok_or_else(|| Error::format("resource name runs past end"))?;
            if adler32(name) != stored {
                return Err(Error::format(format!(
                    "checksum mismatch for resource at {name_at}"
                )));
            }
            files.push(DbFile {
                name: String::from_utf8_lossy(name).into_owned(),
                db_id,
            });
        }
    }
    Ok((structs, files))
}

/// Parses an inflated V2 x64 database.
///
/// # Errors
///
/// [`Error::Format`] for anything that does not follow the layout, [`Error::Limit`] for
/// absurd counts.
pub fn parse_db(d: &[u8], limits: &Limits) -> Result<Database> {
    if d.len() < 12 + 12 {
        return Err(Error::format("database too short"));
    }
    let mut version = [0u8; 12];
    version.copy_from_slice(&d[..12]);

    // Chunk 2: metadata.
    if i32_at(d, 12)? != 2 {
        return Err(Error::format(
            "first chunk is not the metadata chunk (id 2)",
        ));
    }
    let meta_size =
        usize::try_from(i64_at(d, 16)?).map_err(|_| Error::format("bad metadata size"))?;
    let blob = 24usize;
    let blob_end = blob
        .checked_add(meta_size)
        .filter(|e| *e <= d.len())
        .ok_or_else(|| Error::format("metadata chunk runs past end"))?;
    let meta = &d[..blob_end];

    let dir = |i: usize| -> Result<(usize, usize)> {
        let field = blob + i * 8;
        let rel = i32_at(meta, field)?;
        let n = count(i32_at(meta, field + 4)?, "table", limits)?;
        Ok((target(field, rel)?, n))
    };
    // Builds up to 1139 keep the resource names in table 1 and the struct names in table 2.
    // Build 1148 has the struct names in table 1 and no name table at all.
    let (one, two) = (dir(1)?, dir(2)?);
    let (structs, mut files) = match read_tables(meta, one, two, limits) {
        Ok(t) => t,
        Err(first) => match read_tables(meta, (0, 0), one, limits) {
            // Only believed when every entry really is a struct name.
            Ok(t)
                if !t.0.is_empty()
                    && t.0.iter().all(|n| !n.starts_with("struct "))
                    && structs_are_names(meta, one) =>
            {
                t
            }
            _ => return Err(first),
        },
    };
    files.sort_by_key(|f| f.db_id);

    // Chunk 3: data image.
    let mut pos = blob_end;
    if i32_at(d, pos)? != 3 {
        return Err(Error::format("chunk 3 (data) expected after metadata"));
    }
    let data_size =
        usize::try_from(i64_at(d, pos + 4)?).map_err(|_| Error::format("bad data size"))?;
    let data_start = pos + 12;
    let data_end = data_start
        .checked_add(data_size)
        .filter(|e| *e <= d.len())
        .ok_or_else(|| Error::format("data chunk runs past end"))?;
    pos = data_end;

    // Chunk 4: pointer fixes, `size` counts 16 byte entries.
    if i32_at(d, pos)? != 4 {
        return Err(Error::format("chunk 4 (fixes) expected after data"));
    }
    let fix_count =
        usize::try_from(i64_at(d, pos + 4)?).map_err(|_| Error::format("bad fix count"))?;
    let fixes_at = pos + 12;
    pos = fix_count
        .checked_mul(16)
        .and_then(|n| n.checked_add(fixes_at))
        .filter(|e| *e <= d.len())
        .ok_or_else(|| Error::format("fixes chunk runs past end"))?;

    // Optional chunks 5 and 6.
    let mut packs = Vec::new();
    if pos < d.len() {
        if i32_at(d, pos)? != 5 {
            return Err(Error::format("chunk 5 expected after fixes"));
        }
        let n = usize::try_from(i64_at(d, pos + 4)?).map_err(|_| Error::format("bad count"))?;
        pos = n
            .checked_mul(8)
            .and_then(|b| b.checked_add(pos + 12))
            .filter(|e| *e <= d.len())
            .ok_or_else(|| Error::format("pak refs run past end"))?;
        if pos < d.len() {
            if i32_at(d, pos)? != 6 {
                return Err(Error::format("chunk 6 expected after chunk 5"));
            }
            let n = count(
                i32::try_from(i64_at(d, pos + 4)?).map_err(|_| Error::format("bad pack count"))?,
                "pack",
                limits,
            )?;
            pos += 12;
            for _ in 0..n {
                let len =
                    usize::try_from(i64_at(d, pos)?).map_err(|_| Error::format("bad pack name"))?;
                let raw = d
                    .get(
                        pos + 8
                            ..pos
                                .checked_add(8 + len)
                                .ok_or_else(|| Error::format("overflow"))?,
                    )
                    .ok_or_else(|| Error::format("pack name runs past end"))?;
                let units: Vec<u16> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                packs.push(String::from_utf16_lossy(&units));
                pos += 8 + len;
            }
        }
    }

    Ok(Database {
        version,
        structs,
        files,
        data: data_start..data_end,
        fix_count,
        fixes_at,
        packs,
    })
}

impl Database {
    /// Struct type name of each object whose pointer-fix entry carries a type tag.
    ///
    /// A tag is a fix of kind 0 with the "external" bit set whose address is the
    /// object's database id and whose value is an index into [`Database::structs`].
    #[must_use]
    pub fn type_tags<'a>(&'a self, payload: &[u8]) -> std::collections::HashMap<u64, &'a str> {
        let mut out = std::collections::HashMap::new();
        for i in 0..self.fix_count {
            let at = self.fixes_at + i * 16;
            let (Ok(data), Ok(value)) = (i64_at(payload, at), i64_at(payload, at + 8)) else {
                break;
            };
            if data & 7 != 4 {
                continue;
            }
            let addr = ((data >> 3) as u64).wrapping_mul(8);
            if let Some(s) = usize::try_from(value)
                .ok()
                .and_then(|v| self.structs.get(v))
            {
                out.insert(addr, s.as_str());
            }
        }
        out
    }
}

/// Fields of an `NDb::Texture` object (layout verified on 1122 objects, see `docs/formats/bin-db.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureRecord {
    /// Database id of the object.
    pub db_id: u64,
    /// `TextureElementType` value (0 DXT1, 1 DXT3, 2 DXT5, 3 RGBA, 4 L8, ...).
    pub kind: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Mip level count as stored in the object.
    pub mips: u32,
}

impl Database {
    /// Texture sizes by lower-case resource name (names ending in `(Texture).xdb`) as
    /// `(width, height)`; lets a caller drop the large payload.
    #[must_use]
    pub fn texture_sizes(&self, payload: &[u8]) -> std::collections::HashMap<String, (u32, u32)> {
        let mut out = std::collections::HashMap::new();
        for f in &self.files {
            if f.name.ends_with("(Texture).xdb")
                && let Some(r) = self.texture_record(payload, f.db_id)
            {
                out.insert(f.name.to_ascii_lowercase(), (r.width, r.height));
            }
        }
        out
    }

    /// Read the texture record at `db_id`; `None` if it does not fit in the data image.
    #[must_use]
    pub fn texture_record(&self, payload: &[u8], db_id: u64) -> Option<TextureRecord> {
        let base = self.data.start.checked_add(usize::try_from(db_id).ok()?)?;
        let obj = payload.get(base..base.checked_add(160)?)?;
        let w = |o: usize| u32::from_le_bytes([obj[o], obj[o + 1], obj[o + 2], obj[o + 3]]);
        Some(TextureRecord {
            db_id,
            mips: w(128),
            width: w(136),
            height: w(140),
            kind: w(144),
        })
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::adler32;

    fn pad8(v: &mut Vec<u8>) {
        while !v.len().is_multiple_of(8) {
            v.push(0);
        }
    }

    fn put32(v: &mut [u8], at: usize, x: i32) {
        v[at..at + 4].copy_from_slice(&x.to_le_bytes());
    }

    /// Builds a small V2 x64 database: `files` as (name, db id), `structs` names, a data
    /// image of `data_len` bytes and `tags` as (offset, struct index) type fixes.
    pub fn build(
        files: &[(&str, u64)],
        structs: &[&str],
        data_len: usize,
        tags: &[(u64, i64)],
    ) -> Vec<u8> {
        let mut m = vec![0u8; 56];
        let buckets = 3usize;
        // file table: bucket array first
        let barr = m.len();
        m.resize(barr + buckets * 8, 0);
        for b in 0..buckets {
            let mine: Vec<_> = files
                .iter()
                .filter(|(n, _)| adler32(n.as_bytes()) as usize % buckets == b)
                .collect();
            if mine.is_empty() {
                continue;
            }
            pad8(&mut m);
            let recs = m.len();
            m.resize(recs + mine.len() * 16, 0);
            for (i, (name, id)) in mine.iter().enumerate() {
                pad8(&mut m);
                let blk = m.len();
                m.extend_from_slice(&1i32.to_le_bytes());
                m.extend_from_slice(&adler32(name.as_bytes()).to_le_bytes());
                m.extend_from_slice(name.as_bytes());
                m.push(0);
                let rec = recs + i * 16;
                put32(&mut m, rec, (blk - rec) as i32);
                put32(&mut m, rec + 4, name.len() as i32 + 9);
                m[rec + 8..rec + 16].copy_from_slice(&(*id as i64).to_le_bytes());
            }
            put32(&mut m, barr + b * 8, (recs - (barr + b * 8)) as i32);
            put32(&mut m, barr + b * 8 + 4, mine.len() as i32);
        }
        pad8(&mut m);
        let sarr = m.len();
        m.resize(sarr + structs.len() * 16, 0);
        for (i, s) in structs.iter().enumerate() {
            let full = format!("struct NDb::{s}");
            let at = m.len();
            m.extend_from_slice(full.as_bytes());
            m.push(0);
            let e = sarr + i * 16;
            put32(&mut m, e, (at - e) as i32);
            put32(&mut m, e + 4, full.len() as i32 + 1);
        }
        pad8(&mut m);
        put32(&mut m, 8, (barr - 8) as i32);
        put32(&mut m, 12, buckets as i32);
        put32(&mut m, 16, (sarr - 16) as i32);
        put32(&mut m, 20, structs.len() as i32);

        let mut out = vec![
            0xCC, 0x02, 0x2E, 0x59, 0x73, 4, 0, 0, 0x9F, 0x11, 0x89, 0x26,
        ];
        out.extend_from_slice(&2i32.to_le_bytes());
        out.extend_from_slice(&(m.len() as i64).to_le_bytes());
        out.extend_from_slice(&m);
        out.extend_from_slice(&3i32.to_le_bytes());
        out.extend_from_slice(&(data_len as i64).to_le_bytes());
        out.extend(std::iter::repeat_n(0u8, data_len));
        out.extend_from_slice(&4i32.to_le_bytes());
        out.extend_from_slice(&(tags.len() as i64).to_le_bytes());
        for (off, idx) in tags {
            out.extend_from_slice(&(((off / 8) << 3) | 2).to_le_bytes());
            out.extend_from_slice(&idx.to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::build;
    use super::*;

    #[test]
    fn parses_names_structs_and_tags() {
        let d = build(
            &[
                ("Maps/A/MapResource.xdb", 64),
                ("Creatures/Beast.(Texture).xdb", 0),
                ("x.xdb", 200),
            ],
            &["Texture", "MapResource"],
            256,
            &[(0, 0), (64, 1)],
        );
        let db = parse_db(&d, &Limits::default()).expect("parse");
        assert_eq!(db.version[..4], [0xCC, 0x02, 0x2E, 0x59]);
        assert_eq!(db.structs, ["Texture", "MapResource"]);
        let names: Vec<_> = db
            .files
            .iter()
            .map(|f| (f.name.as_str(), f.db_id))
            .collect();
        assert_eq!(
            names,
            [
                ("Creatures/Beast.(Texture).xdb", 0),
                ("Maps/A/MapResource.xdb", 64),
                ("x.xdb", 200)
            ]
        );
        assert_eq!(db.data.len(), 256);
        assert_eq!(db.fix_count, 2);
        assert!(db.packs.is_empty());
    }

    #[test]
    fn build_1148_has_structs_in_table_one_and_no_names() {
        let mut d = build(&[], &["Texture", "Geometry"], 32, &[(0, 0)]);
        // Directory entries sit at 24 (chunk header 12 + 12 version bytes): move the struct
        // table into slot 1 and leave slot 2 empty, as the 1148 client does.
        let rel = i32::from_le_bytes(d[40..44].try_into().expect("4 bytes"));
        d[32..36].copy_from_slice(&(rel + 8).to_le_bytes());
        d[36..40].copy_from_slice(&2i32.to_le_bytes());
        d[40..48].fill(0);
        let db = parse_db(&d, &Limits::default()).expect("parse");
        assert_eq!(db.structs, ["Texture", "Geometry"]);
        assert!(db.files.is_empty());
    }

    #[test]
    fn damaged_files_never_panic() {
        let d = build(&[("a.xdb", 0), ("b.xdb", 8)], &["T"], 32, &[(0, 0)]);
        for cut in 0..d.len() {
            let _ = parse_db(&d[..cut], &Limits::default());
        }
        for i in 0..d.len() {
            let mut x = d.clone();
            x[i] ^= 0xFF;
            let _ = parse_db(&x, &Limits::default());
        }
    }

    #[test]
    fn checksum_mismatch_is_detected() {
        let mut d = build(&[("abc.xdb", 0)], &["T"], 8, &[]);
        let pos = d.windows(7).position(|w| w == b"abc.xdb").expect("name");
        d[pos] = b'z';
        assert!(matches!(
            parse_db(&d, &Limits::default()),
            Err(Error::Format(_))
        ));
    }

    #[test]
    fn signature_is_found_plain_and_in_zlib() {
        let mut v = vec![0xCC, 2, 0x2E, 0x59, 0x73, 4, 0, 0, 1, 2, 3, 4];
        v.extend([0u8; 20]);
        assert_eq!(db_signature(&v).map(|h| h[0]), Some(0xCC));
        let z = crate::pak::testutil::zlib(&v);
        assert_eq!(db_signature(&z).map(|h| h[8]), Some(1));
        assert!(db_signature(b"hello world, not a database").is_none());
        assert!(db_signature(&[]).is_none());
    }

    #[test]
    fn adler32_known_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn texture_record_reads_fields() {
        let mut payload = vec![0u8; 400];
        let base = 100;
        payload[base + 128..base + 132].copy_from_slice(&6u32.to_le_bytes());
        payload[base + 136..base + 140].copy_from_slice(&256u32.to_le_bytes());
        payload[base + 140..base + 144].copy_from_slice(&128u32.to_le_bytes());
        payload[base + 144..base + 148].copy_from_slice(&2u32.to_le_bytes());
        let db = Database {
            version: [0; 12],
            structs: vec![],
            files: vec![],
            data: base..payload.len(),
            fix_count: 0,
            fixes_at: 0,
            packs: vec![],
        };
        let r = db.texture_record(&payload, 0).expect("record");
        assert_eq!((r.width, r.height, r.kind, r.mips), (256, 128, 2, 6));
        assert!(db.texture_record(&payload, 300).is_none());
    }

    #[test]
    fn probe_layout_tells_layouts_apart() {
        let mut b32 = vec![0u8; 48];
        b32[4] = 8;
        assert_eq!(probe_layout(&b32), Layout::Bits32);
        let mut v2 = vec![0u8; 48];
        v2[12] = 2;
        v2[44..48].copy_from_slice(&1498i32.to_le_bytes());
        assert_eq!(probe_layout(&v2), Layout::V2);
        v2[44..48].copy_from_slice(&65521i32.to_le_bytes());
        assert_eq!(probe_layout(&v2), Layout::Unsupported);
        assert_eq!(probe_layout(&[1, 2, 3]), Layout::Unsupported);
        assert_eq!(probe_layout(&[]), Layout::Unsupported);
    }

    #[test]
    fn texture_sizes_are_keyed_by_lower_case_name() {
        let mut payload = vec![0u8; 400];
        payload[136..140].copy_from_slice(&256u32.to_le_bytes());
        payload[140..144].copy_from_slice(&128u32.to_le_bytes());
        let db = Database {
            version: [0; 12],
            structs: vec![],
            files: vec![
                DbFile {
                    name: "Maps/X/A.(Texture).xdb".into(),
                    db_id: 0,
                },
                DbFile {
                    name: "Maps/X/B.(Geometry).xdb".into(),
                    db_id: 0,
                },
                DbFile {
                    name: "Maps/X/C.(Texture).xdb".into(),
                    db_id: 900,
                },
            ],
            data: 0..payload.len(),
            fix_count: 0,
            fixes_at: 0,
            packs: vec![],
        };
        let sizes = db.texture_sizes(&payload);
        assert_eq!(sizes.len(), 1);
        assert_eq!(sizes.get("maps/x/a.(texture).xdb"), Some(&(256, 128)));
    }
}
