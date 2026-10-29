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
    cargo_monorepo(dir);
    // A repo marker, so `project_root` resolves from a package directory too.
    fs::write(dir.path().join("momo.toml"), "").expect("seed momo.toml");
}

/// The mise + Cargo sources of a two-crate monorepo, without `momo.toml`.
///
/// `init` scaffolds `momo.toml` itself and refuses to overwrite one, so a test
/// driving `init` needs the tree before any config exists.
fn cargo_monorepo(dir: &TempDir) {
    fs::write(
        dir.path().join("mise.toml"),
        "monorepo_root = true\n\n[monorepo]\nconfig_roots = [\"crates/*\"]\n",
    )
    .expect("seed mise.toml");
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

/// A two-crate Cargo workspace whose members inherit the shared version.
///
/// The root carries `[workspace.package].version`; each member's
/// `version.workspace = true` resolves to that one scalar.
fn inherited_workspace(dir: &TempDir) {
    fs::write(
        dir.path().join("mise.toml"),
        "monorepo_root = true\n\n[monorepo]\nconfig_roots = [\"crates/*\"]\n",
    )
    .expect("seed mise.toml");
    fs::write(dir.path().join("momo.toml"), "").expect("seed momo.toml");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n\n\
         [workspace.package]\nversion = \"0.0.0\"\n",
    )
    .expect("seed the workspace manifest");

    for name in ["a", "b"] {
        let package = dir.path().join("crates").join(name);
        fs::create_dir_all(package.join("src")).expect("create the crate dir");
        fs::write(
            package.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion.workspace = true\nedition = \"2021\"\n"),
        )
        .expect("seed the crate manifest");
        fs::write(package.join("src/lib.rs"), "pub fn f() {}\n").expect("seed the crate source");
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

#[test]
fn init_version_seeds_every_packages_ledger() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    cargo_monorepo(&dir);

    momo(&dir)
        .args(["init", "--version", "5.0.0"])
        .assert()
        .success();

    // Each Package reads its own directory's ledger, so the seed must land
    // there rather than at the repo root no Package reads.
    for name in ["a", "b"] {
        assert_eq!(
            fs::read_to_string(dir.path().join(format!("crates/{name}/.momo/version")))
                .expect("the package ledger was seeded"),
            "5.0.0\n",
            "cargo:{name} was not seeded"
        );
    }
    assert!(
        !dir.path().join(".momo/version").exists(),
        "the seed landed at the repo root, where no Package reads it"
    );

    // And the seeded version is what a release would bump from.
    let assert = momo(&dir)
        .args(["-p", "cargo:a", "status", "--version-only"])
        .assert()
        .success();
    assert_eq!(
        String::from_utf8_lossy(&assert.get_output().stdout),
        "5.0.0\n"
    );
}

#[test]
fn status_reports_each_package_and_its_next_version() {
    if !require_tool("mise") {
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

/// [`monorepo`] with `crates/b` declaring `requirement` on `crates/a`.
fn monorepo_with_range(dir: &TempDir, requirement: &str) {
    monorepo(dir);
    fs::write(
        dir.path().join("crates/b/Cargo.toml"),
        format!(
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\na = {{ path = \"../a\", version = \"{requirement}\" }}\n"
        ),
    )
    .expect("seed the dependent manifest");
}

#[test]
fn status_reports_a_dependent_range_the_release_would_break() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();

    // The human rendering names the dependent and its range.
    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "cascade: cargo:b requires cargo:a =0.0.1 — unsatisfied at 0.1.0",
        ));

    let assert = momo(&dir)
        .args(["status", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the status report is one JSON document");
    let cascade = value["packages"][0]["cascade"]
        .as_array()
        .expect("a `cascade` array");
    assert_eq!(cascade.len(), 1, "{stdout}");
    assert_eq!(cascade[0]["dependent"], "cargo:b", "{stdout}");
    assert_eq!(cascade[0]["dependency"], "cargo:a", "{stdout}");
    assert_eq!(cascade[0]["requirement"], "=0.0.1", "{stdout}");
    assert_eq!(cascade[0]["target"], "0.1.0", "{stdout}");
    assert_eq!(cascade[0]["satisfied"], serde_json::Value::Bool(false));
}

#[test]
fn a_satisfied_dependent_range_is_not_reported() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    // `cargo:a` targets 0.1.0, which `^0.1.0` satisfies, so there is nothing to
    // report — an absent entry is the satisfied case.
    monorepo_with_range(&dir, "^0.1.0");
    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();

    let assert = momo(&dir)
        .args(["status", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the status report is one JSON document");
    assert_eq!(
        value["packages"][0]["cascade"],
        serde_json::json!([]),
        "{stdout}"
    );
}

#[test]
fn a_dry_run_release_reports_the_cascade_it_would_leave() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();

    let assert = momo(&dir)
        .args(["-p", "cargo:a", "release", "--dry-run", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the release report is one JSON document");
    let cascade = value["packages"][0]["cascade"]
        .as_array()
        .expect("a `cascade` array");
    assert_eq!(cascade.len(), 1, "{stdout}");
    assert_eq!(cascade[0]["dependent"], "cargo:b", "{stdout}");
    assert_eq!(cascade[0]["satisfied"], serde_json::Value::Bool(false));
    // Without `--sync-ranges` nothing is rewritten, so there is no `ranges` entry.
    assert_eq!(
        value["packages"][0]["ranges"],
        serde_json::json!([]),
        "{stdout}"
    );
}

/// Give both members an Unreleased Change, so both are in the release set.
fn add_to_both(dir: &TempDir) {
    for package in ["cargo:a", "cargo:b"] {
        momo(dir)
            .args(["-p", package, "add", "Add a thing"])
            .assert()
            .success();
    }
}

#[test]
fn release_sync_ranges_rewrites_a_dependent_range_in_one_transaction() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    add_to_both(&dir);

    let assert = momo(&dir)
        .args(["--all", "release", "--sync-ranges", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the release report is one JSON document");

    // The dependent's manifest carries both the new version scalar and the
    // widened range, composed into the one file momo was already writing.
    let b = fs::read_to_string(dir.path().join("crates/b/Cargo.toml")).expect("b manifest");
    assert!(b.contains("version = \"0.1.0\""), "{b}");
    assert!(b.contains("version = \"=0.1.0\""), "{b}");
    assert!(!b.contains("=0.0.1"), "{b}");

    // The rewrite is reported against the Package whose Release caused it.
    let packages = value["packages"].as_array().expect("a `packages` array");
    let a = packages
        .iter()
        .find(|package| package["name"] == "cargo:a")
        .expect("cargo:a in the report");
    assert_eq!(
        a["ranges"],
        serde_json::json!([{
            "dependent": "cargo:b",
            "dependency": "cargo:a",
            "requirement": "=0.0.1",
            "target": "0.1.0",
        }]),
        "{stdout}"
    );
}

#[test]
fn release_sync_ranges_names_the_rewrite_in_the_human_report() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    add_to_both(&dir);

    momo(&dir)
        .args(["--all", "release", "--sync-ranges"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "range: cargo:b requires cargo:a =0.0.1 — rewritten for 0.1.0",
        ));
}

#[test]
fn a_release_without_sync_ranges_leaves_the_dependent_alone() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    add_to_both(&dir);

    // Opt-in and default off: the dry run reports the unsatisfied edge and
    // rewrites nothing, and the real run leaves the range as the project wrote
    // it.
    let assert = momo(&dir)
        .args(["--all", "release", "--dry-run"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("cascade: cargo:b requires cargo:a =0.0.1 — unsatisfied at 0.1.0"),
        "{stdout}"
    );
    assert!(!stdout.contains("range:"), "{stdout}");

    momo(&dir).args(["--all", "release"]).assert().success();
    let b = fs::read_to_string(dir.path().join("crates/b/Cargo.toml")).expect("b manifest");
    assert!(b.contains("version = \"=0.0.1\""), "{b}");
}

#[test]
fn a_candidate_never_rewrites_a_dependent_range() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo_with_range(&dir, "=0.0.1");
    add_to_both(&dir);

    momo(&dir).args(["--all", "candidate"]).assert().success();

    // `=0.0.1` is not satisfied by `0.1.0-rc.1`, so a Candidate that rewrote it
    // would widen every internal range spuriously. The version still moves.
    let b = fs::read_to_string(dir.path().join("crates/b/Cargo.toml")).expect("b manifest");
    assert!(b.contains("version = \"=0.0.1\""), "{b}");
    assert!(b.contains("version = \"0.1.0-rc.1\""), "{b}");
}

#[test]
fn package_scopes_a_command_and_all_covers_every_package() {
    if !require_tool("mise") {
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
fn a_multi_package_release_stamps_one_date() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    for package in ["cargo:a", "cargo:b"] {
        momo(&dir)
            .args(["-p", package, "add", "Add a thing"])
            .assert()
            .success();
    }

    momo(&dir).args(["--all", "release"]).assert().success();

    // The date is read once, before the package loop, so every heading in a
    // single run carries the same day. The midnight-UTC straddle that motivated
    // the hoist cannot be forced here, but the shared date can be pinned.
    let a =
        fs::read_to_string(dir.path().join("crates/a/CHANGELOG.md")).expect("crate a changelog");
    let b =
        fs::read_to_string(dir.path().join("crates/b/CHANGELOG.md")).expect("crate b changelog");
    let date_a = release_date(&a, "0.1.0");
    let date_b = release_date(&b, "0.1.0");
    assert!(
        today_or_yesterday().contains(&date_a),
        "release stamped `{date_a}`, not today or yesterday UTC"
    );
    assert_eq!(
        date_a, date_b,
        "a multi-package release stamped two different dates"
    );
}

#[test]
fn a_multi_package_release_json_is_one_document() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    for package in ["cargo:a", "cargo:b"] {
        momo(&dir)
            .args(["-p", package, "add", "Add a thing"])
            .assert()
            .success();
    }

    let assert = momo(&dir)
        .args(["--all", "release", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    // `from_str` rejects trailing content, so this pins the regression: a
    // per-Package emit printed one complete document per Package.
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout is one JSON document");
    assert_eq!(value["dry_run"], serde_json::Value::Bool(false), "{stdout}");

    let packages = value["packages"].as_array().expect("a `packages` array");
    let mut names: Vec<&str> = packages
        .iter()
        .filter_map(|package| package["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["cargo:a", "cargo:b"], "{stdout}");
    for package in packages {
        assert_eq!(package["version"], "0.1.0", "{stdout}");
    }
}

#[test]
fn a_multi_package_release_output_file_holds_every_package() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    for package in ["cargo:a", "cargo:b"] {
        momo(&dir)
            .args(["-p", package, "add", "Add a thing"])
            .assert()
            .success();
    }
    let path = dir.path().join("release.json");

    let assert = momo(&dir)
        .args(["--all", "release", "--format", "json", "--output"])
        .arg(&path)
        .assert()
        .success();
    assert!(
        assert.get_output().stdout.is_empty(),
        "`--output` still wrote to stdout"
    );

    // The write happens once, so the file is the whole report — not the last
    // Package's, which is what a per-Package `fs::write` left behind.
    let written = fs::read_to_string(&path).expect("read the report file");
    let value: serde_json::Value =
        serde_json::from_str(&written).expect("the file is one JSON document");
    let packages = value["packages"].as_array().expect("a `packages` array");
    let mut names: Vec<&str> = packages
        .iter()
        .filter_map(|package| package["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["cargo:a", "cargo:b"], "{written}");
}

#[test]
fn a_member_with_an_inherited_version_syncs_the_shared_workspace_version() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    inherited_workspace(&dir);

    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:a", "release"])
        .assert()
        .success();

    let root = fs::read_to_string(dir.path().join("Cargo.toml")).expect("workspace manifest");
    assert!(
        root.contains("[workspace.package]\nversion = \"0.1.0\""),
        "the shared workspace version was not rewritten:\n{root}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("crates/a/Cargo.toml")).expect("member manifest"),
        "[package]\nname = \"a\"\nversion.workspace = true\nedition = \"2021\"\n",
        "the member's own manifest was rewritten"
    );
}

#[test]
fn a_shared_lockfile_is_warned_about_once() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    inherited_workspace(&dir);
    fs::write(dir.path().join("Cargo.lock"), "# @generated by Cargo.\n")
        .expect("seed the lockfile");
    // Both members inherit the root's one scalar, so both sync the root manifest
    // and both leave the same root lockfile stale.
    for name in ["a", "b"] {
        momo(&dir)
            .args(["-p", &format!("cargo:{name}"), "add", "Add a thing"])
            .assert()
            .success();
    }

    let assert = momo(&dir)
        .args(["--all", "release", "--format", "json"])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("the release report is one JSON document");
    assert_eq!(
        value["packages"]
            .as_array()
            .expect("a `packages` array")
            .len(),
        2,
        "both members must release for the shared lockfile to be at stake:\n{stdout}"
    );
    assert_eq!(
        stderr.matches("Cargo.lock").count(),
        1,
        "the shared lockfile was warned about more than once:\n{stderr}"
    );
}

#[test]
fn a_shared_workspace_version_with_different_targets_fails_the_run() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    inherited_workspace(&dir);
    // `add` targets 0.1.0 and `fix` targets 0.0.1: one shared scalar, two targets.
    momo(&dir)
        .args(["-p", "cargo:a", "add", "Add a thing"])
        .assert()
        .success();
    momo(&dir)
        .args(["-p", "cargo:b", "fix", "Fix a thing"])
        .assert()
        .success();

    momo(&dir)
        .args(["--all", "release"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("a shared manifest"))
        .stderr(predicate::str::contains("cargo:a"))
        .stderr(predicate::str::contains("cargo:b"));

    // The refusal is a whole-run pre-flight: nothing was released.
    let root = fs::read_to_string(dir.path().join("Cargo.toml")).expect("workspace manifest");
    assert!(root.contains("version = \"0.0.0\""), "{root}");
    for name in ["a", "b"] {
        let changelog = fs::read_to_string(dir.path().join(format!("crates/{name}/CHANGELOG.md")))
            .expect("the recorded changelog");
        assert!(
            !changelog.contains("## [0.1.0]"),
            "cargo:{name} was released anyway:\n{changelog}"
        );
        assert!(
            !dir.path()
                .join(format!("crates/{name}/.momo/version"))
                .exists(),
            "cargo:{name}'s version file was advanced"
        );
    }
}

#[test]
fn a_recording_verb_defaults_to_the_current_directory() {
    if !require_tool("mise") {
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
    if !require_tool("mise") {
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
fn a_slug_collision_is_rejected() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    // Pin `cargo:b` to `cargo:a`'s slug: the two would share a tag prefix and a
    // Fragment store, so discovery must refuse rather than pick one.
    fs::write(
        dir.path().join("momo.toml"),
        "[packages.\"cargo:b\"]\nslug = \"cargo-a\"\n",
    )
    .expect("seed config");

    momo(&dir)
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("share the slug `cargo-a`"));
}

#[test]
fn an_ambiguous_package_name_is_rejected() {
    if !require_tool("mise") {
        return;
    }
    let dir = TempDir::new().expect("temp dir");
    monorepo(&dir);
    // Give `cargo:a` the slug `b`, so the bare name `b` names both it and
    // `cargo:b`'s bare name.
    fs::write(
        dir.path().join("momo.toml"),
        "[packages.\"cargo:a\"]\nslug = \"b\"\n",
    )
    .expect("seed config");

    momo(&dir)
        .args(["-p", "b", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("matches more than one package"))
        .stderr(predicate::str::contains("cargo:a"))
        .stderr(predicate::str::contains("cargo:b"));
}

#[test]
fn fragments_are_stored_per_package() {
    if !require_tool("mise") {
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
    if !require_tool("mise") || !require_tool("jj") {
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
        .args(["-p", "cargo:a", "tag", "--rev", "@"])
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
    if !require_tool("mise") || !require_tool("jj") {
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
        .args(["-p", "cargo:a", "tag", "--rev", "@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha-v0.1.0"));
}

#[test]
fn a_mise_toml_without_monorepo_root_is_silent() {
    let dir = TempDir::new().expect("temp dir");
    // A task environment that is not a monorepo: the graph's refusal is
    // expected, so momo degrades to a single Package without a word.
    fs::write(dir.path().join("mise.toml"), "[tools]\n").expect("seed mise.toml");

    momo(&dir)
        .arg("status")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());

    // The single-package fallback still records and reports as before.
    momo(&dir).args(["add", "Add a thing"]).assert().success();
    assert!(dir.path().join("CHANGELOG.md").exists());
}

/// A `PATH` whose `mise` always fails, so the graph cannot be read.
#[cfg(unix)]
fn failing_mise_path(dir: &TempDir) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;

    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).expect("create the shim directory");
    let shim = bin.join("mise");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'mise: cannot read the graph' >&2\nexit 1\n",
    )
    .expect("write the mise shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))
        .expect("make the shim executable");

    let mut path = bin.into_os_string();
    if let Some(existing) = std::env::var_os("PATH") {
        path.push(":");
        path.push(existing);
    }
    path
}

#[test]
#[cfg(unix)]
fn a_monorepo_whose_graph_cannot_be_read_warns() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "monorepo_root = true\n").expect("seed mise.toml");
    let path = failing_mise_path(&dir);

    momo(&dir)
        .env("PATH", path)
        .arg("status")
        .assert()
        .success()
        .stderr(predicate::str::contains("package discovery unavailable"));
}

#[test]
#[cfg(unix)]
fn requesting_all_on_an_unreadable_monorepo_fails_loudly() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "monorepo_root = true\n").expect("seed mise.toml");
    let path = failing_mise_path(&dir);

    momo(&dir)
        .env("PATH", path)
        .args(["--all", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("package discovery failed"));
}

#[test]
#[cfg(unix)]
fn requesting_a_package_on_an_unreadable_monorepo_fails_loudly() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("mise.toml"), "monorepo_root = true\n").expect("seed mise.toml");
    let path = failing_mise_path(&dir);

    momo(&dir)
        .env("PATH", path)
        .args(["-p", "cargo:a", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("package discovery failed"));
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
