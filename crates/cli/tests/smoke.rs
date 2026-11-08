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
