//! Data for a file browser: a directory tree built from archive entries and a guess
//! of what a file is, so that a shell (CLI or app) can pick a preview without parsing formats.

use crate::pak::Entry;
use std::collections::BTreeMap;

/// What a file looks like, judged by name and the first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Plain text or XML-like text, and `.loc` localization files.
    Text,
    /// Texture data (`.dds`, `.tga`, `.png`, or a `.bin` texture container).
    Texture,
    /// FMOD sound bank (`.fsb`, `.bsb`).
    Audio,
    /// Video (`.ogv`).
    Video,
    /// Skeletal animation clip (`.(SkeletalAnimation).bin`).
    Animation,
    /// Geometry `.bin`; only found by looking into the data (see `preview::sniff`).
    Model,
    /// Resource database (`.xdb`, or `.bin` with the V2 signature).
    Database,
    /// Anything else (a `.bin` is a texture, a model or something else; the name cannot tell).
    Other,
}

impl FileKind {
    /// Every kind, in the order UIs list them.
    pub const ALL: [Self; 8] = [
        Self::Model,
        Self::Texture,
        Self::Animation,
        Self::Audio,
        Self::Video,
        Self::Text,
        Self::Database,
        Self::Other,
    ];

    /// The kind with the given [`name`](Self::name), if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Short stable name for UIs and the CLI.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Texture => "texture",
            Self::Audio => "audio",
            Self::Model => "model",
            Self::Animation => "animation",
            Self::Video => "video",
            Self::Database => "database",
            Self::Other => "other",
        }
    }
}

/// Guesses the kind of a file from its name and (optionally) its first bytes.
#[must_use]
pub fn classify(name: &str, head: &[u8]) -> FileKind {
    let mut lower = name.to_ascii_lowercase();
    // Backups (`x.cfg.bak`, `x.log.old`) are classified by the name they were made from.
    for suffix in [".bak", ".old", ".orig", ".tmp"] {
        if let Some(stem) = lower.strip_suffix(suffix) {
            lower = stem.to_owned();
            break;
        }
    }
    let lower = lower;
    // The game names resources `<name>.(<Type>).bin`; the tag tells textures from models.
    if lower.ends_with(".bin") {
        if lower.contains(".(texture)") {
            return FileKind::Texture;
        }
        if lower.contains(".(geometry)") {
            return FileKind::Model;
        }
        if lower.contains(".(skeletalanimation)") {
            return FileKind::Animation;
        }
    }
    let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
    match ext {
        "txt" | "xml" | "lua" | "json" | "ini" | "cfg" | "csv" | "html" | "htm" | "xsl" | "loc"
        | "log" | "md" | "yml" | "yaml" | "toml" | "bat" | "ps1" | "sh" | "xaml" | "rev"
        | "version" | "ver" => {
            return FileKind::Text;
        }
        "dds" | "tga" | "png" | "jpg" | "jpeg" | "bmp" | "gif" | "ico" | "cur" => {
            return FileKind::Texture;
        }
        "fsb" | "bsb" => return FileKind::Audio,
        "ogv" => return FileKind::Video,
        "xdb" => return FileKind::Database,
        _ => {}
    }
    if head.starts_with(b"DDS ") || head.starts_with(b"\x89PNG") {
        return FileKind::Texture;
    }
    if head.starts_with(b"FSB5") {
        return FileKind::Audio;
    }
    if crate::db::db_signature(head).is_some() {
        return FileKind::Database;
    }
    FileKind::Other
}

/// A node of the directory tree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Node {
    /// Sub-directories by name (sorted).
    pub dirs: BTreeMap<String, Node>,
    /// Files directly in this directory: name and index into the entry list given to [`build_tree`].
    pub files: BTreeMap<String, usize>,
}

impl Node {
    /// Number of files in this directory and below.
    #[must_use]
    pub fn total_files(&self) -> usize {
        self.files.len() + self.dirs.values().map(Node::total_files).sum::<usize>()
    }

    /// Finds a directory by a `/` separated path; the empty path is this node.
    #[must_use]
    pub fn dir(&self, path: &str) -> Option<&Node> {
        path.split('/')
            .filter(|c| !c.is_empty())
            .try_fold(self, |n, c| n.dirs.get(c))
    }
}

/// Builds a tree from entries. A path that is both a file and a directory prefix keeps both.
#[must_use]
pub fn build_tree(entries: &[Entry]) -> Node {
    let mut root = Node::default();
    for (i, e) in entries.iter().enumerate() {
        let comps: Vec<&str> = e.path.components().collect();
        let Some((file, dirs)) = comps.split_last() else {
            continue;
        };
        let mut node = &mut root;
        for d in dirs {
            node = node.dirs.entry((*d).to_string()).or_default();
        }
        node.files.insert((*file).to_string(), i);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, PakArchive, pak::testutil::make_zip};
    use std::io::Cursor;

    #[test]
    fn classify_by_name_and_magic() {
        assert_eq!(classify("a/b.XML", b""), FileKind::Text);
        assert_eq!(classify("x.bsb", b""), FileKind::Audio);
        assert_eq!(classify("x.ogv", b"OggS"), FileKind::Video);
        assert_eq!(classify("noext", b"DDS \0\0"), FileKind::Texture);
        assert_eq!(classify("noext", b"FSB5"), FileKind::Audio);
        assert_eq!(classify("noext", b"zzzz"), FileKind::Other);
        assert_eq!(classify("", b""), FileKind::Other);
        assert_eq!(classify("A.(Texture).bin", b""), FileKind::Texture);
        assert_eq!(classify("A.hi.(Texture).bin", b""), FileKind::Texture);
        assert_eq!(classify("a/B.(Geometry).BIN", b""), FileKind::Model);
        assert_eq!(
            classify("B.Run.(SkeletalAnimation).bin", b""),
            FileKind::Animation
        );
    }

    #[test]
    fn tree_groups_files_by_directory() {
        let zip = make_zip(&[
            ("a/b/c.txt", b"1", false),
            ("a/d.txt", b"2", false),
            ("e.txt", b"3", false),
        ]);
        let pak = PakArchive::open(Cursor::new(zip), Limits::default()).expect("open");
        let tree = build_tree(pak.entries());
        assert_eq!(tree.total_files(), 3);
        assert_eq!(tree.files.len(), 1);
        let a = tree.dir("a").expect("a");
        assert_eq!(a.files.len(), 1);
        assert_eq!(a.dir("b").expect("b").files.len(), 1);
        assert!(tree.dir("a/zzz").is_none());
        assert_eq!(tree.dir("").map(Node::total_files), Some(3));
    }
}
