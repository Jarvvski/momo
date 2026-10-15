//! The Node manifest adapter: `package.json`, edited by byte span.
//!
//! momo rewrites only the top-level `version` string value, spliced in place by
//! [`json::set_version`]. Nothing is re-rendered, so indentation, key order, the
//! newline style and whether the file ends in a newline survive byte-for-byte.
//! Keying on the exact top-level key `version` leaves `packageManager`,
//! `engines`, `publishConfig` and any `version` nested in `dependencies` or
//! `devDependencies` untouched by construction.
//!
//! A Node lockfile (`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`,
//! `bun.lockb`) embeds the package's version and lives at the workspace root,
//! not beside the package, so it is found by walking up. It is *reported* as
//! possibly stale and left for the package manager to refresh — never rewritten.

mod json;

#[cfg(test)]
mod roundtrip;

use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;

use crate::domain::manifest::{
    DependencyRequirement, ManifestError, ManifestPlan, ManifestSync, ManifestWrite, SkipReason,
};

use json::JsonError;

/// The conventional Node manifest name.
const MANIFEST: &str = "package.json";

/// The lockfiles a Node package manager maintains, in reporting order.
const LOCKFILES: [&str; 5] = [
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "bun.lock",
];

/// The Node manifest adapter.
pub struct NodeManifest;

impl ManifestSync for NodeManifest {
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf> {
        let path = match override_path {
            Some(override_path) => package_dir.join(override_path),
            None => package_dir.join(MANIFEST),
        };
        path.is_file().then_some(path)
    }

    fn render(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError> {
        let text = read(manifest)?;

        Ok(match set_version(&text, manifest, version)? {
            Resolved::Write { text } => ManifestPlan {
                lockfiles: lockfiles(manifest),
                writes: vec![ManifestWrite {
                    path: manifest.to_path_buf(),
                    text,
                }],
                skipped: Vec::new(),
            },
            Resolved::Skip { reason } => ManifestPlan {
                writes: Vec::new(),
                lockfiles: Vec::new(),
                skipped: vec![(manifest.to_path_buf(), reason)],
            },
        })
    }

    fn write(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError> {
        let plan = self.render(manifest, version)?;
        for write in &plan.writes {
            fs::write(&write.path, &write.text).map_err(|source| ManifestError::Write {
                path: write.path.clone(),
                source,
            })?;
        }

        Ok(plan)
    }

    /// The dependencies `package.json` declares.
    ///
    /// Not read yet: the cascade evaluates Cargo ranges only and reports a Node
    /// dependent as unchecked without consulting this. An empty list therefore
    /// means *not checked*, never "no dependencies".
    fn dependencies(&self, _manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        Ok(Vec::new())
    }

    /// npm ranges are not evaluated yet, so no Node requirement is ever reported
    /// satisfied.
    fn satisfied(&self, _requirement: &str, _version: &Version) -> Option<bool> {
        None
    }
}

/// Read `manifest`, attributing a failure to its path.
fn read(manifest: &Path) -> Result<String, ManifestError> {
    fs::read_to_string(manifest).map_err(|source| ManifestError::Read {
        path: manifest.to_path_buf(),
        source,
    })
}

/// The outcome of resolving one `package.json` against a Version.
#[derive(Debug, PartialEq, Eq)]
enum Resolved {
    /// The manifest's new text.
    Write {
        /// The manifest's new text.
        text: String,
    },
    /// Nothing to write, and why.
    Skip {
        /// Why the manifest was left alone.
        reason: SkipReason,
    },
}

/// Render `text` with its top-level `version` set to `version`.
///
/// `Resolved::Skip` when the manifest holds no version momo owns, or already
/// carries `version` — in both cases there is nothing to write, and the reason
/// says which.
fn set_version(text: &str, manifest: &Path, version: &Version) -> Result<Resolved, ManifestError> {
    if json::version_span(text)
        .map_err(|source| invalid(manifest, source))?
        .is_none()
    {
        return Ok(Resolved::Skip {
            reason: SkipReason::NoVersion,
        });
    }
    match json::set_version(text, &version.to_string())
        .map_err(|source| invalid(manifest, source))?
    {
        Some(text) => Ok(Resolved::Write { text }),
        None => Ok(Resolved::Skip {
            reason: SkipReason::AlreadyCurrent,
        }),
    }
}

/// Attribute a JSON scan failure to `manifest`.
fn invalid(manifest: &Path, source: JsonError) -> ManifestError {
    ManifestError::Parse {
        path: manifest.to_path_buf(),
        source: Box::new(source),
    }
}

/// The lockfiles the package manager keeps at the workspace root.
///
/// A Node lockfile sits beside the workspace root, not the package, so it is
/// found by walking up from the manifest. The walk stops at the first ancestor
/// that holds one — a repo mid-migration may hold two, an `npm` and a `pnpm`
/// lockfile, and both are reported. Empty when none is found.
fn lockfiles(manifest: &Path) -> Vec<PathBuf> {
    let Some(dir) = manifest.parent() else {
        return Vec::new();
    };
    for ancestor in dir.ancestors() {
        let found: Vec<PathBuf> = LOCKFILES
            .iter()
            .map(|name| ancestor.join(name))
            .filter(|path| path.is_file())
            .collect();
        if !found.is_empty() {
            return found;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    /// The manifest path the tests attribute errors to.
    fn manifest() -> &'static Path {
        Path::new("package.json")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    /// The rendered text of a rewrite, panicking on a skip.
    fn updated(text: &str, version: &Version) -> String {
        match set_version(text, manifest(), version).expect("valid JSON") {
            Resolved::Write { text } => text,
            Resolved::Skip { reason } => panic!("expected a write, got a skip: {reason}"),
        }
    }

    #[test]
    fn two_space_indentation_survives() {
        let original =
            "{\n  \"name\": \"ui\",\n  \"version\": \"1.0.0\",\n  \"private\": true\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"name\": \"ui\",\n  \"version\": \"2.0.0\",\n  \"private\": true\n}\n"
        );
    }

    #[test]
    fn four_space_indentation_survives() {
        let original = "{\n    \"version\": \"1.0.0\"\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n    \"version\": \"2.0.0\"\n}\n"
        );
    }

    #[test]
    fn tab_indentation_survives() {
        let original = "{\n\t\"name\": \"ui\",\n\t\"version\": \"1.0.0\"\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n\t\"name\": \"ui\",\n\t\"version\": \"2.0.0\"\n}\n"
        );
    }

    #[test]
    fn key_order_survives() {
        let original =
            "{\n  \"version\": \"1.0.0\",\n  \"name\": \"ui\",\n  \"license\": \"MIT\"\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"version\": \"2.0.0\",\n  \"name\": \"ui\",\n  \"license\": \"MIT\"\n}\n"
        );
    }

    #[test]
    fn a_trailing_newline_survives() {
        let original = "{\"version\": \"1.0.0\"}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\"version\": \"2.0.0\"}\n"
        );
    }

    #[test]
    fn a_missing_trailing_newline_survives() {
        let original = "{\"version\": \"1.0.0\"}";
        let updated = updated(original, &version("2.0.0"));
        assert_eq!(updated, "{\"version\": \"2.0.0\"}");
        assert!(!updated.ends_with('\n'), "a trailing newline was added");
    }

    #[test]
    fn crlf_line_endings_survive() {
        let original = "{\r\n  \"name\": \"ui\",\r\n  \"version\": \"1.0.0\"\r\n}\r\n";
        let updated = updated(original, &version("2.0.0"));
        assert_eq!(
            updated,
            "{\r\n  \"name\": \"ui\",\r\n  \"version\": \"2.0.0\"\r\n}\r\n"
        );
        assert!(
            !updated.replace("\r\n", "").contains('\n'),
            "a bare LF leaked into a CRLF manifest: {updated:?}"
        );
    }

    #[test]
    fn a_toolchain_pin_and_config_keys_are_untouched() {
        let original = "{\n  \"name\": \"ui\",\n  \"version\": \"1.0.0\",\n  \"packageManager\": \"pnpm@9.0.0\",\n  \"engines\": { \"node\": \">=20\" },\n  \"publishConfig\": { \"access\": \"public\" }\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"name\": \"ui\",\n  \"version\": \"2.0.0\",\n  \"packageManager\": \"pnpm@9.0.0\",\n  \"engines\": { \"node\": \">=20\" },\n  \"publishConfig\": { \"access\": \"public\" }\n}\n"
        );
    }

    #[test]
    fn a_dependency_version_is_untouched() {
        let original = "{\n  \"name\": \"ui\",\n  \"version\": \"1.0.0\",\n  \"dependencies\": { \"version\": \"^3.0.0\" },\n  \"devDependencies\": { \"other\": \"workspace:*\" }\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"name\": \"ui\",\n  \"version\": \"2.0.0\",\n  \"dependencies\": { \"version\": \"^3.0.0\" },\n  \"devDependencies\": { \"other\": \"workspace:*\" }\n}\n"
        );
    }

    #[test]
    fn a_nested_version_is_untouched() {
        let original =
            "{\n  \"version\": \"1.0.0\",\n  \"nested\": { \"version\": \"9.9.9\" }\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"version\": \"2.0.0\",\n  \"nested\": { \"version\": \"9.9.9\" }\n}\n"
        );
    }

    #[test]
    fn a_private_package_still_bumps_its_version() {
        let original =
            "{\n  \"name\": \"ui\",\n  \"private\": true,\n  \"version\": \"1.0.0\"\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"name\": \"ui\",\n  \"private\": true,\n  \"version\": \"2.0.0\"\n}\n"
        );
    }

    #[test]
    fn a_duplicate_version_takes_the_last_one() {
        let original = "{\n  \"version\": \"1.0.0\",\n  \"version\": \"1.1.0\"\n}\n";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "{\n  \"version\": \"1.0.0\",\n  \"version\": \"2.0.0\"\n}\n"
        );
    }

    #[test]
    fn a_manifest_without_a_version_is_skipped() {
        assert_eq!(
            set_version("{\"name\": \"ui\"}", manifest(), &version("2.0.0")).expect("valid JSON"),
            Resolved::Skip {
                reason: SkipReason::NoVersion,
            }
        );
    }

    #[test]
    fn an_unchanged_version_is_not_rewritten() {
        assert_eq!(
            set_version("{\"version\": \"2.0.0\"}", manifest(), &version("2.0.0"))
                .expect("valid JSON"),
            Resolved::Skip {
                reason: SkipReason::AlreadyCurrent,
            }
        );
    }

    #[test]
    fn invalid_json_is_reported_against_the_path() {
        let err = set_version("{\"version\": }", manifest(), &version("2.0.0"))
            .expect_err("invalid JSON is an error");
        assert!(err.to_string().contains("package.json"), "{err}");
        assert!(matches!(err, ManifestError::Parse { .. }), "{err}");
    }

    #[test]
    fn locate_finds_the_conventional_manifest() {
        let dir = TempDir::new().expect("temp dir");
        assert_eq!(NodeManifest.locate(dir.path(), None), None);

        fs::write(dir.path().join("package.json"), "{}").expect("seed the manifest");
        assert_eq!(
            NodeManifest.locate(dir.path(), None),
            Some(dir.path().join("package.json"))
        );
    }

    #[test]
    fn locate_prefers_the_configured_override() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("package.json"), "{}").expect("seed the conventional manifest");
        fs::write(dir.path().join("alt.json"), "{}").expect("seed the override");

        assert_eq!(
            NodeManifest.locate(dir.path(), Some(Path::new("alt.json"))),
            Some(dir.path().join("alt.json"))
        );
    }

    #[test]
    fn locate_skips_an_override_that_does_not_exist() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("package.json"), "{}").expect("seed the manifest");

        assert_eq!(
            NodeManifest.locate(dir.path(), Some(Path::new("nope.json"))),
            None
        );
    }

    #[test]
    fn a_lockfile_beside_the_package_is_found() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, "{}").expect("seed the manifest");
        let lockfile = dir.path().join("pnpm-lock.yaml");
        fs::write(&lockfile, "lockfileVersion: '9.0'\n").expect("seed the lockfile");

        assert_eq!(lockfiles(&manifest), vec![lockfile]);
    }

    #[test]
    fn a_lockfile_is_found_by_walking_up_to_the_workspace_root() {
        let dir = TempDir::new().expect("temp dir");
        let package = dir.path().join("packages/ui");
        fs::create_dir_all(&package).expect("create the package dir");
        let manifest = package.join("package.json");
        fs::write(&manifest, "{}").expect("seed the manifest");
        let lockfile = dir.path().join("pnpm-lock.yaml");
        fs::write(&lockfile, "lockfileVersion: '9.0'\n").expect("seed the root lockfile");

        assert_eq!(lockfiles(&manifest), vec![lockfile]);
    }

    #[test]
    fn two_lockfiles_at_the_root_are_both_reported() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, "{}").expect("seed the manifest");
        let npm = dir.path().join("package-lock.json");
        let pnpm = dir.path().join("pnpm-lock.yaml");
        fs::write(&npm, "{}").expect("seed the npm lockfile");
        fs::write(&pnpm, "lockfileVersion: '9.0'\n").expect("seed the pnpm lockfile");

        assert_eq!(lockfiles(&manifest), vec![npm, pnpm]);
    }

    #[test]
    fn the_nearest_lockfile_wins_over_one_further_up() {
        // The walk stops at the nearest ancestor holding a lockfile, so a stray
        // lockfile above the workspace root is not reached.
        let dir = TempDir::new().expect("temp dir");
        let workspace = dir.path().join("workspace");
        let package = workspace.join("packages/ui");
        fs::create_dir_all(&package).expect("create the package dir");
        let manifest = package.join("package.json");
        fs::write(&manifest, "{}").expect("seed the manifest");
        fs::write(dir.path().join("pnpm-lock.yaml"), "outer\n").expect("seed the outer lockfile");
        let inner = workspace.join("pnpm-lock.yaml");
        fs::write(&inner, "inner\n").expect("seed the inner lockfile");

        assert_eq!(lockfiles(&manifest), vec![inner]);
    }

    #[test]
    fn no_lockfile_anywhere_is_empty() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, "{}").expect("seed the manifest");

        assert!(lockfiles(&manifest).is_empty());
    }
}
