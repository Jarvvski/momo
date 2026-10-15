//! Filesystem round-trips for the Node manifest adapter: a write changes only
//! the version value, and a lockfile is reported rather than rewritten.

use std::fs;
use std::path::Path;

use crate::adapters::manifest::NodeManifest;
use crate::domain::manifest::{ManifestEdits, ManifestSync, SkipReason};
use semver::Version;
use tempfile::TempDir;

/// A `package.json` with a toolchain pin, a dependency, and unusual spacing.
const MANIFEST: &str = "\
{
  \"name\": \"@acme/ui\",
  \"version\": \"0.1.0\",
  \"packageManager\": \"pnpm@9.0.0\",
  \"dependencies\": {
    \"version\": \"^3.0.0\"
  }
}
";

/// [`MANIFEST`] with the version bumped to `0.2.0`.
const UPDATED: &str = "\
{
  \"name\": \"@acme/ui\",
  \"version\": \"0.2.0\",
  \"packageManager\": \"pnpm@9.0.0\",
  \"dependencies\": {
    \"version\": \"^3.0.0\"
  }
}
";

/// The pnpm lockfile header, standing in for a real one.
const LOCKFILE: &str = "lockfileVersion: '9.0'\n";

fn version(text: &str) -> Version {
    text.parse().expect("a valid version")
}

/// The edits that sync only a version, leaving every range alone.
fn edits(text: &str) -> ManifestEdits {
    ManifestEdits::version(version(text))
}

#[test]
fn writing_a_manifest_rewrites_only_the_version_value() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("package.json");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = NodeManifest
        .write(&path, &edits("0.2.0"))
        .expect("write the manifest");

    assert_eq!(plan.writes.len(), 1, "{plan:?}");
    assert_eq!(plan.writes[0].path, path);
    assert!(plan.lockfiles.is_empty(), "{plan:?}");
    assert!(plan.skipped.is_empty(), "{plan:?}");
    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        UPDATED
    );
}

#[test]
fn a_lockfile_at_the_workspace_root_is_reported_and_left_alone() {
    let dir = TempDir::new().expect("temp dir");
    let package = dir.path().join("packages/ui");
    fs::create_dir_all(&package).expect("create the package dir");
    let path = package.join("package.json");
    let lockfile = dir.path().join("pnpm-lock.yaml");
    fs::write(&path, MANIFEST).expect("seed the manifest");
    fs::write(&lockfile, LOCKFILE).expect("seed the root lockfile");

    let plan = NodeManifest
        .write(&path, &edits("0.2.0"))
        .expect("write the manifest");

    assert_eq!(plan.lockfiles, vec![lockfile.clone()]);
    assert_eq!(
        fs::read_to_string(&lockfile).expect("read the lockfile"),
        LOCKFILE,
        "the lockfile was rewritten"
    );
}

#[test]
fn an_already_current_version_writes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("package.json");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = NodeManifest
        .write(&path, &edits("0.1.0"))
        .expect("write the manifest");

    assert!(plan.writes.is_empty(), "{plan:?}");
    assert_eq!(
        plan.skipped,
        vec![(path.clone(), SkipReason::AlreadyCurrent)],
        "{plan:?}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        MANIFEST
    );
}

#[test]
fn a_manifest_without_a_version_is_reported_as_skipped() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("package.json");
    fs::write(&path, "{\n  \"name\": \"@acme/ui\"\n}\n").expect("seed the manifest");

    let plan = NodeManifest
        .write(&path, &edits("0.2.0"))
        .expect("write the manifest");

    assert!(plan.writes.is_empty(), "{plan:?}");
    assert_eq!(
        plan.skipped,
        vec![(path.clone(), SkipReason::NoVersion)],
        "{plan:?}"
    );
}

#[test]
fn writing_a_crlf_manifest_preserves_its_line_endings() {
    /// A CRLF manifest with no trailing newline.
    const CRLF: &str = "{\r\n  \"name\": \"@acme/ui\",\r\n  \"version\": \"0.1.0\"\r\n}";

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("package.json");
    fs::write(&path, CRLF).expect("seed the manifest");

    NodeManifest
        .write(&path, &edits("0.2.0"))
        .expect("write the manifest");

    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        "{\r\n  \"name\": \"@acme/ui\",\r\n  \"version\": \"0.2.0\"\r\n}"
    );
}

#[test]
fn rendering_a_manifest_leaves_the_file_untouched() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("package.json");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = NodeManifest
        .render(&path, &edits("0.2.0"))
        .expect("render the manifest");

    assert_eq!(plan.writes.len(), 1, "{plan:?}");
    assert_eq!(plan.writes[0].path, path);
    assert_eq!(plan.writes[0].text, UPDATED);
    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        MANIFEST,
        "render rewrote the manifest"
    );
}

#[test]
fn locate_finds_the_conventional_manifest() {
    let dir = TempDir::new().expect("temp dir");
    assert_eq!(NodeManifest.locate(dir.path(), None), None);

    fs::write(dir.path().join("package.json"), MANIFEST).expect("seed the manifest");
    assert_eq!(
        NodeManifest.locate(dir.path(), None),
        Some(dir.path().join("package.json"))
    );
}

#[test]
fn locate_prefers_the_configured_override() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("package.json"), MANIFEST).expect("seed the conventional manifest");
    fs::write(dir.path().join("alt.json"), MANIFEST).expect("seed the override");

    assert_eq!(
        NodeManifest.locate(dir.path(), Some(Path::new("alt.json"))),
        Some(dir.path().join("alt.json"))
    );
}
