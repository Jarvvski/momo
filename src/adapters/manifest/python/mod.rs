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
//! beside a dynamic declaration is a contradiction momo refuses to resolve —
//! except for `poetry-dynamic-versioning`, whose declaration *requires* a static
//! placeholder scalar, so the scalar beside it is the placeholder, not a second
//! source of truth, and the project is still skipped.
//!
//! The value written is the PEP 440 form of the semver Version
//! ([`pep440::render`]): the tag keeps its semver spelling (`v1.2.0-rc.1`) while
//! the manifest holds `1.2.0rc1`.
//!
//! `setup.cfg` (INI) and `setup.py` (code) are out of scope: a project whose
//! only manifest is one of those stays a silent skip.
//!
//! Dependency ranges are read and evaluated by [`requirement`]: a PEP 508
//! requirement string from `[project].dependencies`, and Poetry's
//! `[tool.poetry.dependencies]`. A requirement momo cannot evaluate is
//! *not checked* rather than assumed satisfied, and a range is rewritten only
//! when it is a single-term form momo understands.

mod pep440;
mod requirement;
#[cfg(test)]
mod roundtrip;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::manifest::{
    DependencyRangeEdit, DependencyRequirement, ManifestEdits, ManifestError, ManifestPlan,
    ManifestSync, ManifestWrite, SkipReason,
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
        if edits.version.is_none() && edits.ranges.is_empty() {
            return Ok(ManifestPlan::default());
        }
        let text = toml::read(manifest)?;
        let mut doc = toml::parse(&text, manifest)?;
        let mut plan = ManifestPlan::default();
        let mut changed = false;

        if let Some(version) = &edits.version {
            match apply_version(&mut doc, manifest, version)? {
                Applied::Write => changed = true,
                Applied::Skip { path, reason } => plan.skipped.push((path, reason)),
            }
        }
        for edit in &edits.ranges {
            if widen_range(&mut doc, edit) {
                plan.ranges.push(edit.clone());
                changed = true;
            }
        }

        if changed {
            plan.writes.push(ManifestWrite {
                path: manifest.to_path_buf(),
                text: toml::preserve_line_endings(&text, doc.to_string()),
            });
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

    /// The dependencies `pyproject.toml` declares.
    ///
    /// Two layouts are read: PEP 621 `[project].dependencies` (PEP 508
    /// requirement strings) and Poetry's `[tool.poetry.dependencies]`. A
    /// requirement without a version specifier — a bare name, a direct URL, a
    /// `path`/`git` table — constrains nothing momo could break, so it is
    /// omitted rather than reported as satisfied. Poetry's `python` key names
    /// the interpreter, not a package, so it is omitted too.
    fn dependencies(&self, manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        let text = toml::read(manifest)?;
        let doc = toml::parse(&text, manifest)?;
        let mut requirements = Vec::new();

        if let Some(dependencies) = doc
            .get("project")
            .and_then(|project| project.get("dependencies"))
            .and_then(Item::as_array)
        {
            for element in dependencies.iter() {
                let Some(text) = element.as_str() else {
                    continue;
                };
                if let Some(parsed) = requirement::parse_requirement(text) {
                    requirements.push(DependencyRequirement {
                        package: parsed.name,
                        requirement: parsed.specifier,
                    });
                }
            }
        }

        if let Some(dependencies) = doc
            .get("tool")
            .and_then(|tool| tool.get("poetry"))
            .and_then(|poetry| poetry.get("dependencies"))
            .and_then(Item::as_table_like)
        {
            for (name, item) in dependencies.iter() {
                // `python` names the interpreter requirement, not a package.
                if name == "python" {
                    continue;
                }
                let Some(requirement) = poetry_specifier(item) else {
                    continue;
                };
                requirements.push(DependencyRequirement {
                    package: name.to_owned(),
                    requirement,
                });
            }
        }

        Ok(requirements)
    }

    /// Whether `version` satisfies the PEP 440 or Poetry constraint
    /// `requirement`.
    ///
    /// `None` when the constraint is not one momo evaluates, so an edge it
    /// cannot read is reported as *not checked*, never as satisfied.
    fn satisfied(&self, requirement: &str, version: &Version) -> Option<bool> {
        requirement::satisfied(requirement, version)
    }

    /// Widen a PEP 440 or Poetry constraint to admit `target`, preserving its
    /// operator form.
    fn widen(&self, requirement: &str, target: &Version) -> Option<String> {
        requirement::widen(requirement, target)
    }
}

/// The specifier a `[tool.poetry.dependencies]` value declares.
///
/// A Poetry dependency is either a bare specifier (`acme = "^1.2"`) or a table
/// holding one under `version`. A table with no `version` — a `path`, `git` or
/// `url` entry — carries no requirement and is omitted, as is an array of
/// per-environment tables momo cannot evaluate.
fn poetry_specifier(item: &Item) -> Option<String> {
    if let Some(specifier) = item.as_str() {
        return Some(specifier.to_owned());
    }
    item.as_table_like()?
        .get("version")?
        .as_str()
        .map(str::to_owned)
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
/// A thin wrapper over [`apply_version`] for the tests and the round-trips; the
/// render path applies the version and any range edits to one document.
fn set_version(text: &str, manifest: &Path, version: &Version) -> Result<Resolved, ManifestError> {
    let mut doc = toml::parse(text, manifest)?;
    match apply_version(&mut doc, manifest, version)? {
        Applied::Write => Ok(Resolved::Write {
            path: manifest.to_path_buf(),
            text: toml::preserve_line_endings(text, doc.to_string()),
        }),
        Applied::Skip { path, reason } => Ok(Resolved::Skip { path, reason }),
    }
}

/// What applying a Version to a parsed document produced.
enum Applied {
    /// At least one version scalar was rewritten.
    Write,
    /// Nothing was written, and why.
    Skip {
        /// The manifest momo considered.
        path: PathBuf,
        /// Why it was left alone.
        reason: SkipReason,
    },
}

/// Apply `version` to every version scalar `doc` declares.
///
/// [`SkipReason::Dynamic`] when the version is computed elsewhere,
/// [`SkipReason::NoVersion`] when no version scalar exists, and
/// [`SkipReason::AlreadyCurrent`] when every scalar already holds `version`.
/// [`ManifestError::Conflict`] when a static scalar sits beside a dynamic
/// declaration that does not require one, and when `version` has no PEP 440
/// form to write.
fn apply_version(
    doc: &mut DocumentMut,
    manifest: &Path,
    version: &Version,
) -> Result<Applied, ManifestError> {
    let sites = version_sites(doc);

    if let Some(backend) = dynamic_backend(doc) {
        // A backend that requires a static placeholder — `poetry-dynamic-versioning`
        // — is skipped even when a scalar is present: the scalar *is* the
        // placeholder, so it is not a contradiction.
        if sites.is_empty() || backend.requires_placeholder_scalar() {
            return Ok(Applied::Skip {
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
        return Ok(Applied::Skip {
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
        if let Some(item) = site.item_mut(doc) {
            changed |= toml::set_string(item, &target);
        }
    }
    if !changed {
        return Ok(Applied::Skip {
            path: manifest.to_path_buf(),
            reason: SkipReason::AlreadyCurrent,
        });
    }

    Ok(Applied::Write)
}

/// Widen the requirement `edit` names in `doc`, in place.
///
/// The requirement is declared either as a PEP 508 string in
/// `[project].dependencies` or as a Poetry specifier in
/// `[tool.poetry.dependencies]`; every declaration holding exactly the
/// requirement momo read is rewritten. Returns whether the document changed:
/// `false` when the form is not one momo rewrites, or no declaration holds it.
fn widen_range(doc: &mut DocumentMut, edit: &DependencyRangeEdit) -> bool {
    let Some(widened) = requirement::widen(&edit.requirement, &edit.target) else {
        return false;
    };
    widen_poetry(doc, edit, &widened) | widen_project(doc, edit, &widened)
}

/// Widen a Poetry `[tool.poetry.dependencies]` declaration of `edit`'s package.
///
/// Each level is reached through `as_table_like_mut` rather than `Item::get_mut`
/// directly: the latter inserts an `Item::None` placeholder for a missing key,
/// which would render as a spurious empty table.
fn widen_poetry(doc: &mut DocumentMut, edit: &DependencyRangeEdit, widened: &str) -> bool {
    let Some(dependencies) = doc
        .get_mut("tool")
        .and_then(Item::as_table_like_mut)
        .and_then(|tool| tool.get_mut("poetry"))
        .and_then(Item::as_table_like_mut)
        .and_then(|poetry| poetry.get_mut("dependencies"))
        .and_then(Item::as_table_like_mut)
    else {
        return false;
    };
    let Some(item) = dependencies.get_mut(&edit.name) else {
        return false;
    };
    if item.as_str() == Some(edit.requirement.as_str()) {
        return toml::set_string(item, widened);
    }
    let Some(version) = item
        .as_table_like_mut()
        .and_then(|table| table.get_mut("version"))
    else {
        return false;
    };
    if version.as_str() == Some(edit.requirement.as_str()) {
        return toml::set_string(version, widened);
    }
    false
}

/// Widen a PEP 508 `[project].dependencies` declaration of `edit`'s package.
fn widen_project(doc: &mut DocumentMut, edit: &DependencyRangeEdit, widened: &str) -> bool {
    let Some(dependencies) = doc
        .get_mut("project")
        .and_then(Item::as_table_like_mut)
        .and_then(|project| project.get_mut("dependencies"))
        .and_then(Item::as_array_mut)
    else {
        return false;
    };
    let mut changed = false;
    for element in dependencies.iter_mut() {
        let Some(text) = element.as_str() else {
            continue;
        };
        let Some(parsed) = requirement::parse_requirement(text) else {
            continue;
        };
        if parsed.name != edit.name || parsed.specifier != edit.requirement {
            continue;
        }
        let updated = format!("{}{}{}", parsed.prefix, widened, parsed.suffix);
        changed |= toml::set_value(element, &updated);
    }
    changed
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
    let tool = || doc.get("tool");
    // `poetry-dynamic-versioning` is checked first: its PEP 621 mode also sets
    // `[project].dynamic = ["version"]`, and it is the plugin (with its required
    // placeholder) that computes the version, not bare PEP 621.
    if tool()
        .and_then(|tool| tool.get("poetry-dynamic-versioning"))
        .is_some()
    {
        return Some(DynamicBackend::PoetryDynamic);
    }
    if dynamic_version_list(doc) {
        return Some(DynamicBackend::Project);
    }
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
    /// `[tool.poetry-dynamic-versioning]`, which requires a placeholder scalar.
    PoetryDynamic,
}

impl DynamicBackend {
    /// Whether the declaration *requires* a static placeholder scalar.
    ///
    /// `poetry-dynamic-versioning` is the only backend that does: Poetry still
    /// requires `tool.poetry.version` to be present, so the plugin's docs tell
    /// you to keep `version = "0.0.0"` as a placeholder. A scalar beside that
    /// declaration is expected, so it is not a static/dynamic conflict. Every
    /// other backend computes the version outright, so a scalar beside it really
    /// is a mix momo refuses to resolve.
    fn requires_placeholder_scalar(self) -> bool {
        matches!(self, DynamicBackend::PoetryDynamic)
    }
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
    fn a_poetry_dynamic_versioning_placeholder_is_skipped_not_refused() {
        // The plugin's classic layout: Poetry still requires `tool.poetry.version`,
        // so the scalar is the plugin's placeholder, not a static version.
        let original = "\
[tool.poetry]
name = \"acme\"
version = \"0.0.0\"

[tool.poetry-dynamic-versioning]
enable = true
";
        assert_eq!(
            skipped(original, &version("1.0.0")),
            SkipReason::Dynamic("poetry-dynamic-versioning".to_owned())
        );
    }

    #[test]
    fn a_poetry_dynamic_versioning_pep_621_placeholder_is_skipped() {
        // PEP 621 mode: `[project].dynamic` plus the same required placeholder.
        // The plugin computes the version, so the placeholder is left alone.
        let original = "\
[project]
name = \"acme\"
dynamic = [\"version\"]

[tool.poetry]
version = \"0.0.0\"

[tool.poetry-dynamic-versioning]
enable = true
";
        assert_eq!(
            skipped(original, &version("1.0.0")),
            SkipReason::Dynamic("poetry-dynamic-versioning".to_owned())
        );
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

    /// The requirements `text` declares, read from `manifest`.
    fn requirements(text: &str) -> Vec<DependencyRequirement> {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("pyproject.toml");
        fs::write(&path, text).expect("seed the manifest");
        PythonManifest
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

    /// Render `edits` against a manifest seeded on disk with `text`.
    fn rendered(text: &str, edits: &ManifestEdits) -> ManifestPlan {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("pyproject.toml");
        fs::write(&path, text).expect("seed the manifest");
        PythonManifest
            .render(&path, edits)
            .expect("render the manifest")
    }

    #[test]
    fn a_pep_508_requirement_reports_its_name_and_specifier() {
        let text = "\
[project]
name = \"acme\"
dependencies = [\"acme>=1.2,<2\", \"other[extra]~=3.4\", \"bare\", \"direct @ https://x/y.whl\"]
";
        assert_eq!(
            requirements(text),
            vec![
                DependencyRequirement {
                    package: "acme".to_owned(),
                    requirement: ">=1.2,<2".to_owned(),
                },
                DependencyRequirement {
                    package: "other".to_owned(),
                    requirement: "~=3.4".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn a_poetry_dependency_reports_its_specifier() {
        let text = "\
[tool.poetry.dependencies]
python = \"^3.11\"
acme = \"^1.2\"
other = { version = \"~2.0\", optional = true }
local = { path = \"../local\" }
";
        assert_eq!(
            requirements(text),
            vec![
                DependencyRequirement {
                    package: "acme".to_owned(),
                    requirement: "^1.2".to_owned(),
                },
                DependencyRequirement {
                    package: "other".to_owned(),
                    requirement: "~2.0".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn a_requirement_is_matched_with_pep_440_semantics() {
        let python = PythonManifest;
        assert_eq!(python.satisfied(">=1.2,<2", &version("1.5.0")), Some(true));
        assert_eq!(python.satisfied(">=1.2,<2", &version("2.0.0")), Some(false));
        assert_eq!(python.satisfied("~=1.4", &version("1.9.0")), Some(true));
        assert_eq!(python.satisfied("^1.2", &version("2.0.0")), Some(false));
        assert_eq!(python.satisfied("not a specifier", &version("1.0.0")), None);
    }

    #[test]
    fn a_python_range_is_widened_by_preserving_its_operator() {
        let python = PythonManifest;
        for (requirement, expected) in [
            ("^1.2", "^2.0.0"),
            ("~1.2", "~2.0.0"),
            (">=1.2", ">=2.0.0"),
            ("==1.2.3", "==2.0.0"),
        ] {
            assert_eq!(
                python.widen(requirement, &version("2.0.0")).as_deref(),
                Some(expected),
                "`{requirement}` widened wrongly"
            );
        }
        for requirement in ["*", ">=1.2,<2", "!=1.2.3", "==1.2.*"] {
            assert_eq!(
                python.widen(requirement, &version("2.0.0")),
                None,
                "`{requirement}` was rewritten instead of refused"
            );
        }
    }

    #[test]
    fn a_pep_508_requirement_is_widened_in_place_and_keeps_its_marker() {
        let original = "\
[project]
name = \"b\"
version = \"0.1.0\"
dependencies = [\"acme[extra]>=0.1.0 ; python_version < \\\"3.12\\\"\"]
";
        let plan = rendered(original, &widening("acme", ">=0.1.0", "0.2.0"));

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        let text = &plan.writes[0].text;
        // The specifier is re-anchored; the name, extras and marker survive.
        assert!(text.contains("acme[extra]>=0.2.0"), "{text}");
        assert!(text.contains("python_version <"), "{text}");
        assert!(!text.contains(">=0.1.0"), "{text}");
        assert_eq!(plan.ranges.len(), 1, "{plan:?}");
    }

    #[test]
    fn a_poetry_requirement_is_widened_in_place() {
        let original = "\
[tool.poetry]
name = \"b\"
version = \"0.1.0\"

[tool.poetry.dependencies]
acme = { version = \"^0.1.0\", optional = true }
";
        let plan = rendered(original, &widening("acme", "^0.1.0", "0.2.0"));

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert!(
            plan.writes[0]
                .text
                .contains("acme = { version = \"^0.2.0\", optional = true }"),
            "{plan:?}"
        );
        assert_eq!(plan.ranges.len(), 1, "{plan:?}");
    }

    #[test]
    fn a_version_and_a_range_compose_into_one_write() {
        let original = "\
[project]
name = \"b\"
version = \"0.1.0\"
dependencies = [\"acme>=0.1.0\"]
";
        let edits = ManifestEdits {
            version: Some(version("0.2.0")),
            ranges: widening("acme", ">=0.1.0", "0.2.0").ranges,
        };
        let plan = rendered(original, &edits);

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert_eq!(
            plan.writes[0].text,
            "\
[project]
name = \"b\"
version = \"0.2.0\"
dependencies = [\"acme>=0.2.0\"]
"
        );
    }

    #[test]
    fn an_unwritable_requirement_leaves_the_manifest_untouched() {
        let original = "[project]\ndependencies = [\"acme>=0.1.0,<0.2.0\"]\n";
        let plan = rendered(original, &widening("acme", ">=0.1.0,<0.2.0", "0.2.0"));

        assert!(plan.writes.is_empty(), "{plan:?}");
        assert!(plan.ranges.is_empty(), "{plan:?}");
    }

    #[test]
    fn widening_creates_no_spurious_table() {
        // `[tool.uv.sources]` makes `tool` present but `poetry` absent; reaching
        // into it must not leave an empty `poetry = {}` behind.
        let original = "\
[project]
name = \"ui\"
version = \"0.1.0\"
dependencies = [\"acme==0.0.1\"]

[tool.uv.sources]
acme = { workspace = true }
";
        let plan = rendered(original, &widening("acme", "==0.0.1", "0.1.0"));

        let text = &plan.writes[0].text;
        assert!(text.contains("acme==0.1.0"), "{text}");
        assert!(
            !text.contains("poetry"),
            "a spurious table was created: {text}"
        );
    }
}
