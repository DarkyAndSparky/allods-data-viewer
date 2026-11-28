//! A virtual file system over a game data folder.
//!
//! Real directories and `.pak` archives are shown as one tree: a `.pak` file behaves like a
//! directory, so `Packs/Models.pak/creatures/x.bin` is a valid path. Paths use `/` and are
//! relative to the opened root. The desktop app is a thin shell over this crate.

use adv_core::archive::{Archive, Kind};
use adv_core::browse::{FileKind, Node, build_tree, classify};
use adv_core::loc::LocFile;
use adv_core::preview::{self, Options, Preview};
use adv_core::{Layer, Limits};
use adv_core::{db, db32};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Errors of the session layer; the text is English and technical, the app translates it.
#[derive(Debug)]
pub enum SessionError {
    /// The virtual path is not valid (empty parts, `..`, drive letters).
    BadPath(String),
    /// Nothing exists at the path.
    NotFound(String),
    /// The item is a directory where a file is needed, or the other way round.
    WrongKind(String),
    /// A file could not be read or parsed.
    Failed(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadPath(s) => write!(f, "invalid path: {s}"),
            Self::NotFound(s) => write!(f, "not found: {s}"),
            Self::WrongKind(s) => write!(f, "wrong kind of item: {s}"),
            Self::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// One row of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Name without the parent path.
    pub name: String,
    /// True for directories and for `.pak` archives (which can be entered).
    pub is_dir: bool,
    /// True for a `.pak` archive.
    pub is_pak: bool,
    /// File size in bytes (files only).
    pub size: Option<u64>,
    /// Number of files below (directories inside a `.pak` only).
    pub files: Option<usize>,
    /// Guessed kind by name (files only).
    pub kind: FileKind,
}

/// Totals over everything reachable from the root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    /// Number of files (archive entries and real files; archives themselves are not counted).
    pub files: u64,
    /// Number of `.pak` archives.
    pub paks: u64,
    /// Total size of the files in bytes.
    pub bytes: u64,
    /// Files and bytes per kind, judged by name only (a plain `.bin` counts as `Other`).
    pub kinds: Vec<(FileKind, u64, u64)>,
}

/// Content of `Profiles/game.version` in the game data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    /// Four-character tag at offset 4.
    pub head: String,
    /// Version text such as `17.0.4.2.78`.
    pub version: String,
}

/// Parses `game.version`: bytes 4..8 are a tag, bytes 8..12 a little-endian length, then the
/// version text. Returns `None` for anything shorter or not text.
#[must_use]
pub fn parse_game_version(data: &[u8]) -> Option<GameVersion> {
    let head = std::str::from_utf8(data.get(4..8)?).ok()?;
    let len = usize::try_from(u32::from_le_bytes(data.get(8..12)?.try_into().ok()?)).ok()?;
    let text = std::str::from_utf8(data.get(12..12usize.checked_add(len)?)?).ok()?;
    Some(GameVersion {
        head: head.trim_matches('\0').to_string(),
        version: text.trim_matches('\0').trim().to_string(),
    })
}

/// Plain listing of a parsed `game.version`: the version line, then every directory with its
/// files and sizes. No words, so it needs no translation.
fn game_version_text(v: &adv_core::gamever::GameVersion) -> String {
    use std::fmt::Write;
    let mut out = format!("{} ({})\n", v.version, v.tag);
    for dir in &v.dirs {
        let _ = writeln!(out, "\n{}", dir.path);
        for f in &dir.files {
            let _ = writeln!(out, "  {}\t{}", f.name, f.size);
        }
    }
    out
}

/// Completions for a path typed in the path box: the folders and archive files (`.pak`, `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.gz`) in the typed
/// directory whose names start with the typed rest (ASCII case-insensitive). Folders end with
/// the separator the user typed. Needs a separator in the input; at most 50 results, sorted.
#[must_use]
pub fn complete_path(input: &str) -> Vec<String> {
    const MAX: usize = 50;
    let Some(cut) = input.rfind(['/', '\\']) else {
        return Vec::new();
    };
    let (head, rest) = input.split_at(cut + 1);
    let sep = &input[cut..=cut];
    let Ok(entries) = std::fs::read_dir(head) else {
        return Vec::new();
    };
    let rest_lower = rest.to_ascii_lowercase();
    let mut out: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().starts_with(&rest_lower) {
                return None;
            }
            let is_dir = e.file_type().ok()?.is_dir();
            if !is_dir && !is_pak(&name) {
                return None;
            }
            Some(if is_dir {
                format!("{head}{name}{sep}")
            } else {
                format!("{head}{name}")
            })
        })
        .collect();
    out.sort_by_key(|p| p.trim_end_matches(['/', '\\']).to_ascii_lowercase());
    out.truncate(MAX);
    out
}

/// Escapes text for XML or HTML and drops characters XML cannot carry (control characters
/// other than tab, line feed and carriage return).
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if u32::from(c) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// The entry of `dir` whose name equals `name` ignoring ASCII case.
fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

/// A page of `.loc` items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocPage {
    /// Items in the whole file.
    pub total: usize,
    /// Items that match the query (all of them for an empty query).
    pub matched: usize,
    /// Position of the first returned item among the matches.
    pub offset: usize,
    /// The items of this page.
    pub items: Vec<adv_core::loc::LocItem>,
}

/// One hit of a text search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextHit {
    /// Virtual path of the file.
    pub path: String,
    /// Item number inside a `.loc` file, to open the viewer on that item.
    pub id: Option<u32>,
    /// Where the text matched: a `.loc` key or line, shortened to one line.
    pub snippet: String,
}

/// Longest plain text file the text search reads, in bytes (`.loc` files are bounded by the
/// session limits instead).
const MAX_SEARCHED_BYTES: usize = 2 * 1024 * 1024;
/// Most hits one file can give, so a single `.loc` with thousands of matches cannot fill the list.
const MAX_HITS_PER_FILE: usize = 5;

/// Cuts a one-line excerpt of `text` around the first case-insensitive match of `needle`
/// (already lower case); `None` when there is no match.
fn excerpt(text: &str, needle: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(needle)?;
    // Lowercasing can change byte lengths, so work on the character index of the match.
    let char_at = lower[..at].chars().count();
    let chars: Vec<char> = text.chars().collect();
    let from = char_at.saturating_sub(40);
    let to = (char_at + needle.chars().count() + 80).min(chars.len());
    let one_line: String = chars[from.min(chars.len())..to]
        .iter()
        .map(|c| if c.is_control() { ' ' } else { *c })
        .collect();
    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.push_str(one_line.trim());
    if to < chars.len() {
        out.push('…');
    }
    Some(out)
}

/// Build information from a `<revision>.rev` text file shipped next to the game data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildInfo {
    /// `Version = 1.1.02.0`.
    pub version: String,
    /// `Revision: 168362`.
    pub revision: String,
    /// Date part of `Last Changed Date`, such as `2010-06-03`.
    pub date: String,
}

/// Parses a `.rev` file (plain `key: value` lines, non-UTF-8 bytes are tolerated). Returns
/// `None` when no version is found.
#[must_use]
pub fn parse_build_info(data: &[u8]) -> Option<BuildInfo> {
    let text = String::from_utf8_lossy(data.get(..data.len().min(8192))?);
    let mut info = BuildInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(['=', ':']) else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "version" if info.version.is_empty() => info.version = value.to_string(),
            "revision" if info.revision.is_empty() => info.revision = value.to_string(),
            "last changed date" if info.date.is_empty() => {
                info.date = value.split_whitespace().next().unwrap_or("").to_string();
            }
            _ => {}
        }
    }
    (!info.version.is_empty()).then_some(info)
}

struct OpenPak {
    pak: Archive<BufReader<File>>,
    tree: Node,
}

/// Texture width and height by lower-case resource name.
/// Largest texture picture embedded in an exported model.
const MAX_TEXTURE_PNG: usize = 16 * 1024 * 1024;

type TextureSizes = HashMap<String, (u32, u32)>;
/// Texture files of all archives: lower-case path inside the archive to (archive, real path).
type TextureIndex = HashMap<String, Vec<(PathBuf, String)>>;
type ModelBindings = HashMap<String, Vec<adv_core::binding::ElementBinding>>;

/// An opened data folder or single archive.
pub struct Session {
    root: PathBuf,
    /// Set when a single `.pak` was opened: only this archive is shown in the root.
    only: Option<String>,
    limits: Limits,
    paks: Mutex<HashMap<PathBuf, OpenPak>>,
    /// The last parsed `.loc` file, so paging through it does not parse it again.
    loc_cache: Mutex<Option<(String, Arc<LocFile>)>>,
    /// The `BaseLoc*.pak` that holds the resource databases, found on first use.
    base_pak: std::sync::OnceLock<Option<PathBuf>>,
    /// Texture records of the resource databases (`pack` and `Maps_<name>`), by lower-case name.
    texdbs: Mutex<HashMap<String, Arc<TextureSizes>>>,
    /// Model elements and their textures from the 7.0 pack, by lower-case `…(Geometry).xdb`.
    bindings: Mutex<Option<Arc<ModelBindings>>>,
    /// Every `(Texture).bin` of every archive of the folder, built on first use. A model's
    /// texture may lie in another archive than the model (client 1.1 keeps the default skins in
    /// `BaseLoc*.pak`).
    tex_index: std::sync::OnceLock<TextureIndex>,
    /// The video the player is reading (virtual path and decoder), so frames come in order.
    video: Mutex<Option<(String, adv_core::video::VideoStream)>>,
}

/// Where a virtual path points.
enum Target {
    /// A real file or directory.
    Fs(PathBuf),
    /// Inside an archive; `inner` is the path in it (empty for the archive itself).
    Pak { file: PathBuf, inner: String },
}

/// Collects the paks that may carry the resource databases (`BaseLoc*.pak`, `Bin.pak`,
/// `System*.pak`; not the HiRes or Mini variants) up to three levels deep.
fn collect_base_paks(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if p.is_dir() {
            collect_base_paks(&p, depth + 1, out);
        } else if (name.starts_with("baseloc")
            || name.starts_with("bin.")
            || name.starts_with("system"))
            && name.ends_with(".pak")
            && !name.contains("hires")
            && !name.contains("mini")
        {
            out.push(p);
        }
    }
}

/// `.pak` archives, plain `.zip` files (mods, add-ons), `.tar`, `.tar.gz`, `.tgz` and `.gz` files
/// are entered like folders.
/// Collects every `.pak` below `dir` (up to three levels deep) except the HiRes and Mini variants.
fn collect_all_paks(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if p.is_dir() {
            collect_all_paks(&p, depth + 1, out);
        } else if name.ends_with(".pak") && !name.contains("hires") && !name.contains("mini") {
            out.push(p);
        }
    }
}

fn is_pak(name: &str) -> bool {
    Kind::from_name(name).is_some()
}

fn split(vpath: &str) -> Result<Vec<&str>, SessionError> {
    let mut out = Vec::new();
    for c in vpath.split('/') {
        if c.is_empty() {
            continue;
        }
        if c == ".." || c == "." || c.contains(['\\', ':', '\0']) {
            return Err(SessionError::BadPath(vpath.to_string()));
        }
        out.push(c);
    }
    Ok(out)
}

impl Session {
    /// Opens a data folder, or a single `.pak` file (shown as the only item of the root).
    ///
    /// # Errors
    ///
    /// Fails when the path does not exist or is neither a directory nor a file.
    pub fn open(path: &Path) -> Result<Self, SessionError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| SessionError::Failed(format!("{}: {e}", path.display())))?;
        let (root, only) = if meta.is_dir() {
            (path.to_path_buf(), None)
        } else {
            let parent = path
                .parent()
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (parent, Some(name))
        };
        let session = Self {
            root,
            only,
            limits: Limits::default(),
            paks: Mutex::new(HashMap::new()),
            loc_cache: Mutex::new(None),
            video: Mutex::new(None),
            base_pak: std::sync::OnceLock::new(),
            texdbs: Mutex::new(HashMap::new()),
            bindings: Mutex::new(None),
            tex_index: std::sync::OnceLock::new(),
        };
        if let Some(name) = &session.only {
            if !is_pak(name) {
                return Err(SessionError::WrongKind(format!(
                    "{}: expected a folder or an archive (.pak, .zip, .tar, .tar.gz, .tgz, .gz)",
                    path.display()
                )));
            }
            // Fail early on a damaged archive.
            session.with_pak(path, |_| Ok(()))?;
        }
        Ok(session)
    }

    /// The folder the virtual paths start at.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn with_pak<T>(
        &self,
        file: &Path,
        f: impl FnOnce(&mut OpenPak) -> Result<T, SessionError>,
    ) -> Result<T, SessionError> {
        let mut map = self
            .paks
            .lock()
            .map_err(|_| SessionError::Failed("internal lock poisoned".into()))?;
        if !map.contains_key(file) {
            let fh = File::open(file)
                .map_err(|e| SessionError::Failed(format!("{}: {e}", file.display())))?;
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let pak = Archive::open(BufReader::new(fh), &name, self.limits)
                .map_err(|e| SessionError::Failed(format!("{}: {e}", file.display())))?;
            let tree = build_tree(pak.entries());
            map.insert(file.to_path_buf(), OpenPak { pak, tree });
        }
        let open = map
            .get_mut(file)
            .ok_or_else(|| SessionError::Failed("archive cache".into()))?;
        f(open)
    }

    fn resolve(&self, vpath: &str) -> Result<Target, SessionError> {
        let parts = split(vpath)?;
        if let (Some(only), Some(first)) = (&self.only, parts.first())
            && first != only
        {
            return Err(SessionError::NotFound(vpath.to_string()));
        }
        let mut cur = self.root.clone();
        for (i, part) in parts.iter().enumerate() {
            cur.push(part);
            if is_pak(part) && cur.is_file() {
                return Ok(Target::Pak {
                    file: cur,
                    inner: parts[i + 1..].join("/"),
                });
            }
            if !cur.exists() {
                return Err(SessionError::NotFound(vpath.to_string()));
            }
        }
        Ok(Target::Fs(cur))
    }

    /// Lists a directory (the empty path is the root).
    ///
    /// # Errors
    ///
    /// Fails for an invalid or missing path, or a path that is a file.
    pub fn list(&self, vpath: &str) -> Result<Vec<Item>, SessionError> {
        let mut items = match self.resolve(vpath)? {
            Target::Fs(dir) => {
                if !dir.is_dir() {
                    return Err(SessionError::WrongKind(vpath.to_string()));
                }
                self.list_fs(&dir)?
            }
            Target::Pak { file, inner } => self.with_pak(&file, |open| {
                let node = open
                    .tree
                    .dir(&inner)
                    .ok_or_else(|| SessionError::NotFound(vpath.to_string()))?;
                let entries = open.pak.entries();
                let mut v: Vec<Item> = node
                    .dirs
                    .iter()
                    .map(|(n, c)| Item {
                        name: n.clone(),
                        is_dir: true,
                        is_pak: false,
                        size: None,
                        files: Some(c.total_files()),
                        kind: FileKind::Other,
                    })
                    .collect();
                v.extend(node.files.iter().map(|(n, &i)| Item {
                    name: n.clone(),
                    is_dir: false,
                    is_pak: false,
                    size: entries.get(i).map(|e| e.size),
                    files: None,
                    kind: classify(n, &[]),
                }));
                Ok(v)
            })?,
        };
        items.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(items)
    }

    fn list_fs(&self, dir: &Path) -> Result<Vec<Item>, SessionError> {
        let rd = std::fs::read_dir(dir)
            .map_err(|e| SessionError::Failed(format!("{}: {e}", dir.display())))?;
        let mut out = Vec::new();
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if dir == self.root && self.only.as_ref().is_some_and(|o| *o != name) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let pak = meta.is_file() && is_pak(&name);
            out.push(Item {
                is_dir: meta.is_dir() || pak,
                is_pak: pak,
                size: meta.is_file().then_some(meta.len()),
                files: None,
                kind: if meta.is_file() {
                    classify(&name, &[])
                } else {
                    FileKind::Other
                },
                name,
            });
        }
        Ok(out)
    }

    /// Reads a whole file (inflating the extra zlib layer of archive entries).
    ///
    /// # Errors
    ///
    /// Fails for a missing path, a directory, a size over the limit or a damaged archive.
    pub fn read(&self, vpath: &str) -> Result<Vec<u8>, SessionError> {
        match self.resolve(vpath)? {
            Target::Fs(p) => {
                if !p.is_file() {
                    return Err(SessionError::WrongKind(vpath.to_string()));
                }
                let f =
                    File::open(&p).map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))?;
                let cap = self.limits.max_inflated_size;
                let mut buf = Vec::new();
                f.take(cap + 1)
                    .read_to_end(&mut buf)
                    .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))?;
                if buf.len() as u64 > cap {
                    return Err(SessionError::Failed(format!("{vpath}: file is too large")));
                }
                Ok(buf)
            }
            Target::Pak { file, inner } => self.with_pak(&file, |open| {
                let entry = open
                    .pak
                    .entries()
                    .iter()
                    .find(|e| e.path.as_str() == inner)
                    .cloned()
                    .ok_or_else(|| SessionError::NotFound(vpath.to_string()))?;
                open.pak
                    .read(&entry, Layer::Decoded)
                    .map(|(d, _)| d)
                    .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
            }),
        }
    }

    /// Looks into up to 500 files and tells their real kind (models and textures are both
    /// `.bin`). A file that cannot be read counts as `Other`.
    #[must_use]
    pub fn kinds(&self, paths: &[String]) -> Vec<(String, FileKind)> {
        paths
            .iter()
            .take(500)
            .map(|p| {
                let name = p.rsplit('/').next().unwrap_or(p);
                let kind = self
                    .read(p)
                    .map_or(FileKind::Other, |d| preview::sniff(name, &d, &self.limits));
                (p.clone(), kind)
            })
            .collect()
    }

    /// The game version from the first `game.version` found in the opened folder, one or two
    /// folders above it, or the `Profiles` folder of any of them. Names are matched
    /// case-insensitively.
    #[must_use]
    pub fn game_version(&self) -> Option<GameVersion> {
        // The opened folder and up to two levels above it (opening `Packs`, `data` or the
        // client root all work).
        for dir in self.root.ancestors().take(3) {
            let dir = dir.to_path_buf();
            let profiles = child_ci(&dir, "Profiles");
            for place in profiles.into_iter().chain(std::iter::once(dir)) {
                if let Some(file) = child_ci(&place, "game.version")
                    && let Some(v) = std::fs::read(file)
                        .ok()
                        .and_then(|d| parse_game_version(&d))
                {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Build information from the first `*.rev` file (up to 64 KiB) in the opened folder or its
    /// parent, when readable.
    #[must_use]
    pub fn build_info(&self) -> Option<BuildInfo> {
        let dirs = [Some(self.root.as_path()), self.root.parent()];
        for dir in dirs.into_iter().flatten() {
            let Ok(rd) = std::fs::read_dir(dir) else {
                continue;
            };
            for e in rd.flatten() {
                let path = e.path();
                let is_rev = path
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("rev"));
                let small = e.metadata().is_ok_and(|m| m.is_file() && m.len() <= 65_536);
                if is_rev
                    && small
                    && let Some(i) = std::fs::read(&path)
                        .ok()
                        .as_deref()
                        .and_then(parse_build_info)
                {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Counts the files below the root, by kind. Damaged archives and unreadable folders are
    /// skipped.
    ///
    /// # Errors
    ///
    /// Fails only when the root cannot be read.
    pub fn census(&self) -> Result<Census, SessionError> {
        let mut by_kind: HashMap<&'static str, (FileKind, u64, u64)> = HashMap::new();
        let mut total = Census::default();
        self.walk("", 0, &mut |_, item| {
            if item.is_pak {
                total.paks += 1;
            }
            if !item.is_dir {
                let size = item.size.unwrap_or(0);
                total.files += 1;
                total.bytes += size;
                let e = by_kind.entry(item.kind.name()).or_insert((item.kind, 0, 0));
                e.1 += 1;
                e.2 += size;
            }
            true
        })?;
        total.kinds = FileKind::ALL
            .iter()
            .filter_map(|k| by_kind.get(k.name()).copied())
            .collect();
        Ok(total)
    }

    /// Paths of files of one kind (by name), at most `limit`, in tree order.
    ///
    /// # Errors
    ///
    /// Fails only when the root cannot be read.
    pub fn find_kind(&self, kind: FileKind, limit: usize) -> Result<Vec<String>, SessionError> {
        let mut hits = Vec::new();
        if limit == 0 {
            return Ok(hits);
        }
        self.walk("", 0, &mut |path, item| {
            if !item.is_dir && item.kind == kind {
                hits.push(path.to_string());
            }
            hits.len() < limit
        })?;
        Ok(hits)
    }

    /// Visits every item below `vdir`; the visitor returns false to stop the whole walk.
    fn walk(
        &self,
        vdir: &str,
        depth: usize,
        visit: &mut dyn FnMut(&str, &Item) -> bool,
    ) -> Result<bool, SessionError> {
        const MAX_DEPTH: usize = 32;
        if depth > MAX_DEPTH {
            return Ok(true);
        }
        for item in self.list(vdir)? {
            let path = if vdir.is_empty() {
                item.name.clone()
            } else {
                format!("{vdir}/{}", item.name)
            };
            if !visit(&path, &item) {
                return Ok(false);
            }
            if item.is_dir {
                // A damaged archive or an unreadable folder must not stop the walk.
                if let Ok(false) = self.walk(&path, depth + 1, visit) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Finds files whose name contains `query` (case-insensitive), in real folders and inside
    /// every `.pak`, and returns their virtual paths. Stops after `limit` hits.
    ///
    /// # Errors
    ///
    /// Fails only when the root cannot be read; damaged archives are skipped.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<String>, SessionError> {
        self.search_in("", query, limit)
    }

    /// Like [`Session::search`], but only below the virtual folder `scope` ("" is everything).
    ///
    /// # Errors
    /// Fails when `scope` cannot be listed.
    pub fn search_in(
        &self,
        scope: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, SessionError> {
        let needle = query.trim().to_lowercase();
        let mut hits = Vec::new();
        if needle.is_empty() || limit == 0 {
            return Ok(hits);
        }
        self.search_dir(scope, &needle, limit, &mut hits, 0)?;
        Ok(hits)
    }

    fn search_dir(
        &self,
        vdir: &str,
        needle: &str,
        limit: usize,
        hits: &mut Vec<String>,
        depth: usize,
    ) -> Result<(), SessionError> {
        const MAX_DEPTH: usize = 32;
        if depth > MAX_DEPTH {
            return Ok(());
        }
        for item in self.list(vdir)? {
            if hits.len() >= limit {
                return Ok(());
            }
            let path = if vdir.is_empty() {
                item.name.clone()
            } else {
                format!("{vdir}/{}", item.name)
            };
            if !item.is_dir && item.name.to_lowercase().contains(needle) {
                hits.push(path.clone());
            }
            if item.is_dir {
                // A damaged archive or an unreadable folder must not stop the search.
                let _ = self.search_dir(&path, needle, limit, hits, depth + 1);
            }
        }
        Ok(())
    }

    /// Finds files whose text contains `query` (case-insensitive): plain text files, and the keys
    /// and texts inside `.loc` files. Looks into real folders and every `.pak`. Plain text files over
    /// 2 MiB and unreadable files are skipped. Stops after `limit` hits.
    ///
    /// # Errors
    ///
    /// Fails only when the root cannot be read.
    pub fn search_text(&self, query: &str, limit: usize) -> Result<Vec<TextHit>, SessionError> {
        self.search_text_in("", query, limit)
    }

    /// Like [`Session::search_text`], but only below the virtual folder `scope`.
    ///
    /// # Errors
    /// Fails when `scope` cannot be listed.
    pub fn search_text_in(
        &self,
        scope: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<TextHit>, SessionError> {
        let needle = query.trim().to_lowercase();
        let mut hits = Vec::new();
        if needle.is_empty() || limit == 0 {
            return Ok(hits);
        }
        self.walk(scope, 0, &mut |path, item| {
            if item.is_dir {
                return true;
            }
            let is_loc = path.to_ascii_lowercase().ends_with(".loc");
            if !is_loc && item.kind != FileKind::Text {
                return true;
            }
            if !is_loc && item.size.is_some_and(|n| n > MAX_SEARCHED_BYTES as u64) {
                return true;
            }
            let Ok(data) = self.read(path) else {
                return true;
            };
            let mut found: Vec<(Option<u32>, String)> = Vec::new();
            if is_loc {
                if let Ok(loc) = adv_core::loc::read_loc(std::io::Cursor::new(&data), &self.limits)
                {
                    for it in &loc.items {
                        let key = it.key.as_deref().unwrap_or("");
                        if let Some(x) = excerpt(&it.text, &needle) {
                            let line = if key.is_empty() {
                                x
                            } else {
                                format!("{key}: {x}")
                            };
                            found.push((Some(it.id), line));
                        } else if key.to_lowercase().contains(&needle) {
                            found.push((Some(it.id), key.to_string()));
                        }
                        if found.len() >= MAX_HITS_PER_FILE {
                            break;
                        }
                    }
                }
            } else if let Some(x) = excerpt(&adv_core::preview::decode_text(&data), &needle) {
                found.push((None, x));
            }
            for (id, snippet) in found {
                if hits.len() >= limit {
                    break;
                }
                hits.push(TextHit {
                    path: path.to_string(),
                    id,
                    snippet,
                });
            }
            hits.len() < limit
        })?;
        Ok(hits)
    }

    /// One page of the items of a `.loc` file whose text or key contains `query` (an empty
    /// query lists everything). The parsed file is kept for the next call.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a `.loc` file.
    pub fn loc_page(
        &self,
        vpath: &str,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<LocPage, SessionError> {
        let cached = self.loc_cache.lock().ok().and_then(|g| {
            g.as_ref()
                .filter(|(p, _)| p == vpath)
                .map(|(_, f)| Arc::clone(f))
        });
        let file = if let Some(f) = cached {
            f
        } else {
            let data = self.read(vpath)?;
            let f = adv_core::loc::read_loc(std::io::Cursor::new(&data), &self.limits)
                .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))?;
            let f = Arc::new(f);
            if let Ok(mut g) = self.loc_cache.lock() {
                *g = Some((vpath.to_string(), Arc::clone(&f)));
            }
            f
        };
        let needle = query.trim().to_lowercase();
        let limit = limit.min(500);
        let (matched, items) = if needle.is_empty() {
            let items = file
                .items
                .iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect();
            (file.items.len(), items)
        } else {
            let hit = |it: &&adv_core::loc::LocItem| {
                it.text.to_lowercase().contains(&needle)
                    || it
                        .key
                        .as_deref()
                        .is_some_and(|k| k.to_lowercase().contains(&needle))
            };
            let mut matched = 0;
            let mut items = Vec::new();
            for it in file.items.iter().filter(hit) {
                if matched >= offset && items.len() < limit {
                    items.push(it.clone());
                }
                matched += 1;
            }
            (matched, items)
        };
        Ok(LocPage {
            total: file.items.len(),
            matched,
            offset,
            items,
        })
    }

    /// Finds the `BaseLoc*.pak` that carries the resource databases (`Bin/pack.bin` and
    /// `Bin/Maps_*.bin`). Several language variants may exist; Russian is preferred, then
    /// English, then the first by name.
    fn base_pak(&self) -> Option<&Path> {
        self.base_pak
            .get_or_init(|| {
                let mut found = Vec::new();
                collect_base_paks(&self.root, 0, &mut found);
                found.sort_by_key(|p| {
                    let n = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    let rank: u8 = if n.contains("rus") {
                        0
                    } else if n.contains("eng") {
                        1
                    } else {
                        2
                    };
                    (rank, n)
                });
                found.into_iter().find(|p| {
                    self.with_pak(p, |o| {
                        Ok(o.pak
                            .entries()
                            .iter()
                            .any(|e| e.path.as_str().eq_ignore_ascii_case("bin/pack.bin")))
                    })
                    .unwrap_or(false)
                })
            })
            .as_deref()
    }

    /// Texture records of one database (`pack` or a map name), loaded once and cached.
    fn texture_db(&self, key: &str, entry: &str) -> Arc<TextureSizes> {
        if let Ok(map) = self.texdbs.lock()
            && let Some(db) = map.get(key)
        {
            return Arc::clone(db);
        }
        let index = self.load_texture_db(entry).unwrap_or_default();
        let index = Arc::new(index);
        if let Ok(mut map) = self.texdbs.lock() {
            map.insert(key.to_owned(), Arc::clone(&index));
        }
        index
    }

    /// A loose copy of a database (`Bin/pack.bin` unpacked next to the data), searched up to
    /// three levels below the opened folder.
    fn loose_db(&self, entry: &str) -> Option<Vec<u8>> {
        fn walk(dir: &Path, depth: u32, tail: &str) -> Option<PathBuf> {
            for e in std::fs::read_dir(dir).ok()?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if depth < 3
                        && let Some(f) = walk(&p, depth + 1, tail)
                    {
                        return Some(f);
                    }
                } else if p
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase()
                    .ends_with(tail)
                {
                    return Some(p);
                }
            }
            None
        }
        let tail = format!("/{}", entry.to_ascii_lowercase());
        let file = walk(&self.root, 0, &tail)?;
        let meta = std::fs::metadata(&file).ok()?;
        if meta.len() > self.limits.max_entry_size {
            return None;
        }
        std::fs::read(file).ok()
    }

    fn load_texture_db(&self, entry: &str) -> Option<TextureSizes> {
        let raw = match self.base_pak() {
            Some(base) => self
                .with_pak(base, |open| {
                    let e = open
                        .pak
                        .entries()
                        .iter()
                        .find(|e| e.path.as_str().eq_ignore_ascii_case(entry))
                        .cloned()
                        .ok_or_else(|| SessionError::NotFound(entry.to_owned()))?;
                    open.pak
                        .read(&e, Layer::Raw)
                        .map(|(d, _)| d)
                        .map_err(|e| SessionError::Failed(format!("{entry}: {e}")))
                })
                .ok(),
            None => None,
        };
        let raw = match raw {
            Some(r) => r,
            None => self.loose_db(entry)?,
        };
        // Look at the first bytes before inflating hundreds of megabytes.
        let layout = db::probe_layout(&raw);
        if layout == db::Layout::Unsupported {
            return None;
        }
        let data = db::inflate_db(raw.as_slice(), &self.limits).ok()?;
        drop(raw);
        if layout == db::Layout::Bits32 {
            let db = db32::parse(&data, &self.limits).ok()?;
            let layout = adv_core::binding::ElementLayout::for_family(
                adv_core::version::identify(&data)
                    .map_or(adv_core::version::Family::Other, |i| i.family()),
            );
            if entry.eq_ignore_ascii_case("Bin/pack.bin")
                && let Ok(fx) = adv_core::fixes::parse_fixes(&data, db.data.end, &self.limits)
                && let Ok(map) =
                    adv_core::binding::all_geometries_with(&layout, &db, &data, &fx, &self.limits)
                && let Ok(mut slot) = self.bindings.lock()
            {
                *slot = Some(Arc::new(map));
            }
            return Some(db.texture_sizes(&data));
        }
        let db = db::parse_db(&data, &self.limits).ok()?;
        Some(db.texture_sizes(&data))
    }

    /// Width and height of a texture from the resource database, when one is available.
    fn texture_db_size(&self, vpath: &str) -> Option<(u32, u32)> {
        let Target::Pak { inner, .. } = self.resolve(vpath).ok()? else {
            return None;
        };
        let lower = inner.to_ascii_lowercase();
        let stem = lower
            .strip_suffix(".(texture).bin")?
            .trim_end_matches(".hi");
        let key = format!("{stem}.(texture).xdb");
        let hit = |db: &TextureSizes| db.get(&key).copied().filter(|&(w, h)| w > 0 && h > 0);
        if let Some(size) = hit(&self.texture_db("pack", "Bin/pack.bin")) {
            return Some(size);
        }
        let map = key.strip_prefix("maps/")?.split('/').next()?;
        let entry = format!("Bin/Maps_{map}.bin");
        hit(&self.texture_db(&format!("map:{map}"), &entry))
    }

    /// The sharp-level sibling of a texture, when it exists: `X.(Texture).bin` has
    /// `X.hi.(Texture).bin`, and a plain `X.bin` has `X.hi.bin`.
    fn hi_sibling(&self, vpath: &str) -> Option<Vec<u8>> {
        let candidate = if let Some(stem) = vpath.strip_suffix(".(Texture).bin") {
            if stem.ends_with(".hi") {
                return None;
            }
            format!("{stem}.hi.(Texture).bin")
        } else {
            let stem = vpath.strip_suffix(".bin")?;
            if stem.ends_with(".hi") {
                return None;
            }
            format!("{stem}.hi.bin")
        };
        self.read(&candidate).ok()
    }

    /// Builds a preview of a file.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read; a file that cannot be shown in its kind still
    /// yields a hex dump.
    pub fn preview(&self, vpath: &str, opts: Options) -> Result<Preview, SessionError> {
        let data = self.read(vpath)?;
        let hi = self.hi_sibling(vpath);
        let name = vpath.rsplit('/').next().unwrap_or(vpath);
        if name.eq_ignore_ascii_case("game.version")
            && let Ok(v) = adv_core::gamever::parse(&data, &self.limits)
        {
            return Ok(Preview::Text {
                text: game_version_text(&v),
                truncated: false,
            });
        }
        let known = if name.to_ascii_lowercase().ends_with(".(texture).bin") {
            self.texture_db_size(vpath)
        } else {
            None
        };
        Ok(preview::preview_sized(
            name,
            &data,
            hi.as_deref(),
            opts,
            known,
            &self.limits,
        ))
    }

    /// Packs a geometry `.bin` for the 3D viewer (see [`adv_core::preview::mesh_bytes`]).
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a consistent geometry file.
    pub fn mesh(&self, vpath: &str) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        preview::mesh_bytes(&data, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// A text file or `.loc` as `txt` (decoded UTF-8; for `.loc` one `key<TAB>text` line per item),
    /// `html` (a small page with the text escaped) or `xml` (the text escaped, `.loc` as items).
    ///
    /// # Errors
    /// Fails when the file cannot be read, is a `.loc` that does not parse, or `format` is unknown.
    pub fn export_text(&self, vpath: &str, format: &str) -> Result<Vec<u8>, SessionError> {
        use std::fmt::Write;
        let data = self.read(vpath)?;
        let name = vpath.rsplit('/').next().unwrap_or(vpath);
        let items: Option<Vec<(String, u32, String)>> =
            if name.to_ascii_lowercase().ends_with(".loc") {
                let loc = adv_core::loc::read_loc(std::io::Cursor::new(&data), &self.limits)
                    .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))?;
                Some(
                    loc.items
                        .into_iter()
                        .map(|i| (i.key.unwrap_or_default(), i.id, i.text))
                        .collect(),
                )
            } else {
                None
            };
        let plain = || match &items {
            Some(list) => list.iter().fold(String::new(), |mut out, (k, id, t)| {
                let key = if k.is_empty() {
                    id.to_string()
                } else {
                    k.clone()
                };
                let _ = writeln!(out, "{key}\t{}", t.replace(['\r', '\n'], " "));
                out
            }),
            None => preview::decode_text(&data),
        };
        let text = match format {
            "txt" => plain(),
            "html" => format!(
                "<!doctype html>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<pre>{}</pre>\n",
                xml_escape(name),
                xml_escape(&plain())
            ),
            "xml" => {
                let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
                if let Some(list) = &items {
                    let _ = writeln!(out, "<loc name=\"{}\">", xml_escape(name));
                    for (k, id, t) in list {
                        let _ = writeln!(
                            out,
                            "  <item id=\"{id}\" key=\"{}\">{}</item>",
                            xml_escape(k),
                            xml_escape(t)
                        );
                    }
                    out.push_str("</loc>\n");
                } else {
                    let _ = writeln!(
                        out,
                        "<text name=\"{}\">{}</text>",
                        xml_escape(name),
                        xml_escape(&plain())
                    );
                }
                out
            }
            other => {
                return Err(SessionError::Failed(format!(
                    "unknown text format {other:?}"
                )));
            }
        };
        Ok(text.into_bytes())
    }

    /// The model as a binary glTF file.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a consistent geometry file.
    pub fn glb(&self, vpath: &str) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        preview::mesh_glb(&data, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// The model as a binary glTF file with one animation clip (`clip` is the virtual path of an
    /// `X.<Clip>.(SkeletalAnimation).bin`).
    ///
    /// # Errors
    ///
    /// Fails when a file cannot be read, is not consistent, or the clip does not fit the model.
    pub fn glb_animated(&self, vpath: &str, clip: &str) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        let clip_data = self.read(clip)?;
        let name = clip.rsplit('/').next().unwrap_or(clip);
        preview::mesh_glb_animated(&data, &clip_data, name, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{clip}: {e}")))
    }

    /// Element bindings of a model from the resource database (7.0 packs only) and the part of
    /// the model's path before the resource name (where its textures are looked up).
    fn bindings_of(
        &self,
        vpath: &str,
    ) -> Result<(Vec<adv_core::binding::ElementBinding>, String), SessionError> {
        let stem = vpath
            .strip_suffix(".(Geometry).bin")
            .or_else(|| vpath.strip_suffix(".(geometry).bin"))
            .ok_or_else(|| SessionError::NotFound(format!("{vpath}: not a Geometry file")))?;
        drop(self.texture_db("pack", "Bin/pack.bin"));
        let map = self.bindings.lock().ok().and_then(|g| g.clone()).ok_or_else(|| {
            SessionError::NotFound(
                "no resource database with model bindings: open the client folder that holds the resource database (Bin/pack.bin in BaseLoc*.pak or Bin.pak); client 7.0 and client 1.1 are supported"
                    .to_owned(),
            )
        })?;
        // The resource name is the longest tail of the path that the database knows; what comes
        // before it is the folder of the data (a pak, or the unpacked client root).
        let lower = stem.to_ascii_lowercase();
        let mut start = 0;
        loop {
            let key = format!("{}.(geometry).xdb", &lower[start..]);
            if let Some(list) = map.get(&key) {
                return Ok((list.clone(), stem[..start].to_owned()));
            }
            match lower[start..].find('/') {
                Some(n) => start += n + 1,
                None => {
                    return Err(SessionError::NotFound(format!(
                        "{vpath}: the model is not in the resource database"
                    )));
                }
            }
        }
    }

    /// The index of all texture files of the folder (see [`Session::tex_index`]).
    fn texture_index(&self) -> &TextureIndex {
        self.tex_index.get_or_init(|| {
            let mut files = Vec::new();
            match &self.only {
                Some(name) => files.push(self.root.join(name)),
                None => collect_all_paks(&self.root, 0, &mut files),
            }
            files.sort();
            let mut index = TextureIndex::new();
            for file in files {
                let entries: Vec<String> = self
                    .with_pak(&file, |o| {
                        Ok(o.pak
                            .entries()
                            .iter()
                            .map(|e| e.path.as_str().to_owned())
                            .filter(|p| p.to_ascii_lowercase().ends_with(".(texture).bin"))
                            .collect())
                    })
                    .unwrap_or_default();
                for real in entries {
                    index
                        .entry(real.to_ascii_lowercase())
                        .or_default()
                        .push((file.clone(), real));
                }
            }
            index
        })
    }

    /// Virtual path of the texture `stem` (the resource name without `.(texture).xdb`, any case).
    ///
    /// The archive of the model (`prefix` ends with its name) comes first, then the Russian and
    /// English base archives, then the rest by name. Lookups ignore case, as the names in the
    /// database are lower case while the archives keep the original spelling. Without a hit in
    /// any archive the name is tried as a loose file next to the model.
    fn texture_vpath(&self, prefix: &str, stem: &str) -> String {
        let want = format!("{stem}.(texture).bin").to_ascii_lowercase();
        let own = prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if let Some(list) = self.texture_index().get(&want) {
            let rank = |file: &Path| {
                let n = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let lang = if n.contains("rus") {
                    1u8
                } else if n.contains("eng") {
                    2
                } else {
                    3
                };
                (u8::from(n != own), lang, n)
            };
            if let Some((file, real)) = list.iter().min_by_key(|(f, _)| rank(f)) {
                let rel = file.strip_prefix(&self.root).unwrap_or(file);
                let rel: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                return format!("{}/{real}", rel.join("/"));
            }
        }
        format!("{prefix}{stem}.(Texture).bin")
    }

    /// The model as a binary glTF file with the textures of its elements and, optionally, one
    /// animation clip. Textures are looked up by name in all archives of the folder (the model's
    /// own archive first).
    ///
    /// # Errors
    ///
    /// Fails when the pack has no binding for the model (only 7.0 packs are supported), or a
    /// file cannot be read or is not consistent.
    pub fn glb_textured(
        &self,
        vpath: &str,
        clip: Option<&str>,
        texture: Option<&str>,
    ) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        let mut cache: HashMap<String, Option<Vec<u8>>> = HashMap::new();
        let mut parts = Vec::new();
        if let Some(tex) = texture {
            // A texture chosen by hand covers every element; the element ranges come from the
            // database when it knows the model, otherwise the whole mesh is one part.
            let png = match self.preview(tex, Options::default())? {
                Preview::Image { png, .. } if png.len() <= MAX_TEXTURE_PNG => png,
                _ => {
                    return Err(SessionError::Failed(format!(
                        "{tex}: not a texture picture"
                    )));
                }
            };
            let ranges: Vec<(u32, u32)> = match self.bindings_of(vpath) {
                Ok((elements, _)) => elements
                    .iter()
                    .map(|e| (e.index_begin, e.index_end))
                    .collect(),
                Err(_) => vec![(0, u32::MAX)],
            };
            for (a, z) in ranges {
                parts.push(adv_core::geometry::MeshPart {
                    index_begin: a,
                    index_end: z,
                    png: Some(png.clone()),
                });
            }
        } else {
            let (elements, prefix) = self.bindings_of(vpath)?;
            let prefix = prefix.as_str();
            for e in &elements {
                let png = e.diffuse.as_deref().and_then(|name| {
                    cache
                        .entry(name.to_owned())
                        .or_insert_with(|| {
                            let stem = name.strip_suffix(".(texture).xdb")?;
                            let path = self.texture_vpath(prefix, stem);
                            match self.preview(&path, Options::default()).ok()? {
                                Preview::Image { png, .. } if png.len() <= MAX_TEXTURE_PNG => {
                                    Some(png)
                                }
                                _ => None,
                            }
                        })
                        .clone()
                });
                parts.push(adv_core::geometry::MeshPart {
                    index_begin: e.index_begin,
                    index_end: e.index_end,
                    png,
                });
            }
        }
        let clip_data = match clip {
            Some(c) => Some((self.read(c)?, c.rsplit('/').next().unwrap_or(c).to_owned())),
            None => None,
        };
        preview::mesh_glb_textured(
            &data,
            &parts,
            clip_data.as_ref().map(|(d, n)| (d.as_slice(), n.as_str())),
            &self.limits,
        )
        .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// Animation clips that belong to a model: `.(SkeletalAnimation).bin` files in its folder and
    /// in an `Animations` folder next to it, as full virtual paths in name order.
    ///
    /// # Errors
    ///
    /// Fails when the path of the model is invalid.
    pub fn clips_of(&self, vpath: &str) -> Result<Vec<String>, SessionError> {
        let dir = vpath.rsplit_once('/').map_or("", |(d, _)| d);
        let join = |d: &str, n: &str| {
            if d.is_empty() {
                n.to_owned()
            } else {
                format!("{d}/{n}")
            }
        };
        let mut out = Vec::new();
        for d in [dir.to_owned(), join(dir, "Animations")] {
            if let Ok(items) = self.list(&d) {
                for it in items {
                    if !it.is_dir && it.kind == FileKind::Animation {
                        out.push(join(&d, &it.name));
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// The model as Wavefront OBJ text.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a consistent geometry file.
    pub fn obj(&self, vpath: &str) -> Result<String, SessionError> {
        let data = self.read(vpath)?;
        let name = vpath.rsplit('/').next().unwrap_or(vpath);
        preview::mesh_obj(&data, name, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// Opens a video for the player and returns its size, subsampling, rate and frame count.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a Theora video.
    pub fn video_open(&self, vpath: &str) -> Result<adv_core::video::VideoMeta, SessionError> {
        let data = self.read(vpath)?;
        let stream = adv_core::video::VideoStream::open(&data, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))?;
        let meta = stream.meta();
        if let Ok(mut g) = self.video.lock() {
            *g = Some((vpath.to_owned(), stream));
        }
        Ok(meta)
    }

    /// Decodes up to `count` frames (at most 64) starting at frame `start` of a video, as planar
    /// YCbCr one frame after another. Fewer frames come back at the end of the video. Playing
    /// forward is cheap; going back restarts the decoder from the first frame.
    ///
    /// # Errors
    ///
    /// Fails when the video cannot be opened or a frame cannot be decoded.
    pub fn video_frames(
        &self,
        vpath: &str,
        start: u64,
        count: usize,
    ) -> Result<Vec<u8>, SessionError> {
        let fail = |e: adv_core::Error| SessionError::Failed(format!("{vpath}: {e}"));
        let cached = self
            .video
            .lock()
            .map(|g| g.as_ref().is_some_and(|(p, _)| p == vpath))
            .unwrap_or(false);
        if !cached {
            self.video_open(vpath)?;
        }
        let mut guard = self
            .video
            .lock()
            .map_err(|_| SessionError::Failed("video state is unavailable".to_owned()))?;
        let Some((_, stream)) = guard.as_mut() else {
            return Err(SessionError::Failed(format!("{vpath}: video is not open")));
        };
        if start < stream.position() {
            stream.rewind().map_err(fail)?;
        }
        while stream.position() < start {
            if stream.next_frame().map_err(fail)?.is_none() {
                return Ok(Vec::new());
            }
        }
        let mut out = Vec::new();
        for _ in 0..count.min(64) {
            match stream.next_frame().map_err(fail)? {
                Some(f) => out.extend_from_slice(&f.into_bytes()),
                None => break,
            }
        }
        Ok(out)
    }

    /// The soundtrack of a video as WAV (empty when the video has no sound).
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or its sound is damaged.
    pub fn video_audio(&self, vpath: &str) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        preview::video_audio_wav(&data, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// Decodes stream `index` of a sound bank to MP3 (highest constant bitrate).
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read, the stream cannot be decoded or MP3 cannot carry it.
    pub fn audio_mp3(&self, vpath: &str, index: usize) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        preview::audio_mp3(&data, index, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }

    /// Decodes stream `index` of a sound bank to WAV.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or the stream cannot be decoded.
    pub fn audio_wav(&self, vpath: &str, index: usize) -> Result<Vec<u8>, SessionError> {
        let data = self.read(vpath)?;
        preview::audio_wav(&data, index, &self.limits)
            .map_err(|e| SessionError::Failed(format!("{vpath}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pak(path: &Path, files: &[(&str, &[u8])]) {
        let mut zw = zip::ZipWriter::new(File::create(path).expect("create"));
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (n, d) in files {
            zw.start_file(*n, o).expect("start");
            zw.write_all(d).expect("write");
        }
        zw.finish().expect("finish");
    }

    fn data_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::create_dir(dir.path().join("Packs")).expect("mkdir");
        pak(
            &dir.path().join("Packs/A.pak"),
            &[
                ("x/hello.txt", b"hi"),
                ("x/y/z.bin", b"\x01\x02"),
                ("top.txt", b"t"),
            ],
        );
        std::fs::write(dir.path().join("loose.txt"), b"loose").expect("write");
        dir
    }

    fn version_bytes(head: &[u8; 4], text: &str) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 0];
        v.extend_from_slice(head);
        v.extend_from_slice(&u32::try_from(text.len()).expect("len").to_le_bytes());
        v.extend_from_slice(text.as_bytes());
        v
    }

    fn stored_zlib(data: &[u8]) -> Vec<u8> {
        let len = u16::try_from(data.len()).expect("small");
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        let mut z = vec![0x78, 0x01, 0x01];
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend_from_slice(data);
        z.extend(((b << 16) | a).to_be_bytes());
        z
    }

    fn lp(s: &str) -> Vec<u8> {
        let mut v = u32::try_from(s.len()).expect("len").to_le_bytes().to_vec();
        v.extend(s.as_bytes());
        v
    }

    #[test]
    fn game_version_file_previews_as_a_listing() {
        let mut m = 1u32.to_le_bytes().to_vec();
        m.extend(20u32.to_le_bytes());
        m.extend(lp("/bin/"));
        m.extend(1u32.to_le_bytes());
        m.extend(lp("a.dll"));
        m.extend(7u32.to_le_bytes());
        m.extend([9u8; 32]);
        m.push(2);
        let z = stored_zlib(&m);
        let mut f = vec![0, 0, 0, 0];
        f.extend(b"ver5");
        f.extend(lp("1.2.3"));
        f.extend(u32::try_from(z.len()).expect("len").to_le_bytes());
        f.extend(z);
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("game.version"), &f).expect("write");
        let s = Session::open(dir.path()).expect("open");
        match s
            .preview("game.version", Options::default())
            .expect("preview")
        {
            Preview::Text { text, .. } => {
                assert!(text.starts_with("1.2.3 (ver5)\n"), "{text}");
                assert!(text.contains("/bin/\n  a.dll\t7"), "{text}");
            }
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn damaged_game_version_file_still_previews() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("game.version"), b"\0\0\0\0ver5").expect("write");
        let s = Session::open(dir.path()).expect("open");
        assert!(s.preview("game.version", Options::default()).is_ok());
    }

    #[test]
    fn game_version_is_read_from_the_profiles_folder() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(s.game_version(), None);
        std::fs::create_dir(dir.path().join("profiles")).expect("mkdir");
        std::fs::write(
            dir.path().join("profiles/Game.Version"),
            version_bytes(b"AOGV", "17.0.4.2.78"),
        )
        .expect("write");
        let v = s.game_version().expect("version");
        assert_eq!(
            (v.head.as_str(), v.version.as_str()),
            ("AOGV", "17.0.4.2.78")
        );
    }

    /// A layout 1 `.loc` payload (see `docs/formats/loc.md`).
    fn loc_v1(items: &[(&str, &str)]) -> Vec<u8> {
        let utf16 = |s: &str| {
            let mut v: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
            v.extend_from_slice(&[0, 0]);
            while !v.len().is_multiple_of(4) {
                v.push(0);
            }
            v
        };
        let count = items.len();
        let table_len = count * 12;
        let (mut keys, mut key_pos) = (Vec::new(), Vec::new());
        for (i, (k, _)) in items.iter().enumerate() {
            key_pos.push((table_len + keys.len() - i * 12, k.len() + 1));
            keys.extend_from_slice(k.as_bytes());
            keys.push(0);
        }
        while !keys.len().is_multiple_of(4) {
            keys.push(0);
        }
        let sizes_pos = table_len + keys.len();
        let (mut texts, mut sizes) = (Vec::new(), Vec::new());
        for (_, t) in items {
            sizes.push((t.encode_utf16().count(), texts.len()));
            texts.extend_from_slice(&utf16(t));
        }
        let mut out = Vec::new();
        for v in [0u32, sizes_pos as u32, 8, count as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for (i, (pos, size)) in key_pos.iter().enumerate() {
            for v in [*pos as u32, *size as u32, i as u32] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&keys);
        for (len, pos) in sizes {
            out.extend_from_slice(&(len as u32).to_le_bytes());
            out.extend_from_slice(&(pos as u32).to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 8]);
        out.extend_from_slice(&texts);
        out
    }

    #[test]
    fn loc_files_are_paged_filtered_and_searched() {
        let dir = data_dir();
        let items: Vec<(String, String)> = (0..12)
            .map(|i| (format!("Spells/S{i}.Name.txt"), format!("Spell number {i}")))
            .chain([(
                "Spells/Fire.Name.txt".to_string(),
                "Огненный шар".to_string(),
            )])
            .collect();
        let refs: Vec<(&str, &str)> = items
            .iter()
            .map(|(k, t)| (k.as_str(), t.as_str()))
            .collect();
        std::fs::write(dir.path().join("pack.loc"), loc_v1(&refs)).expect("write");
        let s = Session::open(dir.path()).expect("open");

        let p = s.loc_page("pack.loc", "", 0, 5).expect("page");
        assert_eq!((p.total, p.matched, p.items.len()), (13, 13, 5));
        let p = s.loc_page("pack.loc", "", 10, 5).expect("page");
        assert_eq!(
            p.items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![10, 11, 12]
        );
        let p = s.loc_page("pack.loc", "ШАР", 0, 50).expect("filter");
        assert_eq!((p.matched, p.items[0].id), (1, 12));
        let p = s.loc_page("pack.loc", "s1", 0, 50).expect("filter by key");
        assert_eq!(p.matched, 3, "S1, S10, S11: {:?}", p.items);
        assert!(s.loc_page("loose.txt", "", 0, 5).is_err());
        assert!(s.loc_page("missing.loc", "", 0, 5).is_err());

        let hits = s.search_text("огненный", 10).expect("search");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!((hits[0].path.as_str(), hits[0].id), ("pack.loc", Some(12)));
        let pv = preview::preview(
            "pack.loc",
            &std::fs::read(dir.path().join("pack.loc")).expect("read"),
            None,
            Options::default(),
            &Limits::default(),
        );
        assert_eq!(
            pv,
            Preview::Loc {
                count: 13,
                keys: true
            }
        );
    }

    #[test]
    fn text_search_looks_inside_plain_and_utf16_texts() {
        let dir = data_dir();
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(
            "Огненный ШАР\nдалеко"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        pak(
            &dir.path().join("Packs/B.pak"),
            &[("t/spell.txt", &utf16), ("t/other.bin", b"fireball")],
        );
        std::fs::write(dir.path().join("note.txt"), b"A Fireball hits.\nNext line").expect("write");
        let s = Session::open(dir.path()).expect("open");

        let hits = s.search_text("  fireball ", 10).expect("search");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].path, "note.txt");
        assert!(hits[0].snippet.contains("Fireball hits."));
        assert!(!hits[0].snippet.contains('\n'));

        let hits = s.search_text("шар", 10).expect("search");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].path.ends_with("t/spell.txt"));
        assert!(hits[0].snippet.contains("ШАР"));

        assert!(s.search_text("", 10).expect("empty").is_empty());
        assert!(s.search_text("zzz-none", 10).expect("none").is_empty());
        assert_eq!(s.search_text("l", 1).expect("limit").len(), 1);
    }

    #[test]
    fn excerpt_is_short_and_marks_cuts() {
        let long = format!("{}needle{}", "a".repeat(100), "b".repeat(200));
        let x = excerpt(&long, "needle").expect("hit");
        assert!(x.starts_with('…') && x.ends_with('…') && x.contains("needle"));
        assert!(x.chars().count() < 140);
        assert_eq!(
            excerpt("short needle", "needle").as_deref(),
            Some("short needle")
        );
        assert_eq!(excerpt("abc", "x"), None);
    }

    #[test]
    fn build_info_is_read_from_rev_text() {
        let rev = b"DATE = 04.06.2010\nVersion = 1.1.02.0\nURL: svn://x/y\nRevision: 168362\n\
                    Last Changed Date: 2010-06-03 22:54:08 +0400 (\xd2\xe1)\n";
        let i = parse_build_info(rev).expect("info");
        assert_eq!(i.version, "1.1.02.0");
        assert_eq!(i.revision, "168362");
        assert_eq!(i.date, "2010-06-03");
        assert_eq!(parse_build_info(b""), None);
        assert_eq!(parse_build_info(b"nothing here\n\xff\xfe"), None);
    }

    #[test]
    fn game_version_is_found_in_the_root_the_parent_and_its_profiles() {
        let bytes = version_bytes(b"ver5", "1.2.3");
        // client/{data/Packs, Profiles/game.version}: opening `data` still finds it.
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::create_dir_all(dir.path().join("client/data/Packs")).expect("mkdir");
        std::fs::create_dir_all(dir.path().join("client/Profiles")).expect("mkdir");
        std::fs::write(dir.path().join("client/Profiles/game.version"), &bytes).expect("write");
        let s = Session::open(&dir.path().join("client/data")).expect("open");
        assert_eq!(s.game_version().expect("parent profiles").version, "1.2.3");
        // Opening `Packs` two levels below the client root works as well.
        let s = Session::open(&dir.path().join("client/data/Packs")).expect("open");
        assert_eq!(
            s.game_version().expect("grandparent profiles").version,
            "1.2.3"
        );
        // Directly in the opened folder.
        let flat = tempfile::tempdir().expect("tmp");
        std::fs::write(flat.path().join("Game.Version"), &bytes).expect("write");
        let s = Session::open(flat.path()).expect("open");
        assert_eq!(s.game_version().expect("root").version, "1.2.3");
    }

    #[test]
    fn complete_path_lists_folders_and_paks_by_prefix() {
        let dir = tempfile::tempdir().expect("tmp");
        for d in ["Data", "data2", "Other"] {
            std::fs::create_dir(dir.path().join(d)).expect("mkdir");
        }
        std::fs::write(dir.path().join("Data.pak"), b"x").expect("write");
        std::fs::write(dir.path().join("data.txt"), b"x").expect("write");
        let base = format!("{}/", dir.path().display());
        let got = complete_path(&format!("{base}da"));
        assert_eq!(
            got,
            vec![
                format!("{base}Data/"),
                format!("{base}Data.pak"),
                format!("{base}data2/")
            ]
        );
        assert_eq!(complete_path(&base).len(), 4);
        assert!(complete_path(&format!("{base}zzz")).is_empty());
        assert!(complete_path("no separator").is_empty());
        assert!(complete_path("").is_empty());
        assert!(complete_path("/definitely/not/a/folder/x").is_empty());
    }

    #[test]
    fn damaged_game_version_is_none() {
        assert_eq!(parse_game_version(b""), None);
        assert_eq!(parse_game_version(&[0; 11]), None);
        let mut long = version_bytes(b"AOGV", "1.2");
        long[8] = 99; // claims more text than there is
        assert_eq!(parse_game_version(&long), None);
        let mut bad = version_bytes(b"AOGV", "ab");
        bad[12] = 0xff;
        assert_eq!(parse_game_version(&bad), None);
    }

    #[test]
    fn census_counts_files_by_kind() {
        let dir = data_dir();
        pak(
            &dir.path().join("Packs/B.pak"),
            &[
                ("m/a.(Geometry).bin", b"12345"),
                ("m/a.(Texture).bin", b"123"),
                ("s/music.fsb", b"1234"),
            ],
        );
        let s = Session::open(dir.path()).expect("open");
        let c = s.census().expect("census");
        assert_eq!((c.files, c.paks), (7, 2));
        assert_eq!(c.bytes, 2 + 2 + 1 + 5 + 5 + 3 + 4);
        let get = |k: FileKind| c.kinds.iter().find(|e| e.0 == k).map(|e| (e.1, e.2));
        assert_eq!(get(FileKind::Text), Some((3, 8)));
        assert_eq!(get(FileKind::Model), Some((1, 5)));
        assert_eq!(get(FileKind::Texture), Some((1, 3)));
        assert_eq!(get(FileKind::Audio), Some((1, 4)));
        assert_eq!(get(FileKind::Video), None);
    }

    #[test]
    fn find_kind_lists_paths_and_respects_the_limit() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        let all = s.find_kind(FileKind::Text, 10).expect("find");
        assert_eq!(all.len(), 3);
        assert!(all.contains(&"Packs/A.pak/x/hello.txt".to_string()));
        assert_eq!(s.find_kind(FileKind::Text, 2).expect("find").len(), 2);
        assert!(s.find_kind(FileKind::Video, 10).expect("find").is_empty());
    }

    #[test]
    fn folder_and_archive_look_like_one_tree() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        let root = s.list("").expect("root");
        assert_eq!(
            root.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["Packs", "loose.txt"]
        );
        let packs = s.list("Packs").expect("packs");
        assert!(packs[0].is_pak && packs[0].is_dir);
        let inside = s.list("Packs/A.pak").expect("inside");
        assert_eq!(
            inside.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["x", "top.txt"]
        );
        assert_eq!(inside[0].files, Some(2));
        let x = s.list("/Packs/A.pak/x/").expect("x");
        assert_eq!(x[0].name, "y");
        assert_eq!(x[1].kind, FileKind::Text);
    }

    #[test]
    fn files_are_read_and_previewed() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(s.read("Packs/A.pak/x/hello.txt").expect("read"), b"hi");
        assert_eq!(s.read("loose.txt").expect("read"), b"loose");
        match s
            .preview("Packs/A.pak/x/hello.txt", Options::default())
            .expect("preview")
        {
            Preview::Text { text, .. } => assert_eq!(text, "hi"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            s.preview("Packs/A.pak/x/y/z.bin", Options::default())
                .expect("preview"),
            Preview::Binary { len: 2, .. }
        ));
    }

    #[test]
    fn bad_paths_are_errors_not_panics() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        for bad in ["..", "Packs/../..", "C:/x", "a\\b", "Packs/./A.pak"] {
            assert!(
                matches!(s.list(bad), Err(SessionError::BadPath(_))),
                "{bad}"
            );
        }
        assert!(matches!(s.list("nope"), Err(SessionError::NotFound(_))));
        assert!(matches!(
            s.list("loose.txt"),
            Err(SessionError::WrongKind(_))
        ));
        assert!(matches!(s.read("Packs"), Err(SessionError::WrongKind(_))));
        assert!(matches!(
            s.read("Packs/A.pak/missing"),
            Err(SessionError::NotFound(_))
        ));
        assert!(matches!(
            s.list("Packs/A.pak/missing"),
            Err(SessionError::NotFound(_))
        ));
        assert!(s.audio_wav("loose.txt", 0).is_err());
    }

    #[test]
    fn export_helpers_fail_cleanly_on_other_files() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        assert!(s.obj("loose.txt").is_err());
        assert!(s.glb("loose.txt").is_err());
        assert!(s.mesh("loose.txt").is_err());
        assert!(s.obj("nope").is_err());
    }

    #[test]
    fn sharp_texture_sibling_is_found_by_the_game_naming() {
        let dir = tempfile::tempdir().expect("tmp");
        pak(
            &dir.path().join("T.pak"),
            &[
                ("a.(Texture).bin", b"lo"),
                ("a.hi.(Texture).bin", b"hi"),
                ("b.bin", b"x"),
            ],
        );
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(
            s.hi_sibling("T.pak/a.(Texture).bin").as_deref(),
            Some(&b"hi"[..])
        );
        assert!(s.hi_sibling("T.pak/a.hi.(Texture).bin").is_none());
        assert!(s.hi_sibling("T.pak/b.bin").is_none());
    }

    #[test]
    fn kinds_look_into_files() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        let k = s.kinds(&[
            "Packs/A.pak/x/hello.txt".into(),
            "Packs/A.pak/x/y/z.bin".into(),
            "missing".into(),
        ]);
        assert_eq!(k[0].1, FileKind::Text);
        assert_eq!(k[1].1, FileKind::Other);
        assert_eq!(k[2].1, FileKind::Other);
    }

    #[test]
    fn search_looks_into_folders_and_archives() {
        let dir = data_dir();
        pak(&dir.path().join("Packs/bad.pak"), &[]);
        std::fs::write(dir.path().join("Packs/broken.pak"), b"nope").expect("write");
        let s = Session::open(dir.path()).expect("open");
        let mut hits = s.search("HELLO", 10).expect("search");
        hits.sort();
        assert_eq!(hits, ["Packs/A.pak/x/hello.txt"]);
        let mut txt = s.search(".txt", 10).expect("search");
        txt.sort();
        assert_eq!(
            txt,
            [
                "Packs/A.pak/top.txt",
                "Packs/A.pak/x/hello.txt",
                "loose.txt"
            ]
        );
        assert_eq!(s.search(".txt", 1).expect("search").len(), 1);
        assert!(s.search("  ", 5).expect("search").is_empty());
        assert!(s.search("zzz", 5).expect("search").is_empty());
    }

    #[test]
    fn search_can_be_limited_to_a_folder() {
        let dir = data_dir();
        let s = Session::open(dir.path()).expect("open");
        let mut inside = s.search_in("Packs/A.pak", ".txt", 10).expect("search");
        inside.sort();
        assert_eq!(inside, ["Packs/A.pak/top.txt", "Packs/A.pak/x/hello.txt"]);
        assert_eq!(
            s.search_in("Packs/A.pak/x", ".txt", 10).expect("search"),
            ["Packs/A.pak/x/hello.txt"]
        );
        assert_eq!(s.search_in("", ".txt", 10).expect("search").len(), 3);
        assert!(s.search_in("Packs/nope", ".txt", 10).is_err());
        assert!(
            s.search_text_in("Packs/A.pak/x", "zzz", 10)
                .expect("text search")
                .is_empty()
        );
    }

    #[test]
    fn texts_and_loc_files_export_as_txt_html_and_xml() {
        let dir = data_dir();
        std::fs::write(dir.path().join("note.txt"), b"a < b & \"c\"\x01\n").expect("write");
        std::fs::write(
            dir.path().join("p.loc"),
            loc_v1(&[("Spells/Fire.Name.txt", "Fire <&> ball")]),
        )
        .expect("write");
        let s = Session::open(dir.path()).expect("open");
        let get = |p: &str, f: &str| {
            String::from_utf8(s.export_text(p, f).expect("export")).expect("utf8")
        };
        assert_eq!(get("note.txt", "txt"), "a < b & \"c\"\x01\n");
        let html = get("note.txt", "html");
        assert!(
            html.contains("<pre>a &lt; b &amp; &quot;c&quot;\n</pre>"),
            "{html}"
        );
        let xml = get("note.txt", "xml");
        assert!(
            xml.contains("<text name=\"note.txt\">a &lt; b &amp; &quot;c&quot;\n</text>"),
            "{xml}"
        );
        assert_eq!(get("p.loc", "txt"), "Spells/Fire.Name.txt\tFire <&> ball\n");
        let xml = get("p.loc", "xml");
        assert!(
            xml.contains("key=\"Spells/Fire.Name.txt\">Fire &lt;&amp;&gt; ball</item>"),
            "{xml}"
        );
        assert!(s.export_text("note.txt", "pdf").is_err());
        assert!(s.export_text("missing.txt", "txt").is_err());
        std::fs::write(dir.path().join("bad.loc"), b"junk").expect("write");
        let s = Session::open(dir.path()).expect("open");
        assert!(s.export_text("bad.loc", "txt").is_err());
    }

    #[test]
    fn zip_files_are_browsed_like_paks() {
        let dir = tempfile::tempdir().expect("tmp");
        pak(
            &dir.path().join("Mod.ZIP"),
            &[("docs/readme.txt", b"hello")],
        );
        let s = Session::open(dir.path()).expect("open");
        let root = s.list("").expect("root");
        assert!(root.iter().any(|i| i.name == "Mod.ZIP" && i.is_pak));
        let inner = s.list("Mod.ZIP/docs").expect("inside");
        assert_eq!(inner[0].name, "readme.txt");
        assert_eq!(s.read("Mod.ZIP/docs/readme.txt").expect("read"), b"hello");
        assert_eq!(
            complete_path(&format!("{}/Mo", dir.path().display())).len(),
            1
        );
        // A single zip can be opened directly, a plain text file cannot.
        assert!(Session::open(&dir.path().join("Mod.ZIP")).is_ok());
        std::fs::write(dir.path().join("a.txt"), b"x").expect("write");
        assert!(Session::open(&dir.path().join("a.txt")).is_err());
    }

    #[test]
    fn single_pak_and_damaged_pak() {
        let dir = data_dir();
        let one = Session::open(&dir.path().join("Packs/A.pak")).expect("open");
        let names: Vec<_> = one
            .list("A.pak")
            .expect("list")
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["x", "top.txt"]);
        let root: Vec<_> = one
            .list("")
            .expect("root")
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(root, ["A.pak"]);
        assert!(one.read("../loose.txt").is_err());

        let bad = dir.path().join("bad.pak");
        std::fs::write(&bad, b"not a zip").expect("write");
        assert!(Session::open(&bad).is_err());
        assert!(Session::open(&dir.path().join("loose.txt")).is_err());
        assert!(Session::open(&dir.path().join("missing")).is_err());
    }

    fn tar_file(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, data) in files {
            let mut h = [0u8; 512];
            h[..name.len()].copy_from_slice(name.as_bytes());
            h[124..135].copy_from_slice(format!("{:011o}", data.len()).as_bytes());
            h[156] = b'0';
            h[257..262].copy_from_slice(b"ustar");
            let sum: u32 = h
                .iter()
                .enumerate()
                .map(|(i, &b)| {
                    if (148..156).contains(&i) {
                        32
                    } else {
                        u32::from(b)
                    }
                })
                .sum();
            h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
            out.extend_from_slice(&h);
            out.extend_from_slice(data);
            out.resize(out.len().div_ceil(512) * 512, 0);
        }
        out.extend_from_slice(&[0u8; 1024]);
        out
    }

    #[test]
    fn textures_are_found_in_other_archives_and_in_any_letter_case() {
        let dir = tempfile::tempdir().expect("tmp");
        let root = dir.path();
        // The model lies in A.pak; the skin only in BaseLoceng.pak, spelled in mixed case.
        pak(&root.join("A.pak"), &[("Chars/Hero.(Geometry).bin", b"g")]);
        pak(
            &root.join("BaseLoceng.pak"),
            &[("Chars/HeroSkin.(Texture).bin", b"t")],
        );
        pak(
            &root.join("Zeta.pak"),
            &[
                ("Chars/HeroSkin.(Texture).bin", b"t"),
                ("Chars/Hat.(Texture).bin", b"t"),
            ],
        );
        let s = Session::open(root).expect("open");
        // Names from the database are lower case; the English base archive beats `Zeta.pak`.
        assert_eq!(
            s.texture_vpath("A.pak/", "chars/heroskin"),
            "BaseLoceng.pak/Chars/HeroSkin.(Texture).bin"
        );
        assert_eq!(
            s.texture_vpath("A.pak/", "chars/hat"),
            "Zeta.pak/Chars/Hat.(Texture).bin"
        );
        // A texture of the model's own archive comes first.
        pak(
            &root.join("A.pak"),
            &[
                ("Chars/Hero.(Geometry).bin", b"g"),
                ("Chars/HeroSkin.(Texture).bin", b"t"),
            ],
        );
        let s = Session::open(root).expect("open");
        assert_eq!(
            s.texture_vpath("A.pak/", "chars/heroskin"),
            "A.pak/Chars/HeroSkin.(Texture).bin"
        );
        // Unknown name: the old same-folder path is returned (a loose file may be there).
        assert_eq!(
            s.texture_vpath("A.pak/", "chars/none"),
            "A.pak/chars/none.(Texture).bin"
        );
    }

    #[test]
    fn texture_lookup_stays_inside_a_single_opened_archive() {
        let dir = tempfile::tempdir().expect("tmp");
        pak(&dir.path().join("A.pak"), &[("X/One.(Texture).bin", b"t")]);
        pak(&dir.path().join("B.pak"), &[("X/Two.(Texture).bin", b"t")]);
        let s = Session::open(&dir.path().join("A.pak")).expect("open");
        assert_eq!(
            s.texture_vpath("A.pak/", "x/one"),
            "A.pak/X/One.(Texture).bin"
        );
        assert_eq!(
            s.texture_vpath("A.pak/", "x/two"),
            "A.pak/x/two.(Texture).bin"
        );
    }

    #[test]
    fn textured_glb_needs_a_binding_from_the_database() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("Hero.(Geometry).bin"), b"x").expect("write");
        let s = Session::open(dir.path()).expect("open");
        assert!(matches!(
            s.glb_textured("Hero.(Geometry).bin", None, None),
            Err(SessionError::NotFound(_))
        ));
        assert!(
            s.glb_textured("Missing.(Geometry).bin", None, None)
                .is_err()
        );
    }

    #[test]
    fn clips_of_finds_the_clips_next_to_a_model() {
        let dir = tempfile::tempdir().expect("tmp");
        let m = dir.path().join("Hero");
        std::fs::create_dir_all(m.join("Animations")).expect("dirs");
        std::fs::write(m.join("Hero.(Geometry).bin"), b"x").expect("write");
        std::fs::write(m.join("Hero.Walk.(SkeletalAnimation).bin"), b"x").expect("write");
        std::fs::write(m.join("Animations/Hero.Idle.(SkeletalAnimation).bin"), b"x")
            .expect("write");
        std::fs::write(m.join("Animations/notes.txt"), b"x").expect("write");
        let s = Session::open(dir.path()).expect("open");
        let clips = s.clips_of("Hero/Hero.(Geometry).bin").expect("clips");
        assert_eq!(
            clips,
            [
                "Hero/Animations/Hero.Idle.(SkeletalAnimation).bin",
                "Hero/Hero.Walk.(SkeletalAnimation).bin"
            ]
        );
        assert!(
            s.glb_animated("Hero/Hero.(Geometry).bin", &clips[0])
                .is_err()
        );
        assert!(s.clips_of("Missing/x.bin").expect("none").is_empty());
    }

    #[test]
    fn tar_tgz_and_gz_are_browsed_like_folders() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tmp");
        let tar = tar_file(&[("sub/a.txt", b"hello"), ("b.txt", b"world")]);
        std::fs::write(dir.path().join("m.tar"), &tar).expect("write");
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(&tar).expect("gz");
        std::fs::write(dir.path().join("m.tgz"), e.finish().expect("finish")).expect("write");
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(b"plain").expect("gz");
        std::fs::write(dir.path().join("note.txt.gz"), e.finish().expect("finish")).expect("write");
        let s = Session::open(dir.path()).expect("open");
        let root = s.list("").expect("root");
        assert!(root.iter().all(|i| i.is_pak), "{root:?}");
        for archive in ["m.tar", "m.tgz"] {
            let names: Vec<_> = s
                .list(archive)
                .expect("list")
                .into_iter()
                .map(|i| i.name)
                .collect();
            assert_eq!(names, ["sub", "b.txt"], "{archive}");
            assert_eq!(
                s.read(&format!("{archive}/sub/a.txt")).expect("read"),
                b"hello"
            );
        }
        assert_eq!(s.read("note.txt.gz/note.txt").expect("read"), b"plain");
        assert!(Session::open(&dir.path().join("m.tgz")).is_ok());
    }

    #[test]
    fn base_pak_prefers_russian_then_english_and_skips_hires() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::create_dir(dir.path().join("Packs")).expect("mkdir");
        for n in [
            "BaseLocfra.pak",
            "BaseLoceng.pak",
            "BaseLocrus.pak",
            "BaseLocrus.HiRes.pak",
        ] {
            pak(&dir.path().join("Packs").join(n), &[("Bin/pack.bin", b"x")]);
        }
        let s = Session::open(dir.path()).expect("open");
        let picked = s.base_pak().expect("found");
        assert_eq!(picked.file_name().expect("name"), "BaseLocrus.pak");
        std::fs::remove_file(dir.path().join("Packs/BaseLocrus.pak")).expect("rm");
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(
            s.base_pak().expect("found").file_name().expect("name"),
            "BaseLoceng.pak"
        );
    }

    #[test]
    fn base_pak_needs_the_database_file_itself_not_just_a_bin_folder() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::create_dir(dir.path().join("Packs")).expect("mkdir");
        // A localisation pack with other files in Bin/ is not the database.
        pak(
            &dir.path().join("Packs/BaseLoceng.pak"),
            &[("Bin/other.txt", b"x")],
        );
        pak(&dir.path().join("Packs/Bin.pak"), &[("Bin/pack.bin", b"x")]);
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(
            s.base_pak().expect("found").file_name().expect("name"),
            "Bin.pak"
        );
    }

    #[test]
    fn base_pak_also_finds_bin_pak_and_skips_packs_without_a_database() {
        let dir = tempfile::tempdir().expect("tmp");
        pak(
            &dir.path().join("System.Mini.pak"),
            &[("Bin/pack.bin", b"x")],
        );
        pak(&dir.path().join("System.0.pak"), &[("Other/a.txt", b"x")]);
        pak(&dir.path().join("Bin.pak"), &[("Bin/pack.bin", b"x")]);
        let s = Session::open(dir.path()).expect("open");
        // `System.Mini.pak` is skipped, `Bin.pak` wins over `System.0.pak`, which has no `Bin/`.
        assert_eq!(
            s.base_pak().expect("found").file_name().expect("name"),
            "Bin.pak"
        );
    }

    #[test]
    fn texture_without_database_still_previews() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut tex = 0i32.to_le_bytes().to_vec();
        tex.extend(128i32.to_le_bytes());
        tex.extend([0u8; 128]);
        pak(&dir.path().join("T.pak"), &[("a.(Texture).bin", &tex)]);
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(s.texture_db_size("T.pak/a.(Texture).bin"), None);
        assert!(matches!(
            s.preview("T.pak/a.(Texture).bin", Options::default()),
            Ok(Preview::Image { .. })
        ));
    }

    #[test]
    fn damaged_database_is_ignored() {
        let dir = tempfile::tempdir().expect("tmp");
        pak(
            &dir.path().join("BaseLocrus.pak"),
            &[("Bin/pack.bin", b"not a database")],
        );
        pak(
            &dir.path().join("M.pak"),
            &[("Maps/X/a.(Texture).bin", &[0u8; 8])],
        );
        let s = Session::open(dir.path()).expect("open");
        assert_eq!(s.texture_db_size("M.pak/Maps/X/a.(Texture).bin"), None);
    }
}
