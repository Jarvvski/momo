//! The Python manifest adapter: `pyproject.toml`, edited in place with
//! `toml_edit`.
//!
//! Two layouts can hold the version: `[project].version` (PEP 621) and
//! `[tool.poetry].version`. A Poetry 2 project migrating to PEP 621 carries
//! both, so when both are present **both** are rewritten — one document, two
//! scalar edits, never leaving one stale. The format-preserving edit itself is
//! [`super::toml`]; this adapter owns only which keys hold the version.
//!
//! A project whose version is computed elsewhere is *skipped*, not guessed:
//! `[project].dynamic = ["version"]` and the setuptools-scm, setuptools
//! dynamic, hatch-vcs and poetry-dynamic-versioning backends all mean the tag
//! *is* the version, so writing a scalar would break the setup. A static scalar
//! beside a dynamic declaration is a contradiction momo refuses to resolve.
//!
//! The value written is the PEP 440 form of the semver Version
//! ([`pep440::render`]): the tag keeps its semver spelling (`v1.2.0-rc.1`) while
//! the manifest holds `1.2.0rc1`.
//!
//! `setup.cfg` (INI) and `setup.py` (code) are out of scope: a project whose
//! only manifest is one of those stays a silent skip.

mod pep440;
#[cfg(test)]
mod roundtrip;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::manifest::{
    DependencyRequirement, ManifestEdits, ManifestError, ManifestPlan, ManifestSync, ManifestWrite,
    SkipReason,
};
use semver::Version;
use toml_edit::{DocumentMut, Item};

use super::toml;

/// The conventional Python manifest name.
const MANIFEST: &str = "pyproject.toml";

/// The Python manifest adapter.
pub struct PythonManifest;

impl ManifestSync for PythonManifest {
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
        let Some(version) = &edits.version else {
            // PEP 440 ranges are not rewritten yet, so an edits value carrying
            // no version has nothing for this adapter to write.
            return Ok(ManifestPlan::default());
        };
        let text = toml::read(manifest)?;

        Ok(match set_version(&text, manifest, version)? {
            Resolved::Write { path, text } => ManifestPlan {
                writes: vec![ManifestWrite { path, text }],
                ..ManifestPlan::default()
            },
            Resolved::Skip { path, reason } => ManifestPlan {
                skipped: vec![(path, reason)],
                ..ManifestPlan::default()
            },
        })
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

    /// The dependencies `pyproject.toml` declares.
    ///
    /// Not read yet: the cascade evaluates Cargo ranges only and reports a
    /// Python dependent as unchecked without consulting this. An empty list
    /// therefore means *not checked*, never "no dependencies".
    fn dependencies(&self, _manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        Ok(Vec::new())
    }

    /// PEP 440 ranges are not evaluated yet, so no Python requirement is ever
    /// reported satisfied.
    fn satisfied(&self, _requirement: &str, _version: &Version) -> Option<bool> {
        None
    }

    /// PEP 440 ranges are not rewritten yet, so no Python requirement is ever
    /// widened.
    fn widen(&self, _requirement: &str, _target: &Version) -> Option<String> {
        None
    }
}

/// The outcome of resolving one manifest against a Version.
#[derive(Debug, PartialEq, Eq)]
enum Resolved {
    /// The manifest to write and its new text.
    Write {
        /// The manifest targeted.
        path: PathBuf,
        /// The manifest's new text.
        text: String,
    },
    /// Nothing to write, and why.
    Skip {
        /// The manifest momo considered.
        path: PathBuf,
        /// Why it was left alone.
        reason: SkipReason,
    },
}

/// Render `text` with every version scalar set to `version`.
///
/// [`SkipReason::Dynamic`] when the version is computed elsewhere,
/// [`SkipReason::NoVersion`] when no version scalar exists, and
/// [`SkipReason::AlreadyCurrent`] when every scalar already holds `version`.
/// [`ManifestError::Conflict`] when a static scalar sits beside a dynamic
/// declaration, and when `version` has no PEP 440 form to write.
fn set_version(text: &str, manifest: &Path, version: &Version) -> Result<Resolved, ManifestError> {
    let mut doc = toml::parse(text, manifest)?;
    let sites = version_sites(&doc);

    if let Some(backend) = dynamic_backend(&doc) {
        if sites.is_empty() {
            return Ok(Resolved::Skip {
                path: manifest.to_path_buf(),
                reason: SkipReason::Dynamic(backend.to_string()),
            });
        }
        return Err(ManifestError::Conflict {
            path: manifest.to_path_buf(),
            message: format!(
                "the version is both static and dynamic ({backend}); remove the scalar or the \
                 dynamic declaration"
            ),
        });
    }

    if sites.is_empty() {
        return Ok(Resolved::Skip {
            path: manifest.to_path_buf(),
            reason: SkipReason::NoVersion,
        });
    }

    let target = pep440::render(version).map_err(|source| ManifestError::Conflict {
        path: manifest.to_path_buf(),
        message: source.to_string(),
    })?;

    let mut changed = false;
    for site in sites {
        if let Some(item) = site.item_mut(&mut doc) {
            changed |= toml::set_string(item, &target);
        }
    }
    if !changed {
        return Ok(Resolved::Skip {
            path: manifest.to_path_buf(),
            reason: SkipReason::AlreadyCurrent,
        });
    }

    Ok(Resolved::Write {
        path: manifest.to_path_buf(),
        text: toml::preserve_line_endings(text, doc.to_string()),
    })
}

/// A key that holds a version scalar momo owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    /// `[project].version` — PEP 621.
    Project,
    /// `[tool.poetry].version`.
    Poetry,
}

impl Site {
    /// The item this site names in `doc`, when the key is present.
    fn item_mut(self, doc: &mut DocumentMut) -> Option<&mut Item> {
        match self {
            Site::Project => doc.get_mut("project")?.get_mut("version"),
            Site::Poetry => doc.get_mut("tool")?.get_mut("poetry")?.get_mut("version"),
        }
    }
}

/// Every version scalar `doc` declares, in rewrite order.
///
/// `[project].version` first, then `[tool.poetry].version`, so a Poetry 2
/// project mid-migration has both rewritten together. Only string scalars
/// count: a `version` table (`[tool.setuptools.dynamic].version`) is a dynamic
/// spec, not a version momo owns.
fn version_sites(doc: &DocumentMut) -> Vec<Site> {
    let mut sites = Vec::new();
    if is_scalar(
        doc.get("project")
            .and_then(|project| project.get("version")),
    ) {
        sites.push(Site::Project);
    }
    if is_scalar(
        doc.get("tool")
            .and_then(|tool| tool.get("poetry"))
            .and_then(|poetry| poetry.get("version")),
    ) {
        sites.push(Site::Poetry);
    }
    sites
}

/// Whether `item` is a string scalar.
fn is_scalar(item: Option<&Item>) -> bool {
    item.is_some_and(|item| item.as_str().is_some())
}

/// The backend computing `doc`'s version, when one is declared.
///
/// Each of these means the version comes from somewhere other than a scalar —
/// the VCS, a file, an attribute — so writing one would fight the setup. A
/// project declares at most one; the first found wins.
fn dynamic_backend(doc: &DocumentMut) -> Option<DynamicBackend> {
    if dynamic_version_list(doc) {
        return Some(DynamicBackend::Project);
    }
    let tool = || doc.get("tool");
    if tool().and_then(|tool| tool.get("setuptools_scm")).is_some() {
        return Some(DynamicBackend::SetuptoolsScm);
    }
    if tool()
        .and_then(|tool| tool.get("setuptools"))
        .and_then(|setuptools| setuptools.get("dynamic"))
        .and_then(|dynamic| dynamic.get("version"))
        .is_some()
    {
        return Some(DynamicBackend::SetuptoolsDynamic);
    }
    if tool()
        .and_then(|tool| tool.get("hatch"))
        .and_then(|hatch| hatch.get("version"))
        .is_some()
    {
        return Some(DynamicBackend::Hatch);
    }
    if tool()
        .and_then(|tool| tool.get("poetry-dynamic-versioning"))
        .is_some()
    {
        return Some(DynamicBackend::PoetryDynamic);
    }
    None
}

/// Whether `[project].dynamic` lists `version`.
fn dynamic_version_list(doc: &DocumentMut) -> bool {
    doc.get("project")
        .and_then(|project| project.get("dynamic"))
        .and_then(|dynamic| dynamic.as_array())
        .is_some_and(|list| list.iter().any(|item| item.as_str() == Some("version")))
}

/// Where a dynamic version comes from, named in a [`SkipReason::Dynamic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DynamicBackend {
    /// PEP 621 `[project].dynamic = ["version"]`.
    Project,
    /// `[tool.setuptools_scm]`.
    SetuptoolsScm,
    /// `[tool.setuptools.dynamic].version`.
    SetuptoolsDynamic,
    /// `[tool.hatch.version]` (hatch-vcs).
    Hatch,
    /// `[tool.poetry-dynamic-versioning]`.
    PoetryDynamic,
}

impl fmt::Display for DynamicBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DynamicBackend::Project => "PEP 621 [project].dynamic",
            DynamicBackend::SetuptoolsScm => "setuptools-scm",
            DynamicBackend::SetuptoolsDynamic => "setuptools [tool.setuptools.dynamic].version",
            DynamicBackend::Hatch => "hatch-vcs",
            DynamicBackend::PoetryDynamic => "poetry-dynamic-versioning",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest path the tests attribute errors to.
    fn manifest() -> &'static Path {
        Path::new("pyproject.toml")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    /// The rendered text of a rewrite, panicking on a skip.
    fn updated(text: &str, version: &Version) -> String {
        match set_version(text, manifest(), version).expect("valid TOML") {
            Resolved::Write { text, .. } => text,
            Resolved::Skip { reason, .. } => panic!("expected a write, got a skip: {reason}"),
        }
    }

    /// The skip reason, panicking on a write.
    fn skipped(text: &str, version: &Version) -> SkipReason {
        match set_version(text, manifest(), version).expect("valid TOML") {
            Resolved::Skip { reason, .. } => reason,
            Resolved::Write { text, .. } => panic!("expected a skip, got a write:\n{text}"),
        }
    }

    #[test]
    fn a_project_version_is_rewritten_and_everything_else_survives() {
        let original = "\
# The project manifest.
[project]
name = \"acme\"   # the name
version = \"0.1.0\" # bumped by momo
requires-python = \">=3.11\"

[tool.ruff]
# a tool table
line-length = 88
";

        assert_eq!(
            updated(original, &version("1.2.3")),
            "\
# The project manifest.
[project]
name = \"acme\"   # the name
version = \"1.2.3\" # bumped by momo
requires-python = \">=3.11\"

[tool.ruff]
# a tool table
line-length = 88
"
        );
    }

    #[test]
    fn a_poetry_version_is_rewritten() {
        let original = "\
[tool.poetry]
name = \"acme\"
version = \"0.1.0\"
";
        assert_eq!(
            updated(original, &version("2.0.0")),
            "[tool.poetry]\nname = \"acme\"\nversion = \"2.0.0\"\n"
        );
    }

    #[test]
    fn both_sites_are_rewritten_together() {
        let original = "\
[project]
name = \"acme\"
version = \"0.1.0\"

[tool.poetry]
version = \"0.1.0\"
";
        assert_eq!(
            updated(original, &version("1.0.0")),
            "[project]\nname = \"acme\"\nversion = \"1.0.0\"\n\n[tool.poetry]\nversion = \"1.0.0\"\n"
        );
    }

    #[test]
    fn a_candidate_is_written_in_pep_440_form() {
        let original = "[project]\nname = \"acme\"\nversion = \"0.1.0\"\n";
        assert_eq!(
            updated(original, &version("1.2.0-rc.1")),
            "[project]\nname = \"acme\"\nversion = \"1.2.0rc1\"\n"
        );
    }

    #[test]
    fn an_unrepresentable_pre_release_is_a_conflict() {
        let original = "[project]\nname = \"acme\"\nversion = \"0.1.0\"\n";
        let err = set_version(original, manifest(), &version("1.2.0-dev.1"))
            .expect_err("a dev pre-release has no PEP 440 form");
        assert!(matches!(err, ManifestError::Conflict { .. }), "{err}");
        assert!(err.to_string().contains("pyproject.toml"), "{err}");
    }

    #[test]
    fn a_dynamic_project_version_is_skipped_with_its_backend() {
        let original = "\
[project]
name = \"acme\"
dynamic = [\"version\"]
";
        assert_eq!(
            skipped(original, &version("1.0.0")),
            SkipReason::Dynamic("PEP 621 [project].dynamic".to_owned())
        );
    }

    #[test]
    fn a_dynamic_list_without_version_is_not_dynamic() {
        let original = "\
[project]
name = \"acme\"
version = \"0.1.0\"
dynamic = [\"readme\"]
";
        assert_eq!(
            updated(original, &version("1.0.0")),
            "[project]\nname = \"acme\"\nversion = \"1.0.0\"\ndynamic = [\"readme\"]\n"
        );
    }

    #[test]
    fn each_dynamic_backend_is_skipped_with_its_name() {
        let cases = [
            (
                "[tool.setuptools_scm]\nversion_scheme = \"guess-next-dev\"\n",
                "setuptools-scm",
            ),
            (
                "[tool.setuptools.dynamic]\nversion = {file = \"VERSION\"}\n",
                "setuptools [tool.setuptools.dynamic].version",
            ),
            ("[tool.hatch.version]\nsource = \"vcs\"\n", "hatch-vcs"),
            (
                "[tool.poetry-dynamic-versioning]\nenable = true\n",
                "poetry-dynamic-versioning",
            ),
        ];

        for (original, name) in cases {
            assert_eq!(
                skipped(original, &version("1.0.0")),
                SkipReason::Dynamic(name.to_owned()),
                "{original}"
            );
        }
    }

    #[test]
    fn a_dynamic_declaration_beside_a_static_scalar_is_a_conflict() {
        let original = "\
[project]
name = \"acme\"
version = \"0.1.0\"
dynamic = [\"version\"]
";
        let err = set_version(original, manifest(), &version("1.0.0"))
            .expect_err("a static scalar beside a dynamic declaration conflicts");
        assert!(matches!(err, ManifestError::Conflict { .. }), "{err}");
        assert!(err.to_string().contains("PEP 621"), "{err}");
    }

    #[test]
    fn a_manifest_without_a_version_is_skipped() {
        let original = "[project]\nname = \"acme\"\n";
        assert_eq!(skipped(original, &version("1.0.0")), SkipReason::NoVersion);
    }

    #[test]
    fn a_dynamic_version_table_is_not_a_scalar() {
        // `[tool.setuptools.dynamic].version` is a table, never a scalar momo
        // would rewrite; the backend detection owns it instead.
        let original = "[tool.setuptools.dynamic]\nversion = {attr = \"acme.__version__\"}\n";
        assert_eq!(
            skipped(original, &version("1.0.0")),
            SkipReason::Dynamic("setuptools [tool.setuptools.dynamic].version".to_owned())
        );
    }

    #[test]
    fn an_unchanged_version_is_not_rewritten() {
        let original = "[project]\nname = \"acme\"\nversion = \"1.2.3\"\n";
        assert_eq!(
            skipped(original, &version("1.2.3")),
            SkipReason::AlreadyCurrent
        );
    }

    #[test]
    fn a_partially_current_pair_is_rewritten_wholesale() {
        let original = "\
[project]
version = \"1.2.3\"

[tool.poetry]
version = \"0.1.0\"
";
        assert_eq!(
            updated(original, &version("1.2.3")),
            "[project]\nversion = \"1.2.3\"\n\n[tool.poetry]\nversion = \"1.2.3\"\n"
        );
    }

    #[test]
    fn invalid_toml_is_reported_against_the_path() {
        let err = set_version("version = \n", manifest(), &version("1.0.0"))
            .expect_err("invalid TOML is an error");
        assert!(err.to_string().contains("pyproject.toml"), "{err}");
    }

    #[test]
    fn a_crlf_manifest_keeps_its_line_endings() {
        let original =
            "# The project manifest.\r\n[project]\r\nname = \"acme\"\r\nversion = \"0.1.0\"\r\n";
        let updated = updated(original, &version("1.2.3"));
        assert_eq!(
            updated,
            "# The project manifest.\r\n[project]\r\nname = \"acme\"\r\nversion = \"1.2.3\"\r\n"
        );
        assert!(
            !updated.replace("\r\n", "").contains('\n'),
            "a bare LF leaked into a CRLF manifest: {updated:?}"
        );
    }

    #[test]
    fn a_manifest_without_a_trailing_newline_keeps_none() {
        let original = "[project]\nname = \"acme\"\nversion = \"0.1.0\"";
        assert_eq!(
            updated(original, &version("1.2.3")),
            "[project]\nname = \"acme\"\nversion = \"1.2.3\""
        );
    }
}
