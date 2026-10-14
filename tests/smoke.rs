//! Smoke test: the binary starts and reports its version.

use std::process::Command;

/// Run the binary with `args`, asserting it succeeded, and return its stdout.
fn stdout(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_momo"))
        .args(args)
        .output()
        .expect("failed to run momo");

    assert!(output.status.success(), "momo {args:?} exited non-zero");

    String::from_utf8(output.stdout).expect("stdout was not UTF-8")
}

#[test]
fn prints_version() {
    let stdout = stdout(&["--version"]);
    assert!(
        stdout.trim().starts_with("momo "),
        "unexpected stdout: {stdout:?}"
    );
}

#[test]
fn the_version_verb_matches_the_version_flag() {
    assert_eq!(stdout(&["version"]), stdout(&["--version"]));
}
