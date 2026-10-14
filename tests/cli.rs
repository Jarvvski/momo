//! Integration tests for the command surface, `init`, and the config layer.

use std::ffi::OsString;
use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// The canonical commands on the surface, in help order.
///
/// This is the whole surface: `version` and `completions` sit here but not in
/// momo's `INSTALLABLE_VERBS`, because neither gets a `momo:*` task wrapper
/// (see `tests/install.rs`).
const COMMANDS: [&str; 11] = [
    "break",
    "add",
    "fix",
    "status",
    "release",
    "candidate",
    "tag",
    "init",
    "install",
    "version",
    "completions",
];

/// The global flags on the command surface.
const GLOBAL_FLAGS: [&str; 7] = [
    "--dry-run",
    "--format",
    "--json",
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
        "MOMO_REMOTE",
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

    for verb in COMMANDS {
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
    // Deliberate clap contract: the `[alias: …]` marker is clap's own
    // rendering, pinned here as a smoke check on the command surface.
    for alias in ["b", "a", "f", "rc"] {
        assert!(
            stdout.contains(&format!("[alias: {alias}]")),
            "`momo --help` is missing the `{alias}` alias:\n{stdout}"
        );
    }
}

#[test]
fn every_verb_help_has_usage_examples() {
    let dir = TempDir::new().expect("temp dir");

    let assert = momo(&dir).arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Examples:"),
        "`momo --help` has no examples:\n{stdout}"
    );
    assert!(
        stdout.contains("momo add \"Add a shiny thing\""),
        "`momo --help` is missing the quick-start block:\n{stdout}"
    );

    for verb in COMMANDS {
        let assert = momo(&dir).args([verb, "--help"]).assert().success();
        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(
            stdout.contains("Examples:") && stdout.contains(&format!("momo {verb}")),
            "`momo {verb} --help` has no examples:\n{stdout}"
        );
    }
}

#[test]
fn each_verb_help_shows_the_global_flags() {
    let dir = TempDir::new().expect("temp dir");
    for verb in COMMANDS {
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
fn completions_emit_a_non_empty_script_to_stdout() {
    let dir = TempDir::new().expect("temp dir");

    let assert = momo(&dir).args(["completions", "bash"]).assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        !stdout.trim().is_empty(),
        "`momo completions bash` emitted nothing"
    );
    assert!(
        stdout.contains("momo"),
        "`momo completions bash` never names the binary:\n{stdout}"
    );
}

#[test]
fn change_verb_aliases_resolve_to_their_verbs() {
    let dir = TempDir::new().expect("temp dir");
    let changes = [
        ("b", "a breaking change"),
        ("a", "an added change"),
        ("f", "a fixed change"),
    ];
    for (alias, text) in changes {
        momo(&dir).args([alias, text]).assert().success();
    }

    let changelog = fs::read_to_string(dir.path().join("CHANGELOG.md")).expect("changelog written");
    for (_, text) in changes {
        assert!(changelog.contains(text), "`{text}` missing:\n{changelog}");
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
fn init_writes_defaults_not_the_environment() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .env("MOMO_TAG_PREFIX", "rel-")
        .arg("init")
        .assert()
        .success();
    let text = fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml was written");
    assert!(text.contains("tag_prefix = \"v\""), "{text}");
    assert!(!text.contains("rel-"), "{text}");
}

#[test]
fn a_lazy_write_does_not_bake_the_environment_into_momo_toml() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .env("MOMO_TAG_PREFIX", "rel-")
        .env("MOMO_RECORDING", "fragments")
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    let text = fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml was created");
    assert!(text.contains("tag_prefix = \"v\""), "{text}");
    assert!(text.contains("recording = \"changelog\""), "{text}");
    assert!(!text.contains("rel-"), "{text}");
    assert!(!text.contains("fragments"), "{text}");
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
    momo(&dir).arg("status").assert().success();
    assert!(
        !dir.path().join("momo.toml").exists(),
        "a read-only command created momo.toml"
    );
    assert!(
        !dir.path().join("CHANGELOG.md").exists(),
        "a read-only command wrote a changelog"
    );
}

#[test]
fn the_release_verbs_report_their_own_errors() {
    let dir = TempDir::new().expect("temp dir");

    // With nothing Unreleased, `release` and `candidate` refuse.
    for args in [&["release"][..], &["candidate"][..], &["rc"][..]] {
        momo(&dir)
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("nothing unreleased"));
    }

    // With no repository, `tag` has nothing to tag.
    momo(&dir)
        .arg("tag")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no VCS detected"));
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
fn init_reports_defaults_ignoring_the_file_and_the_environment() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("momo.toml"), "recording = \"fragments\"\n").expect("seed momo.toml");

    // Neither a seeded file nor an environment override leaks into what `init`
    // would write: it scaffolds the defaults, so a transient `MOMO_*` value is
    // never baked into the committed file.
    momo(&dir)
        .env("MOMO_RECORDING", "fragments")
        .args(["init", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("recording = \"changelog\"")
                .and(predicate::str::contains("tag_prefix = \"v\"")),
        );

    // A dry run never writes.
    assert_eq!(
        fs::read_to_string(dir.path().join("momo.toml")).expect("momo.toml readable"),
        "recording = \"fragments\"\n"
    );
}

#[test]
fn init_version_records_a_start_version() {
    let dir = TempDir::new().expect("temp dir");

    momo(&dir)
        .args(["init", "--version", "1.2.3"])
        .assert()
        .success();

    assert!(dir.path().join("momo.toml").exists(), "momo.toml missing");
    assert_eq!(
        fs::read_to_string(dir.path().join(".momo/version")).expect(".momo/version written"),
        "1.2.3\n"
    );
}

#[test]
fn init_version_lifts_the_version_the_next_release_bumps() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["init", "--version", "1.2.3"])
        .assert()
        .success();
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();

    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("→ next: 1.3.0"));
}

#[test]
fn init_version_rejects_a_bad_version() {
    let dir = TempDir::new().expect("temp dir");

    momo(&dir)
        .args(["init", "--version", "not-a-version"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--version"));

    assert!(
        !dir.path().join("momo.toml").exists(),
        "a rejected version still wrote momo.toml"
    );
}

#[test]
fn init_dry_run_with_a_version_writes_nothing() {
    let dir = TempDir::new().expect("temp dir");

    momo(&dir)
        .args(["init", "--version", "1.2.3", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains(".momo/version"));

    assert!(!dir.path().join("momo.toml").exists());
    assert!(!dir.path().join(".momo/version").exists());
}

#[test]
fn output_flags_are_rejected_on_verbs_without_a_report() {
    let dir = TempDir::new().expect("temp dir");
    let cases: [(&[&str], &str, &str); 21] = [
        (&["add", "a thing", "--format", "json"], "--format", "add"),
        (&["add", "a thing", "--json"], "--json", "add"),
        (
            &["add", "a thing", "--version-only"],
            "--version-only",
            "add",
        ),
        (
            &["add", "a thing", "--output", "out.txt"],
            "--output",
            "add",
        ),
        (
            &["break", "a thing", "--format", "json"],
            "--format",
            "break",
        ),
        (&["break", "a thing", "--json"], "--json", "break"),
        (
            &["fix", "a thing", "--version-only"],
            "--version-only",
            "fix",
        ),
        (
            &["fix", "a thing", "--output", "out.txt"],
            "--output",
            "fix",
        ),
        (&["fix", "a thing", "--json"], "--json", "fix"),
        (&["init", "--format", "json"], "--format", "init"),
        (&["init", "--json"], "--json", "init"),
        (&["init", "--version-only"], "--version-only", "init"),
        (&["init", "--output", "out.txt"], "--output", "init"),
        (&["install", "--format", "json"], "--format", "install"),
        (&["install", "--json"], "--json", "install"),
        (&["install", "--version-only"], "--version-only", "install"),
        (&["install", "--output", "out.txt"], "--output", "install"),
        (
            &["completions", "bash", "--format", "json"],
            "--format",
            "completions",
        ),
        (&["completions", "bash", "--json"], "--json", "completions"),
        (
            &["completions", "bash", "--version-only"],
            "--version-only",
            "completions",
        ),
        (
            &["completions", "bash", "--output", "out.txt"],
            "--output",
            "completions",
        ),
    ];

    for (args, flag, verb) in cases {
        momo(&dir)
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains(flag).and(predicate::str::contains(verb)));
    }
}

#[test]
fn the_json_shorthand_and_the_format_flag_conflict() {
    let dir = TempDir::new().expect("temp dir");
    momo(&dir)
        .args(["status", "--json", "--format", "json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--format").and(predicate::str::contains("--json")));
}

#[test]
fn reporting_verbs_still_accept_the_output_flags() {
    let dir = TempDir::new().expect("temp dir");

    for args in [
        &["status", "--format", "json"][..],
        &["status", "--json"][..],
        &["status", "--version-only"][..],
    ] {
        momo(&dir).args(args).assert().success();
    }

    // `release` and `candidate` need something Unreleased each.
    momo(&dir)
        .args(["add", "Add a shiny thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["release", "--version-only"])
        .assert()
        .success();

    momo(&dir)
        .args(["add", "Add another thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["candidate", "--version-only"])
        .assert()
        .success();
}
