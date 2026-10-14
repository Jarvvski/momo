//! The golden path: a bare `git init` repository with **no** `momo.toml`.
//!
//! This pins zero-config as a contract, not a claim. The path — `add` → `status`
//! → `release --dry-run` → `release` → `tag` — runs against the built-in
//! defaults alone: the test writes no configuration, and asserts the versions
//! the path produces and that the only `momo.toml` that appears is the one momo
//! generates from those defaults.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use tempfile::TempDir;

mod common;

use common::require_tool;

/// The `MOMO_*` variables the CLI reads; every test clears them.
const MOMO_VARS: [&str; 7] = [
    "MOMO_RECORDING",
    "MOMO_CHANGELOG",
    "MOMO_CHANGES_DIR",
    "MOMO_TAG_PREFIX",
    "MOMO_REMOTE",
    "MOMO_TASK_ENV",
    "MOMO_RC_FORMAT",
];

/// A `momo` command rooted at `cwd`, with every `MOMO_*` variable cleared.
fn momo(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("momo").expect("the momo binary is built");
    cmd.current_dir(cwd);
    for name in MOMO_VARS {
        cmd.env_remove(name);
    }
    cmd
}

/// A project directory named `proj` under `dir`, so the Package name is stable.
fn project(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("proj");
    fs::create_dir_all(&path).expect("create the project dir");
    path
}

/// Run `momo args` in `cwd` and return its stdout.
fn stdout(cwd: &Path, args: &[&str]) -> String {
    let assert = momo(cwd).args(args).assert().success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

/// Read a file the path expects a run to have written.
fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read a written file")
}

/// Run `git args` in `cwd`, asserting success, and return its trimmed stdout.
fn git(cwd: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git: {err}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A bare `git init` repository with a local identity — and no `momo.toml`.
fn git_repo(cwd: &Path) {
    git(cwd, &["init", "--quiet"]);
    git(cwd, &["config", "user.email", "momo@example.com"]);
    git(cwd, &["config", "user.name", "momo"]);
    fs::write(cwd.join("a.txt"), "hi\n").expect("seed a tracked file");
    git(cwd, &["add", "a.txt"]);
    git(cwd, &["commit", "--quiet", "--message", "first"]);
}

#[test]
fn the_golden_path_needs_no_configuration() {
    if !require_tool("git") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    git_repo(&proj);
    // The contract under test: nothing below writes a config file by hand.
    assert!(!proj.join("momo.toml").exists());

    // 1. Record a change.
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    // 2. Inspect: `status` reports the current version, the next, and the change.
    let status = stdout(&proj, &["status"]);
    assert!(status.contains("current: —"), "{status}");
    assert!(status.contains("→ next: 0.1.0"), "{status}");
    assert!(status.contains("- Add a shiny thing"), "{status}");

    // 3. `release --dry-run` reports the same version and writes nothing.
    let changelog = read(&proj.join("CHANGELOG.md"));
    let dry_run = stdout(&proj, &["release", "--dry-run"]);
    assert!(dry_run.contains("would release 0.1.0"), "{dry_run}");
    assert_eq!(read(&proj.join("CHANGELOG.md")), changelog);
    assert!(!proj.join(".momo/version").exists());

    // 4. `release` mints it, writes the changelog and records the version.
    let released = stdout(&proj, &["release"]);
    assert!(released.contains("released 0.1.0"), "{released}");
    assert_eq!(read(&proj.join(".momo/version")), "0.1.0\n");
    assert!(read(&proj.join("CHANGELOG.md")).contains("## [0.1.0]"));

    // 5. Commit the release, then tag it.
    git(&proj, &["add", "--all"]);
    git(&proj, &["commit", "--quiet", "--message", "release 0.1.0"]);
    let tagged = stdout(&proj, &["tag"]);
    assert!(tagged.contains("v0.1.0"), "{tagged}");
    assert_eq!(git(&proj, &["tag", "--list"]), "v0.1.0");

    // The released version is now both current and next.
    let after = stdout(&proj, &["status"]);
    assert!(after.contains("current: 0.1.0"), "{after}");
    assert!(after.contains("→ next: 0.1.0"), "{after}");

    // No hand-written config was needed: whatever `momo.toml` exists is the one
    // momo generated from the defaults.
    let config = read(&proj.join("momo.toml"));
    assert!(config.starts_with("# momo configuration."), "{config}");
}
