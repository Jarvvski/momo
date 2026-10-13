//! Smoke test: the binary starts and reports its version.

use std::process::Command;

#[test]
fn prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_momo"))
        .arg("--version")
        .output()
        .expect("failed to run momo");

    assert!(output.status.success(), "momo exited non-zero");

    let stdout = String::from_utf8(output.stdout).expect("stdout was not UTF-8");
    assert!(
        stdout.trim().starts_with("momo "),
        "unexpected stdout: {stdout:?}"
    );
}
