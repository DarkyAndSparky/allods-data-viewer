//! Resource database of the older 32-bit layout (Cloud Pirates 1.5, game version 7.0).
//!
//! The file is a zlib stream; inflated it is a list of chunks `u32 id, u32 size, body`.
//! Layout and status are described in `docs/formats/bin-db.md`.

use crate::db::{DbFile, adler32};
use crate::error::{Error, Result};
use crate::limits::Limits;

const MAX_TABLE: usize = 1 << 20;

/// A parsed 32-bit database (metadata and the data image range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database32 {
    /// Struct names without the `struct NDb::` prefix.
    pub structs: Vec<String>,
    /// Named resources, sorted by database id.
    pub files: Vec<DbFile>,
    /// Range of the data image inside the inflated payload.
    pub data: std::ops::Range<usize>,
}

/// Texture object fields (100 byte record).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Texture32 {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Mip level count.
    pub mips: u32,
    /// `TextureElementType` value (0 DXT1, 1 DXT3, 2 DXT5, 3 RGBA, 4 L8).
    pub kind: u32,
    /// Flag word at +84; 0 means 8 byte blocks, 2 means 16 byte blocks in all samples.
    pub alpha: u32,
}

fn u32_at(d: &[u8], pos: usize) -> Result<u32> {
    let end = pos
        .checked_add(4)
        .ok_or_else(|| Error::format("offset overflow"))?;
    let b = d
        .get(pos..end)
        .ok_or_else(|| Error::format(format!("read past end at {pos}")))?;
    Ok(u32::from_le_bytes([
        b.first().copied().unwrap_or(0),
        b.get(1).copied().unwrap_or(0),
        b.get(2).copied().unwrap_or(0),
        b.get(3).copied().unwrap_or(0),
    ]))
}

/// `field + rel` with `rel` a signed 32-bit offset.
fn target(field: usize, rel: u32) -> Result<usize> {
    let rel = i64::from(rel as i32);
    usize::try_from(i64::try_from(field).map_err(|_| Error::format("offset overflow"))? + rel)
        .map_err(|_| Error::format("negative target"))
}

fn table_count(v: u32, limits: &Limits) -> Result<usize> {
    let n = usize::try_from(v).map_err(|_| Error::format("bad count"))?;
    if n > MAX_TABLE.max(usize::try_from(limits.max_entries).unwrap_or(usize::MAX)) {
        return Err(Error::limit(format!("table count {n} too large")));
    }
    Ok(n)
}

/// Finds the chunk with `id`; returns the body range.
fn chunk(d: &[u8], id: u32) -> Result<std::ops::Range<usize>> {
    let mut pos = 0usize;
    for _ in 0..16 {
        if pos >= d.len() {
            break;
        }
        let cid = u32_at(d, pos)?;
        let size = usize::try_from(u32_at(d, pos.saturating_add(4))?)
            .map_err(|_| Error::format("bad chunk size"))?;
        let start = pos
            .checked_add(8)
            .ok_or_else(|| Error::format("offset overflow"))?;
        let end = start
            .checked_add(size)
            .filter(|e| *e <= d.len())
            .ok_or_else(|| Error::format(format!("chunk {cid} runs past end")))?;
        if cid == id {
            return Ok(start..end);
        }
        pos = end;
    }
    Err(Error::format(format!("chunk {id} not found")))
}

/// Parses an inflated 32-bit database.
///
/// # Errors
///
/// [`Error::Format`] for anything that does not follow the layout, [`Error::Limit`] for
/// tables larger than the limits.
pub fn parse(d: &[u8], limits: &Limits) -> Result<Database32> {
    let meta_r = chunk(d, 2)?;
    let meta = d
        .get(meta_r.start..meta_r.end)
        .ok_or_else(|| Error::format("metadata out of range"))?;
    let dir = |i: usize| -> Result<(usize, usize)> {
        let field = i * 8;
        let rel = u32_at(meta, field)?;
        let n = table_count(u32_at(meta, field + 4)?, limits)?;
        Ok((target(field, rel)?, n))
    };
    let (files_at, files_n) = dir(0)?;
    let (structs_at, structs_n) = dir(1)?;

    let mut structs = Vec::new();
    for i in 0..structs_n {
        let e = structs_at
            .checked_add(i * 12)
            .ok_or_else(|| Error::format("overflow"))?;
        let at = target(e, u32_at(meta, e)?)?;
        let size =
            usize::try_from(u32_at(meta, e + 4)?).map_err(|_| Error::format("bad name size"))?;
        let raw = meta
            .get(
                at..at
                    .checked_add(size)
                    .ok_or_else(|| Error::format("overflow"))?,
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

    let mut files = Vec::new();
    for b in 0..files_n {
        let bucket = files_at
            .checked_add(b * 8)
            .ok_or_else(|| Error::format("overflow"))?;
        let n = table_count(u32_at(meta, bucket + 4)?, limits)?;
        if n == 0 {
            continue;
        }
        let recs = target(bucket, u32_at(meta, bucket)?)?;
        for r in 0..n {
            if files.len() as u64 >= limits.max_entries {
                return Err(Error::limit("too many resources"));
            }
            let rec = recs
                .checked_add(r * 12)
                .ok_or_else(|| Error::format("overflow"))?;
            let size = usize::try_from(u32_at(meta, rec + 4)?)
                .ok()
                .filter(|s| *s >= 9)
                .ok_or_else(|| Error::format("bad name block size"))?;
            let db_id = u64::from(u32_at(meta, rec + 8)?);
            let blk = target(rec, u32_at(meta, rec)?)?;
            let stored = u32_at(meta, blk + 4)?;
            let name_at = blk + 8;
            let name = meta
                .get(
                    name_at
                        ..name_at
                            .checked_add(size - 9)
                            .ok_or_else(|| Error::format("overflow"))?,
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
    files.sort_by_key(|f| f.db_id);
    let data = chunk(d, 3)?;
    Ok(Database32 {
        structs,
        files,
        data,
    })
}

impl Database32 {
    /// Texture sizes `(width, height)` by lower-case resource name (names ending in
    /// `(Texture).xdb`); lets a caller drop the large inflated payload.
    #[must_use]
    pub fn texture_sizes(&self, payload: &[u8]) -> std::collections::HashMap<String, (u32, u32)> {
        let mut out = std::collections::HashMap::new();
        for f in &self.files {
            if f.name.ends_with("(Texture).xdb")
                && let Some(t) = self.texture(payload, f.db_id)
            {
                out.insert(f.name.to_ascii_lowercase(), (t.width, t.height));
            }
        }
        out
    }

    /// Reads the `Texture` object at `db_id`; `None` if it does not fit in the data image.
    ///
    /// Two object layouts exist. Cloud Pirates 1.5 uses 100 bytes with width at +60;
    /// client 10.0 has one more word before it (104 bytes, width at +64). The layout is
    /// told apart by the `0xFFFF_FFFF` word that follows the width.
    #[must_use]
    pub fn texture(&self, payload: &[u8], db_id: u64) -> Option<Texture32> {
        let base = self.data.start.checked_add(usize::try_from(db_id).ok()?)?;
        let end = base.checked_add(104)?;
        if end > self.data.end || end > payload.len() {
            return None;
        }
        let w = |o: usize| u32_at(payload, base + o).ok();
        let shift = if w(64)? == u32::MAX {
            0
        } else if w(68)? == u32::MAX {
            4
        } else {
            return None;
        };
        Some(Texture32 {
            width: w(60 + shift)?,
            mips: w(68 + shift)?,
            kind: w(72 + shift)?,
            alpha: w(84 + shift)?,
            height: w(88 + shift)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    /// One resource `A/B.xdb` with db id 8, one struct, a data image holding a texture.
    fn build() -> Vec<u8> {
        build_named(b"A/B.xdb")
    }

    fn build_named(name: &[u8]) -> Vec<u8> {
        let mut m = vec![0u8; 24]; // directory 0..3 (3 pairs)
        let mut meta_tail = Vec::new();
        // struct names block
        let sname = b"struct NDb::Texture\0";
        let s_name_at = 24 + meta_tail.len();
        meta_tail.extend_from_slice(sname);
        while (24 + meta_tail.len()) % 4 != 0 {
            meta_tail.push(0);
        }
        let s_rec_at = 24 + meta_tail.len();
        put(&mut meta_tail, (s_name_at as i64 - s_rec_at as i64) as u32);
        put(&mut meta_tail, sname.len() as u32);
        put(&mut meta_tail, 0);
        // name block: u32 version, u32 adler, name, NUL
        let blk_at = 24 + meta_tail.len();
        put(&mut meta_tail, 1);
        put(&mut meta_tail, adler32(name));
        meta_tail.extend_from_slice(name);
        meta_tail.push(0);
        while (24 + meta_tail.len()) % 4 != 0 {
            meta_tail.push(0);
        }
        // one record: rel, size, id
        let rec_at = 24 + meta_tail.len();
        put(&mut meta_tail, (blk_at as i64 - rec_at as i64) as u32);
        put(&mut meta_tail, (name.len() + 9) as u32);
        put(&mut meta_tail, 8);
        // buckets: one bucket pointing at the record
        let bucket_at = 24 + meta_tail.len();
        put(&mut meta_tail, (rec_at as i64 - bucket_at as i64) as u32);
        put(&mut meta_tail, 1);
        let dir = |m: &mut Vec<u8>, i: usize, at: usize, n: u32| {
            let field = i * 8;
            m[field..field + 4].copy_from_slice(&((at as i64 - field as i64) as u32).to_le_bytes());
            m[field + 4..field + 8].copy_from_slice(&n.to_le_bytes());
        };
        dir(&mut m, 0, bucket_at, 1);
        dir(&mut m, 1, s_rec_at, 1);
        m.extend_from_slice(&meta_tail);
        let mut data = [0u8; 8 + 104];
        data[8 + 60..8 + 64].copy_from_slice(&512u32.to_le_bytes());
        data[8 + 64..8 + 68].copy_from_slice(&u32::MAX.to_le_bytes());
        data[8 + 68..8 + 72].copy_from_slice(&8u32.to_le_bytes());
        data[8 + 72..8 + 76].copy_from_slice(&2u32.to_le_bytes());
        data[8 + 84..8 + 88].copy_from_slice(&2u32.to_le_bytes());
        data[8 + 88..8 + 92].copy_from_slice(&256u32.to_le_bytes());
        let mut d = Vec::new();
        for (id, body) in [(0u32, &[0u8; 8][..]), (2, &m[..]), (3, &data[..])] {
            put(&mut d, id);
            put(&mut d, body.len() as u32);
            d.extend_from_slice(body);
        }
        d
    }

    #[test]
    fn parses_names_and_texture() {
        let d = build();
        let db = parse(&d, &Limits::default()).unwrap();
        assert_eq!(db.structs, ["Texture"]);
        assert_eq!(db.files.len(), 1);
        assert_eq!(db.files[0].name, "A/B.xdb");
        let t = db.texture(&d, 8).unwrap();
        assert_eq!(
            (t.width, t.height, t.mips, t.kind, t.alpha),
            (512, 256, 8, 2, 2)
        );
        assert!(db.texture(&d, 1000).is_none());
    }

    /// Real file check; set `ADV_DB32` to an inflated Cloud Pirates `pack.bin`.
    #[test]
    fn real_database_when_available() {
        let Ok(path) = std::env::var("ADV_DB32") else {
            return;
        };
        let d = std::fs::read(path).unwrap();
        let db = parse(&d, &Limits::default()).unwrap();
        assert_eq!(db.files.len(), 42_827);
        assert_eq!(db.structs.len(), 742);
        let t = db
            .files
            .iter()
            .find(|f| f.name.ends_with("Fish_Bones_01_Back.(Texture).xdb"))
            .unwrap();
        let r = db.texture(&d, t.db_id).unwrap();
        assert_eq!((r.width, r.height, r.mips, r.kind), (512, 512, 8, 2));
    }

    #[test]
    fn texture_layout_with_extra_word() {
        let mut d = build();
        // the data chunk is last; the 104 byte object ends it
        let start = d.len() - 104;
        let body = &mut d[start..];
        body.copy_within(60..92, 64);
        body[60..64].copy_from_slice(&0u32.to_le_bytes());
        // width/mips/type/.. now live 4 bytes later and the sentinel at +68
        body[64..68].copy_from_slice(&1024u32.to_le_bytes());
        body[68..72].copy_from_slice(&u32::MAX.to_le_bytes());
        let db = parse(&d, &Limits::default()).unwrap();
        let t = db.texture(&d, 8).unwrap();
        assert_eq!((t.width, t.mips), (1024, 8));
    }

    /// Real file check; set `ADV_DB32_V10` to an inflated client 10.0 `pack.bin`.
    #[test]
    fn real_v10_when_available() {
        let Ok(path) = std::env::var("ADV_DB32_V10") else {
            return;
        };
        let d = std::fs::read(path).unwrap();
        let db = parse(&d, &Limits::default()).unwrap();
        assert_eq!(db.files.len(), 411_035);
        assert_eq!(db.structs.len(), 1369);
        let f = db
            .files
            .iter()
            .find(|f| f.name.ends_with("Sword_1H_B_02CommonSteelFX.(Texture).xdb"))
            .unwrap();
        let t = db.texture(&d, f.db_id).unwrap();
        assert_eq!((t.width, t.height, t.mips), (128, 64, 5));
    }

    #[test]
    fn texture_index_keys_are_lowercase_xdb_names() {
        let d = build_named(b"Maps/X/1_2_SplatMap_0.(Texture).xdb");
        let db = parse(&d, &Limits::default()).unwrap();
        let idx = db.texture_sizes(&d);
        assert_eq!(
            idx.get("maps/x/1_2_splatmap_0.(texture).xdb"),
            Some(&(512, 256))
        );
        // other resource kinds are not indexed
        let d = build();
        let db = parse(&d, &Limits::default()).unwrap();
        assert!(db.texture_sizes(&d).is_empty());
    }

    #[test]
    fn truncated_input_fails() {
        let d = build();
        for cut in [0, 7, 20, d.len() - 50] {
            assert!(parse(&d[..cut], &Limits::default()).is_err());
        }
    }

    #[test]
    fn corrupted_checksum_fails() {
        let mut d = build();
        let at = d.windows(7).position(|w| w == b"A/B.xdb").unwrap();
        d[at] = b'X';
        assert!(parse(&d, &Limits::default()).is_err());
    }
}
