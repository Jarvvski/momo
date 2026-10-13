//! Integration tests for `momo install` into a mise task environment.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// The version the built binary pins into its wrappers.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The verbs that get a `momo:*` task wrapper.
const VERBS: [&str; 9] = [
    "break",
    "add",
    "fix",
    "status",
    "release",
    "candidate",
    "tag",
    "init",
    "install",
];

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

/// Read a file that the test expects to exist.
fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("reading {}: {err}", path.display()))
}

/// The last-modified time of a file.
fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .expect("metadata")
        .modified()
        .expect("modified time")
}

#[test]
fn install_writes_wrappers_and_leaves_mise_toml_byte_identical() {
    let dir = TempDir::new().expect("temp dir");
    let mise = "# hand written, keep me\n[tools]\nrust = \"1.99\"\n";
    fs::write(dir.path().join("mise.toml"), mise).expect("seed mise.toml");

    momo(&dir).arg("install").assert().success();

    let status = dir.path().join("mise-tasks/momo/status");
    let text = read(&status);
    assert!(text.starts_with("#!/usr/bin/env bash\n"), "{text}");
    assert!(
        text.contains(&format!(
            "#MISE tools={{ \"github:Jarvvski/momo\" = \"{VERSION}\" }}"
        )),
        "{text}"
    );
    assert!(text.contains("momo status \"$@\""), "{text}");

    assert_eq!(
        read(&dir.path().join("mise.toml")),
        mise,
        "mise.toml was modified"
    );
}

#[test]
fn install_writes_one_wrapper_per_verb() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");

    momo(&dir).arg("install").assert().success();

    for verb in VERBS {
        let path = dir.path().join("mise-tasks/momo").join(verb);
        assert!(path.is_file(), "no wrapper for `{verb}`");
    }
}

#[test]
fn install_records_the_environment_in_config() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");

    momo(&dir).arg("install").assert().success();

    let config = read(&dir.path().join("momo.toml"));
    assert!(config.contains("task_env = \"mise\""), "{config}");
}

#[test]
fn install_twice_changes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");
    momo(&dir).arg("install").assert().success();

    let wrapper = dir.path().join("mise-tasks/momo/status");
    let first_wrapper = read(&wrapper);
    let first_mtime = modified(&wrapper);
    let first_config = read(&dir.path().join("momo.toml"));
    let first_config_mtime = modified(&dir.path().join("momo.toml"));

    momo(&dir).arg("install").assert().success();

    assert_eq!(read(&wrapper), first_wrapper, "wrapper contents changed");
    assert_eq!(modified(&wrapper), first_mtime, "wrapper was rewritten");
    assert_eq!(
        read(&dir.path().join("momo.toml")),
        first_config,
        "config changed"
    );
    assert_eq!(
        modified(&dir.path().join("momo.toml")),
        first_config_mtime,
        "config was rewritten"
    );
}

#[test]
fn install_without_a_task_environment_warns_and_exits_zero() {
    let dir = TempDir::new().expect("temp dir");

    momo(&dir)
        .arg("install")
        .assert()
        .success()
        .stderr(predicate::str::contains("no task environment"));

    assert!(!dir.path().join("mise-tasks").exists());
    assert!(!dir.path().join("momo.toml").exists());
}

#[test]
fn install_takes_an_explicit_environment() {
    let dir = TempDir::new().expect("temp dir");
    // No mise.toml: the explicit override installs anyway.
    momo(&dir).args(["install", "mise"]).assert().success();
    assert!(dir.path().join("mise-tasks/momo/status").is_file());
}

#[test]
fn install_rejects_an_unknown_environment() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["install", "nonsense"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown task environment"));
}

#[test]
fn install_leaves_a_stale_pin_and_update_refreshes_it() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");
    momo(&dir).arg("install").assert().success();

    // Simulate a wrapper left behind by an older momo.
    let wrapper = dir.path().join("mise-tasks/momo/status");
    fs::write(&wrapper, read(&wrapper).replace(VERSION, "0.0.1")).expect("age the pin");

    // Plain install leaves it alone and points at --update...
    momo(&dir)
        .arg("install")
        .assert()
        .success()
        .stderr(predicate::str::contains("--update"));
    assert!(
        read(&wrapper).contains("0.0.1"),
        "install clobbered a stale pin"
    );

    // ...and --update refreshes it to the running version.
    momo(&dir).args(["install", "--update"]).assert().success();
    let refreshed = read(&wrapper);
    assert!(refreshed.contains(VERSION), "{refreshed}");
    assert!(!refreshed.contains("0.0.1"), "{refreshed}");
}

#[test]
fn install_uninstall_removes_what_install_wrote() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");
    momo(&dir).arg("install").assert().success();
    assert!(dir.path().join("mise-tasks/momo").is_dir());
    assert!(read(&dir.path().join("momo.toml")).contains("task_env"));

    momo(&dir)
        .args(["install", "--uninstall"])
        .assert()
        .success();

    assert!(!dir.path().join("mise-tasks/momo").exists());
    assert!(!read(&dir.path().join("momo.toml")).contains("task_env"));
}

#[test]
fn install_dry_run_changes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "").expect("seed mise.toml");

    momo(&dir).args(["install", "--dry-run"]).assert().success();

    assert!(!dir.path().join("mise-tasks").exists());
    assert!(!dir.path().join("momo.toml").exists());
}

#[test]
fn install_update_and_uninstall_conflict() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["install", "--update", "--uninstall"])
        .assert()
        .failure();
}
