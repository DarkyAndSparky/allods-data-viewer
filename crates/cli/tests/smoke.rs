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

    fn bin(levels: &[(i32, &[u8])]) -> Vec<u8> {
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
