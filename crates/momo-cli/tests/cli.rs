//! Integration tests for the command surface, `init`, and the config layer.

use std::ffi::OsString;
use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// The canonical verbs on the command surface.
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

/// The global flags on the command surface.
const GLOBAL_FLAGS: [&str; 6] = [
    "--dry-run",
    "--format",
    "--version-only",
    "--output",
    "--package",
    "--all",
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
        "MOMO_TASK_ENV",
        "MOMO_RC_FORMAT",
    ] {
        cmd.env_remove(name);
    }
    cmd
}

#[test]
fn help_lists_every_verb_alias_and_global_flag() {
    let dir = TempDir::new().expect("temp dir");
    let assert = momo(&dir).arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    for verb in VERBS {
        assert!(
            stdout.contains(verb),
            "`momo --help` is missing the `{verb}` verb:\n{stdout}"
        );
    }
    for flag in GLOBAL_FLAGS {
        assert!(
            stdout.contains(flag),
            "`momo --help` is missing the `{flag}` flag:\n{stdout}"
        );
    }
    for alias in ["b", "a", "f", "rc"] {
        assert!(
            stdout.contains(&format!("[alias: {alias}]")),
            "`momo --help` is missing the `{alias}` alias:\n{stdout}"
        );
    }
}

#[test]
fn each_verb_help_shows_the_global_flags() {
    let dir = TempDir::new().expect("temp dir");
    for verb in VERBS {
        let assert = momo(&dir).args([verb, "--help"]).assert().success();
        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
        for flag in GLOBAL_FLAGS {
            assert!(
                stdout.contains(flag),
                "`momo {verb} --help` is missing `{flag}`:\n{stdout}"
            );
        }
    }
}

#[test]
fn change_verb_aliases_resolve_to_their_verbs() {
    let dir = TempDir::new().expect("temp dir");
    for (alias, verb) in [("b", "break"), ("a", "add"), ("f", "fix")] {
        momo(&dir)
            .args([alias, "a change"])
            .assert()
            .failure()
            .stderr(predicate::str::contains(format!("`momo {verb}`")));
    }
}

#[test]
fn init_writes_a_valid_momo_toml_and_touches_nothing_else() {
    let dir = TempDir::new().expect("temp dir");
    let sentinel = dir.path().join("keep.txt");
    fs::write(&sentinel, "untouched").expect("write the sentinel file");

    momo(&dir).arg("init").assert().success();

    let text = fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml was written");
    assert!(text.contains("recording = \"changelog\""), "{text}");
    assert!(text.contains("changelog = \"CHANGELOG.md\""), "{text}");
    assert!(text.contains("changes_dir = \".momo/changes\""), "{text}");
    assert!(text.contains("tag_prefix = \"v\""), "{text}");
    assert!(text.contains("tag_kind = \"auto\""), "{text}");
    let _: momo_core::config::ConfigOverrides = text.parse().expect("the file is valid TOML");

    assert_eq!(
        fs::read_to_string(&sentinel).expect("sentinel still readable"),
        "untouched"
    );
    let mut entries: Vec<OsString> = fs::read_dir(dir.path())
        .expect("read the temp dir")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec![OsString::from("keep.txt"), OsString::from("momo.toml")],
        "init created files beyond momo.toml"
    );
}

#[test]
fn init_writes_the_effective_configuration() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .env("MOMO_TAG_PREFIX", "rel-")
        .arg("init")
        .assert()
        .success();
    let text = fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml was written");
    assert!(text.contains("tag_prefix = \"rel-\""), "{text}");
}

#[test]
fn init_refuses_to_overwrite_an_existing_file() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("momo.toml");
    fs::write(&path, "recording = \"fragments\"\n").expect("seed momo.toml");

    momo(&dir)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    assert_eq!(
        fs::read_to_string(&path).expect("momo.toml still readable"),
        "recording = \"fragments\"\n"
    );
}

#[test]
fn a_read_only_command_creates_no_config() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir).arg("status").assert().failure();
    assert!(
        !dir.path().join("momo.toml").exists(),
        "a read-only command created momo.toml"
    );
}

#[test]
fn unimplemented_verbs_exit_non_zero_with_a_clear_message() {
    let dir = TempDir::new().expect("temp dir");
    let invocations: [&[&str]; 10] = [
        &["break", "text"],
        &["add", "text"],
        &["fix", "text"],
        &["status"],
        &["release"],
        &["candidate"],
        &["rc"],
        &["tag"],
        &["install"],
        &["install", "mise"],
    ];
    for args in invocations {
        momo(&dir)
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("not implemented yet"));
    }
}

#[test]
fn init_dry_run_reports_defaults_and_creates_nothing() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["init", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("recording = \"changelog\"")
                .and(predicate::str::contains("changes_dir = \".momo/changes\"")),
        );
    assert!(!dir.path().join("momo.toml").exists());
}

#[test]
fn config_precedence_env_beats_file_beats_defaults() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("momo.toml"), "recording = \"fragments\"\n").expect("seed momo.toml");

    // The file overrides the default...
    momo(&dir)
        .args(["init", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("recording = \"fragments\"")
                .and(predicate::str::contains("tag_prefix = \"v\"")),
        );

    // ...and the environment overrides the file.
    momo(&dir)
        .env("MOMO_RECORDING", "changelog")
        .args(["init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("recording = \"changelog\""));

    // A dry run never writes.
    assert_eq!(
        fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml readable"),
        "recording = \"fragments\"\n"
    );
}
