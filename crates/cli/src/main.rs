//! `adv` command line tool of Allods Data Viewer.

use adv_core::db::{Database, inflate_db, parse_db};
use adv_core::extract::{ExtractOptions, Outcome, extract_entry};
use adv_core::fsb::{parse_bank, unwrap_bank};
use adv_core::loc::{LocFile, LocItem, read_loc};
use adv_core::texture::{Dxt, Level, decode_dxt, level_dims, read_levels, write_dds, write_png};
use adv_core::xdb::TextureDesc;
use adv_core::{Entry, Layer, Limits, PakArchive};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Inspect and extract resources from game archives.
#[derive(Parser)]
#[command(name = "adv", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show summary information about a `.pak` archive.
    Info {
        /// Archive to inspect.
        pak: PathBuf,
        /// Print one JSON document.
        #[arg(long)]
        json: bool,
    },
    /// Show the directory tree of a `.pak` archive with file counts and guessed file kinds.
    Tree {
        /// Archive to show.
        pak: PathBuf,
        /// Start at this directory (use `/` between names).
        #[arg(long, default_value = "")]
        dir: String,
        /// How many directory levels to print.
        #[arg(long, default_value_t = 2)]
        depth: usize,
    },
    /// List the entries of a `.pak` archive.
    List {
        /// Archive to list.
        pak: PathBuf,
        /// Only entries matching this pattern (`*` matches any characters).
        #[arg(long)]
        filter: Option<String>,
        /// Show size, packed size, CRC32 and method.
        #[arg(long)]
        long: bool,
        /// Sort by `name` or `size`.
        #[arg(long, default_value = "name")]
        sort: String,
        /// Print one JSON document.
        #[arg(long)]
        json: bool,
    },
    /// Check the CRC32 of every entry in a `.pak` or in all `.pak` files of a folder.
    Verify {
        /// Archive or folder.
        path: PathBuf,
    },
    /// List the blobs (id, offset, size) of a geometry, animation or map chunk file.
    Blobs {
        /// File (zlib compressed or already inflated).
        file: PathBuf,
    },
    /// Read an animation clip (`X.(SkeletalAnimation).bin`): frame rate, frames, bone names and
    /// the material tracks (texture scrolling, transparency). `--json` for a document.
    Anim {
        /// Clip `.bin` file (zlib compressed or already inflated).
        file: PathBuf,
        /// Print a JSON document instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Read model geometry (`X.(Geometry).bin`): info and OBJ export.
    Geom {
        #[command(subcommand)]
        command: GeomCommand,
    },
    /// Inspect resource databases (`pack.bin` and similar, 64-bit V2 layout).
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    /// Inspect and export sound banks (`.fsb`, `.bsb`).
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    /// Inspect and convert texture `.bin` files.
    Tex {
        #[command(subcommand)]
        command: TexCommand,
    },
    /// Work with `.loc` localization files.
    Loc {
        #[command(subcommand)]
        command: LocCommand,
    },
    /// Search texts and keys in `.loc` files (a file, or a folder with `.loc` and `.pak` files).
    Search {
        /// File or folder to search.
        path: PathBuf,
        /// Text to look for (case-insensitive).
        query: String,
        /// Search keys instead of texts.
        #[arg(long)]
        keys: bool,
        /// Stop after this many matches.
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Extract entries from a `.pak` archive into a folder.
    Extract {
        /// Archive to read.
        pak: PathBuf,
        /// Folder to write into.
        #[arg(short, long)]
        output: PathBuf,
        /// Only entries matching this pattern (`*` matches any characters).
        #[arg(long)]
        filter: Option<String>,
        /// Keep the extra zlib layer instead of removing it.
        #[arg(long)]
        raw: bool,
        /// Replace files that already exist.
        #[arg(long)]
        overwrite: bool,
        /// Write all files into one folder, ignoring paths.
        #[arg(long)]
        flat: bool,
    },
}

#[derive(Subcommand)]
enum GeomCommand {
    /// Show vertex and triangle counts and the guessed vertex size.
    Info {
        /// Geometry `.bin` file (zlib compressed or already inflated).
        file: PathBuf,
        /// Vertex size in bytes instead of the guess (24, 28, 32 or 36).
        #[arg(long)]
        stride: Option<usize>,
    },
    /// Write the mesh as a binary glTF 2.0 file (`.glb`): positions, UVs, triangles.
    Glb {
        /// Geometry `.bin` file (zlib compressed or already inflated).
        file: PathBuf,
        /// Output `.glb` file.
        #[arg(short, long)]
        output: PathBuf,
        /// Vertex size in bytes instead of the guess (24, 28, 32 or 36).
        #[arg(long)]
        stride: Option<usize>,
        /// A `.(SkeletalAnimation).bin` clip of the same model to add as a glTF animation.
        #[arg(long)]
        anim: Option<PathBuf>,
    },
    /// Write the mesh as Wavefront OBJ (positions, UVs, triangles).
    Obj {
        /// Geometry `.bin` file (zlib compressed or already inflated).
        file: PathBuf,
        /// Output `.obj` file.
        #[arg(short, long)]
        output: PathBuf,
        /// Vertex size in bytes instead of the guess (24, 28, 32 or 36).
        #[arg(long)]
        stride: Option<usize>,
    },
}

#[derive(Subcommand)]
enum DbCommand {
    /// Show version, struct types and sizes of a database.
    Info {
        /// Database file (zlib compressed `.bin`).
        file: PathBuf,
    },
    /// List the `.pak` files the database refers to.
    Packs {
        /// Database file (zlib compressed `.bin`).
        file: PathBuf,
    },
    /// Find resource databases inside `.pak` files by their signature. Prints pak, entry, size,
    /// signature (hex) and the client the signature belongs to (`unknown` when it is not known).
    Find {
        /// Folder with `.pak` files (not recursive) or single `.pak` files.
        paths: Vec<PathBuf>,
        /// Copy the databases found to this folder, one subfolder per pak.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List the `Texture` objects (type, size, mips); `--json` for a document.
    Textures {
        /// Database file (zlib compressed `.bin`).
        file: PathBuf,
        /// Only named textures matching this pattern (`*` matches any characters).
        #[arg(long)]
        filter: Option<String>,
        /// Print one JSON document.
        #[arg(long)]
        json: bool,
    },
    /// Show the sounds of characters (`CharacterSoundVariation`) and the sound projects (32-bit layout).
    Sound {
        /// Database file (zlib compressed or already inflated `.bin`, 32-bit layout).
        file: PathBuf,
        /// Only records whose name matches this pattern (`*` matches any characters).
        #[arg(long)]
        filter: Option<String>,
    },
    /// List the named resources of a database with their database ids.
    List {
        /// Database file (zlib compressed `.bin`).
        file: PathBuf,
        /// Only names matching this pattern (`*` matches any characters).
        #[arg(long)]
        filter: Option<String>,
        /// Print one JSON document.
        #[arg(long)]
        json: bool,
    },
}

/// Output format of `adv audio export`.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum AudioFormat {
    Wav,
    Mp3,
}

#[derive(Subcommand)]
enum AudioCommand {
    /// List the streams of a bank: name, rate, channels, duration.
    List {
        /// A `.fsb` or `.bsb` file.
        file: PathBuf,
    },
    /// Decode all streams to WAV (built-in Vorbis decoder; `--vgmstream` uses an external tool).
    Export {
        /// A `.fsb` or `.bsb` file.
        file: PathBuf,
        /// Output folder.
        #[arg(short, long)]
        output: PathBuf,
        /// Use this `vgmstream-cli` instead of the built-in decoder (also `ADV_VGMSTREAM`).
        #[arg(long)]
        vgmstream: Option<PathBuf>,
        /// Output format: `wav` (lossless, default) or `mp3` (320 kbps, or 160 kbps for rates up to 24 kHz).
        #[arg(long, value_enum, default_value_t = AudioFormat::Wav)]
        format: AudioFormat,
    },
}

#[derive(Subcommand)]
enum TexCommand {
    /// Show the mip levels stored in a texture `.bin` (and its `.hi.bin`, if present).
    Info {
        /// Texture file, usually `Name.(Texture).bin`.
        file: PathBuf,
    },
    /// Convert a texture to PNG or DDS. Width, height and format are not stored in the
    /// file, so they must be given.
    Convert {
        /// Texture file, usually `Name.(Texture).bin`.
        file: PathBuf,
        /// Size of mip level 0, for example `512x512`. Default: read from the `.xdb` next to the file.
        #[arg(long)]
        size: Option<String>,
        /// Block format: 1, 3 or 5 (DXT1, DXT3, DXT5). Default: read from the `.xdb`.
        #[arg(long)]
        dxt: Option<u8>,
        /// Read size and format from this resource database (`pack.bin`) instead of an `.xdb`;
        /// the texture is found by file name, or by `--db-id`.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Database id of the texture inside `--db` (see `adv db textures`).
        #[arg(long, requires = "db")]
        db_id: Option<u64>,
        /// Community size list (`textures_database.bin`) used when the size has to be guessed.
        #[arg(long)]
        tdb: Option<PathBuf>,
        /// Pick another candidate size from the guess (see `adv tex info`), counting from 0.
        #[arg(long)]
        index: Option<usize>,
        /// Cut a PNG to the used picture (`realWidth` x `realHeight` from the `.xdb`).
        #[arg(long)]
        crop: bool,
        /// `png` (one level) or `dds` (all levels).
        #[arg(long, default_value = "png")]
        format: String,
        /// Mip level for PNG; default is the largest level available.
        #[arg(long)]
        mip: Option<i32>,
        /// Output file.
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum LocCommand {
    /// Print all items of a `.loc` file (plain or inside a `.pak` via `--entry`).
    Dump {
        /// A `.loc` file, or a `.pak` when `--entry` is given.
        file: PathBuf,
        /// Entry inside the `.pak`.
        #[arg(long)]
        entry: Option<String>,
        /// Output format: `csv`, `tsv` or `json`.
        #[arg(long, default_value = "csv")]
        format: String,
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Start a CSV/TSV file with a UTF-8 byte order mark (for Excel).
        #[arg(long)]
        bom: bool,
    },
}

/// Quotes a field for CSV (RFC 4180) or TSV (tabs and line breaks escaped).
fn field(value: &str, tsv: bool) -> String {
    if tsv {
        value
            .replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
    } else if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn render(file: &LocFile, format: &str, bom: bool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match format {
        "csv" | "tsv" => {
            let tsv = format == "tsv";
            let sep = if tsv { "\t" } else { "," };
            let eol = if tsv { "\n" } else { "\r\n" };
            if bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            out.extend_from_slice(format!("id{sep}key{sep}text{eol}").as_bytes());
            for i in &file.items {
                let key = i.key.as_deref().unwrap_or("");
                let line = format!(
                    "{}{sep}{}{sep}{}{eol}",
                    i.id,
                    field(key, tsv),
                    field(&i.text, tsv)
                );
                out.extend_from_slice(line.as_bytes());
            }
        }
        "json" => {
            let items: Vec<_> = file
                .items
                .iter()
                .map(|i| serde_json::json!({"id": i.id, "key": i.key, "text": i.text}))
                .collect();
            let doc = serde_json::json!({
                "schema": 1,
                "layout": format!("{:?}", file.layout),
                "items": items,
            });
            out = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
            out.push(b'\n');
        }
        other => return Err(format!("unknown format {other:?}, use csv, tsv or json")),
    }
    Ok(out)
}

/// Reads a texture and its sibling `.hi.bin` and returns levels sorted largest first.
fn load_texture(path: &Path) -> Result<Vec<Level>, String> {
    let limits = Limits::default();
    let read = |p: &Path| -> Result<Vec<Level>, String> {
        let f = File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
        read_levels(BufReader::new(f), &limits).map_err(|e| format!("{}: {e}", p.display()))
    };
    let mut levels = read(path)?;
    let name = path.to_string_lossy();
    if let Some(stem) = name.strip_suffix(".bin").filter(|s| !s.ends_with(".hi")) {
        let hi = PathBuf::from(format!("{stem}.hi.bin"));
        if hi.is_file() {
            levels.extend(read(&hi)?);
        }
    }
    levels.sort_by_key(|l| l.id);
    levels.dedup_by_key(|l| l.id);
    Ok(levels)
}

/// Reads the `.xdb` that belongs to a texture `.bin` (same name, `.hi` removed).
fn load_desc(bin: &Path) -> Result<TextureDesc, String> {
    let name = bin.to_string_lossy();
    let stem = name
        .strip_suffix(".bin")
        .ok_or("the texture file must end with .bin to find its .xdb; give --size and --dxt")?;
    let stem = stem.strip_suffix(".hi").unwrap_or(stem);
    let xdb = PathBuf::from(format!("{stem}.xdb"));
    let text = std::fs::read_to_string(&xdb)
        .map_err(|e| format!("{}: {e}. Give --size and --dxt instead", xdb.display()))?;
    TextureDesc::parse(&text).map_err(|e| format!("{}: {e}", xdb.display()))
}

/// Guesses a description from the data alone (and the optional community list).
fn guess_desc(
    file: &Path,
    levels: &[Level],
    tdb: Option<&Path>,
    index: Option<usize>,
    dxt: Option<u8>,
) -> Option<TextureDesc> {
    let forced = match dxt {
        Some(1) => Some(Dxt::Dxt1),
        Some(3) => Some(Dxt::Dxt3),
        Some(5) => Some(Dxt::Dxt5),
        _ => None,
    };
    let entry = tdb.and_then(|p| {
        let f = File::open(p).ok()?;
        let raw = inflate_db(BufReader::new(f), &Limits::default()).ok()?;
        adv_core::tdb::Tdb::parse(&raw)
            .ok()?
            .find(&file.to_string_lossy())
    });
    let mut g = adv_core::texheur::guess(levels, forced.or(entry.and_then(|e| e.format)))?;
    if let Some(i) = index.or(entry.map(|e| usize::from(e.index))) {
        g.index = i.min(g.candidates.len() - 1);
    }
    let (w, h) = g.size();
    let (pw, ph) = entry.map_or((0, 0), |e| (u32::from(e.pad_w), u32::from(e.pad_h)));
    eprintln!(
        "note: no .xdb or database: size {w}x{h} {:?} is a guess{} (other candidates: {})",
        g.format,
        if entry.is_some() {
            " from the community list"
        } else {
            ""
        },
        g.candidates
            .iter()
            .map(|(a, b)| format!("{a}x{b}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Some(TextureDesc {
        width: w,
        height: h,
        real_width: w.saturating_sub(pw).max(1),
        real_height: h.saturating_sub(ph).max(1),
        format: g.format,
        mips: levels.len() as u32,
    })
}

/// Builds a texture description from a resource database: by id, or by the file name.
fn desc_from_db(db_path: &Path, bin: &Path, id: Option<u64>) -> Result<TextureDesc, String> {
    let (payload, db) = load_db(db_path)?;
    let types = db.type_tags(&payload);
    let id = match id {
        Some(id) => id,
        None => {
            let base = bin
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let base = base.strip_suffix(".bin").unwrap_or(&base);
            let base = base.strip_suffix(".hi").unwrap_or(base);
            let base = base.strip_suffix(".(Texture)").unwrap_or(base);
            let hits: Vec<u64> = db
                .files
                .iter()
                .filter(|f| types.get(&f.db_id) == Some(&"Texture"))
                .filter(|f| {
                    let n = f.name.rsplit('/').next().unwrap_or(&f.name);
                    n.strip_suffix(".(Texture).xdb") == Some(base)
                })
                .map(|f| f.db_id)
                .collect();
            match hits.as_slice() {
                [one] => *one,
                [] => {
                    return Err(format!(
                        "no texture named {base:?} in {}",
                        db_path.display()
                    ));
                }
                _ => return Err(format!("several textures named {base:?}; give --db-id")),
            }
        }
    };
    if types.get(&id) != Some(&"Texture") {
        return Err(format!("object {id} is not a Texture"));
    }
    let r = db
        .texture_record(&payload, id)
        .ok_or("texture object is outside the data image")?;
    let format = match r.kind {
        0 => Dxt::Dxt1,
        1 => Dxt::Dxt3,
        2 => Dxt::Dxt5,
        k => return Err(format!("texture type {k} is not a DXT format")),
    };
    Ok(TextureDesc {
        width: r.width,
        height: r.height,
        real_width: r.width,
        real_height: r.height,
        format,
        mips: r.mips,
    })
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .to_ascii_lowercase()
        .split_once('x')
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .ok_or("size must look like 512x512")?;
    let w: u32 = w.parse().map_err(|_| "bad width")?;
    let h: u32 = h.parse().map_err(|_| "bad height")?;
    Ok((w, h))
}

fn load_db(path: &Path) -> Result<(Vec<u8>, Database), String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let limits = Limits::default();
    let payload = inflate_db(BufReader::new(file), &limits)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let db = parse_db(&payload, &limits).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((payload, db))
}

fn unwrap_bank_file(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    unwrap_bank(BufReader::new(file), &Limits::default())
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn load_bank(path: &Path) -> Result<adv_core::fsb::Bank, String> {
    let bytes = unwrap_bank_file(path)?;
    parse_bank(&bytes, &Limits::default()).map_err(|e| format!("{}: {e}", path.display()))
}

fn load_loc(path: &Path, entry: Option<&str>) -> Result<LocFile, String> {
    let limits = Limits::default();
    let err = |e: adv_core::Error| format!("{}: {e}", path.display());
    if let Some(name) = entry {
        let mut pak = open(path)?;
        let e = pak
            .entries()
            .iter()
            .find(|e| e.path.as_str() == name)
            .cloned()
            .ok_or_else(|| format!("no entry {name:?} in {}", path.display()))?;
        let (data, _) = pak.read(&e, Layer::Decoded).map_err(err)?;
        return adv_core::loc::parse_loc(&data, &limits).map_err(err);
    }
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_loc(BufReader::new(file), &limits).map_err(err)
}

fn search_items(label: &str, items: &[LocItem], q: &str, keys: bool, left: &mut usize) {
    for i in items {
        if *left == 0 {
            return;
        }
        let hay = if keys {
            i.key.as_deref().unwrap_or("")
        } else {
            i.text.as_str()
        };
        if hay.to_lowercase().contains(q) {
            let snippet: String = i
                .text
                .chars()
                .take(120)
                .collect::<String>()
                .replace(['\r', '\n'], " ");
            println!(
                "{label}\t{}\t{}\t{snippet}",
                i.id,
                i.key.as_deref().unwrap_or("-")
            );
            *left -= 1;
        }
    }
}

fn collect_files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        names.sort();
        for p in names {
            if !p.is_symlink() {
                collect_files(&p, out)?;
            }
        }
    } else {
        out.push(path.to_path_buf());
    }
    Ok(())
}

/// Matches `text` against `pattern`, where `*` stands for any run of characters.
/// Comparison ignores ASCII case.
fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn open(path: &Path) -> Result<PakArchive<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    PakArchive::open(BufReader::new(file), Limits::default())
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn selected<'a>(pak: &'a PakArchive<BufReader<File>>, filter: Option<&str>) -> Vec<&'a Entry> {
    pak.entries()
        .iter()
        .filter(|e| filter.is_none_or(|f| wildcard(f, e.path.as_str())))
        .collect()
}

fn print_tree(node: &adv_core::browse::Node, depth: usize, indent: usize) {
    let pad = "  ".repeat(indent);
    for (name, child) in &node.dirs {
        println!("{pad}{name}/  ({} files)", child.total_files());
        if depth > 1 {
            print_tree(child, depth - 1, indent + 1);
        }
    }
    for name in node.files.keys() {
        println!(
            "{pad}{name}  [{}]",
            adv_core::browse::classify(name, &[]).name()
        );
    }
}

fn warn_skipped(pak: &PakArchive<BufReader<File>>) {
    for s in pak.skipped() {
        eprintln!("skipped {:?}: {}", s.name, s.reason);
    }
}

/// Runs a subcommand and returns the process exit code.
fn run(command: Command) -> Result<u8, String> {
    match command {
        Command::Info { pak: path, json } => {
            let pak = open(&path)?;
            let total: u64 = pak.entries().iter().map(|e| e.size).sum();
            if json {
                let doc = serde_json::json!({
                    "schema": 1,
                    "file": path.display().to_string(),
                    "entries": pak.entries().len(),
                    "total_size": total,
                    "skipped": pak.skipped().iter().map(|s| serde_json::json!({"name": s.name, "reason": s.reason})).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
                );
                return Ok(u8::from(!pak.skipped().is_empty()) * 2);
            }
            println!("File: {}", path.display());
            println!("Entries: {}", pak.entries().len());
            println!("Total size: {total} bytes");
            println!("Skipped: {}", pak.skipped().len());
            warn_skipped(&pak);
            Ok(0)
        }
        Command::Tree {
            pak: path,
            dir,
            depth,
        } => {
            let pak = open(&path)?;
            let tree = adv_core::browse::build_tree(pak.entries());
            let node = tree
                .dir(&dir)
                .ok_or_else(|| format!("no such directory in the archive: {dir:?}"))?;
            print_tree(node, depth, 0);
            warn_skipped(&pak);
            Ok(0)
        }
        Command::List {
            pak: path,
            filter,
            long,
            sort,
            json,
        } => {
            let pak = open(&path)?;
            let mut list = selected(&pak, filter.as_deref());
            match sort.as_str() {
                "name" => list.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str())),
                "size" => list.sort_by_key(|e| std::cmp::Reverse(e.size)),
                other => return Err(format!("unknown sort order {other:?}, use name or size")),
            }
            if json {
                let rows: Vec<_> = list
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "path": e.path.as_str(),
                            "size": e.size,
                            "packed": e.packed_size,
                            "crc32": format!("{:08X}", e.crc32),
                            "method": format!("{:?}", e.method),
                        })
                    })
                    .collect();
                let doc = serde_json::json!({"schema": 1, "pack": path.display().to_string(), "entries": rows});
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
                );
                return Ok(0);
            }
            for e in list {
                if long {
                    println!(
                        "{:>12} {:>12} {:08X} {:?} {}",
                        e.size, e.packed_size, e.crc32, e.method, e.path
                    );
                } else {
                    println!("{}", e.path);
                }
            }
            warn_skipped(&pak);
            Ok(0)
        }
        Command::Db {
            command: DbCommand::Info { file },
        } => {
            let (payload, db) = load_db(&file)?;
            let hex: String = db.version.iter().map(|b| format!("{b:02X}")).collect();
            println!("Version buffer: {hex}");
            println!("Inflated size: {} bytes", payload.len());
            println!("Named resources: {}", db.files.len());
            println!("Data image: {} bytes", db.data.len());
            println!("Pointer fixes: {}", db.fix_count);
            println!("Referenced paks: {}", db.packs.len());
            println!("Struct types ({}):", db.structs.len());
            for s in &db.structs {
                println!("  {s}");
            }
            Ok(0)
        }
        Command::Db {
            command: DbCommand::Sound { file, filter },
        } => {
            let limits = Limits::default();
            let raw = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let payload = inflate_db(raw.as_slice(), &limits).unwrap_or(raw);
            let err = |e: adv_core::Error| format!("{}: {e}", file.display());
            let db = adv_core::db32::parse(&payload, &limits).map_err(err)?;
            let fixes =
                adv_core::fixes::parse_fixes(&payload, db.data.end, &limits).map_err(err)?;
            let pick = |n: &str| filter.as_deref().is_none_or(|p| wildcard(p, n));
            for (name, p) in adv_core::soundbind::all_projects(&db, &payload, &fixes, &limits)
                .map_err(err)?
                .iter()
                .filter(|(n, _)| pick(n))
            {
                println!("{name}\n  events: {}", p.events_file);
                for b in &p.banks {
                    println!("  bank: {b}");
                }
            }
            for (name, c) in adv_core::soundbind::all_characters(&db, &payload, &fixes, &limits)
                .map_err(err)?
                .iter()
                .filter(|(n, _)| pick(n))
            {
                println!("{name}\n  project: {}", c.project.as_deref().unwrap_or("-"));
                for e in &c.events {
                    println!(
                        "  {} [{}] {}",
                        e.event,
                        e.kind,
                        e.project.as_deref().unwrap_or("-")
                    );
                }
            }
            Ok(0)
        }
        Command::Db {
            command: DbCommand::Packs { file },
        } => {
            let (_, db) = load_db(&file)?;
            for p in &db.packs {
                println!("{p}");
            }
            Ok(0)
        }
        Command::Db {
            command: DbCommand::List { file, filter, json },
        } => {
            let (payload, db) = load_db(&file)?;
            let types = db.type_tags(&payload);
            let rows: Vec<_> = db
                .files
                .iter()
                .filter(|f| filter.as_deref().is_none_or(|p| wildcard(p, &f.name)))
                .collect();
            if json {
                let items: Vec<_> = rows
                    .iter()
                    .map(|f| serde_json::json!({"db_id": f.db_id, "struct": types.get(&f.db_id), "name": f.name}))
                    .collect();
                let doc = serde_json::json!({"schema": 1, "resources": items});
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
                );
            } else {
                for f in rows {
                    println!(
                        "{:>12}  {:<24} {}",
                        f.db_id,
                        types.get(&f.db_id).copied().unwrap_or("?"),
                        f.name
                    );
                }
            }
            Ok(0)
        }
        Command::Blobs { file } => {
            let raw = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let data = adv_core::geometry::inflate(&raw, &Limits::default())
                .map_err(|e| format!("{}: {e}", file.display()))?;
            let blobs = adv_core::blob::parse_blobs(&data)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            println!("id\toffset\tsize");
            for b in blobs {
                println!("{}\t{}\t{}", b.id, b.offset, b.size);
            }
            Ok(0)
        }
        Command::Anim { file, json } => {
            let raw = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let data = adv_core::geometry::inflate(&raw, &Limits::default())
                .map_err(|e| format!("{}: {e}", file.display()))?;
            let fail = |e: adv_core::error::Error| format!("{}: {e}", file.display());
            let info = adv_core::animation::parse(&data).map_err(fail)?;
            let names = adv_core::animation::bone_names(&data).map_err(fail)?;
            let materials = adv_core::animation::material_tracks(&data).map_err(fail)?;
            if json {
                let doc = serde_json::json!({
                    "fps": info.fps,
                    "frames": info.frames,
                    "seconds": info.seconds(),
                    "bones": names,
                    "materials": materials.iter().map(|m| serde_json::json!({
                        "name": m.name,
                        "transparency": m.has_transparency,
                        "translateU": m.has_translate_u,
                        "translateV": m.has_translate_v,
                    })).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
                );
            } else {
                println!("Frame rate: {} per second", info.fps);
                println!("Frames: {} ({:.2} s)", info.frames, info.seconds());
                println!("Bones: {}", names.len());
                for (i, n) in names.iter().enumerate() {
                    println!("  {i:3} {n}");
                }
                println!("Material tracks: {}", materials.len());
                for m in &materials {
                    let mut ch = Vec::new();
                    if m.has_transparency {
                        ch.push("transparency");
                    }
                    if m.has_translate_u {
                        ch.push("U shift");
                    }
                    if m.has_translate_v {
                        ch.push("V shift");
                    }
                    println!("  {}  ({})", m.name, ch.join(", "));
                }
            }
            Ok(0)
        }
        Command::Geom { command } => {
            let (file, stride) = match &command {
                GeomCommand::Info { file, stride }
                | GeomCommand::Obj { file, stride, .. }
                | GeomCommand::Glb { file, stride, .. } => (file.clone(), *stride),
            };
            let raw = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let data = adv_core::geometry::inflate(&raw, &Limits::default())
                .map_err(|e| format!("{}: {e}", file.display()))?;
            let g = adv_core::geometry::parse(&data, stride)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            match command {
                GeomCommand::Info { .. } => {
                    println!(
                        "Vertices: {} (stride {} bytes)\nTriangles: {}\nBlock after the indices: {} bytes",
                        g.vertex_data.len() / g.stride,
                        g.stride,
                        g.indices.len() / 3,
                        g.tail_block
                    );
                    if let Ok(sk) = g.skeleton() {
                        println!("Bones: {}", sk.bones.len());
                        for (i, b) in sk.bones.iter().enumerate() {
                            let parent = b
                                .parent
                                .map_or_else(|| "-".to_owned(), |p| sk.bones[p].name.clone());
                            println!("  {i:3} {}  (parent: {parent})", b.name);
                        }
                    }
                    if let Some(skin) = g.skin() {
                        let used = skin.iter().filter(|s| s.bones[0].is_some()).count();
                        println!("Vertices with bone weights: {used} of {}", skin.len());
                    }
                }
                GeomCommand::Glb { output, anim, .. } => {
                    let glb = if let Some(a) = anim {
                        let raw = std::fs::read(&a).map_err(|e| format!("{}: {e}", a.display()))?;
                        let data = adv_core::geometry::inflate(&raw, &Limits::default())
                            .map_err(|e| format!("{}: {e}", a.display()))?;
                        let clip = adv_core::animation::decode(&data)
                            .map_err(|e| format!("{}: {e}", a.display()))?;
                        let name = a.file_name().map_or_else(
                            || "clip".to_owned(),
                            |n| n.to_string_lossy().into_owned(),
                        );
                        g.to_glb_animated(&clip, &name)
                    } else {
                        g.to_glb()
                    }
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                    std::fs::write(&output, glb)
                        .map_err(|e| format!("{}: {e}", output.display()))?;
                    println!("Wrote {}", output.display());
                }
                GeomCommand::Obj { output, .. } => {
                    let name = file
                        .file_name()
                        .map_or_else(|| "model".to_owned(), |n| n.to_string_lossy().into_owned());
                    std::fs::write(&output, g.to_obj(&name))
                        .map_err(|e| format!("{}: {e}", output.display()))?;
                    println!("Wrote {}", output.display());
                }
            }
            Ok(0)
        }
        Command::Db {
            command: DbCommand::Find { paths, output },
        } => {
            let mut paks: Vec<PathBuf> = Vec::new();
            for p in &paths {
                if p.is_dir() {
                    let rd = std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()))?;
                    paks.extend(
                        rd.filter_map(Result::ok).map(|e| e.path()).filter(|f| {
                            f.extension().is_some_and(|x| x.eq_ignore_ascii_case("pak"))
                        }),
                    );
                } else {
                    paks.push(p.clone());
                }
            }
            paks.sort();
            let limits = Limits::default();
            let mut found = 0usize;
            for pak in &paks {
                let Ok(f) = File::open(pak) else {
                    eprintln!("warning: cannot open {}", pak.display());
                    continue;
                };
                let Ok(mut arc) = PakArchive::open(BufReader::new(f), limits) else {
                    eprintln!("warning: not a readable archive: {}", pak.display());
                    continue;
                };
                let entries = arc.entries().to_vec();
                for e in entries {
                    let name = e.path.to_string();
                    let leaf = name.rsplit('/').next().unwrap_or(&name);
                    if !leaf.to_lowercase().ends_with(".bin") || leaf.contains('(') {
                        continue;
                    }
                    let Ok((data, _)) = arc.read(&e, Layer::Raw) else {
                        continue;
                    };
                    let Some(ident) = adv_core::version::identify(&data) else {
                        continue;
                    };
                    found += 1;
                    let hex = ident.hex();
                    let client = ident.client();
                    let pak_name = pak
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    println!("{pak_name}\t{name}\t{}\t{hex}\t{client}", e.size);
                    if let Some(out) = &output {
                        let stem = pak
                            .file_stem()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let dir = out.join(&stem);
                        std::fs::create_dir_all(&dir)
                            .map_err(|er| format!("{}: {er}", dir.display()))?;
                        let target = dir.join(name.replace('/', "_"));
                        std::fs::write(&target, &data)
                            .map_err(|er| format!("{}: {er}", target.display()))?;
                    }
                }
            }
            eprintln!("{found} database(s) in {} pak file(s)", paks.len());
            Ok(0)
        }
        Command::Db {
            command: DbCommand::Textures { file, filter, json },
        } => {
            let (payload, db) = load_db(&file)?;
            let types = db.type_tags(&payload);
            let names: std::collections::HashMap<u64, &str> = db
                .files
                .iter()
                .map(|f| (f.db_id, f.name.as_str()))
                .collect();
            let mut rows: Vec<(adv_core::db::TextureRecord, Option<&str>)> = types
                .iter()
                .filter(|(_, s)| **s == "Texture")
                .filter_map(|(id, _)| db.texture_record(&payload, *id))
                .map(|r| (r, names.get(&r.db_id).copied()))
                .filter(|(_, n)| match (&filter, n) {
                    (None, _) => true,
                    (Some(p), Some(n)) => wildcard(p, n),
                    (Some(_), None) => false,
                })
                .collect();
            rows.sort_by_key(|(r, _)| r.db_id);
            if json {
                let items: Vec<_> = rows
                    .iter()
                    .map(|(r, n)| {
                        serde_json::json!({"db_id": r.db_id, "name": n, "type": r.kind,
                            "width": r.width, "height": r.height, "mips": r.mips})
                    })
                    .collect();
                let doc = serde_json::json!({"schema": 1, "textures": items});
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
                );
            } else {
                for (r, n) in rows {
                    println!(
                        "{:>12}  {:>5}x{:<5} type {} mips {}  {}",
                        r.db_id,
                        r.width,
                        r.height,
                        r.kind,
                        r.mips,
                        n.unwrap_or("-")
                    );
                }
            }
            Ok(0)
        }
        Command::Audio {
            command: AudioCommand::List { file },
        } => {
            let bank = load_bank(&file)?;
            println!(
                "Streams: {} ({} version {}, codec mode {})",
                bank.streams.len(),
                if bank.fsb4 { "FSB4" } else { "FSB5" },
                bank.version,
                bank.mode
            );
            for (i, s) in bank.streams.iter().enumerate() {
                let rate = s.frequency.map_or("?".to_owned(), |f| f.to_string());
                let secs = s.seconds().map_or("?".to_owned(), |v| format!("{v:.2}"));
                println!(
                    "{:>4}  {rate:>6} Hz  {}ch  {secs:>8} s  {}",
                    i + 1,
                    s.channels,
                    s.name
                );
            }
            Ok(0)
        }
        Command::Audio {
            command:
                AudioCommand::Export {
                    file,
                    output,
                    vgmstream,
                    format,
                },
        } => {
            let bytes = unwrap_bank_file(&file)?;
            std::fs::create_dir_all(&output).map_err(|e| format!("{}: {e}", output.display()))?;
            let external =
                vgmstream.or_else(|| std::env::var_os("ADV_VGMSTREAM").map(PathBuf::from));
            if external.is_some() && format == AudioFormat::Mp3 {
                return Err("--format mp3 works with the built-in decoder only".to_owned());
            }
            let Some(tool) = external else {
                let bank = parse_bank(&bytes, &Limits::default()).map_err(|e| e.to_string())?;
                let (mut done, mut failed) = (0usize, 0usize);
                for (i, s) in bank.streams.iter().enumerate() {
                    let safe: String = s
                        .name
                        .chars()
                        .map(|c| {
                            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                                '_'
                            } else {
                                c
                            }
                        })
                        .collect();
                    let ext = if format == AudioFormat::Mp3 {
                        "mp3"
                    } else {
                        "wav"
                    };
                    let target = output.join(format!("{safe}_{}.{ext}", i + 1));
                    let encoded = bank.decode(&bytes, i).and_then(|pcm| {
                        if format == AudioFormat::Mp3 {
                            pcm.to_mp3()
                        } else {
                            Ok(pcm.to_wav())
                        }
                    });
                    match encoded {
                        Ok(data) => {
                            std::fs::write(&target, data)
                                .map_err(|e| format!("{}: {e}", target.display()))?;
                            done += 1;
                        }
                        Err(e) => {
                            eprintln!("stream {} ({}): {e}", i + 1, s.name);
                            failed += 1;
                        }
                    }
                }
                println!("Decoded {done} stream(s) to {}", output.display());
                return Ok(u8::from(failed > 0));
            };
            // vgmstream reads a plain .fsb, so a .bsb is inflated into a temporary file.
            let tmp = output.join(".adv-bank.tmp.fsb");
            std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
            let result = std::process::Command::new(&tool)
                .current_dir(&output)
                .args(["-S", "0", "-o", "?n_?s.wav"])
                .arg(".adv-bank.tmp.fsb")
                .stdout(std::process::Stdio::null())
                .status();
            let _ = std::fs::remove_file(&tmp);
            match result {
                Ok(s) if s.success() => {
                    println!(
                        "Decoded {} stream(s) to {}",
                        parse_bank(&bytes, &Limits::default()).map_or(0, |b| b.streams.len()),
                        output.display()
                    );
                    Ok(0)
                }
                Ok(s) => Err(format!("{} failed with {s}", tool.display())),
                Err(e) => Err(format!(
                    "cannot run {}: {e}. Install vgmstream-cli and pass --vgmstream <path> or set ADV_VGMSTREAM",
                    tool.display()
                )),
            }
        }
        Command::Tex {
            command: TexCommand::Info { file },
        } => {
            let levels = load_texture(&file)?;
            println!("Levels: {}", levels.len());
            for l in &levels {
                println!("  mip {:>2}: {} bytes", l.id, l.data.len());
            }
            if let Some(g) = adv_core::texheur::guess(&levels, None) {
                let (w, h) = g.size();
                println!(
                    "Guess: {w}x{h} {:?} (candidates: {})",
                    g.format,
                    g.candidates
                        .iter()
                        .map(|(a, b)| format!("{a}x{b}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            match load_desc(&file) {
                Ok(d) => println!(
                    "From .xdb: {}x{} (used {}x{}), {:?}, {} mip(s)",
                    d.width, d.height, d.real_width, d.real_height, d.format, d.mips
                ),
                Err(_) => println!(
                    "No .xdb next to the file: width, height and format must be given with --size and --dxt."
                ),
            }
            Ok(0)
        }
        Command::Tex {
            command:
                TexCommand::Convert {
                    file,
                    size,
                    dxt,
                    db,
                    db_id,
                    tdb,
                    index,
                    crop,
                    format,
                    mip,
                    output,
                },
        } => {
            let desc = if size.is_none() || dxt.is_none() || crop {
                Some(match &db {
                    Some(d) => desc_from_db(d, &file, db_id)?,
                    None => match load_desc(&file) {
                        Ok(d) => d,
                        Err(e) => {
                            let lv = load_texture(&file)?;
                            guess_desc(&file, &lv, tdb.as_deref(), index, dxt).ok_or(e)?
                        }
                    },
                })
            } else {
                None
            };
            let (w, h) = match (&size, &desc) {
                (Some(s), _) => parse_size(s)?,
                (None, Some(d)) => (d.width, d.height),
                (None, None) => unreachable!("description is loaded when size is missing"),
            };
            let fmt = match (dxt, &desc) {
                (Some(1), _) => Dxt::Dxt1,
                (Some(3), _) => Dxt::Dxt3,
                (Some(5), _) => Dxt::Dxt5,
                (Some(_), _) => return Err("--dxt must be 1, 3 or 5".into()),
                (None, Some(d)) => d.format,
                (None, None) => unreachable!("description is loaded when dxt is missing"),
            };
            let levels = load_texture(&file)?;
            let bytes = match format.as_str() {
                "png" => {
                    let l = match mip {
                        Some(k) => levels.iter().find(|l| l.id == k),
                        None => levels.first(),
                    }
                    .ok_or("requested mip level is not in the file")?;
                    let (lw, lh) = level_dims(w, h, l.id);
                    let mut rgba = decode_dxt(&l.data, lw, lh, fmt).map_err(|e| e.to_string())?;
                    let (mut ow, mut oh) = (lw, lh);
                    if let (true, Some(d)) = (crop, &desc) {
                        let s = l.id.clamp(0, 31) as u32;
                        ow = (d.real_width >> s).clamp(1, lw);
                        oh = (d.real_height >> s).clamp(1, lh);
                        let mut cut = Vec::with_capacity(ow as usize * oh as usize * 4);
                        for y in 0..oh as usize {
                            let a = y * lw as usize * 4;
                            cut.extend_from_slice(&rgba[a..a + ow as usize * 4]);
                        }
                        rgba = cut;
                    }
                    write_png(ow, oh, &rgba).map_err(|e| e.to_string())?
                }
                "dds" => {
                    let top = levels.first().ok_or("no levels")?.id;
                    let chain: Vec<&[u8]> = levels
                        .iter()
                        .enumerate()
                        .take_while(|(i, l)| l.id == top + *i as i32)
                        .map(|(_, l)| l.data.as_slice())
                        .collect();
                    let (lw, lh) = level_dims(w, h, top);
                    write_dds(lw, lh, fmt, &chain).map_err(|e| e.to_string())?
                }
                other => return Err(format!("unknown format {other:?}, use png or dds")),
            };
            std::fs::write(&output, bytes).map_err(|e| format!("{}: {e}", output.display()))?;
            Ok(0)
        }
        Command::Verify { path } => {
            let mut files = Vec::new();
            collect_files(&path, &mut files).map_err(|e| format!("{}: {e}", path.display()))?;
            files.retain(|f| {
                f.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("pak"))
            });
            let (mut ok, mut bad) = (0u64, 0u64);
            for f in files {
                let mut pak = match open(&f) {
                    Ok(p) => p,
                    Err(e) => {
                        bad += 1;
                        eprintln!("{e}");
                        continue;
                    }
                };
                let list: Vec<Entry> = pak.entries().to_vec();
                for e in &list {
                    match pak.read(e, Layer::Raw) {
                        Ok(_) => ok += 1,
                        Err(err) => {
                            bad += 1;
                            eprintln!("{}!{}: {err}", f.display(), e.path);
                        }
                    }
                }
                bad += pak.skipped().len() as u64;
            }
            println!("Verified: {ok}, problems: {bad}");
            Ok(u8::from(bad > 0))
        }
        Command::Loc {
            command:
                LocCommand::Dump {
                    file,
                    entry,
                    format,
                    output,
                    bom,
                },
        } => {
            let loc = load_loc(&file, entry.as_deref())?;
            let bytes = render(&loc, &format, bom)?;
            match output {
                Some(p) => {
                    std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?
                }
                None => std::io::stdout()
                    .write_all(&bytes)
                    .map_err(|e| e.to_string())?,
            }
            Ok(0)
        }
        Command::Search {
            path,
            query,
            keys,
            limit,
        } => {
            let q = query.to_lowercase();
            let mut files = Vec::new();
            collect_files(&path, &mut files).map_err(|e| format!("{}: {e}", path.display()))?;
            let (mut left, mut problems) = (limit, 0u64);
            for f in files {
                if left == 0 {
                    break;
                }
                let ext = f
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let label = f.display().to_string();
                if ext == "loc" {
                    match load_loc(&f, None) {
                        Ok(l) => search_items(&label, &l.items, &q, keys, &mut left),
                        Err(e) => {
                            problems += 1;
                            eprintln!("skipped: {e}");
                        }
                    }
                } else if ext == "pak" {
                    let Ok(pak) = open(&f) else { continue };
                    let names: Vec<String> = pak
                        .entries()
                        .iter()
                        .filter(|e| e.path.as_str().to_ascii_lowercase().ends_with(".loc"))
                        .map(|e| e.path.as_str().to_owned())
                        .collect();
                    for n in names {
                        match load_loc(&f, Some(&n)) {
                            Ok(l) => {
                                search_items(&format!("{label}!{n}"), &l.items, &q, keys, &mut left)
                            }
                            Err(e) => {
                                problems += 1;
                                eprintln!("skipped: {e}");
                            }
                        }
                    }
                }
            }
            Ok(if problems > 0 { 2 } else { 0 })
        }
        Command::Extract {
            pak: path,
            output,
            filter,
            raw,
            overwrite,
            flat,
        } => {
            let mut pak = open(&path)?;
            let list: Vec<Entry> = selected(&pak, filter.as_deref())
                .into_iter()
                .cloned()
                .collect();
            let options = ExtractOptions {
                layer: if raw { Layer::Raw } else { Layer::Decoded },
                overwrite,
                flat,
            };
            let (mut done, mut existing, mut failed) = (0u64, 0u64, 0u64);
            for e in &list {
                match extract_entry(&mut pak, e, &output, options) {
                    Ok(Outcome::Written { renamed, .. }) => {
                        done += 1;
                        if renamed {
                            eprintln!("renamed for this platform: {}", e.path);
                        }
                    }
                    Ok(Outcome::Exists(p)) => {
                        existing += 1;
                        eprintln!("exists, kept: {}", p.display());
                    }
                    Err(err) => {
                        failed += 1;
                        eprintln!("failed {}: {err}", e.path);
                    }
                }
            }
            warn_skipped(&pak);
            println!("Extracted: {done}, kept existing: {existing}, failed: {failed}");
            let partial = failed > 0 || existing > 0 || !pak.skipped().is_empty();
            Ok(if partial { 2 } else { 0 })
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        println!(
            "Allods Data Viewer {} (core {})",
            env!("CARGO_PKG_VERSION"),
            adv_core::version()
        );
        return ExitCode::SUCCESS;
    };
    // A closed pipe (`adv ... | head`) is not an error worth a backtrace.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = info.to_string();
        if text.contains("Broken pipe") {
            std::process::exit(0);
        }
        default_hook(info);
    }));
    match run(command) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wildcard;

    #[test]
    fn wildcard_matching() {
        assert!(wildcard("*.loc", "Bin/pack.rus.LOC"));
        assert!(wildcard("bin/*", "Bin/a/b"));
        assert!(wildcard("a*c*e", "abcde"));
        assert!(!wildcard("*.loc", "a.bin"));
        assert!(wildcard("*", ""));
        assert!(!wildcard("a", "ab"));
    }
}
