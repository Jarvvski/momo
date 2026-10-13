//! Round-trips against real git and jj repositories in a scratch directory.
//!
//! These are the only tests that spawn a VCS CLI; the adapters' unit tests inject
//! a scripted runner instead. The jj test is skipped where jj is not installed.

use std::path::Path;
use std::process::Command;

use crate::adapters::vcs::{CommandRunner, SystemRunner, detect};
use crate::domain::vcs::{VcsError, VcsKind};
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
    // A scratch repository inherits no identity, so set one here rather than
    // relying on the machine's global jj/git config (a clean runner has none).
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.name", "momo"],
    );
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.email", "momo@example.com"],
    );
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

/// The commit id `rev` resolves to, as a full 40-char hex string.
fn commit_id(cwd: &Path, rev: &str) -> String {
    capture(
        cwd,
        "jj",
        &["log", "-r", rev, "--no-graph", "-T", "commit_id"],
    )
}

#[test]
fn jj_tag_anchors_to_the_commit_holding_the_release() {
    if !have("jj") {
        eprintln!("skipping jj_tag_anchors_to_the_commit_holding_the_release: jj is not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    run(dir.path(), "jj", &["git", "init", "--colocate"]);
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.name", "momo"],
    );
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.email", "momo@example.com"],
    );

    // Record the release, then `jj commit`: the release lands in a real commit
    // and `@` becomes a fresh, empty working-copy commit — the exact shape a
    // pull-request merge leaves behind.
    std::fs::write(dir.path().join("CHANGELOG.md"), "# Changelog\n\n## 0.1.0\n").unwrap();
    run(dir.path(), "jj", &["commit", "--message", "release 0.1.0"]);

    let vcs = detect(dir.path()).unwrap();
    assert!(
        !vcs.is_dirty().unwrap(),
        "`@` should be empty after jj commit"
    );

    let version = Version::new(0, 1, 0);
    vcs.create_tag("v", &version, "Release 0.1.0").unwrap();

    // The tag must name the commit that recorded the changelog, not the empty
    // working copy above it.
    let tagged = commit_id(dir.path(), "v0.1.0");
    assert_eq!(tagged, commit_id(dir.path(), "@-"));
    assert_ne!(
        tagged,
        commit_id(dir.path(), "@"),
        "the tag must not point at the empty working-copy commit"
    );
    // That commit is the one whose diff added the changelog...
    assert!(
        capture(dir.path(), "jj", &["diff", "-r", "v0.1.0", "--summary"]).contains("CHANGELOG.md"),
        "the tag must name the commit that holds the changelog"
    );
    // ...and the release description, not the empty working copy's.
    assert_eq!(
        capture(
            dir.path(),
            "jj",
            &[
                "log",
                "-r",
                "v0.1.0",
                "--no-graph",
                "-T",
                "description.first_line()"
            ]
        ),
        "release 0.1.0"
    );
}

#[test]
fn jj_tag_anchors_to_a_dirty_working_copy() {
    if !have("jj") {
        eprintln!("skipping jj_tag_anchors_to_a_dirty_working_copy: jj is not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    run(dir.path(), "jj", &["git", "init", "--colocate"]);
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.name", "momo"],
    );
    run(
        dir.path(),
        "jj",
        &["config", "set", "--repo", "user.email", "momo@example.com"],
    );

    // The release is still uncommitted in `@`; the tag must name it directly.
    std::fs::write(dir.path().join("CHANGELOG.md"), "# Changelog\n\n## 0.1.0\n").unwrap();

    let vcs = detect(dir.path()).unwrap();
    assert!(vcs.is_dirty().unwrap());

    // Capture `@` before tagging: jj advances the working copy once the tag
    // makes the tagged commit immutable, so `@` afterwards is a new commit.
    let release = commit_id(dir.path(), "@");

    let version = Version::new(0, 1, 0);
    vcs.create_tag("v", &version, "Release 0.1.0").unwrap();

    assert_eq!(commit_id(dir.path(), "v0.1.0"), release);
    assert!(
        capture(dir.path(), "jj", &["diff", "-r", "v0.1.0", "--summary"]).contains("CHANGELOG.md"),
        "the tag must name the commit that holds the changelog"
    );
}
