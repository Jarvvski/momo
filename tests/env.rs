//! Integration tests for the `MOMO_*` environment layer.
//!
//! `ConfigOverrides::from_env` parses eight keys; the rest of the suite sets
//! only `MOMO_RECORDING`, `MOMO_TAG_PREFIX` and `MOMO_REMOTE`. These drive the
//! live keys end to end, proving the documented precedence `env > file >
//! default`. `MOMO_TAG_KIND` and `MOMO_TASK_ENV` are inert today (tickets 21/22)
//! and are deliberately absent until those decide their fate.

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

/// A `momo` command rooted at `dir`, with every `MOMO_*` variable cleared.
fn momo(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("momo").expect("the momo binary is built");
    cmd.current_dir(dir.path());
    for name in [
        "MOMO_RECORDING",
        "MOMO_CHANGELOG",
        "MOMO_CHANGES_DIR",
        "MOMO_TAG_PREFIX",
        "MOMO_TAG_KIND",
        "MOMO_REMOTE",
        "MOMO_TASK_ENV",
        "MOMO_RC_FORMAT",
    ] {
        cmd.env_remove(name);
    }
    cmd
}

/// Seed `momo.toml` with `text` when it is non-empty.
fn seed(dir: &TempDir, text: &str) {
    if !text.is_empty() {
        fs::write(dir.path().join("momo.toml"), text).expect("seed momo.toml");
    }
}

/// Run `momo add` under `env`, asserting success.
fn add(dir: &TempDir, env: Option<(&str, &str)>) {
    let mut cmd = momo(dir);
    if let Some((name, value)) = env {
        cmd.env(name, value);
    }
    cmd.args(["add", "Add a shiny thing"]).assert().success();
}

#[test]
fn momo_changelog_beats_the_file_and_the_default() {
    // (label, momo.toml, env override, written, not written)
    let cases = [
        ("default", "", None, "CHANGELOG.md", ""),
        (
            "file",
            "changelog = \"FILE.md\"\n",
            None,
            "FILE.md",
            "CHANGELOG.md",
        ),
        (
            "env",
            "changelog = \"FILE.md\"\n",
            Some(("MOMO_CHANGELOG", "HISTORY.md")),
            "HISTORY.md",
            "FILE.md",
        ),
    ];

    for (label, file, env, written, absent) in cases {
        let dir = TempDir::new().expect("temp dir");
        seed(&dir, file);
        add(&dir, env);

        assert!(
            dir.path().join(written).exists(),
            "{label}: expected {written} to be written"
        );
        if !absent.is_empty() {
            assert!(
                !dir.path().join(absent).exists(),
                "{label}: {absent} should not have been written"
            );
        }
    }
}

#[test]
fn momo_changes_dir_beats_the_file_and_the_default() {
    let fragment = "add-add-a-shiny-thing.md";
    let file_dir = "file-changes";
    let env_dir = "env-changes";
    // (label, momo.toml, env override, written, not written)
    let cases = [
        (
            "default",
            "recording = \"fragments\"\n",
            None,
            format!(".momo/changes/{fragment}"),
            String::new(),
        ),
        (
            "file",
            "recording = \"fragments\"\nchanges_dir = \"file-changes\"\n",
            None,
            format!("{file_dir}/{fragment}"),
            format!(".momo/changes/{fragment}"),
        ),
        (
            "env",
            "recording = \"fragments\"\nchanges_dir = \"file-changes\"\n",
            Some(("MOMO_CHANGES_DIR", env_dir)),
            format!("{env_dir}/{fragment}"),
            format!("{file_dir}/{fragment}"),
        ),
    ];

    for (label, file, env, written, absent) in cases {
        let dir = TempDir::new().expect("temp dir");
        seed(&dir, file);
        add(&dir, env);

        assert!(
            dir.path().join(&written).exists(),
            "{label}: expected {written} to be written"
        );
        if !absent.is_empty() {
            assert!(
                !dir.path().join(&absent).exists(),
                "{label}: {absent} should not have been written"
            );
        }
    }
}

#[test]
fn momo_rc_format_beats_the_file_and_the_default() {
    // (label, momo.toml, env override, the candidate version printed)
    let cases = [
        ("default", "", None, "0.1.0-rc.1"),
        ("file", "rc_format = \"-beta.{n}\"\n", None, "0.1.0-beta.1"),
        (
            "env",
            "rc_format = \"-beta.{n}\"\n",
            Some(("MOMO_RC_FORMAT", "-rc{n}")),
            "0.1.0-rc1",
        ),
    ];

    for (label, file, env, expected) in cases {
        let dir = TempDir::new().expect("temp dir");
        seed(&dir, file);
        add(&dir, None);

        let mut cmd = momo(&dir);
        if let Some((name, value)) = env {
            cmd.env(name, value);
        }
        let assert = cmd.args(["candidate", "--version-only"]).assert().success();
        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

        assert_eq!(stdout, format!("{expected}\n"), "{label}: {stdout}");
    }
}
