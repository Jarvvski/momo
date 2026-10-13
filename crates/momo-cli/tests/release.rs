//! Integration tests for the Release lifecycle: `release`, `candidate`, `tag`.
//!
//! The release logic is unit-tested with a fake `Vcs`; these drive the real
//! binary, and the repository-backed cases use a scratch jj repo (skipped where
//! jj is not installed).

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

/// Seed a `momo.toml` selecting a recording model.
fn seed_recording(dir: &TempDir, value: &str) {
    fs::write(
        dir.path().join("momo.toml"),
        format!("recording = \"{value}\"\n"),
    )
    .expect("seed momo.toml");
}

/// Read a file under `dir`, panicking when it is missing.
fn read(dir: &TempDir, path: &str) -> String {
    fs::read_to_string(dir.path().join(path)).expect("read file")
}

/// Today's date, `YYYY-MM-DD`, as the binary stamps it.
fn today() -> String {
    time::OffsetDateTime::now_utc().date().to_string()
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

/// A colocated jj repository with a local identity, ready to release from.
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

/// The number of commits the repository holds, including the root.
fn commit_count(cwd: &Path) -> usize {
    capture(
        cwd,
        "jj",
        &[
            "log",
            "--no-graph",
            "-r",
            "all()",
            "-T",
            "commit_id ++ \"\\n\"",
        ],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .count()
}

#[test]
fn release_promotes_the_changelog_with_the_minted_version_and_today() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stdout(predicate::str::contains("released 0.1.0"));

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(
        changelog.contains(&format!("## [0.1.0] - {}", today())),
        "{changelog}"
    );
    assert!(!changelog.contains("## [Unreleased]"), "{changelog}");
    assert!(changelog.contains("- Add a shiny thing"), "{changelog}");
    assert_eq!(read(&dir, ".momo/version"), "0.1.0\n");
}

#[test]
fn release_consumes_fragments() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    assert!(
        dir.path()
            .join(".momo/changes/add-add-a-shiny-thing.md")
            .exists()
    );

    momo(&dir).arg("release").assert().success();

    assert!(
        !dir.path()
            .join(".momo/changes/add-add-a-shiny-thing.md")
            .exists(),
        "release left the Fragment behind"
    );
    let changelog = read(&dir, "CHANGELOG.md");
    assert!(changelog.contains("## [0.1.0]"), "{changelog}");
    assert!(changelog.contains("- Add a shiny thing"), "{changelog}");
}

#[test]
fn release_errors_when_nothing_is_unreleased() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("nothing unreleased"));
}

#[test]
fn tag_errors_when_no_vcs_is_present() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir).arg("release").assert().success();

    momo(&dir)
        .arg("tag")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no VCS detected"));
}

#[test]
fn candidate_numbers_increment_and_restart_when_the_target_changes() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir)
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.1.0-rc.1"));
    momo(&dir)
        .arg("rc")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.1.0-rc.2"));

    // The Fragments survive a Candidate, and the Changelog keeps them Unreleased.
    assert!(read(&dir, "CHANGELOG.md").contains("## [Unreleased]"));

    // A Release drops the suffix and consumes the shared Changes.
    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stdout(predicate::str::contains("released 0.1.0"));

    // The target is now 0.2.0, so the counter restarts at 1.
    momo(&dir)
        .args(["add", "Add another thing"])
        .assert()
        .success();
    momo(&dir)
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.2.0-rc.1"));
}

#[test]
fn release_leaves_the_repository_untouched_but_tag_names_the_released_version() {
    if !have("jj") {
        eprintln!("skipping: jj is not on PATH");
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let before = commit_count(dir.path());
    momo(&dir).arg("release").assert().success();

    // No commit: the changes sit in the working-copy commit, and no tag exists.
    assert_eq!(commit_count(dir.path()), before, "release created a commit");
    assert_eq!(
        capture(dir.path(), "jj", &["tag", "list"]),
        "",
        "release tagged"
    );
    assert!(
        capture(dir.path(), "jj", &["diff", "--summary"]).contains("CHANGELOG.md"),
        "release did not leave the Changelog in the working copy"
    );

    // `tag` creates the tag for the version momo just released...
    momo(&dir)
        .arg("tag")
        .assert()
        .success()
        .stdout(predicate::str::contains("v0.1.0"));
    assert!(
        capture(dir.path(), "jj", &["tag", "list"]).starts_with("v0.1.0:"),
        "tag was not created"
    );

    // ...and refuses to overwrite it.
    momo(&dir)
        .arg("tag")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // The next Release takes its version from the tag the last `tag` created.
    momo(&dir)
        .args(["add", "Add another thing"])
        .assert()
        .success();
    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stdout(predicate::str::contains("released 0.2.0"));
}
