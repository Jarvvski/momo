//! Round-trips against real git and jj repositories in a scratch directory.
//!
//! These are the only tests that spawn a VCS CLI; the adapters' unit tests inject
//! a scripted runner instead. The jj test is skipped where jj is not installed.

use std::path::Path;
use std::process::Command;

use momo_core::vcs::{VcsError, VcsKind};
use momo_vcs::{CommandRunner, SystemRunner, detect};
use semver::Version;
use tempfile::TempDir;

/// Run `program args` in `cwd`, asserting it succeeds.
fn run(cwd: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
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

/// Run `program args` in `cwd` and return its trimmed stdout.
fn capture(cwd: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
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

/// Whether `program` can be spawned at all.
fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn a_missing_binary_is_reported_as_a_spawn_error() {
    let dir = TempDir::new().unwrap();

    let err = SystemRunner
        .run("momo-definitely-not-a-real-binary", &[], dir.path())
        .unwrap_err();
    assert!(matches!(err, VcsError::Spawn { .. }));
}

#[test]
fn git_round_trip() {
    let dir = TempDir::new().unwrap();
    run(dir.path(), "git", &["init", "--quiet"]);
    run(
        dir.path(),
        "git",
        &["config", "user.email", "momo@example.com"],
    );
    run(dir.path(), "git", &["config", "user.name", "momo"]);
    std::fs::write(dir.path().join("a.txt"), "hi\n").unwrap();
    run(dir.path(), "git", &["add", "a.txt"]);
    run(
        dir.path(),
        "git",
        &["commit", "--quiet", "--message", "first"],
    );

    let vcs = detect(dir.path()).unwrap();
    assert_eq!(vcs.kind(), VcsKind::Git);
    assert_eq!(vcs.latest_tag("v").unwrap(), None);
    assert!(!vcs.is_dirty().unwrap());

    let version = Version::new(0, 1, 0);
    vcs.create_tag("v", &version, "Release 0.1.0").unwrap();

    assert_eq!(vcs.latest_tag("v").unwrap().as_ref(), Some(&version));
    // `cat-file -t` reports `tag` only for an annotated tag.
    assert_eq!(
        capture(dir.path(), "git", &["cat-file", "-t", "refs/tags/v0.1.0"]),
        "tag"
    );

    let err = vcs.create_tag("v", &version, "Release 0.1.0").unwrap_err();
    assert!(matches!(err, VcsError::TagAlreadyExists { ref tag } if tag == "v0.1.0"));
    assert_eq!(err.to_string(), "tag `v0.1.0` already exists");

    std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    assert!(vcs.is_dirty().unwrap());
}

#[test]
fn jj_round_trip() {
    if !have("jj") {
        eprintln!("skipping jj_round_trip: jj is not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    run(dir.path(), "jj", &["git", "init", "--colocate"]);
    std::fs::write(dir.path().join("a.txt"), "hi\n").unwrap();

    let vcs = detect(dir.path()).unwrap();
    assert_eq!(vcs.kind(), VcsKind::Jj);
    assert_eq!(vcs.latest_tag("v").unwrap(), None);

    let version = Version::new(0, 1, 0);
    vcs.create_tag("v", &version, "Release 0.1.0").unwrap();

    assert_eq!(vcs.latest_tag("v").unwrap().as_ref(), Some(&version));
    // jj has no annotated tags: the tag reaches git as a lightweight ref, which
    // points straight at a commit.
    let git_root = capture(dir.path(), "jj", &["git", "root"]);
    assert_eq!(
        capture(
            Path::new(&git_root),
            "git",
            &["cat-file", "-t", "refs/tags/v0.1.0"]
        ),
        "commit"
    );

    let err = vcs.create_tag("v", &version, "Release 0.1.0").unwrap_err();
    assert!(matches!(err, VcsError::TagAlreadyExists { ref tag } if tag == "v0.1.0"));
    assert_eq!(err.to_string(), "tag `v0.1.0` already exists");

    // jj snapshots the working copy on every command, so the edit is visible.
    std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    assert!(vcs.is_dirty().unwrap());
}
