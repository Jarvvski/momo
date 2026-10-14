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

mod common;

use common::{release_date, require_tool, today_or_yesterday};

/// A `momo` command rooted at `dir`, with every `MOMO_*` variable cleared.
fn momo(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("momo").expect("the momo binary is built");
    cmd.current_dir(dir.path());
    for name in [
        "MOMO_RECORDING",
        "MOMO_CHANGELOG",
        "MOMO_CHANGES_DIR",
        "MOMO_TAG_PREFIX",
        "MOMO_REMOTE",
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

/// A git repository with a local identity, ready to release from.
fn git_repo(dir: &TempDir) {
    capture(dir.path(), "git", &["init", "--quiet"]);
    capture(
        dir.path(),
        "git",
        &["config", "user.email", "momo@example.com"],
    );
    capture(dir.path(), "git", &["config", "user.name", "momo"]);
    fs::write(dir.path().join("a.txt"), "hi\n").expect("seed a tracked file");
    capture(dir.path(), "git", &["add", "a.txt"]);
    capture(
        dir.path(),
        "git",
        &["commit", "--quiet", "--message", "first"],
    );
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
    let date = release_date(&changelog, "0.1.0");
    assert!(
        today_or_yesterday().contains(&date),
        "release stamped `{date}`, not today or yesterday UTC:\n{changelog}"
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
fn release_preserves_a_hand_written_changelog_body_and_footer() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(
        dir.path().join("momo.toml"),
        "remote = \"git@github.com:Jarvvski/momo.git\"\n",
    )
    .expect("seed momo.toml");
    fs::create_dir_all(dir.path().join(".momo")).expect("create .momo");
    fs::write(dir.path().join(".momo/version"), "1.0.0\n").expect("seed the version");
    // The `### Changed` sits in a *released* section: a historical heading is
    // preserved and never parsed, so it must not block the release.
    let seeded = "\
# Changelog

## [Unreleased]

### Added

- a shiny thing

## [1.0.0] - 2026-01-01

### Changed

- Reworked the parser

### Added

- old

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");

    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stdout(predicate::str::contains("released 1.1.0"));

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(changelog.contains("## [1.1.0]"), "{changelog}");
    assert!(
        changelog.contains("### Changed\n\n- Reworked the parser"),
        "the released `### Changed` section was dropped:\n{changelog}"
    );
    assert!(changelog.contains("- a shiny thing"), "{changelog}");
    assert!(
        changelog.contains("[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/"),
        "the user footer definition was dropped:\n{changelog}"
    );
    assert!(
        changelog.contains("[1.1.0]: https://github.com/Jarvvski/momo/compare/v1.0.0...v1.1.0"),
        "the regenerated footer is missing:\n{changelog}"
    );
    assert!(!changelog.contains("## [Unreleased]"), "{changelog}");
}

#[test]
fn release_in_fragments_mode_refuses_a_changelog_unreleased_section() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    let seeded = "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- a hand-written fix\n";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");
    fs::create_dir_all(dir.path().join(".momo/changes")).expect("create the changes dir");
    fs::write(
        dir.path().join(".momo/changes/add-a-thing.md"),
        "Add a thing\n",
    )
    .expect("seed a fragment");

    momo(&dir)
        .arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unreleased").and(predicate::str::contains("fragments")));

    assert_eq!(read(&dir, "CHANGELOG.md"), seeded);
    assert!(
        dir.path().join(".momo/changes/add-a-thing.md").exists(),
        "a refused release consumed the fragment"
    );
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
fn release_refuses_an_unreleased_heading_momo_cannot_read() {
    let dir = TempDir::new().expect("temp dir");
    let seeded = "\
# Changelog

## [Unreleased]

### Changed

- Reworked the parser
- And again

### Added

- a shiny thing
";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");

    momo(&dir).arg("release").assert().failure().stderr(
        predicate::str::contains("momo.changelog.unversioned-section")
            .and(predicate::str::contains("`### Changed`"))
            .and(predicate::str::contains("2 entries"))
            .and(predicate::str::contains("(line 5)")),
    );

    assert_eq!(
        read(&dir, "CHANGELOG.md"),
        seeded,
        "a refused release rewrote the changelog"
    );
    assert!(
        !dir.path().join(".momo/version").exists(),
        "a refused release advanced the ledger"
    );
}

#[test]
fn candidate_refuses_an_unreleased_heading_momo_cannot_read() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(
        dir.path().join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n### Security\n\n- A fix\n\n### Fixed\n\n- a bug\n",
    )
    .expect("seed the changelog");

    momo(&dir)
        .arg("candidate")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "momo.changelog.unversioned-section",
        ));
}

#[test]
fn status_surfaces_an_unreleased_heading_momo_cannot_read() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(
        dir.path().join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n### Changed\n\n- Reworked the parser\n\n### Added\n\n- a thing\n",
    )
    .expect("seed the changelog");

    momo(&dir)
        .args(["status", "--format", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"unversioned\"")
                .and(predicate::str::contains("\"heading\": \"Changed\""))
                .and(predicate::str::contains("\"entries\": 1")),
        );

    momo(&dir).arg("status").assert().success().stdout(
        predicate::str::contains("Unversioned").and(predicate::str::contains("`### Changed`")),
    );
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
fn a_custom_rc_format_with_a_suffix_advances_the_counter() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .env("MOMO_RC_FORMAT", "-{n}.rc")
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir)
        .env("MOMO_RC_FORMAT", "-{n}.rc")
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.1.0-1.rc"));
    momo(&dir)
        .env("MOMO_RC_FORMAT", "-{n}.rc")
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.1.0-2.rc"));
}

#[test]
fn an_rc_format_without_a_marker_is_rejected() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .env("MOMO_RC_FORMAT", "-rc")
        .arg("candidate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("rc_format"));
}

#[test]
fn release_leaves_the_repository_untouched_but_tag_names_the_released_version() {
    if !require_tool("jj") {
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

    // `tag` creates the tag for the version momo just released. The release is
    // uncommitted, so it names `@` explicitly — the default anchor is `@-`.
    momo(&dir)
        .args(["tag", "--rev", "@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v0.1.0"));
    assert!(
        capture(dir.path(), "jj", &["tag", "list"]).starts_with("v0.1.0:"),
        "tag was not created"
    );

    // ...and refuses to overwrite it.
    momo(&dir)
        .args(["tag", "--rev", "@"])
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

#[test]
fn a_candidate_can_be_tagged_and_the_final_release_drops_the_suffix() {
    if !require_tool("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    // Cut a Candidate and tag it — tagging the pre-release is the point of an RC.
    momo(&dir)
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 0.1.0-rc.1"));
    momo(&dir)
        .args(["tag", "--rev", "@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v0.1.0-rc.1"));
    assert!(
        capture(dir.path(), "jj", &["tag", "list"]).starts_with("v0.1.0-rc.1:"),
        "the Candidate was not tagged"
    );

    // The final Release reuses the same target and drops the suffix.
    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stdout(predicate::str::contains("released 0.1.0"));
}

#[test]
fn init_version_then_tag_creates_the_initial_tag() {
    if !require_tool("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);

    momo(&dir)
        .args(["init", "--version", "1.2.3"])
        .assert()
        .success();

    momo(&dir)
        .args(["tag", "--rev", "@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v1.2.3"));
    assert!(
        capture(dir.path(), "jj", &["tag", "list"]).starts_with("v1.2.3:"),
        "the initial tag was not created"
    );
}

/// Write the pending-release ledger under `dir`.
fn write_ledger(dir: &TempDir, version: &str) {
    fs::create_dir_all(dir.path().join(".momo")).expect("create .momo");
    fs::write(dir.path().join(".momo/version"), format!("{version}\n")).expect("write the ledger");
}

#[test]
fn status_reads_the_tag_while_tag_reads_the_ledger() {
    let dir = TempDir::new().expect("temp dir");
    git_repo(&dir);
    // The repository is at tag `v1.2.3`; the ledger records a minted `1.3.0`.
    capture(dir.path(), "git", &["tag", "v1.2.3"]);
    write_ledger(&dir, "1.3.0");

    // `status` reads the tag source...
    momo(&dir)
        .args(["status", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"current\": \"1.2.3\""));

    // ...and `tag` reads the ledger, which no tag records yet.
    momo(&dir)
        .arg("tag")
        .assert()
        .success()
        .stdout(predicate::str::contains("v1.3.0"));
    assert_eq!(
        capture(dir.path(), "git", &["tag", "--list"]),
        "v1.2.3\nv1.3.0"
    );
}

#[test]
fn tag_refuses_a_ledger_older_than_the_newest_tag() {
    let dir = TempDir::new().expect("temp dir");
    git_repo(&dir);
    capture(dir.path(), "git", &["tag", "v1.2.3"]);
    write_ledger(&dir, "0.9.0");

    momo(&dir).arg("tag").assert().failure().stderr(
        predicate::str::contains("older than the newest tag")
            .and(predicate::str::contains("1.2.3")),
    );
    assert_eq!(capture(dir.path(), "git", &["tag", "--list"]), "v1.2.3");
}

#[test]
fn status_distinguishes_a_never_released_package_from_a_zero_release() {
    // Never released: no tags, so `current` is null.
    let fresh = TempDir::new().expect("temp dir");
    git_repo(&fresh);
    momo(&fresh)
        .args(["status", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"current\": null"));

    // A genuine `0.0.0` Release is a version, not an absence.
    let zero = TempDir::new().expect("temp dir");
    git_repo(&zero);
    capture(zero.path(), "git", &["tag", "v0.0.0"]);
    momo(&zero)
        .args(["status", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"current\": \"0.0.0\""));
}

#[test]
fn a_break_after_a_tagged_candidate_refuses_the_stale_release() {
    if !require_tool("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);

    // Start at a released `1.0.0`.
    momo(&dir)
        .args(["init", "--version", "1.0.0"])
        .assert()
        .success();
    momo(&dir).arg("tag").assert().success();

    // Cut and tag a Candidate from an Add.
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir)
        .arg("candidate")
        .assert()
        .success()
        .stdout(predicate::str::contains("cut 1.1.0-rc.1"));
    momo(&dir)
        .arg("tag")
        .assert()
        .success()
        .stdout(predicate::str::contains("v1.1.0-rc.1"));

    // A Break now implies `2.0.0` from `1.0.0`, so the cut Candidate is stale.
    momo(&dir)
        .args(["break", "Break a thing"])
        .assert()
        .success();
    momo(&dir).arg("release").assert().failure().stderr(
        predicate::str::contains("1.1.0-rc.1").and(predicate::str::contains("cut a new candidate")),
    );
}

#[test]
fn tag_defaults_to_the_committed_release_at_parent() {
    if !require_tool("jj") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    jj_repo(&dir);
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir).arg("release").assert().success();

    // The documented flow: commit the release, then tag. `@` is now the empty
    // working-copy commit and `@-` is the release.
    capture(dir.path(), "jj", &["commit", "--message", "release 0.1.0"]);
    // A stray file makes `@` dirty for a reason unrelated to the release.
    fs::write(dir.path().join("stray.txt"), "unrelated\n").expect("seed a stray file");
    let release = capture(
        dir.path(),
        "jj",
        &["log", "-r", "@-", "--no-graph", "-T", "commit_id"],
    );

    momo(&dir).arg("tag").assert().success();

    let tagged = capture(
        dir.path(),
        "jj",
        &["log", "-r", "v0.1.0", "--no-graph", "-T", "commit_id"],
    );
    assert_eq!(tagged, release, "the tag must name the committed release");
}

#[test]
fn a_remote_key_in_the_config_writes_the_changelog_footer() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(
        dir.path().join("momo.toml"),
        "remote = \"git@github.com:Jarvvski/momo.git\"\n",
    )
    .expect("seed momo.toml");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir).arg("release").assert().success();

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(
        changelog.contains("[0.1.0]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0"),
        "{changelog}"
    );
    assert!(!changelog.contains("[Unreleased]:"), "{changelog}");
}

#[test]
fn a_vcs_remote_momo_cannot_read_warns_but_still_releases() {
    let dir = TempDir::new().expect("temp dir");
    git_repo(&dir);
    capture(
        dir.path(),
        "git",
        &["remote", "add", "origin", "not-a-remote"],
    );
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir)
        .arg("release")
        .assert()
        .success()
        .stderr(predicate::str::contains("not recognised"));

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(changelog.contains("## [0.1.0]"), "{changelog}");
    assert!(!changelog.contains("[0.1.0]:"), "{changelog}");
}

#[test]
fn an_unparseable_remote_in_the_config_fails_the_run() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("momo.toml"), "remote = \"not a remote\"\n").expect("seed momo.toml");

    momo(&dir)
        .arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("remote").and(predicate::str::contains("not a remote")));
}

#[test]
fn an_unparseable_remote_in_the_environment_fails_the_run() {
    let dir = TempDir::new().expect("temp dir");

    momo(&dir)
        .env("MOMO_REMOTE", "not a remote")
        .arg("status")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("MOMO_REMOTE").and(predicate::str::contains("not a remote")),
        );
}

/// Every staging file left anywhere under `dir`.
fn staging_files(dir: &TempDir) -> Vec<String> {
    fn walk(path: &Path, found: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".tmp"))
            {
                found.push(path.display().to_string());
            }
        }
    }

    let mut found = Vec::new();
    walk(dir.path(), &mut found);
    found.sort();
    found
}

#[test]
fn a_failed_manifest_parse_promotes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    fs::write(dir.path().join("Cargo.toml"), "version = \n").expect("seed an invalid manifest");

    momo(&dir)
        .arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cargo.toml"));

    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "the changelog was promoted"
    );
    assert!(
        !dir.path().join(".momo/version").exists(),
        "the version file was advanced"
    );
    assert!(
        dir.path()
            .join(".momo/changes/add-add-a-shiny-thing.md")
            .exists(),
        "the fragment was consumed"
    );
    assert_eq!(staging_files(&dir), Vec::<String>::new());

    // The retry finishes the job once the manifest is valid.
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.0.0\"\n",
    )
    .expect("fix the manifest");
    momo(&dir).arg("release").assert().success();

    assert!(read(&dir, "CHANGELOG.md").contains("## [0.1.0]"));
    assert_eq!(
        read(&dir, "Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.1.0\"\n"
    );
}

#[test]
fn a_dry_run_rejects_an_unparseable_manifest() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    fs::write(dir.path().join("Cargo.toml"), "version = \n").expect("seed an invalid manifest");

    momo(&dir)
        .args(["release", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cargo.toml"));

    assert!(!dir.path().join(".momo/version").exists());
}

#[test]
fn a_dry_run_reports_the_manifest_without_writing_it() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    let seeded = "[package]\nname = \"proj\"\nversion = \"0.0.0\"\n";
    fs::write(dir.path().join("Cargo.toml"), seeded).expect("seed the manifest");

    momo(&dir)
        .args(["release", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("would write").and(predicate::str::contains("Cargo.toml")),
        );

    assert_eq!(
        read(&dir, "Cargo.toml"),
        seeded,
        "the dry run wrote the manifest"
    );
    assert!(!dir.path().join(".momo/version").exists());
}

#[cfg(unix)]
#[test]
fn an_unwritable_directory_fails_before_anything_is_promoted() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555))
        .expect("make the project read-only");
    let failed = momo(&dir).arg("release").assert().failure();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755))
        .expect("restore the project");

    failed.stderr(predicate::str::contains("writing"));
    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "the changelog was promoted"
    );
    assert!(
        !dir.path().join(".momo/version").exists(),
        "the version file was advanced"
    );
    assert!(
        dir.path()
            .join(".momo/changes/add-add-a-shiny-thing.md")
            .exists(),
        "the fragment was consumed"
    );
    assert_eq!(staging_files(&dir), Vec::<String>::new());

    // With the directory writable again, the retry finishes the release.
    momo(&dir).arg("release").assert().success();
    assert!(read(&dir, "CHANGELOG.md").contains("## [0.1.0]"));
    assert_eq!(read(&dir, ".momo/version"), "0.1.0\n");
}

#[test]
fn an_inherited_manifest_version_fails_the_release() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"proj\"\nversion.workspace = true\n",
    )
    .expect("seed an inherited manifest");

    momo(&dir)
        .arg("release")
        .assert()
        .failure()
        .stderr(predicate::str::contains("inherits its version"));

    assert!(!dir.path().join("CHANGELOG.md").exists());
    assert!(!dir.path().join(".momo/version").exists());
    assert!(
        dir.path()
            .join(".momo/changes/add-add-a-shiny-thing.md")
            .exists()
    );
}
