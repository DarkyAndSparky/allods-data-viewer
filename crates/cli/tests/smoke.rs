//! End to end smoke tests for the compiled binary.

use std::process::Command;

fn adv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_adv"))
}

#[test]
fn prints_version_line_without_arguments() {
    let output = adv().output().expect("failed to run adv");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with(&format!("Allods Data Viewer {}", env!("CARGO_PKG_VERSION"))),
        "unexpected output: {stdout}"
    );
}

#[test]
fn version_flag_works() {
    let output = adv().arg("--version").output().expect("failed to run adv");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_flag_fails() {
    let output = adv()
        .arg("--no-such-flag")
        .output()
        .expect("failed to run adv");

    assert!(!output.status.success());
}

mod pak_commands {
    use std::io::Write;
    use std::process::Command;

    fn adv() -> Command {
        Command::new(env!("CARGO_BIN_EXE_adv"))
    }

    fn make_pak(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("t.pak");
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zw.start_file("a/one.txt", opts).expect("start");
        zw.write_all(b"one").expect("write");
        zw.start_file("two.bin", opts).expect("start");
        zw.write_all(b"22").expect("write");
        zw.finish().expect("finish");
        path
    }

    #[test]
    fn tree_shows_directories_and_kinds() {
        let dir = tempfile::tempdir().expect("tmp");
        let pak = make_pak(dir.path());
        let out = adv().arg("tree").arg(&pak).output().expect("run");
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(text.contains("a/  (1 files)"), "{text}");
        assert!(text.contains("  one.txt  [text]"), "{text}");
        assert!(text.contains("two.bin  [other]"), "{text}");

        let out = adv()
            .arg("tree")
            .arg(&pak)
            .args(["--dir", "nope"])
            .output()
            .expect("run");
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("no such directory"));
    }

    #[test]
    fn list_info_and_extract_work() {
        let dir = tempfile::tempdir().expect("tmp");
        let pak = make_pak(dir.path());

        let out = adv().arg("list").arg(&pak).output().expect("run");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a/one.txt\ntwo.bin\n");

        let out = adv().arg("info").arg(&pak).output().expect("run");
        assert!(String::from_utf8_lossy(&out.stdout).contains("Entries: 2"));

        let dest = dir.path().join("out");
        let out = adv()
            .arg("extract")
            .arg(&pak)
            .arg("-o")
            .arg(&dest)
            .args(["--filter", "*.txt"])
            .output()
            .expect("run");
        assert!(out.status.success());
        assert_eq!(std::fs::read(dest.join("a/one.txt")).expect("read"), b"one");
        assert!(!dest.join("two.bin").exists());

        let again = adv()
            .arg("extract")
            .arg(&pak)
            .arg("-o")
            .arg(&dest)
            .output()
            .expect("run");
        assert_eq!(
            again.status.code(),
            Some(2),
            "existing files give partial success"
        );
    }

    #[test]
    fn verify_and_json_work() {
        let dir = tempfile::tempdir().expect("tmp");
        let pak = make_pak(dir.path());
        let out = adv().arg("verify").arg(dir.path()).output().expect("run");
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("Verified: 2, problems: 0"));

        let out = adv()
            .args(["list", "--json"])
            .arg(&pak)
            .output()
            .expect("run");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("\"schema\": 1") && text.contains("a/one.txt"));
        let out = adv()
            .args(["info", "--json"])
            .arg(&pak)
            .output()
            .expect("run");
        assert!(String::from_utf8_lossy(&out.stdout).contains("\"entries\": 2"));

        let mut bytes = std::fs::read(&pak).expect("read");
        let pos = bytes.windows(5).position(|w| w == b"onePK").expect("find");
        bytes[pos] ^= 0xFF;
        std::fs::write(&pak, bytes).expect("write");
        let out = adv().arg("verify").arg(&pak).output().expect("run");
        assert_eq!(out.status.code(), Some(1));
    }

    #[test]
    fn missing_file_is_an_error() {
        let out = adv().args(["info", "no-such.pak"]).output().expect("run");
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains("error:"));
    }
}

mod loc_commands {
    use std::process::Command;

    /// Minimal layout 2 payload with the given texts (see docs/formats/loc.md).
    fn v2(texts: &[&str]) -> Vec<u8> {
        let mut area = Vec::new();
        let mut desc = Vec::new();
        for t in texts {
            desc.push((t.encode_utf16().count() as u64, area.len() as u64));
            let mut s: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
            s.extend_from_slice(&[0, 0]);
            while !s.len().is_multiple_of(8) {
                s.push(0);
            }
            area.extend_from_slice(&s);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&0x2689_119Fu64.to_le_bytes());
        out.extend_from_slice(&((texts.len() * 2) as u64).to_le_bytes());
        for (l, o) in desc {
            out.extend_from_slice(&l.to_le_bytes());
            out.extend_from_slice(&o.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(&area);
        out
    }

    #[test]
    fn dump_and_search() {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("t.loc");
        std::fs::write(&file, v2(&["Hello, \"world\"", "Камень"])).expect("write");
        let adv = || Command::new(env!("CARGO_BIN_EXE_adv"));

        let out = adv()
            .args(["loc", "dump"])
            .arg(&file)
            .output()
            .expect("run");
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).expect("utf8");
        assert_eq!(
            text,
            "id,key,text\r\n0,,\"Hello, \"\"world\"\"\"\r\n1,,Камень\r\n"
        );

        let out = adv()
            .args(["loc", "dump", "--format", "json"])
            .arg(&file)
            .output()
            .expect("run");
        assert!(String::from_utf8_lossy(&out.stdout).contains("\"layout\": \"V2\""));

        let out = adv()
            .arg("search")
            .arg(dir.path())
            .arg("КАМЕНЬ")
            .output()
            .expect("run");
        assert!(String::from_utf8_lossy(&out.stdout).contains("Камень"));

        let bad = dir.path().join("bad.loc");
        std::fs::write(&bad, b"garbage garbage garbage").expect("write");
        let out = adv().args(["loc", "dump"]).arg(&bad).output().expect("run");
        assert_eq!(out.status.code(), Some(1));
    }
}

mod tex_commands {
    use std::process::Command;

    pub fn bin(levels: &[(i32, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        for (id, d) in levels {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&(d.len() as i32).to_le_bytes());
            v.extend_from_slice(d);
        }
        v
    }

    #[test]
    fn info_and_convert() {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("T.(Texture).bin");
        // level 1 of an 8x8 DXT1 texture (4x4 pixels = one block) and level 0 sibling
        std::fs::write(&file, bin(&[(1, &[0, 0xF8, 0, 0, 0, 0, 0, 0])])).expect("write");
        let hi = dir.path().join("T.(Texture).hi.bin");
        std::fs::write(&hi, bin(&[(0, &[0u8; 32])])).expect("write");
        let adv = || Command::new(env!("CARGO_BIN_EXE_adv"));

        let out = adv()
            .args(["tex", "info"])
            .arg(&file)
            .output()
            .expect("run");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("Levels: 2") && text.contains("mip  0: 32 bytes"));

        let png = dir.path().join("o.png");
        let out = adv()
            .args(["tex", "convert", "--size", "8x8", "--dxt", "1", "-o"])
            .arg(&png)
            .arg(&file)
            .output()
            .expect("run");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(&std::fs::read(&png).expect("read")[..4], b"\x89PNG");

        let dds = dir.path().join("o.dds");
        let out = adv()
            .args([
                "tex", "convert", "--size", "8x8", "--dxt", "1", "--format", "dds", "-o",
            ])
            .arg(&dds)
            .arg(&file)
            .output()
            .expect("run");
        assert!(out.status.success());
        assert_eq!(std::fs::read(&dds).expect("read").len(), 128 + 32 + 8);

        let out = adv()
            .args(["tex", "convert", "--size", "16x16", "--dxt", "1", "-o"])
            .arg(&png)
            .arg(&file)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1), "wrong size must fail");
    }
}

mod audio_commands {
    use std::process::Command;

    /// Smallest FSB5 with one 48 kHz mono stream named "Hello".
    fn fsb() -> Vec<u8> {
        let bits: u64 = (9 << 1) | (48_000u64 << 34);
        let mut names = 4u32.to_le_bytes().to_vec();
        names.extend_from_slice(b"Hello\0");
        let mut out = b"FSB5".to_vec();
        for v in [1u32, 1, 8, names.len() as u32, 0, 15] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(60, 0);
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(&names);
        out
    }

    #[test]
    fn list_and_export_errors() {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("a.fsb");
        std::fs::write(&file, fsb()).expect("write");
        let adv = || Command::new(env!("CARGO_BIN_EXE_adv"));

        let out = adv()
            .args(["audio", "list"])
            .arg(&file)
            .output()
            .expect("run");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(text.contains("Streams: 1") && text.contains("48000 Hz") && text.contains("Hello"));

        let out = adv()
            .args(["audio", "export", "--vgmstream", "no-such-tool-xyz", "-o"])
            .arg(dir.path().join("o"))
            .arg(&file)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains("cannot run"));
        assert!(!dir.path().join("o/.adv-bank.tmp.fsb").exists());

        // The built-in decoder reports a stream it cannot decode and exits with 1.
        let out = adv()
            .env_remove("ADV_VGMSTREAM")
            .args(["audio", "export", "-o"])
            .arg(dir.path().join("o2"))
            .arg(&file)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("stream 1") && err.contains("Vorbis"), "{err}");

        std::fs::write(&file, b"junk junk junk").expect("write");
        let out = adv()
            .args(["audio", "list"])
            .arg(&file)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1));
    }

    #[test]
    fn size_and_format_come_from_the_xdb() {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("U.(UITexture).bin");
        std::fs::write(
            &file,
            super::tex_commands::bin(&[(0, &[0, 0xF8, 0, 0, 0, 0, 0, 0].repeat(4))]),
        )
        .expect("write");
        std::fs::write(
            dir.path().join("U.(UITexture).xdb"),
            "<UITexture><mipsNumber>1</mipsNumber><type>DXT1</type><width>8</width><height>8</height><realWidth>5</realWidth><realHeight>3</realHeight></UITexture>",
        )
        .expect("write");
        let adv = || Command::new(env!("CARGO_BIN_EXE_adv"));
        let png = dir.path().join("o.png");
        let out = adv()
            .args(["tex", "convert", "--crop", "-o"])
            .arg(&png)
            .arg(&file)
            .output()
            .expect("run");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let data = std::fs::read(&png).expect("read");
        assert_eq!(&data[16..24], &[0, 0, 0, 5, 0, 0, 0, 3], "cropped to 5x3");

        let info = adv()
            .args(["tex", "info"])
            .arg(&file)
            .output()
            .expect("run");
        assert!(String::from_utf8_lossy(&info.stdout).contains("8x8 (used 5x3)"));

        std::fs::remove_file(dir.path().join("U.(UITexture).xdb")).expect("rm");
        let out = adv()
            .args(["tex", "convert", "-o"])
            .arg(&png)
            .arg(&file)
            .output()
            .expect("run");
        assert!(out.status.success(), "a guess is made without an .xdb");
        assert!(String::from_utf8_lossy(&out.stderr).contains("guess"));

        // 24 bytes of DXT1 data are 48 pixels: no power-of-two size fits, so nothing to guess.
        let odd = dir.path().join("V.(UITexture).bin");
        std::fs::write(&odd, super::tex_commands::bin(&[(0, &[0u8; 24])])).expect("write");
        let out = adv()
            .args(["tex", "convert", "-o"])
            .arg(&png)
            .arg(&odd)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1));
    }
}

mod db_commands {
    use std::process::Command;

    #[test]
    fn find_locates_databases_inside_paks() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tmp");
        let mut db = vec![0xCC, 2, 0x2E, 0x59, 0x73, 4, 0, 0, 1, 2, 3, 4];
        db.extend([0u8; 16]);
        let pak = dir.path().join("Some.Mini.pak");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&pak).expect("create"));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("Maps_A.bin", opts).expect("start");
        zip.write_all(&db).expect("write");
        zip.start_file("x/B.(Texture).bin", opts).expect("start");
        zip.write_all(&db).expect("write");
        zip.start_file("notes.bin", opts).expect("start");
        zip.write_all(b"not a database").expect("write");
        zip.finish().expect("finish");
        let out_dir = dir.path().join("dbs");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["db", "find"])
            .arg(dir.path())
            .arg("-o")
            .arg(&out_dir)
            .output()
            .expect("run");
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.contains("Maps_A.bin") && text.contains("CC022E59"),
            "{text}"
        );
        assert!(
            !text.contains("Texture") && !text.contains("notes"),
            "{text}"
        );
        assert!(out_dir.join("Some.Mini").join("Maps_A.bin").exists());
    }

    #[test]
    fn find_names_the_client_of_a_32_bit_database() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tmp");
        // Chunk 0 (size 8, the signature of client 1.1.00.41), then the start of chunk 1.
        let mut db = Vec::new();
        db.extend(0u32.to_le_bytes());
        db.extend(8u32.to_le_bytes());
        db.extend([0x24, 0x19, 0xFD, 0x3C, 0x1B, 0x6C, 0x73, 0x8A]);
        db.extend(1u32.to_le_bytes());
        db.extend([0u8; 16]);
        let pak = dir.path().join("Bin.pak");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&pak).expect("create"));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("Bin/pack.bin", opts).expect("start");
        zip.write_all(&db).expect("write");
        zip.finish().expect("finish");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["db", "find"])
            .arg(dir.path())
            .output()
            .expect("run");
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.contains("Bin/pack.bin")
                && text.contains("2419FD3C1B6C738A")
                && text.contains("Allods Online 1.1.00.41"),
            "{text}"
        );
    }

    /// A clip with one bone named `Bip01`, no material tracks and a 2 frame, 30 per second header.
    fn clip_with_one_named_bone() -> Vec<u8> {
        let mut d = vec![0u8; 64];
        d[8..10].copy_from_slice(&30u16.to_le_bytes());
        d[10..12].copy_from_slice(&2u16.to_le_bytes());
        d[12..16].copy_from_slice(&(64u32 - 12).to_le_bytes());
        d[16..20].copy_from_slice(&1u32.to_le_bytes());
        d.extend([0u8; 20]);
        let table = d.len();
        d.extend([0u8; 8]);
        let text = d.len();
        d.extend(b"Bip01\0\0");
        d[table..table + 4].copy_from_slice(&((text - table) as u32).to_le_bytes());
        d[table + 4..table + 8].copy_from_slice(&7u32.to_le_bytes());
        d[20..24].copy_from_slice(&((table - 20) as u32).to_le_bytes());
        d[24..28].copy_from_slice(&1u32.to_le_bytes());
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
        d
    }

    #[test]
    fn anim_info_lists_bones_and_materials() {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("X.(SkeletalAnimation).bin");
        std::fs::write(&file, clip_with_one_named_bone()).expect("write");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["anim", "--json"])
            .arg(&file)
            .output()
            .expect("run");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
        assert_eq!(doc["fps"], 30);
        assert_eq!(doc["frames"], 2);
        assert_eq!(doc["bones"][0], "Bip01");
        assert_eq!(doc["materials"].as_array().map(Vec::len), Some(0));
        let text = Command::new(env!("CARGO_BIN_EXE_adv"))
            .arg("anim")
            .arg(&file)
            .output()
            .expect("run");
        let text = String::from_utf8_lossy(&text.stdout);
        assert!(
            text.contains("Bones: 1")
                && text.contains("Bip01")
                && text.contains("Material tracks: 0"),
            "{text}"
        );
        // Not a clip: a clean error, exit code 1.
        std::fs::write(&file, b"not a clip at all, just text").expect("write");
        let bad = Command::new(env!("CARGO_BIN_EXE_adv"))
            .arg("anim")
            .arg(&file)
            .output()
            .expect("run");
        assert_eq!(bad.status.code(), Some(1));
    }

    #[test]
    fn info_and_errors() {
        let dir = tempfile::tempdir().expect("tmp");
        let bad = dir.path().join("x.bin");
        std::fs::write(&bad, b"not a database at all").expect("write");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["db", "info"])
            .arg(&bad)
            .output()
            .expect("run");
        assert!(!out.status.success());
        assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
    }

    #[test]
    fn convert_with_bad_db_reports_error() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = dir.path().join("pack.bin");
        std::fs::write(&db, b"junk").expect("write");
        let tex = dir.path().join("A.(Texture).bin");
        std::fs::write(&tex, b"x").expect("write");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["tex", "convert"])
            .arg(&tex)
            .arg("--db")
            .arg(&db)
            .arg("-o")
            .arg(dir.path().join("o.png"))
            .output()
            .expect("run");
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("pack.bin"));
    }
}

mod geom_commands {
    use std::process::Command;

    fn sample() -> Vec<u8> {
        let mut v = Vec::new();
        for i in 0..3u32 {
            for f in [i as f32, 0.0, 0.0, 0.5, 0.5] {
                v.extend(f.to_le_bytes());
            }
            v.extend([0u8; 4]);
        }
        let mut d = vec![0u8; 4];
        d.extend((v.len() as u32).to_le_bytes());
        d.extend(&v);
        d.extend(1u32.to_le_bytes());
        d.extend(6u32.to_le_bytes());
        for i in [0u16, 1, 2] {
            d.extend(i.to_le_bytes());
        }
        d.extend(2u32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d
    }

    #[test]
    fn info_and_obj() {
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("M.(Geometry).bin");
        std::fs::write(&f, sample()).expect("write");
        let adv = || Command::new(env!("CARGO_BIN_EXE_adv"));
        let out = adv().args(["geom", "info"]).arg(&f).output().expect("run");
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("Vertices: 3 (stride 24"), "{text}");
        assert!(text.contains("Triangles: 1"), "{text}");
        let obj = dir.path().join("m.obj");
        let out = adv()
            .args(["geom", "obj"])
            .arg(&f)
            .arg("-o")
            .arg(&obj)
            .output()
            .expect("run");
        assert!(out.status.success());
        assert!(
            std::fs::read_to_string(&obj)
                .expect("read")
                .contains("f 1/1 2/2 3/3")
        );
        let glb = dir.path().join("m.glb");
        let out = adv()
            .args(["geom", "glb"])
            .arg(&f)
            .arg("-o")
            .arg(&glb)
            .output()
            .expect("run");
        assert!(out.status.success());
        let bytes = std::fs::read(&glb).expect("read");
        assert_eq!(&bytes[0..4], b"glTF");
        let jl = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(&bytes[20..20 + jl]).expect("valid JSON");
        assert_eq!(json["asset"]["version"], "2.0");
        assert_eq!(json["accessors"][0]["count"], 3);
        assert_eq!(json["accessors"][2]["count"], 3);
    }

    #[test]
    fn damaged_file_is_an_error() {
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("bad.bin");
        std::fs::write(&f, [1u8, 2, 3]).expect("write");
        let out = Command::new(env!("CARGO_BIN_EXE_adv"))
            .args(["geom", "info"])
            .arg(&f)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1));
    }
}
