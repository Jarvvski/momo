//! Integration tests for the CI-facing output surface: `--format json`,
//! `--version-only`, `--output`, and `status --check`.
//!
//! These drive the real binary. A project is created in a directory named
//! `proj` so the reported Package name is deterministic.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
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

/// The CI variables momo must never read.
const CI_VARS: [(&str, &str); 6] = [
    ("CI", "true"),
    ("GITHUB_ACTIONS", "true"),
    ("GITHUB_REF", "refs/heads/main"),
    ("GITHUB_SHA", "deadbeef"),
    ("GITHUB_EVENT_NAME", "push"),
    ("GITHUB_TOKEN", "not-a-real-token"),
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

/// Read a file the test expects a run to have written.
fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read a written file")
}

#[test]
fn status_format_json_emits_the_documented_shape() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let json = stdout(&proj, &["status", "--format", "json"]);

    assert_eq!(
        json,
        r#"{
  "unreleased": true,
  "packages": [
    {
      "name": "proj",
      "current": "0.0.0",
      "next": "0.1.0",
      "changes": [
        {
          "kind": "add",
          "text": "Add a shiny thing"
        }
      ]
    }
  ]
}
"#
    );
}

#[test]
fn status_format_json_is_stable_across_runs() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["fix", "Fix a nasty bug"])
        .assert()
        .success();

    assert_eq!(
        stdout(&proj, &["status", "--format", "json"]),
        stdout(&proj, &["status", "--format", "json"])
    );
}

#[test]
fn status_format_json_reports_nothing_unreleased() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);

    let json = stdout(&proj, &["status", "--format", "json"]);

    assert!(json.contains("\"unreleased\": false"), "{json}");
    assert!(json.contains("\"next\": \"0.0.0\""), "{json}");
    assert!(json.contains("\"changes\": []"), "{json}");
}

#[test]
fn status_version_only_prints_just_the_version() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);

    assert_eq!(stdout(&proj, &["status", "--version-only"]), "0.0.0\n");

    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    assert_eq!(stdout(&proj, &["status", "--version-only"]), "0.1.0\n");
}

#[test]
fn status_check_exits_non_zero_when_nothing_is_unreleased() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);

    momo(&proj)
        .args(["status", "--check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nothing unreleased"));
}

#[test]
fn status_check_exits_zero_when_something_is_unreleased() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&proj)
        .args(["status", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("→ next: 0.1.0"));
}

#[test]
fn status_check_still_renders_the_requested_format() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);

    // Nothing unreleased: the gate fails, but the JSON is still on stdout.
    momo(&proj)
        .args(["status", "--check", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"unreleased\": false"));
}

#[test]
fn output_writes_the_same_content_as_stdout() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    for args in [
        &["status"][..],
        &["status", "--format", "json"][..],
        &["status", "--version-only"][..],
    ] {
        let expected = stdout(&proj, args);
        let path = dir.path().join(format!("out-{}", args.len()));
        let path_arg = path.to_string_lossy().into_owned();

        let mut with_output: Vec<&str> = args.to_vec();
        with_output.push("--output");
        with_output.push(&path_arg);
        momo(&proj).args(&with_output).assert().success();

        assert_eq!(read(&path), expected, "`--output` diverged for {args:?}");
    }
}

#[test]
fn output_leaves_stdout_empty() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    let path = dir.path().join("status.json");

    momo(&proj)
        .args(["status", "--format", "json", "--output"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    assert!(read(&path).starts_with("{\n"), "the file did not hold JSON");
}

#[test]
fn release_version_only_prints_the_minted_version() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    assert_eq!(stdout(&proj, &["release", "--version-only"]), "0.1.0\n");
}

#[test]
fn candidate_version_only_prints_the_candidate_version() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    assert_eq!(
        stdout(&proj, &["candidate", "--version-only"]),
        "0.1.0-rc.1\n"
    );
}

#[test]
fn release_format_json_reports_the_minted_version() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let json = stdout(&proj, &["release", "--format", "json"]);

    assert!(json.starts_with("{\n"), "{json}");
    assert!(json.contains("\"version\": \"0.1.0\""), "{json}");
    assert!(json.contains("\"dry_run\": false"), "{json}");
    assert!(json.contains("CHANGELOG.md"), "{json}");
}

#[test]
fn tag_output_writes_the_tag_to_a_file() {
    if !require_tool("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    jj_repo(&proj);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&proj).arg("release").assert().success();
    let path = dir.path().join("tag.json");

    momo(&proj)
        .args(["tag", "--format", "json", "--output"])
        .arg(&path)
        .assert()
        .success();

    let json = read(&path);
    assert!(json.contains("\"tag\": \"v0.1.0\""), "{json}");
    assert!(json.contains("\"version\": \"0.1.0\""), "{json}");
}

#[test]
fn no_ci_environment_variable_changes_the_output() {
    let dir = TempDir::new().expect("temp dir");
    let proj = project(&dir);
    momo(&proj)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    for args in [&["status"][..], &["status", "--format", "json"][..]] {
        let mut plain_cmd = momo(&proj);
        for (name, _) in CI_VARS {
            plain_cmd.env_remove(name);
        }
        let plain_assert = plain_cmd.args(args).assert().success();
        let plain = String::from_utf8_lossy(&plain_assert.get_output().stdout).into_owned();

        let mut ci = momo(&proj);
        for (name, value) in CI_VARS {
            ci.env(name, value);
        }
        let assert = ci.args(args).assert().success();
        let sniffed = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();

        assert_eq!(sniffed, plain, "CI variables changed `{args:?}`");
    }
}

/// Run `program args` in `cwd`, asserting success.
fn run(cwd: &Path, program: &str, args: &[&str]) {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {program}: {err}"));
    assert!(
        output.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A colocated jj repository with a local identity, ready to release from.
fn jj_repo(cwd: &Path) {
    run(cwd, "jj", &["git", "init", "--colocate"]);
    run(cwd, "jj", &["config", "set", "--repo", "user.name", "momo"]);
    run(
        cwd,
        "jj",
        &["config", "set", "--repo", "user.email", "momo@example.com"],
    );
    fs::write(cwd.join("a.txt"), "hi\n").expect("seed a tracked file");
}
