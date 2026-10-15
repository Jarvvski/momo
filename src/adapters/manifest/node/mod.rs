//! The Node manifest adapter: `package.json`, edited by byte span.
//!
//! momo rewrites only the top-level `version` string value and the string
//! values of a dependency's range, each spliced in place by [`json::splice`].
//! Nothing is re-rendered, so indentation, key order, the newline style and
//! whether the file ends in a newline survive byte-for-byte. Keying on the
//! exact top-level key `version` leaves `packageManager`, `engines`,
//! `publishConfig` and any `version` nested in `dependencies` or
//! `devDependencies` untouched by construction.
//!
//! npm ranges are read and evaluated by [`range`], a small hand-rolled
//! evaluator that refuses what it does not understand: a protocol entry
//! (`workspace:`, `file:`, `git:`, an `npm:` alias) is omitted from the
//! dependencies momo reports, and a range it cannot parse is *not checked*
//! rather than assumed satisfied.
//!
//! A Node lockfile (`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`,
//! `bun.lockb`) embeds the package's version and lives at the workspace root,
//! not beside the package, so it is found by walking up. It is *reported* as
//! possibly stale and left for the package manager to refresh — never rewritten.

mod json;
mod range;

#[cfg(test)]
mod roundtrip;

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use semver::Version;

use crate::domain::manifest::{
    DependencyRequirement, ManifestEdits, ManifestError, ManifestPlan, ManifestSync, ManifestWrite,
    SkipReason,
};

use json::JsonError;

/// The conventional Node manifest name.
const MANIFEST: &str = "package.json";

/// The dependency tables npm resolves, by the name each is declared under.
///
/// Every one is scanned, because mise's project graph reports an edge for a
/// dependency declared in any of them — a `devDependencies` entry is as much an
/// edge as a `dependencies` one.
const DEPENDENCY_TABLES: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

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

    fn render(
        &self,
        manifest: &Path,
        edits: &ManifestEdits,
    ) -> Result<ManifestPlan, ManifestError> {
        if edits.version.is_none() && edits.ranges.is_empty() {
            return Ok(ManifestPlan::default());
        }
        let text = read(manifest)?;
        let mut plan = ManifestPlan::default();
        let mut replacements: Vec<(Range<usize>, String)> = Vec::new();

        if let Some(version) = &edits.version {
            match set_version(&text, manifest, version)? {
                Resolved::Write { span } => replacements.push((span, version.to_string())),
                Resolved::Skip { reason } => {
                    plan.skipped.push((manifest.to_path_buf(), reason));
                }
            }
        }

        if !edits.ranges.is_empty() {
            let entries = json::dependency_entries(&text, &DEPENDENCY_TABLES)
                .map_err(|source| invalid(manifest, source))?;
            for edit in &edits.ranges {
                // A form momo cannot rewrite is left as the project wrote it and
                // reported by `for_package`, never guessed at.
                let Some(widened) = self.widen(&edit.requirement, &edit.target) else {
                    continue;
                };
                let mut applied = false;
                for entry in &entries {
                    if entry.name == edit.name && entry.requirement == edit.requirement {
                        replacements.push((entry.value.clone(), widened.clone()));
                        applied = true;
                    }
                }
                if applied {
                    plan.ranges.push(edit.clone());
                }
            }
        }

        if !replacements.is_empty() {
            plan.writes.push(ManifestWrite {
                path: manifest.to_path_buf(),
                text: json::splice(&text, &mut replacements),
            });
            plan.lockfiles = lockfiles(manifest);
        }
        Ok(plan)
    }

    fn write(&self, manifest: &Path, edits: &ManifestEdits) -> Result<ManifestPlan, ManifestError> {
        let plan = self.render(manifest, edits)?;
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
    /// Every npm dependency table is scanned. A protocol entry — `workspace:`,
    /// `file:`, `link:`, `portal:`, `git:`/`github:` and friends, an `npm:`
    /// alias, or a tarball URL — names *where* a dependency comes from, not
    /// which versions are acceptable, so it is omitted rather than reported as
    /// satisfied: it constrains nothing a Release could break. No npm range
    /// contains a `:`.
    fn dependencies(&self, manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        let text = read(manifest)?;
        let entries = json::dependency_entries(&text, &DEPENDENCY_TABLES)
            .map_err(|source| invalid(manifest, source))?;
        Ok(entries
            .into_iter()
            .filter(|entry| !entry.requirement.contains(':'))
            .map(|entry| DependencyRequirement {
                package: entry.name,
                requirement: entry.requirement,
            })
            .collect())
    }

    /// Whether `version` satisfies the npm range `requirement`.
    ///
    /// `None` when the range is not one momo understands — a protocol entry, or
    /// a grammar this evaluator refuses — so an edge it cannot read is reported
    /// as *not checked*, never as satisfied.
    fn satisfied(&self, requirement: &str, version: &Version) -> Option<bool> {
        range::satisfied(requirement, version)
    }

    /// Widen an npm range to admit `target`, preserving its operator form.
    fn widen(&self, requirement: &str, target: &Version) -> Option<String> {
        range::widen(requirement, target)
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
    /// The byte span of the top-level `version` value to replace.
    Write {
        /// The span of the value literal, quotes included.
        span: Range<usize>,
    },
    /// Nothing to write, and why.
    Skip {
        /// Why the manifest was left alone.
        reason: SkipReason,
    },
}

/// Locate `text`'s top-level `version` value, when momo should write `version`.
///
/// `Resolved::Skip` when the manifest holds no version momo owns, or already
/// carries `version` — in both cases there is nothing to write, and the reason
/// says which.
fn set_version(text: &str, manifest: &Path, version: &Version) -> Result<Resolved, ManifestError> {
    let Some(span) = json::version_span(text).map_err(|source| invalid(manifest, source))? else {
        return Ok(Resolved::Skip {
            reason: SkipReason::NoVersion,
        });
    };
    if json::inner(text, &span) == version.to_string() {
        return Ok(Resolved::Skip {
            reason: SkipReason::AlreadyCurrent,
        });
    }
    Ok(Resolved::Write { span })
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

    use crate::domain::manifest::DependencyRangeEdit;
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
            Resolved::Write { span } => json::splice(text, &mut [(span, version.to_string())]),
            Resolved::Skip { reason } => panic!("expected a write, got a skip: {reason}"),
        }
    }

    /// The requirements `text` declares, read from `manifest`.
    fn requirements(text: &str) -> Vec<DependencyRequirement> {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("package.json");
        fs::write(&path, text).expect("seed the manifest");
        NodeManifest
            .dependencies(&path)
            .expect("read the dependencies")
    }

    /// The edits that widen `name`'s `requirement` to admit `target`.
    fn widening(name: &str, requirement: &str, target: &str) -> ManifestEdits {
        ManifestEdits {
            version: None,
            ranges: vec![DependencyRangeEdit {
                name: name.to_owned(),
                requirement: requirement.to_owned(),
                target: version(target),
            }],
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

    #[test]
    fn every_dependency_table_is_read() {
        let text = "{\n  \"dependencies\": {\"a\": \"^1.0.0\"},\n  \
                    \"devDependencies\": {\"b\": \"~2.0.0\"},\n  \
                    \"peerDependencies\": {\"c\": \"3.0.0\"},\n  \
                    \"optionalDependencies\": {\"d\": \"*\"}\n}\n";

        assert_eq!(
            requirements(text),
            vec![
                DependencyRequirement {
                    package: "a".to_owned(),
                    requirement: "^1.0.0".to_owned(),
                },
                DependencyRequirement {
                    package: "b".to_owned(),
                    requirement: "~2.0.0".to_owned(),
                },
                DependencyRequirement {
                    package: "c".to_owned(),
                    requirement: "3.0.0".to_owned(),
                },
                DependencyRequirement {
                    package: "d".to_owned(),
                    requirement: "*".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn a_protocol_entry_is_omitted_rather_than_reported() {
        // A protocol entry constrains nothing momo could break, so it is not a
        // requirement at all.
        let text = "{\n  \"dependencies\": {\n    \
                    \"local\": \"workspace:*\",\n    \
                    \"file\": \"file:../other\",\n    \
                    \"git\": \"github:acme/other\",\n    \
                    \"alias\": \"npm:other@^1.0.0\",\n    \
                    \"real\": \"^2.0.0\"\n  }\n}\n";

        assert_eq!(
            requirements(text),
            vec![DependencyRequirement {
                package: "real".to_owned(),
                requirement: "^2.0.0".to_owned(),
            }]
        );
    }

    #[test]
    fn a_requirement_is_matched_with_npms_semantics() {
        let node = NodeManifest;
        assert_eq!(node.satisfied("^1.0.0", &version("1.5.0")), Some(true));
        assert_eq!(node.satisfied("^1.0.0", &version("2.0.0")), Some(false));
        assert_eq!(node.satisfied("~1.2", &version("1.3.0")), Some(false));
        assert_eq!(
            node.satisfied(">=1.0.0 <2.0.0", &version("1.5.0")),
            Some(true)
        );
        assert_eq!(node.satisfied("*", &version("9.9.9")), Some(true));
        assert_eq!(
            node.satisfied("workspace:*", &version("1.0.0")),
            None,
            "a protocol entry is not a range"
        );
    }

    #[test]
    fn a_node_range_is_widened_by_preserving_its_operator() {
        let node = NodeManifest;
        for (requirement, expected) in [
            ("^1.0.0", "^2.0.0"),
            ("~1.2", "~2.0.0"),
            ("1.2.3", "2.0.0"),
            (">=1.0.0", ">=2.0.0"),
        ] {
            assert_eq!(
                node.widen(requirement, &version("2.0.0")).as_deref(),
                Some(expected),
                "`{requirement}` widened wrongly"
            );
        }
        for requirement in ["*", "1.x", ">1.0.0", "<2.0.0", "^1.0.0 || ^2.0.0"] {
            assert_eq!(
                node.widen(requirement, &version("2.0.0")),
                None,
                "`{requirement}` was rewritten instead of refused"
            );
        }
    }

    #[test]
    fn a_range_is_widened_in_place_and_the_rest_of_the_file_survives() {
        let original = "{\n  \"name\": \"@acme/ui\",\n  \"version\": \"0.1.0\",\n  \
                        \"dependencies\": {\"a\": \"^0.1.0\"},\n  \
                        \"scripts\": {\"a\": \"^0.1.0\"}\n}\n";
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("package.json");
        fs::write(&path, original).expect("seed the manifest");

        let plan = NodeManifest
            .render(&path, &widening("a", "^0.1.0", "0.2.0"))
            .expect("render the manifest");

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert_eq!(
            plan.writes[0].text,
            "{\n  \"name\": \"@acme/ui\",\n  \"version\": \"0.1.0\",\n  \
             \"dependencies\": {\"a\": \"^0.2.0\"},\n  \
             \"scripts\": {\"a\": \"^0.1.0\"}\n}\n"
        );
        assert_eq!(plan.ranges.len(), 1, "{plan:?}");
    }

    #[test]
    fn a_version_and_a_range_compose_into_one_write() {
        let original = "{\n  \"version\": \"0.1.0\",\n  \"dependencies\": {\"a\": \"^0.1.0\"}\n}\n";
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("package.json");
        fs::write(&path, original).expect("seed the manifest");
        let edits = ManifestEdits {
            version: Some(version("0.2.0")),
            ranges: widening("a", "^0.1.0", "0.2.0").ranges,
        };

        let plan = NodeManifest
            .render(&path, &edits)
            .expect("render the manifest");

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert_eq!(
            plan.writes[0].text,
            "{\n  \"version\": \"0.2.0\",\n  \"dependencies\": {\"a\": \"^0.2.0\"}\n}\n"
        );
    }

    #[test]
    fn an_unwritable_range_leaves_the_manifest_untouched() {
        let original = "{\"version\": \"0.1.0\", \"dependencies\": {\"a\": \"1.x\"}}\n";
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("package.json");
        fs::write(&path, original).expect("seed the manifest");

        let plan = NodeManifest
            .render(&path, &widening("a", "1.x", "2.0.0"))
            .expect("render the manifest");

        assert!(plan.writes.is_empty(), "{plan:?}");
        assert!(plan.ranges.is_empty(), "{plan:?}");
    }
}
