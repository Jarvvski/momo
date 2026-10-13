//! Integration tests for monorepo discovery and targeting.
//!
//! These drive the real binary against a scratch mise monorepo. Discovery needs
//! `mise` on PATH, and the tagging cases need `jj`; each test skips when the
//! binary it needs is missing.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use predicates::prelude::*;
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
        "MOMO_TASK_ENV",
        "MOMO_RC_FORMAT",
    ] {
        cmd.env_remove(name);
    }
    cmd
}

/// Whether `program` can be spawned at all.
fn have(program: &str) -> bool {
    StdCommand::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Run `program args` in `cwd` and return its trimmed stdout.
fn capture(cwd: &Path, program: &str, args: &[&str]) -> String {
    let output = StdCommand::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {program}: {err}"));
    assert!(
        output.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A two-crate Cargo workspace declared as a mise monorepo.
fn monorepo(dir: &TempDir) {
    fs::write(
        dir.path().join("mise.toml"),
        "monorepo_root = true\n\n[monorepo]\nconfig_roots = [\"crates/*\"]\n",
    )
    .expect("seed mise.toml");
    // A repo marker, so `project_root` resolves from a package directory too.
    fs::write(dir.path().join("momo.toml"), "").expect("seed momo.toml");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
    )
    .expect("seed workspace manifest");

    for name in ["a", "b"] {
        let package = dir.path().join("crates").join(name);
        fs::create_dir_all(package.join("src")).expect("create the crate dir");
        fs::write(
            package.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("seed crate manifest");
        fs::write(package.join("src/lib.rs"), "pub fn f() {}\n").expect("seed crate source");
    }
}

/// A colocated jj repository with a local identity.
fn jj_repo(dir: &TempDir) {
    capture(dir.path(), "jj", &["git", "init", "--colocate"]);
    capture(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.name", "momo"],
    );
    capture(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.email", "momo@example.com"],
    );
    fs::write(dir.path().join("a.txt"), "hi\n").expect("seed a tracked file");
}

/// Skip a test when discovery or tagging is unavailable on this machine.
fn require(program: &str) -> bool {
    if have(program) {
        true
    } else {
        eprintln!("skipping: {program} is not on PATH");
        false
    }
}

#[test]
fn status_reports_each_package_and_its_next_version() {
    if !require("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();

    let assert = momo(&dir).arg("status").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(stdout.contains("  cargo:a"), "{stdout}");
    assert!(stdout.contains("  cargo:b"), "{stdout}");
    // cargo:a has an Unreleased Change; cargo:b has none.
    assert!(stdout.contains("- Add a thing"), "{stdout}");
    assert!(stdout.contains("→ next: 0.1.0"), "{stdout}");
    assert!(stdout.contains("→ next: 0.0.0"), "{stdout}");
}

#[test]
fn package_scopes_a_command_and_all_covers_every_package() {
    if !require("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);

    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "release"])
        .assert()
        .success();
    assert!(dir.path().join("crates/a/CHANGELOG.md").exists());
    assert!(
        !dir.path().join("crates/b/CHANGELOG.md").exists(),
        "-p cargo:a released cargo:b too"
    );

    momo(&dir)
        .args(["-p", "cargo:b", "add", "Add b"])
        .assert()
        .success();
    momo(&dir).args(["--all", "release"]).assert().success();
    assert!(dir.path().join("crates/b/CHANGELOG.md").exists());
}

#[test]
fn a_recording_verb_defaults_to_the_current_directory() {
    if !require("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);

    let mut cmd = momo(&dir);
    cmd.current_dir(dir.path().join("crates/a"));
    cmd.args(["add", "Add a thing"]).assert().success();

    assert!(
        dir.path().join("crates/a/CHANGELOG.md").exists(),
        "the change did not land in the package containing the cwd"
    );
    assert!(!dir.path().join("crates/b/CHANGELOG.md").exists());
    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "the change landed at the repo root instead of the package"
    );
}

#[test]
fn an_unknown_package_is_rejected() {
    if !require("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);

    momo(&dir)
        .args(["-p", "cargo:zzz", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no package matches"));
}

#[test]
fn fragments_are_stored_per_package() {
    if !require("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    fs::write(dir.path().join("momo.toml"), "recording = \"fragments\"\n").expect("seed config");

    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();

    assert!(
        dir.path()
            .join(".momo/changes/cargo-a/add-add-a-thing.md")
            .exists(),
        "the Fragment did not land under the package slug"
    );
    assert!(
        !dir.path()
            .join(".momo/changes/cargo-b/add-add-a-thing.md")
            .exists(),
        "the Fragment leaked into another package"
    );
}

#[test]
fn tags_are_created_per_package_as_slug_v_version() {
    if !require("mise") || !require("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);
    monorepo(&dir);

    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "release"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "tag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo-a-v0.1.0"));

    assert!(
        capture(dir.path(), "jj", &["tag", "list"]).starts_with("cargo-a-v0.1.0:"),
        "the per-package tag was not created"
    );
}

#[test]
fn a_configured_slug_overrides_the_derived_one() {
    if !require("mise") || !require("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);
    monorepo(&dir);
    fs::write(
        dir.path().join("momo.toml"),
        "[packages.\"cargo:a\"]\nslug = \"alpha\"\n",
    )
    .expect("seed config");

    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "release"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "tag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha-v0.1.0"));
}

#[test]
fn discovery_failure_degrades_to_a_single_package_with_a_warning() {
    let dir = TempDir::new().expect("temp dir");
    // A task environment with no `monorepo_root`, so discovery cannot succeed.
    fs::write(dir.path().join("mise.toml"), "[tools]\n").expect("seed mise.toml");

    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stderr(predicate::str::contains("single-package"));

    // The single-package fallback still records and reports as before.
    momo(&dir).args(["add", "Add a thing"]).assert().success();
    assert!(dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn a_repo_without_a_task_environment_is_a_single_package() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir).args(["add", "Add a thing"]).assert().success();
    momo(&dir).arg("release").assert().success();

    // The single-package layout is unchanged: the Changelog and version file sit
    // at the repo root, not under a package directory.
    let changelog = fs::read_to_string(dir.path().join("CHANGELOG.md")).expect("changelog written");
    assert!(changelog.contains("## [0.1.0]"), "{changelog}");
    assert_eq!(
        fs::read_to_string(dir.path().join(".momo/version")).expect("version written"),
        "0.1.0\n"
    );

    // `status` names the single Package after the repo directory.
    let name = dir
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("the temp dir has a name");
    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("  {name}")));
}
