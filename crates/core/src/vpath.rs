//! Validated paths inside archives.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// A path inside an archive: UTF-8, `/` separated, relative, no `.` or `..`.
///
/// Hostile archives may contain names such as `../../x`, `/etc/passwd` or `C:\x`. A
/// `VPath` can only be built from a name that is safe to join to an output folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VPath(String);

impl VPath {
    /// Validates `name` and returns it as a path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Format`] with the reason when the name is empty, too long,
    /// absolute, has a drive letter, a backslash, a NUL or control character, an empty
    /// component, or a `.` / `..` component.
    pub fn parse(name: &str, max_len: usize) -> Result<Self> {
        if name.is_empty() {
            return Err(Error::format("empty path"));
        }
        if name.len() > max_len {
            return Err(Error::format(format!("path longer than {max_len} bytes")));
        }
        if name.chars().any(|c| c.is_control()) {
            return Err(Error::format("path contains a control character"));
        }
        if name.contains('\\') {
            return Err(Error::format("path contains a backslash"));
        }
        if name.starts_with('/') {
            return Err(Error::format("absolute path"));
        }
        let bytes = name.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            return Err(Error::format("path starts with a drive letter"));
        }
        for part in name.split('/') {
            match part {
                "" => return Err(Error::format("empty path component")),
                "." | ".." => return Err(Error::format("path contains . or .. component")),
                _ => {}
            }
        }
        Ok(Self(name.to_owned()))
    }

    /// The path as stored in the archive.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Path components.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Builds the location under `base` for extraction.
    ///
    /// With `flat` only the file name is used. On Windows characters that are not
    /// allowed in file names are replaced with `_`, and reserved device names get a
    /// leading `_`; the returned flag says whether anything was changed.
    #[must_use]
    pub fn to_native(&self, base: &Path, flat: bool) -> (PathBuf, bool) {
        let mut out = base.to_path_buf();
        let mut changed = false;
        let parts: Vec<&str> = self.components().collect();
        let parts = if flat {
            &parts[parts.len() - 1..]
        } else {
            &parts[..]
        };
        for part in parts {
            let (clean, c) = sanitize_component(part);
            changed |= c;
            out.push(clean);
        }
        (out, changed)
    }
}

/// Characters Windows does not allow in names.
const WINDOWS_FORBIDDEN: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

/// Reserved device names on Windows (compared case-insensitively, without extension).
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Makes one path component safe for the current platform. On other platforms than
/// Windows the component is returned unchanged.
fn sanitize_component(part: &str) -> (String, bool) {
    sanitize_for(part, cfg!(windows))
}

/// Platform independent core of [`sanitize_component`], so it can be tested everywhere.
fn sanitize_for(part: &str, windows: bool) -> (String, bool) {
    if !windows {
        return (part.to_owned(), false);
    }
    let mut changed = false;
    let mut s: String = part
        .chars()
        .map(|c| {
            if WINDOWS_FORBIDDEN.contains(&c) {
                changed = true;
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = s.trim_end_matches(['.', ' ']).len();
    if trimmed != s.len() {
        s.truncate(trimmed);
        changed = true;
    }
    if s.is_empty() {
        s.push('_');
        changed = true;
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
        changed = true;
    }
    (s, changed)
}

impl std::fmt::Display for VPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(name: &str) -> VPath {
        VPath::parse(name, 1024).expect("valid path")
    }

    #[test]
    fn accepts_normal_paths() {
        assert_eq!(ok("Bin/pack.rus.loc").as_str(), "Bin/pack.rus.loc");
        assert_eq!(ok("Creatures/Beast.(Geometry).bin").components().count(), 2);
        assert!(VPath::parse("файл/имя.txt", 1024).is_ok());
    }

    #[test]
    fn rejects_hostile_paths() {
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../x",
            "a/./b",
            "a//b",
            "C:/x",
            "c:x",
            "a\\b",
            "a\0b",
            "a\nb",
            "..",
            ".",
            "a/",
        ] {
            assert!(VPath::parse(bad, 1024).is_err(), "must reject {bad:?}");
        }
    }

    #[test]
    fn rejects_too_long_path() {
        let long = "a".repeat(1025);
        assert!(VPath::parse(&long, 1024).is_err());
        assert!(VPath::parse(&"a".repeat(1024), 1024).is_ok());
    }

    #[test]
    fn joins_under_base_and_flattens() {
        let base = Path::new("out");
        let (p, _) = ok("a/b/c.txt").to_native(base, false);
        assert_eq!(p, Path::new("out").join("a").join("b").join("c.txt"));
        let (p, _) = ok("a/b/c.txt").to_native(base, true);
        assert_eq!(p, Path::new("out").join("c.txt"));
    }

    #[test]
    fn windows_sanitizing_replaces_forbidden_and_reserved() {
        assert_eq!(
            sanitize_for("a:b?.txt", true),
            ("a_b_.txt".to_owned(), true)
        );
        assert_eq!(sanitize_for("NUL.txt", true), ("_NUL.txt".to_owned(), true));
        assert_eq!(sanitize_for("com1", true), ("_com1".to_owned(), true));
        assert_eq!(sanitize_for("name. ", true), ("name".to_owned(), true));
        assert_eq!(sanitize_for("ok.txt", true), ("ok.txt".to_owned(), false));
        assert_eq!(sanitize_for("a:b", false), ("a:b".to_owned(), false));
    }
}
