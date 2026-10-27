//! Integration tests for the recording verbs, `status`, and the version source.

use std::fs;

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

#[test]
fn add_records_a_change_under_unreleased() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(changelog.contains("## [Unreleased]"), "{changelog}");
    assert!(changelog.contains("### Added"), "{changelog}");
    assert!(changelog.contains("- Add a shiny thing"), "{changelog}");
}

#[test]
fn a_change_verb_rejects_empty_text() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicate::str::contains("recording `add`"));

    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "an empty change wrote a changelog"
    );
    assert!(
        !dir.path().join("momo.toml").exists(),
        "a rejected change created the config"
    );
}

#[test]
fn a_change_verb_flattens_an_injected_section_heading() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "x\n## [9.9.9] - 2000-01-01"])
        .assert()
        .success();

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(
        changelog.contains("- x ## [9.9.9] - 2000-01-01"),
        "{changelog}"
    );
    assert!(
        !changelog.lines().any(|line| line.starts_with("## [9.9.9]")),
        "a Change forged a released section:\n{changelog}"
    );
}

#[test]
fn each_verb_records_its_kind() {
    let dir = TempDir::new().expect("temp dir");
    for (verb, heading, text) in [
        ("break", "### Breaking", "a break"),
        ("add", "### Added", "an add"),
        ("fix", "### Fixed", "a fix"),
    ] {
        momo(&dir).args([verb, text]).assert().success();

        let changelog = read(&dir, "CHANGELOG.md");
        let at = changelog
            .find(heading)
            .unwrap_or_else(|| panic!("`{heading}` missing:\n{changelog}"));
        assert!(
            changelog[at..].contains(text),
            "`{verb}` did not land under {heading}:\n{changelog}"
        );
    }
}
#[test]
fn a_recording_verb_preserves_the_document_it_does_not_own() {
    let dir = TempDir::new().expect("temp dir");
    let seeded = "\
# Changelog

## [Unreleased]

<!-- keep this note -->

### Changed

- Reworked the parser
* a star bullet

## [1.0.0] - 2026-01-01

### Added

- old

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");

    momo(&dir).args(["fix", "a fix"]).assert().success();

    let changelog = read(&dir, "CHANGELOG.md");
    assert!(changelog.contains("<!-- keep this note -->"), "{changelog}");
    assert!(
        changelog.contains("### Changed\n\n- Reworked the parser"),
        "the `### Changed` section was dropped:\n{changelog}"
    );
    assert!(changelog.contains("* a star bullet"), "{changelog}");
    assert!(
        changelog.contains("[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/"),
        "the user footer definition was dropped:\n{changelog}"
    );
    assert!(changelog.contains("### Fixed\n\n- a fix"), "{changelog}");
    assert!(changelog.contains("## [1.0.0] - 2026-01-01"), "{changelog}");
}

#[test]
fn a_duplicate_unreleased_section_is_refused_naming_the_line() {
    let dir = TempDir::new().expect("temp dir");
    let seeded = "# Changelog\n\n## [Unreleased]\n\n- one\n\n## [Unreleased]\n\n- two\n";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");

    momo(&dir)
        .args(["add", "a thing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 7"));

    assert_eq!(
        read(&dir, "CHANGELOG.md"),
        seeded,
        "a refused record rewrote the changelog"
    );
}

#[test]
fn a_failed_record_leaves_no_generated_config() {
    let dir = TempDir::new().expect("temp dir");
    let seeded = "# Changelog\n\n## [Unreleased]\n\n- one\n\n## [Unreleased]\n\n- two\n";
    fs::write(dir.path().join("CHANGELOG.md"), seeded).expect("seed the changelog");

    momo(&dir).args(["add", "a thing"]).assert().failure();

    assert!(
        !dir.path().join("momo.toml").exists(),
        "a record that failed at runtime created the config"
    );
}

#[test]
fn fragments_mode_writes_one_fragment_per_change() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");

    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["fix", "Fix a nasty bug"])
        .assert()
        .success();

    assert_eq!(
        read(&dir, ".momo/changes/add-add-a-shiny-thing.md"),
        "Add a shiny thing\n"
    );
    assert_eq!(
        read(&dir, ".momo/changes/fix-fix-a-nasty-bug.md"),
        "Fix a nasty bug\n"
    );
    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "fragments mode wrote a changelog"
    );
}

#[test]
fn status_lists_unreleased_changes_and_ends_with_the_next_version() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["fix", "Fix a nasty bug"])
        .assert()
        .success();

    let assert = momo(&dir).arg("status").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("- Add a shiny thing"), "{stdout}");
    assert!(stdout.contains("- Fix a nasty bug"), "{stdout}");
    // Deliberate UX contract: the indentation of the closing `→ next:` line is
    // part of the human rendering, so it is pinned exactly rather than loosely.
    assert_eq!(stdout.lines().last(), Some("    → next: 0.1.0"), "{stdout}");
}

#[test]
fn status_reads_fragments_too() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    momo(&dir).args(["break", "Break it"]).assert().success();

    momo(&dir).arg("status").assert().success().stdout(
        predicate::str::contains("- Break it").and(predicate::str::contains("→ next: 0.1.0")),
    );
}

#[test]
fn status_takes_the_current_version_from_the_version_file() {
    let dir = TempDir::new().expect("temp dir");
    fs::create_dir_all(dir.path().join(".momo")).expect("create .momo");
    fs::write(dir.path().join(".momo/version"), "1.2.3\n").expect("seed version");

    momo(&dir).args(["add", "an add"]).assert().success();
    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("→ next: 1.3.0"));

    momo(&dir).args(["break", "a break"]).assert().success();
    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("→ next: 2.0.0"));
}

#[test]
fn a_repository_with_nothing_unreleased_reports_the_current_version() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("→ next: 0.0.0"));
}

#[test]
fn dry_run_on_a_recording_verb_writes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["--dry-run", "add", "Add a shiny thing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would write"));

    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "dry run wrote a changelog"
    );
    assert!(
        !dir.path().join("momo.toml").exists(),
        "dry run wrote a config"
    );
    assert!(!dir.path().join(".momo").exists(), "dry run wrote .momo");
}

#[test]
fn dry_run_in_fragments_mode_writes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");

    momo(&dir)
        .args(["add", "--dry-run", "Add a shiny thing"])
        .assert()
        .success();

    assert!(
        !dir.path().join(".momo").exists(),
        "dry run wrote a fragment"
    );
    assert_eq!(read(&dir, "momo.toml"), "recording = \"fragments\"\n");
}

#[test]
fn a_recording_verb_creates_the_config_lazily() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    assert!(
        dir.path().join("momo.toml").exists(),
        "recording did not create momo.toml"
    );
}

#[test]
fn change_verb_aliases_record_their_kind() {
    let dir = TempDir::new().expect("temp dir");
    for (alias, heading) in [
        ("b", "### Breaking"),
        ("a", "### Added"),
        ("f", "### Fixed"),
    ] {
        momo(&dir).args([alias, "a change"]).assert().success();
        assert!(
            read(&dir, "CHANGELOG.md").contains(heading),
            "alias `{alias}` did not record a Change"
        );
    }
}

#[test]
fn recording_leaves_no_temporary_files_behind() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let mut names: Vec<String> = fs::read_dir(dir.path())
        .expect("read the project dir")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["CHANGELOG.md", "momo.toml"]);
}

#[test]
fn a_taken_fragment_name_gets_a_numeric_suffix() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    let changes = dir.path().join(".momo/changes");
    fs::create_dir_all(&changes).expect("create the changes dir");
    fs::write(changes.join("add-add-a-thing.md"), "first\n").expect("seed the base fragment");

    momo(&dir).args(["add", "Add a thing"]).assert().success();

    assert_eq!(read(&dir, ".momo/changes/add-add-a-thing.md"), "first\n");
    assert_eq!(
        read(&dir, ".momo/changes/add-add-a-thing-2.md"),
        "Add a thing\n"
    );
}

#[test]
fn an_unrecognised_fragment_file_fails_naming_the_file() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    let changes = dir.path().join(".momo/changes");
    fs::create_dir_all(&changes).expect("create the changes dir");
    // `changed-` is not a Kind, so this file would be invisible to the bump
    // and the report — momo refuses it instead of silently skipping it.
    fs::write(changes.join("changed-reworked.md"), "Reworked\n").expect("seed a stray file");
    fs::write(changes.join("add-a-thing.md"), "Add a thing\n").expect("seed a fragment");

    momo(&dir).arg("status").assert().failure().stderr(
        predicate::str::contains("changed-reworked.md")
            .and(predicate::str::contains("break, add, fix")),
    );
}

#[test]
fn a_readme_and_a_dotfile_in_the_changes_dir_are_ignored() {
    let dir = TempDir::new().expect("temp dir");
    seed_recording(&dir, "fragments");
    let changes = dir.path().join(".momo/changes");
    fs::create_dir_all(&changes).expect("create the changes dir");
    fs::write(
        changes.join("README.md"),
        "Write fragments as `<kind>-<slug>.md`.\n",
    )
    .expect("seed a readme");
    fs::write(changes.join(".gitkeep"), "").expect("seed a dotfile");
    fs::write(changes.join("add-a-thing.md"), "Add a thing\n").expect("seed a fragment");

    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("- Add a thing"));
}
