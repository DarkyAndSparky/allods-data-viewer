//! `adv` command line tool of Allods Data Viewer.

use adv_core::extract::{ExtractOptions, Outcome, extract_entry};
use adv_core::loc::{LocFile, LocItem, read_loc};
use adv_core::texture::{Dxt, Level, decode_dxt, level_dims, read_levels, write_dds, write_png};
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
        /// Size of mip level 0, for example `512x512`.
        #[arg(long)]
        size: String,
        /// Block format: 1, 3 or 5 (DXT1, DXT3, DXT5).
        #[arg(long)]
        dxt: u8,
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
        Command::Tex {
            command: TexCommand::Info { file },
        } => {
            let levels = load_texture(&file)?;
            println!("Levels: {}", levels.len());
            for l in &levels {
                println!("  mip {:>2}: {} bytes", l.id, l.data.len());
            }
            println!("Width, height and format are not stored in the file; use --size and --dxt.");
            Ok(0)
        }
        Command::Tex {
            command:
                TexCommand::Convert {
                    file,
                    size,
                    dxt,
                    format,
                    mip,
                    output,
                },
        } => {
            let (w, h) = parse_size(&size)?;
            let fmt = match dxt {
                1 => Dxt::Dxt1,
                3 => Dxt::Dxt3,
                5 => Dxt::Dxt5,
                _ => return Err("--dxt must be 1, 3 or 5".into()),
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
                    let rgba = decode_dxt(&l.data, lw, lh, fmt).map_err(|e| e.to_string())?;
                    write_png(lw, lh, &rgba).map_err(|e| e.to_string())?
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
