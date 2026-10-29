//! The Go manifest adapter: `go.mod`, read-only.
//!
//! A Go module has no version scalar — its version *is* the git tag — so momo
//! never writes a `go.mod`: [`GoManifest::render`] and [`GoManifest::write`]
//! report a skip rather than invent one. The adapter exists to answer the
//! cascade's question: which modules this one `require`s, and whether a new
//! Version satisfies the pin.
//!
//! Go has no ranges. A `require` names one exact version, and semantic import
//! versioning gives a module its `/vN` suffix only from major 2 onward — `v0`
//! and `v1` share one unsuffixed path, so a `v0`→`v1` upgrade is a pin bump,
//! not an import-path change. momo's rule is therefore: a pin is satisfied
//! when the target is at least it and the two share a module path, meaning
//! majors 0 and 1 count as one path while majors ≥ 2 must match exactly. A
//! target in a different module path is reported as unsatisfied — the fix is
//! an import-path change, not a pin bump — while `widen` bumps the pin to the
//! target for the cases where that is what the project wants. The adapter is
//! read-only, so the cascade reports a Go edge but never rewrites it.

use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;

use crate::domain::manifest::{
    DependencyRequirement, ManifestEdits, ManifestError, ManifestPlan, ManifestSync, SkipReason,
};

/// The conventional Go manifest name.
const MANIFEST: &str = "go.mod";

/// The Go manifest adapter.
pub struct GoManifest;

impl ManifestSync for GoManifest {
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf> {
        let path = match override_path {
            Some(override_path) => package_dir.join(override_path),
            None => package_dir.join(MANIFEST),
        };
        path.is_file().then_some(path)
    }

    /// A Go module holds no version scalar momo owns, so nothing is written:
    /// the manifest is reported as skipped, never invented.
    fn render(
        &self,
        manifest: &Path,
        _edits: &ManifestEdits,
    ) -> Result<ManifestPlan, ManifestError> {
        Ok(ManifestPlan {
            skipped: vec![(manifest.to_path_buf(), SkipReason::NoVersion)],
            ..ManifestPlan::default()
        })
    }

    /// A Go module is read-only, so [`ManifestSync::write`] writes nothing and
    /// reports the same skip as [`ManifestSync::render`].
    fn write(&self, manifest: &Path, edits: &ManifestEdits) -> Result<ManifestPlan, ManifestError> {
        self.render(manifest, edits)
    }

    /// The modules `go.mod` requires, each pinned to one version.
    ///
    /// Both `require` forms are read — the block and the single-line — and an
    /// `// indirect` comment is ignored, because an indirect requirement is
    /// still a requirement the graph reports an edge for.
    fn dependencies(&self, manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        let text = read(manifest)?;
        Ok(requires(&text))
    }

    /// Whether `version` satisfies the pinned `requirement`.
    ///
    /// `None` when the pin is not a Go version momo can read. The rule: the
    /// target must share the pin's module path — majors 0 and 1 are one path,
    /// majors ≥ 2 must match exactly — and be at least the pinned version.
    fn satisfied(&self, requirement: &str, version: &Version) -> Option<bool> {
        let pinned = parse_version(requirement)?;
        Some(
            module_path_major(pinned.major) == module_path_major(version.major)
                && *version >= pinned,
        )
    }

    /// Bump the pin to `target`.
    ///
    /// Go has no range to preserve, so the rewrite is the target's Go version.
    /// The result is verified to admit `target` before it is returned.
    fn widen(&self, requirement: &str, target: &Version) -> Option<String> {
        parse_version(requirement)?;
        let widened = format!("v{target}");
        match self.satisfied(&widened, target) {
            Some(true) => Some(widened),
            _ => None,
        }
    }
}

/// The module path a major selects.
///
/// Go's semantic import versioning appends a `/vN` suffix only from major 2
/// onward, so majors 0 and 1 name one unsuffixed path while every major from 2
/// up names its own.
fn module_path_major(major: u64) -> u64 {
    if major < 2 { 0 } else { major }
}

/// Read `manifest`, attributing a failure to its path.
fn read(manifest: &Path) -> Result<String, ManifestError> {
    fs::read_to_string(manifest).map_err(|source| ManifestError::Read {
        path: manifest.to_path_buf(),
        source,
    })
}

/// Parse a Go version (`v1.2.3`, `v0.0.0-20240101000000-abcdef`) as semver.
///
/// Go versions always carry a leading `v`, which semver does not; anything
/// without one, or that is not semver, is refused rather than guessed at.
fn parse_version(requirement: &str) -> Option<Version> {
    Version::parse(requirement.strip_prefix('v')?).ok()
}

/// The modules `text` requires, from both `require` forms.
fn requires(text: &str) -> Vec<DependencyRequirement> {
    let mut requirements = Vec::new();
    let mut in_block = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if in_block {
            if line == ")" {
                in_block = false;
            } else if let Some(requirement) = require_entry(line) {
                requirements.push(requirement);
            }
            continue;
        }
        let Some(rest) = line.strip_prefix("require") else {
            continue;
        };
        if !rest.is_empty() && !rest.starts_with([' ', '\t', '(']) {
            continue;
        }
        let rest = rest.trim_start();
        if rest.starts_with('(') {
            in_block = true;
        } else if let Some(requirement) = require_entry(rest) {
            requirements.push(requirement);
        }
    }
    requirements
}

/// One `require` entry: a module path and its pinned version.
fn require_entry(line: &str) -> Option<DependencyRequirement> {
    let mut fields = line.split_whitespace();
    Some(DependencyRequirement {
        package: fields.next()?.to_owned(),
        requirement: fields.next()?.to_owned(),
    })
}

/// `line` up to its `//` comment.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) => &line[..at],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    /// A `go.mod` with both require forms, a comment and an unrelated directive.
    const MODULE: &str = "\
module example.com/acme/api

go 1.22

require example.com/other v1.2.3

require (
	example.com/acme/a v0.1.0
	example.com/acme/b v1.4.0 // indirect
)

replace example.com/other => ../other
";

    fn requirements(text: &str) -> Vec<DependencyRequirement> {
        requires(text)
    }

    #[test]
    fn both_require_forms_are_read() {
        assert_eq!(
            requirements(MODULE),
            vec![
                DependencyRequirement {
                    package: "example.com/other".to_owned(),
                    requirement: "v1.2.3".to_owned(),
                },
                DependencyRequirement {
                    package: "example.com/acme/a".to_owned(),
                    requirement: "v0.1.0".to_owned(),
                },
                DependencyRequirement {
                    package: "example.com/acme/b".to_owned(),
                    requirement: "v1.4.0".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn locate_finds_the_conventional_manifest() {
        let dir = TempDir::new().expect("temp dir");
        assert_eq!(GoManifest.locate(dir.path(), None), None);

        fs::write(dir.path().join("go.mod"), MODULE).expect("seed the manifest");
        assert_eq!(
            GoManifest.locate(dir.path(), None),
            Some(dir.path().join("go.mod"))
        );
    }

    #[test]
    fn a_pin_is_satisfied_within_its_major_and_at_or_above_it() {
        let go = GoManifest;
        assert_eq!(go.satisfied("v0.1.0", &version("0.1.0")), Some(true));
        assert_eq!(go.satisfied("v0.1.0", &version("0.2.0")), Some(true));
        assert_eq!(go.satisfied("v1.4.0", &version("1.4.1")), Some(true));
        assert_eq!(go.satisfied("v1.4.0", &version("1.3.0")), Some(false));
    }

    #[test]
    fn only_major_two_or_greater_is_a_different_module_path() {
        let go = GoManifest;
        assert_eq!(go.satisfied("v1.4.0", &version("2.0.0")), Some(false));
        assert_eq!(go.satisfied("v2.0.0", &version("3.0.0")), Some(false));
        // `v0` and `v1` share one unsuffixed module path.
        assert_eq!(go.satisfied("v0.1.0", &version("1.0.0")), Some(true));
        assert_eq!(go.satisfied("v1.4.0", &version("0.9.0")), Some(false));
    }

    #[test]
    fn an_unreadable_pin_is_refused() {
        let go = GoManifest;
        assert_eq!(go.satisfied("1.2.3", &version("1.2.3")), None);
        assert_eq!(go.satisfied("not a version", &version("1.2.3")), None);
        assert_eq!(go.satisfied("", &version("1.2.3")), None);
    }

    #[test]
    fn a_pin_is_widened_to_the_target() {
        let go = GoManifest;
        assert_eq!(
            go.widen("v0.1.0", &version("0.2.0")).as_deref(),
            Some("v0.2.0")
        );
        assert_eq!(
            go.widen("v1.2.3", &version("1.3.0")).as_deref(),
            Some("v1.3.0")
        );
        assert_eq!(go.widen("not a version", &version("1.0.0")), None);
    }

    #[test]
    fn a_widened_pin_admits_its_target() {
        let go = GoManifest;
        let target = version("0.2.0");
        let widened = go.widen("v0.1.0", &target).expect("a widen");
        assert_eq!(go.satisfied(&widened, &target), Some(true));
    }

    #[test]
    fn rendering_reports_a_skip_rather_than_inventing_a_version() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("go.mod");
        fs::write(&path, MODULE).expect("seed the manifest");

        let plan = GoManifest
            .render(&path, &ManifestEdits::version(version("2.0.0")))
            .expect("render the manifest");

        assert!(plan.writes.is_empty(), "{plan:?}");
        assert_eq!(
            plan.skipped,
            vec![(path.clone(), SkipReason::NoVersion)],
            "{plan:?}"
        );
    }

    #[test]
    fn writing_writes_nothing() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("go.mod");
        fs::write(&path, MODULE).expect("seed the manifest");

        let plan = GoManifest
            .write(&path, &ManifestEdits::version(version("2.0.0")))
            .expect("write the manifest");

        assert!(plan.writes.is_empty(), "{plan:?}");
        assert_eq!(
            fs::read_to_string(&path).expect("read the manifest"),
            MODULE
        );
    }
}
