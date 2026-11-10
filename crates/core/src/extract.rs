//! Safe extraction of archive entries to disk.

use crate::error::{Error, Result};
use crate::pak::{Entry, Layer, PakArchive};
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

/// Options for [`extract_entry`].
#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions {
    /// Which layer to write.
    pub layer: Layer,
    /// Replace files that already exist.
    pub overwrite: bool,
    /// Ignore folders and write only file names.
    pub flat: bool,
}

/// What happened to one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Written to the given path; `renamed` says the name was changed for Windows.
    Written {
        /// Final location.
        path: PathBuf,
        /// The name had to be sanitized.
        renamed: bool,
    },
    /// The target already exists and `overwrite` was not set.
    Exists(PathBuf),
}

/// Refuses to go through symbolic links below `base`.
fn check_no_symlinks(base: &Path, target: &Path) -> Result<()> {
    let rel = target
        .strip_prefix(base)
        .map_err(|_| Error::format("target outside of output folder"))?;
    let mut cur = base.to_path_buf();
    for part in rel.components() {
        cur.push(part);
        match fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::format(format!(
                    "refusing to write through symlink {}",
                    cur.display()
                )));
            }
            Ok(_) | Err(_) => {}
        }
    }
    Ok(())
}

/// Extracts one entry under `out_dir`.
///
/// The file is written next to its target as `<name>.adv-part` and renamed afterwards,
/// so an interrupted run never leaves a half-written file under the final name.
///
/// # Errors
///
/// Fails on read, CRC, limit or write errors.
pub fn extract_entry<R: Read + Seek>(
    pak: &mut PakArchive<R>,
    entry: &Entry,
    out_dir: &Path,
    options: ExtractOptions,
) -> Result<Outcome> {
    let (target, renamed) = entry.path.to_native(out_dir, options.flat);
    check_no_symlinks(out_dir, &target)?;
    if target.exists() && !options.overwrite {
        return Ok(Outcome::Exists(target));
    }
    let (data, _) = pak.read(entry, options.layer)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = target.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".adv-part");
    let tmp = target.with_file_name(tmp_name);
    if let Err(e) = fs::write(&tmp, &data).and_then(|()| fs::rename(&tmp, &target)) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(Outcome::Written {
        path: target,
        renamed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Limits;
    use crate::pak::testutil::{make_zip, zlib};
    use std::io::Cursor;

    const OPTS: ExtractOptions = ExtractOptions {
        layer: Layer::Decoded,
        overwrite: false,
        flat: false,
    };

    fn pak(files: &[(&str, &[u8], bool)]) -> PakArchive<Cursor<Vec<u8>>> {
        PakArchive::open(Cursor::new(make_zip(files)), Limits::default()).expect("open")
    }

    #[test]
    fn writes_files_in_subfolders() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut p = pak(&[("a/b/c.txt", b"data", false)]);
        let e = p.entries()[0].clone();
        let out = extract_entry(&mut p, &e, dir.path(), OPTS).expect("extract");
        assert!(matches!(out, Outcome::Written { .. }));
        assert_eq!(
            fs::read(dir.path().join("a/b/c.txt")).expect("read"),
            b"data"
        );
        assert!(!dir.path().join("a/b/c.txt.adv-part").exists());
    }

    #[test]
    fn decodes_zlib_layer_and_raw_mode_keeps_it() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut p = pak(&[("x.bin", &zlib(b"payload payload"), false)]);
        let e = p.entries()[0].clone();
        extract_entry(&mut p, &e, dir.path(), OPTS).expect("extract");
        assert_eq!(
            fs::read(dir.path().join("x.bin")).expect("read"),
            b"payload payload"
        );
        let raw = ExtractOptions {
            layer: Layer::Raw,
            overwrite: true,
            ..OPTS
        };
        extract_entry(&mut p, &e, dir.path(), raw).expect("extract raw");
        assert_eq!(
            fs::read(dir.path().join("x.bin")).expect("read"),
            zlib(b"payload payload")
        );
    }

    #[test]
    fn existing_file_is_kept_without_overwrite() {
        let dir = tempfile::tempdir().expect("tmp");
        fs::write(dir.path().join("a.txt"), b"old").expect("write");
        let mut p = pak(&[("a.txt", b"new", false)]);
        let e = p.entries()[0].clone();
        let out = extract_entry(&mut p, &e, dir.path(), OPTS).expect("extract");
        assert!(matches!(out, Outcome::Exists(_)));
        assert_eq!(fs::read(dir.path().join("a.txt")).expect("read"), b"old");
    }

    #[test]
    fn flat_mode_drops_folders() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut p = pak(&[("a/b/c.txt", b"x", false)]);
        let e = p.entries()[0].clone();
        let opts = ExtractOptions { flat: true, ..OPTS };
        extract_entry(&mut p, &e, dir.path(), opts).expect("extract");
        assert!(dir.path().join("c.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_write_through_symlink() {
        let dir = tempfile::tempdir().expect("tmp");
        let outside = tempfile::tempdir().expect("tmp2");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).expect("symlink");
        let mut p = pak(&[("link/evil.txt", b"x", false)]);
        let e = p.entries()[0].clone();
        assert!(extract_entry(&mut p, &e, dir.path(), OPTS).is_err());
        assert!(!outside.path().join("evil.txt").exists());
    }
}
