//! Filesystem round-trips for the Python manifest adapter: a write changes only
//! the version scalar, and a dynamic version is reported as skipped.

use std::fs;
use std::path::Path;

use crate::adapters::manifest::PythonManifest;
use crate::domain::manifest::{ManifestSync, SkipReason};
use semver::Version;
use tempfile::TempDir;

/// A manifest with comments, unusual spacing, and a second table.
const MANIFEST: &str = "\
# The project manifest.
[project]
name = \"acme\"   # the name
version = \"0.1.0\" # bumped by momo
requires-python = \">=3.11\"

[tool.ruff]
# a tool table
line-length = 88
";

/// [`MANIFEST`] with the version bumped to `0.2.0`.
const UPDATED: &str = "\
# The project manifest.
[project]
name = \"acme\"   # the name
version = \"0.2.0\" # bumped by momo
requires-python = \">=3.11\"

[tool.ruff]
# a tool table
line-length = 88
";

fn version(text: &str) -> Version {
    text.parse().expect("a valid version")
}

#[test]
fn writing_a_manifest_rewrites_only_the_version_scalar() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("pyproject.toml");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = PythonManifest
        .write(&path, &version("0.2.0"))
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
fn an_already_current_version_writes_nothing() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("pyproject.toml");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = PythonManifest
        .write(&path, &version("0.1.0"))
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
    let path = dir.path().join("pyproject.toml");
    fs::write(&path, "[project]\nname = \"acme\"\n").expect("seed the manifest");

    let plan = PythonManifest
        .write(&path, &version("0.2.0"))
        .expect("write the manifest");

    assert!(plan.writes.is_empty(), "{plan:?}");
    assert_eq!(
        plan.skipped,
        vec![(path.clone(), SkipReason::NoVersion)],
        "{plan:?}"
    );
}

#[test]
fn a_dynamic_version_is_reported_as_skipped_and_left_alone() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("pyproject.toml");
    let dynamic = "[project]\nname = \"acme\"\ndynamic = [\"version\"]\n";
    fs::write(&path, dynamic).expect("seed the manifest");

    let plan = PythonManifest
        .write(&path, &version("0.2.0"))
        .expect("write the manifest");

    assert!(plan.writes.is_empty(), "{plan:?}");
    assert_eq!(
        plan.skipped,
        vec![(
            path.clone(),
            SkipReason::Dynamic("PEP 621 [project].dynamic".to_owned()),
        )],
        "{plan:?}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        dynamic
    );
}

#[test]
fn writing_a_crlf_manifest_preserves_its_line_endings() {
    /// A CRLF manifest with no trailing newline.
    const CRLF: &str =
        "# The project manifest.\r\n[project]\r\nname = \"acme\"\r\nversion = \"0.1.0\"";

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("pyproject.toml");
    fs::write(&path, CRLF).expect("seed the manifest");

    PythonManifest
        .write(&path, &version("0.2.0"))
        .expect("write the manifest");

    assert_eq!(
        fs::read_to_string(&path).expect("read the manifest"),
        "# The project manifest.\r\n[project]\r\nname = \"acme\"\r\nversion = \"0.2.0\""
    );
}

#[test]
fn rendering_a_manifest_leaves_the_file_untouched() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("pyproject.toml");
    fs::write(&path, MANIFEST).expect("seed the manifest");

    let plan = PythonManifest
        .render(&path, &version("0.2.0"))
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
    assert_eq!(PythonManifest.locate(dir.path(), None), None);

    fs::write(dir.path().join("pyproject.toml"), MANIFEST).expect("seed the manifest");
    assert_eq!(
        PythonManifest.locate(dir.path(), None),
        Some(dir.path().join("pyproject.toml"))
    );
}

#[test]
fn locate_prefers_the_configured_override() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("pyproject.toml"), MANIFEST).expect("seed the conventional manifest");
    fs::write(dir.path().join("alt.toml"), MANIFEST).expect("seed the override");

    assert_eq!(
        PythonManifest.locate(dir.path(), Some(Path::new("alt.toml"))),
        Some(dir.path().join("alt.toml"))
    );
}

#[test]
fn locate_skips_an_override_that_does_not_exist() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("pyproject.toml"), MANIFEST).expect("seed the manifest");

    assert_eq!(
        PythonManifest.locate(dir.path(), Some(Path::new("nope.toml"))),
        None
    );
}
