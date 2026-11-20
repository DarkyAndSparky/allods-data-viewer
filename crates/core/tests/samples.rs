//! Regression tests on real game files.
//!
//! They run only when `ADV_SAMPLES` points to a folder with `.pak` files (searched
//! recursively); otherwise every test returns at once. Nothing from the folder is ever
//! copied into the repository. The checks are invariants ("every entry parses, nothing is
//! left over"), not exact numbers, so they hold for any client version.

use adv_core::animation;
use adv_core::geometry;
use adv_core::limits::Limits;
use adv_core::loc::parse_loc;
use adv_core::pak::{Layer, PakArchive};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

fn samples() -> Option<Vec<PathBuf>> {
    let dir = std::env::var_os("ADV_SAMPLES")?;
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pak")) {
                out.push(p);
            }
        }
    }
    out.sort();
    Some(out)
}

fn open(p: &Path) -> Option<PakArchive<BufReader<File>>> {
    PakArchive::open(BufReader::new(File::open(p).ok()?), Limits::default()).ok()
}

#[test]
fn every_entry_of_every_pak_reads_with_a_valid_crc() {
    let Some(paks) = samples() else { return };
    for p in paks {
        let Some(mut a) = open(&p) else { continue };
        let entries = a.entries().to_vec();
        for e in &entries {
            a.read(e, Layer::Raw)
                .unwrap_or_else(|err| panic!("{}: {}: {err}", p.display(), e.path));
        }
    }
}

#[test]
fn every_loc_parses_and_has_unique_ids() {
    let Some(paks) = samples() else { return };
    let mut seen = 0;
    for p in paks {
        let Some(mut a) = open(&p) else { continue };
        let entries = a.entries().to_vec();
        for e in entries
            .iter()
            .filter(|e| e.path.as_str().to_ascii_lowercase().ends_with(".loc"))
        {
            let (d, _) = a.read(e, Layer::Decoded).expect("read");
            let f = parse_loc(&d, &Limits::default())
                .unwrap_or_else(|err| panic!("{}: {}: {err}", p.display(), e.path));
            assert!(!f.items.is_empty(), "{}: empty loc", e.path);
            let mut ids: Vec<u32> = f.items.iter().map(|i| i.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), f.items.len(), "{}: duplicate ids", e.path);
            seen += 1;
        }
    }
    eprintln!("loc files checked: {seen}");
}

#[test]
fn every_geometry_has_indices_inside_the_vertex_buffer() {
    let Some(paks) = samples() else { return };
    let mut seen = 0;
    for p in paks {
        let Some(mut a) = open(&p) else { continue };
        let entries = a.entries().to_vec();
        for e in entries
            .iter()
            .filter(|e| e.path.as_str().ends_with("(Geometry).bin"))
        {
            let (raw, _) = a.read(e, Layer::Raw).expect("read");
            let d = geometry::inflate(&raw, &Limits::default()).expect("inflate");
            let g = geometry::parse(&d, None)
                .unwrap_or_else(|err| panic!("{}: {}: {err}", p.display(), e.path));
            let verts = g.vertex_data.len() / g.stride;
            assert!(
                g.indices.iter().all(|&i| usize::from(i) < verts),
                "{}: index outside the vertex buffer",
                e.path
            );
            seen += 1;
        }
    }
    eprintln!("geometry files checked: {seen}");
}

#[test]
fn every_animation_clip_decodes() {
    let Some(paks) = samples() else { return };
    let mut seen = 0;
    for p in paks {
        let Some(mut a) = open(&p) else { continue };
        let entries = a.entries().to_vec();
        for e in entries
            .iter()
            .filter(|e| e.path.as_str().ends_with("(SkeletalAnimation).bin"))
        {
            let (raw, _) = a.read(e, Layer::Raw).expect("read");
            let d = geometry::inflate(&raw, &Limits::default()).expect("inflate");
            let c = animation::decode(&d)
                .unwrap_or_else(|err| panic!("{}: {}: {err}", p.display(), e.path));
            assert!(c.tracks.iter().all(|t| t.len() == usize::from(c.frames)));
            seen += 1;
        }
    }
    eprintln!("animation clips checked: {seen}");
}

#[test]
fn geometry_animation_and_terrain_files_are_exact_blob_lists() {
    let Some(paks) = samples() else { return };
    let mut seen = 0;
    for p in paks {
        let Some(mut a) = open(&p) else { continue };
        let entries = a.entries().to_vec();
        for e in entries.iter().filter(|e| {
            let n = e.path.as_str();
            n.ends_with("(Geometry).bin")
                || n.ends_with("(SkeletalAnimation).bin")
                || n.ends_with("_terrainDump.bin")
        }) {
            let (raw, _) = a.read(e, Layer::Raw).expect("read");
            let d = geometry::inflate(&raw, &Limits::default()).expect("inflate");
            let blobs = adv_core::blob::parse_blobs(&d)
                .unwrap_or_else(|err| panic!("{}: {}: {err}", p.display(), e.path));
            assert_eq!(blobs[0].id, 0, "{}: first blob id", e.path);
            if e.path.as_str().ends_with("(Geometry).bin") {
                let ids: Vec<u32> = blobs.iter().take(3).map(|b| b.id).collect();
                assert_eq!(ids, [0, 1, 2], "{}: geometry blob ids", e.path);
            }
            seen += 1;
        }
    }
    eprintln!("blob lists checked: {seen}");
}
