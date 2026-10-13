//! The Cargo manifest adapter: `Cargo.toml`, edited in place with `toml_edit`.
//!
//! The edit is format-preserving by construction: the document is parsed into
//! `toml_edit`'s concrete syntax tree, one scalar is replaced, and the tree is
//! rendered back. Comments, key order, and whitespace survive byte-for-byte —
//! only the version value's own representation is rewritten. The source's
//! newline style (LF or CRLF) and whether it ended in a newline are reapplied
//! after rendering, because `toml_edit` normalises both.
//!
//! The lockfile is *not* touched. `Cargo.lock` embeds each workspace package's
//! version and is maintained by Cargo, so momo reports it as possibly stale and
//! leaves refreshing it to the package manager.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::manifest::{ManifestError, ManifestSync, ManifestUpdate};
use semver::Version;
use toml_edit::{DocumentMut, Item, Value};

/// The conventional Cargo manifest name.
const MANIFEST: &str = "Cargo.toml";

/// The lockfile Cargo maintains; momo never rewrites it.
const LOCKFILE: &str = "Cargo.lock";

/// The Rust manifest adapter.
pub struct CargoManifest;

impl ManifestSync for CargoManifest {
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf> {
        let path = match override_path {
            Some(override_path) => package_dir.join(override_path),
            None => package_dir.join(MANIFEST),
        };
        path.is_file().then_some(path)
    }

    fn render(
        &self,
        manifest: &Path,
        version: &Version,
    ) -> Result<Option<(String, ManifestUpdate)>, ManifestError> {
        let text = fs::read_to_string(manifest).map_err(|source| ManifestError::Read {
            path: manifest.to_path_buf(),
            source,
        })?;

        let Some(updated) = set_version(&text, manifest, version)? else {
            return Ok(None);
        };
        Ok(Some((
            updated,
            ManifestUpdate {
                manifest: manifest.to_path_buf(),
                stale_lockfile: lockfile(manifest),
            },
        )))
    }

    fn write(
        &self,
        manifest: &Path,
        version: &Version,
    ) -> Result<Option<ManifestUpdate>, ManifestError> {
        let Some((updated, update)) = self.render(manifest, version)? else {
            return Ok(None);
        };
        fs::write(manifest, updated).map_err(|source| ManifestError::Write {
            path: manifest.to_path_buf(),
            source,
        })?;

        Ok(Some(update))
    }
}

/// The lockfile Cargo keeps beside `manifest`, when there is one.
fn lockfile(manifest: &Path) -> Option<PathBuf> {
    let lockfile = manifest.parent()?.join(LOCKFILE);
    lockfile.is_file().then_some(lockfile)
}

/// Render `text` with the version scalar set to `version`.
///
/// `None` when the manifest holds no version momo owns, or already carries
/// `version` — in both cases there is nothing to write.
fn set_version(
    text: &str,
    manifest: &Path,
    version: &Version,
) -> Result<Option<String>, ManifestError> {
    let mut doc: DocumentMut = text.parse().map_err(|source| ManifestError::Parse {
        path: manifest.to_path_buf(),
        source,
    })?;

    let Some(key) = version_key(&doc) else {
        return Ok(None);
    };
    let key = match key {
        Key::Inherited => {
            return Err(ManifestError::InheritedVersion {
                manifest: manifest.to_path_buf(),
            });
        }
        resolvable => resolvable,
    };
    let target = version.to_string();
    let Some(item) = item_mut(&mut doc, key) else {
        return Ok(None);
    };
    if item.as_str() == Some(target.as_str()) {
        return Ok(None);
    }

    // Replacing the value drops its decor — the whitespace around it and any
    // trailing comment — so put it back around the new scalar.
    let Some(current) = item.as_value_mut() else {
        return Ok(None);
    };
    let decor = current.decor().clone();
    *current = Value::from(target);
    *current.decor_mut() = decor;

    Ok(Some(preserve_line_endings(text, doc.to_string())))
}

/// Reapply `source`'s newline style and final-newline state to `rendered`.
///
/// `toml_edit` always renders LF and always terminates the document, so a CRLF
/// manifest or one without a trailing newline would otherwise be rewritten
/// wholesale. `rendered` is the untouched render of the parsed document.
fn preserve_line_endings(source: &str, rendered: String) -> String {
    let mut rendered = rendered;
    if !source.ends_with('\n') && rendered.ends_with('\n') {
        rendered.pop();
    }
    if uses_crlf(source) {
        rendered = rendered.replace('\n', "\r\n");
    }
    rendered
}

/// Whether `source` uses CRLF line endings throughout.
///
/// A file with any bare `\n`, or no `\r\n` at all, is treated as LF.
fn uses_crlf(source: &str) -> bool {
    source.contains("\r\n") && !source.replace("\r\n", "").contains('\n')
}

/// The key holding a Cargo manifest's authoritative version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    /// `[package].version` — a plain crate.
    Package,
    /// `[workspace.package].version` — the workspace root a member inherits
    /// from, or a virtual manifest.
    Workspace,
    /// `version.workspace = true` with no `[workspace.package].version` in the
    /// same document — an inherited version momo cannot resolve.
    Inherited,
}

/// Resolve the key holding `doc`'s authoritative version.
///
/// `[package].version` wins when it is a string. When it is inherited
/// (`version.workspace = true`), or the manifest has no member version at all
/// (a virtual workspace), the workspace root's `[workspace.package].version` is
/// authoritative. `Key::Inherited` when the version is inherited from a
/// workspace root that is not in this document — [`set_version`] reports that
/// rather than silently skipping. `None` when neither exists, so the manifest
/// is left alone.
fn version_key(doc: &DocumentMut) -> Option<Key> {
    if let Some(item) = doc
        .get("package")
        .and_then(|package| package.get("version"))
    {
        if item.as_str().is_some() {
            return Some(Key::Package);
        }
        if item.get("workspace").and_then(Item::as_bool) == Some(true) {
            return Some(if workspace_version(doc).is_some() {
                Key::Workspace
            } else {
                Key::Inherited
            });
        }
        return None;
    }
    workspace_version(doc).map(|_| Key::Workspace)
}

/// The `[workspace.package].version` item, when the manifest has one.
fn workspace_version(doc: &DocumentMut) -> Option<&Item> {
    doc.get("workspace")?.get("package")?.get("version")
}

/// The mutable item `key` names, when it is still there.
fn item_mut(doc: &mut DocumentMut, key: Key) -> Option<&mut Item> {
    match key {
        Key::Package => doc.get_mut("package")?.get_mut("version"),
        Key::Workspace => doc
            .get_mut("workspace")?
            .get_mut("package")?
            .get_mut("version"),
        // `Inherited` is rejected before an item is looked up.
        Key::Inherited => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest path the tests attribute errors to.
    fn manifest() -> &'static Path {
        Path::new("Cargo.toml")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    #[test]
    fn a_plain_version_is_rewritten_and_everything_else_survives() {
        let original = "\
# The crate manifest.
[package]
name = \"momo\"   # the name
version = \"0.1.0\" # bumped by momo
edition = \"2024\"

[dependencies]
# a dependency
semver = \"1.0\"
";

        let updated = set_version(original, manifest(), &version("1.2.3"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(
            updated,
            "\
# The crate manifest.
[package]
name = \"momo\"   # the name
version = \"1.2.3\" # bumped by momo
edition = \"2024\"

[dependencies]
# a dependency
semver = \"1.0\"
"
        );
    }

    #[test]
    fn an_inherited_version_rewrites_the_workspace_root() {
        let original = "\
[package]
name = \"bar\"
version.workspace = true

[workspace.package]
version = \"1.2.3\"
";

        let updated = set_version(original, manifest(), &version("2.0.0"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(
            updated,
            "\
[package]
name = \"bar\"
version.workspace = true

[workspace.package]
version = \"2.0.0\"
"
        );
    }

    #[test]
    fn a_virtual_manifest_rewrites_the_workspace_version() {
        let original = "\
[workspace]
members = [\"bar\"]

[workspace.package]
version = \"0.1.0\"
";

        let updated = set_version(original, manifest(), &version("0.2.0"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(
            updated,
            "\
[workspace]
members = [\"bar\"]

[workspace.package]
version = \"0.2.0\"
"
        );
    }

    #[test]
    fn an_inherited_version_without_a_workspace_root_is_reported() {
        let original = "[package]\nname = \"bar\"\nversion.workspace = true\n";
        let err = set_version(original, manifest(), &version("1.0.0"))
            .expect_err("an unresolvable inherited version is an error");
        assert!(
            matches!(err, ManifestError::InheritedVersion { .. }),
            "{err}"
        );
        assert!(err.to_string().contains("Cargo.toml"), "{err}");
    }

    #[test]
    fn a_manifest_without_a_version_is_skipped() {
        let original = "[package]\nname = \"momo\"\n";
        assert_eq!(
            set_version(original, manifest(), &version("1.0.0")).expect("valid TOML"),
            None
        );
    }

    #[test]
    fn an_unchanged_version_is_not_rewritten() {
        let original = "[package]\nname = \"momo\"\nversion = \"1.2.3\"\n";
        assert_eq!(
            set_version(original, manifest(), &version("1.2.3")).expect("valid TOML"),
            None
        );
    }

    #[test]
    fn invalid_toml_is_reported_against_the_path() {
        let err = set_version("version = \n", manifest(), &version("1.0.0"))
            .expect_err("invalid TOML is an error");
        assert!(err.to_string().contains("Cargo.toml"), "{err}");
    }

    #[test]
    fn a_crlf_manifest_keeps_its_line_endings() {
        let original =
            "# The crate manifest.\r\n[package]\r\nname = \"momo\"\r\nversion = \"0.1.0\"\r\n";

        let updated = set_version(original, manifest(), &version("1.2.3"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(
            updated,
            "# The crate manifest.\r\n[package]\r\nname = \"momo\"\r\nversion = \"1.2.3\"\r\n"
        );
        assert!(
            !updated.replace("\r\n", "").contains('\n'),
            "a bare LF leaked into a CRLF manifest: {updated:?}"
        );
    }

    #[test]
    fn a_manifest_without_a_trailing_newline_keeps_none() {
        let original = "[package]\nname = \"momo\"\nversion = \"0.1.0\"";

        let updated = set_version(original, manifest(), &version("1.2.3"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(updated, "[package]\nname = \"momo\"\nversion = \"1.2.3\"");
        assert!(!updated.ends_with('\n'), "a trailing newline was added");
    }

    #[test]
    fn a_crlf_manifest_without_a_trailing_newline_keeps_both() {
        let original = "[package]\r\nname = \"momo\"\r\nversion = \"0.1.0\"";

        let updated = set_version(original, manifest(), &version("1.2.3"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(
            updated,
            "[package]\r\nname = \"momo\"\r\nversion = \"1.2.3\""
        );
    }

    #[test]
    fn mixed_line_endings_are_treated_as_lf() {
        let original = "[package]\r\nname = \"momo\"\nversion = \"0.1.0\"\n";

        let updated = set_version(original, manifest(), &version("1.2.3"))
            .expect("valid TOML")
            .expect("a version to write");

        assert_eq!(updated, "[package]\nname = \"momo\"\nversion = \"1.2.3\"\n");
    }
}
