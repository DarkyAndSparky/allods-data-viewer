//! Sound binding of characters in the 32-bit pack (client 1.1 / 7.0 layout).
//!
//! A `CharacterSoundVariation` record lists FMOD events by path; an `FMODProject` record names the
//! `.bev` event project and the `.bsb` banks. Pointers are resolved with the pointer-fix table.
//! Layouts and status (draft, checked on `ElfMale`): `docs/formats/sound-binding.md`.

use std::collections::HashMap;

use crate::db32::Database32;
use crate::error::{Error, Result};
use crate::fixes::PackFix;
use crate::limits::Limits;
use crate::material::FixIndex;

/// `CharacterSoundVariation`: first array triple (pointer, byte size).
const EVENTS_AT: u64 = 24;
/// `CharacterSoundVariation`: reference to the default `FMODProject`.
const PROJECT_AT: u64 = 64;
/// Byte size of one event entry.
const ENTRY_SIZE: u64 = 32;
/// Entry: reference to a `FMODProject`.
const ENTRY_PROJECT_AT: u64 = 8;
/// Entry: event path string triple.
const ENTRY_PATH_AT: u64 = 16;
/// Entry: small number, meaning not known.
const ENTRY_KIND_AT: u64 = 28;
/// `FMODProject`: `.bev` path string triple.
const BEV_AT: u64 = 32;
/// `FMODProject`: bank path array (pointer at +60, byte size at +64), 12-byte string triples.
const BANKS_AT: u64 = 60;
/// More entries or banks than this in one record are treated as corrupted input.
const MAX_ENTRIES: u64 = 8192;
/// Longest accepted string.
const MAX_TEXT: u64 = 1024;

/// One event of a character sound record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEvent {
    /// Resource name of the `FMODProject` that holds the event, when the entry names one.
    pub project: Option<String>,
    /// FMOD event path, e.g. `PC/ELfMale/Animation/ElfMale_attack_Universal`.
    pub event: String,
    /// Number stored next to the event (meaning not known).
    pub kind: u32,
}

/// An `FMODProject`: the event project file and its sound banks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundProject {
    /// Path of the `.bev`/`.fev` file inside the paks.
    pub events_file: String,
    /// Paths of the `.bsb`/`.fsb` banks.
    pub banks: Vec<String>,
}

/// A character sound record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSounds {
    /// Resource name of the default `FMODProject`.
    pub project: Option<String>,
    /// Events in file order.
    pub events: Vec<SoundEvent>,
}

struct Image<'a> {
    payload: &'a [u8],
    data: usize,
    fixes: FixIndex,
    names: HashMap<u64, &'a str>,
}

impl Image<'_> {
    fn word(&self, off: u64) -> Option<u32> {
        let at = usize::try_from(off).ok()?.checked_add(self.data)?;
        let b = self.payload.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes(b.try_into().ok()?))
    }

    /// Resource name behind the reference (fix tag 0) at `off`.
    fn reference(&self, off: u64) -> Option<String> {
        let f = self.fixes.get(off).filter(|f| f.tag == 0)?;
        self.names
            .get(&u64::from(f.value))
            .map(|n| (*n).to_string())
    }

    /// Text of the string triple `(pointer, length, capacity)` at `off`.
    fn text(&self, off: u64) -> Option<String> {
        let f = self.fixes.get(off).filter(|f| f.tag == 1)?;
        let len = u64::from(self.word(off.checked_add(4)?)?);
        if len == 0 || len > MAX_TEXT {
            return None;
        }
        let at = usize::try_from(u64::from(f.value))
            .ok()?
            .checked_add(self.data)?;
        let end = at.checked_add(usize::try_from(len).ok()?)?;
        let raw = self.payload.get(at..end)?;
        Some(String::from_utf8_lossy(raw).into_owned())
    }

    /// Array at `off`: base pointer and element count for `size`-byte elements.
    fn array(&self, off: u64, size: u64) -> Result<Option<(u64, u64)>> {
        let Some(f) = self.fixes.get(off).filter(|f| f.tag == 1) else {
            return Ok(None);
        };
        let Some(bytes) = off.checked_add(4).and_then(|o| self.word(o)) else {
            return Ok(None);
        };
        let bytes = u64::from(bytes);
        if bytes % size != 0 || bytes / size > MAX_ENTRIES {
            return Err(Error::format("sound array size is not plausible"));
        }
        Ok(Some((u64::from(f.value), bytes / size)))
    }
}

fn image<'a>(
    db: &'a Database32,
    payload: &'a [u8],
    fixes: &[PackFix],
    limits: &Limits,
) -> Result<Image<'a>> {
    if fixes.len() as u64 > limits.max_fixes {
        return Err(Error::limit("pack fix count exceeds the limit"));
    }
    Ok(Image {
        payload,
        data: db.data.start,
        fixes: FixIndex::new(fixes),
        names: db
            .files
            .iter()
            .map(|f| (f.db_id, f.name.as_str()))
            .collect(),
    })
}

fn read_character(img: &Image<'_>, root: u64) -> Result<CharacterSounds> {
    let mut events = Vec::new();
    if let Some((base, n)) = img.array(root.saturating_add(EVENTS_AT), ENTRY_SIZE)? {
        for i in 0..n {
            let Some(e) = i.checked_mul(ENTRY_SIZE).and_then(|o| base.checked_add(o)) else {
                break;
            };
            let Some(event) = img.text(e.saturating_add(ENTRY_PATH_AT)) else {
                continue;
            };
            events.push(SoundEvent {
                project: img.reference(e.saturating_add(ENTRY_PROJECT_AT)),
                event,
                kind: img.word(e.saturating_add(ENTRY_KIND_AT)).unwrap_or(0),
            });
        }
    }
    Ok(CharacterSounds {
        project: img.reference(root.saturating_add(PROJECT_AT)),
        events,
    })
}

fn read_project(img: &Image<'_>, root: u64) -> Result<Option<SoundProject>> {
    let Some(events_file) = img.text(root.saturating_add(BEV_AT)) else {
        return Ok(None);
    };
    let mut banks = Vec::new();
    if let Some((base, n)) = img.array(root.saturating_add(BANKS_AT), 12)? {
        for i in 0..n {
            let Some(e) = i.checked_mul(12).and_then(|o| base.checked_add(o)) else {
                break;
            };
            if let Some(t) = img.text(e) {
                banks.push(t);
            }
        }
    }
    Ok(Some(SoundProject { events_file, banks }))
}

/// Sound records of every `CharacterSoundVariation` resource, keyed by resource name. Records that
/// cannot be read are skipped.
///
/// # Errors
/// Returns an error when the pointer-fix table is larger than the limit.
pub fn all_characters(
    db: &Database32,
    payload: &[u8],
    fixes: &[PackFix],
    limits: &Limits,
) -> Result<Vec<(String, CharacterSounds)>> {
    let img = image(db, payload, fixes, limits)?;
    let mut out = Vec::new();
    for f in &db.files {
        if f.name.ends_with("(CharacterSoundVariation).xdb")
            && let Ok(c) = read_character(&img, f.db_id)
        {
            out.push((f.name.clone(), c));
        }
    }
    Ok(out)
}

/// Event project and banks of every `FMODProject` resource, keyed by resource name.
///
/// # Errors
/// Returns an error when the pointer-fix table is larger than the limit.
pub fn all_projects(
    db: &Database32,
    payload: &[u8],
    fixes: &[PackFix],
    limits: &Limits,
) -> Result<Vec<(String, SoundProject)>> {
    let img = image(db, payload, fixes, limits)?;
    let mut out = Vec::new();
    for f in &db.files {
        if f.name.ends_with("(FMODProject).xdb")
            && let Ok(Some(p)) = read_project(&img, f.db_id)
        {
            out.push((f.name.clone(), p));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbFile;

    fn put(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Character at 0 (one event at 200, text at 400), project at 500 (bev text at 600, one bank
    /// triple at 650 with text at 700).
    fn synthetic() -> (Database32, Vec<u8>, Vec<PackFix>) {
        let mut d = vec![0u8; 800];
        let fix = |offset, tag, value| PackFix { offset, tag, value };
        put(&mut d, 28, 32);
        put(&mut d, 200 + 20, 5);
        put(&mut d, 200 + 24, 16);
        put(&mut d, 200 + 28, 4);
        d[400..405].copy_from_slice(b"a/b/c");
        put(&mut d, 500 + 36, 9);
        d[600..610].copy_from_slice(b"SFX/X.bev\0");
        put(&mut d, 500 + 64, 12);
        put(&mut d, 650 + 4, 9);
        d[700..709].copy_from_slice(b"SFX/X.bsb");
        let fixes = vec![
            fix(24, 1, 200),
            fix(64, 0, 500),
            fix(200 + 8, 0, 500),
            fix(200 + 16, 1, 400),
            fix(500 + 32, 1, 600),
            fix(500 + 60, 1, 650),
            fix(650, 1, 700),
        ];
        let db = Database32 {
            structs: Vec::new(),
            files: vec![
                DbFile {
                    name: "C/A.(CharacterSoundVariation).xdb".into(),
                    db_id: 0,
                },
                DbFile {
                    name: "SFX/X.(FMODProject).xdb".into(),
                    db_id: 500,
                },
            ],
            data: 0..800,
        };
        (db, d, fixes)
    }

    #[test]
    fn events_project_and_banks_are_read() {
        let (db, d, fixes) = synthetic();
        let l = Limits::default();
        let c = all_characters(&db, &d, &fixes, &l).unwrap();
        assert_eq!(c.len(), 1);
        let e = &c[0].1.events[0];
        assert_eq!(e.event, "a/b/c");
        assert_eq!(e.kind, 4);
        assert_eq!(e.project.as_deref(), Some("SFX/X.(FMODProject).xdb"));
        assert_eq!(c[0].1.project.as_deref(), Some("SFX/X.(FMODProject).xdb"));
        let p = all_projects(&db, &d, &fixes, &l).unwrap();
        assert_eq!(p[0].1.events_file, "SFX/X.bev");
        assert_eq!(p[0].1.banks, vec!["SFX/X.bsb".to_string()]);
    }

    #[test]
    fn missing_fixes_give_empty_records() {
        let (db, d, _) = synthetic();
        let c = all_characters(&db, &d, &[], &Limits::default()).unwrap();
        assert!(c[0].1.events.is_empty() && c[0].1.project.is_none());
        assert!(
            all_projects(&db, &d, &[], &Limits::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn truncated_image_and_bad_sizes_never_panic() {
        let (db, d, fixes) = synthetic();
        for cut in 0..d.len() {
            let _ = all_characters(&db, &d[..cut], &fixes, &Limits::default());
            let _ = all_projects(&db, &d[..cut], &fixes, &Limits::default());
        }
        let mut bad = d.clone();
        put(&mut bad, 28, 33);
        assert!(
            all_characters(&db, &bad, &fixes, &Limits::default())
                .unwrap()
                .iter()
                .all(|c| c.1.events.is_empty())
        );
        put(&mut bad, 28, 32 * 9000);
        assert!(
            all_characters(&db, &bad, &fixes, &Limits::default())
                .unwrap()
                .iter()
                .all(|c| c.1.events.is_empty())
        );
    }

    #[test]
    fn pointers_outside_the_image_are_ignored() {
        let (db, d, mut fixes) = synthetic();
        fixes[3].value = u32::MAX;
        fixes[0].value = u32::MAX;
        let c = all_characters(&db, &d, &fixes, &Limits::default()).unwrap();
        assert!(c[0].1.events.is_empty());
    }
}
