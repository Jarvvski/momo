//! The `ManifestSync` port: writing a Release's Version into a Package's
//! ecosystem manifest.
//!
//! Adapters live in `momo-manifest` (`Cargo.toml` in v0); the domain never
//! touches the filesystem. A caller injects whichever adapter fits — a real one
//! in production, a fake in tests — so the domain logic stays free of I/O.
//!
//! Syncing is split into a read-only [`ManifestSync::locate`] and a
//! [`ManifestSync::render`], plus [`ManifestSync::write`] which composes them.
//! A caller can render a `--dry-run` — or stage the write in a transaction —
//! without mutating anything.
//!
//! Both return a [`ManifestPlan`] rather than a single edit: an ecosystem may
//! write more than one manifest, and a manifest momo decides *not* to touch is
//! reported with a [`SkipReason`] rather than vanishing into an `Option`. The
//! write is deliberately conservative: only version scalars and dependency
//! ranges change, and a lockfile the ecosystem maintains is *reported* rather
//! than rewritten — refreshing it is the package manager's job, never momo's.
//!
//! One render carries every edit a manifest receives, in a [`ManifestEdits`]:
//! a version scalar and the dependency ranges widened to admit a sibling's new
//! version. Composing them is not a convenience — a version write and a range
//! edit can target the *same* file, and two separate writes to one path would
//! silently overwrite each other.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use semver::Version;

use crate::domain::diagnostic::impl_coded;

/// One manifest a sync would write: where it goes and what it would hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestWrite {
    /// The manifest to write.
    ///
    /// Normally the Package's own manifest, but an ecosystem that inherits a
    /// version from a shared root (a Cargo workspace member's
    /// `version.workspace = true`) writes the root instead, so this is the path
    /// actually changed — which may sit outside the Package's directory.
    pub path: PathBuf,
    /// The manifest's new text, with its formatting preserved.
    pub text: String,
}

/// Why a manifest momo considered was left alone.
///
/// A skip is a decision, not a silent `None`: the caller reports it, so a
/// manifest momo could not sync is never mistaken for one that needed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The manifest holds no version scalar momo owns.
    NoVersion,
    /// The manifest already carries the Version being written.
    AlreadyCurrent,
    /// The version is computed elsewhere, so there is no scalar to write — a
    /// Python `hatch-vcs` project reads it from the VCS. The string names where
    /// the version comes from.
    Dynamic(String),
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::NoVersion => f.write_str("no version momo owns"),
            SkipReason::AlreadyCurrent => f.write_str("already current"),
            SkipReason::Dynamic(origin) => write!(f, "dynamic version ({origin})"),
        }
    }
}

/// What syncing a manifest produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestPlan {
    /// The manifests to write, with their new text.
    ///
    /// One entry per path, whatever number of edits landed on it: a version
    /// scalar and a range edit that target the same file are composed into one
    /// text rather than staged as two writes to the same target.
    pub writes: Vec<ManifestWrite>,
    /// The lockfiles the sync left stale for the package manager to refresh.
    ///
    /// Reported, never rewritten: refreshing a lockfile is the package
    /// manager's job, and a format-preserving editor per ecosystem would cost
    /// far more than it buys.
    pub lockfiles: Vec<PathBuf>,
    /// The manifests considered but not written, each with why.
    pub skipped: Vec<(PathBuf, SkipReason)>,
    /// The dependency ranges this render actually widened, in the order the
    /// edits were asked for.
    ///
    /// An edit that matched no declaration, or whose requirement already admits
    /// the target, is absent — this is what changed, not what was requested.
    pub ranges: Vec<DependencyRangeEdit>,
}

/// A dependency requirement momo would widen to admit a sibling's new Version.
///
/// The requirement is named as the *manifest* resolves it, so a Cargo
/// dependency renamed with `package = "…"` reports the real crate name, not the
/// table key it was declared under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRangeEdit {
    /// The depended-on package, as the manifest resolves it.
    pub name: String,
    /// The requirement the manifest declares now, verbatim.
    ///
    /// Carried so the adapter rewrites exactly the declarations holding it: a
    /// dependency may appear in more than one table, and only the ones whose
    /// requirement matches are touched.
    pub requirement: String,
    /// The Version the requirement must be widened to admit.
    pub target: Version,
}

/// The edits one render applies to a manifest.
///
/// Both halves are optional: a manifest may only need its version synced, only
/// need a range widened (a Package whose own version scalar momo cannot see),
/// or both at once — which is why they are one value rather than two calls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestEdits {
    /// The Version to write, when momo is syncing one.
    pub version: Option<Version>,
    /// The dependency ranges to widen.
    pub ranges: Vec<DependencyRangeEdit>,
}

impl ManifestEdits {
    /// The edits that sync only a version, leaving every range alone.
    pub fn version(version: Version) -> Self {
        Self {
            version: Some(version),
            ranges: Vec::new(),
        }
    }
}

/// A dependency one manifest declares on another package.
///
/// The read-only counterpart to a [`ManifestWrite`]: not what momo would change
/// but what the manifest *asks for*, so a caller can tell whether a Release
/// leaves a dependent's requirement unsatisfied. Only a dependency that carries
/// a version requirement appears — a `path`- or `git`-only dependency
/// constrains nothing momo could break, so it is omitted rather than reported
/// as trivially satisfied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequirement {
    /// The depended-on package's name, as the ecosystem resolves it.
    ///
    /// A Cargo dependency renamed with `package = "…"` reports the real name,
    /// not the table key it was declared under.
    pub package: String,
    /// The declared requirement, verbatim, e.g. `^1.0`.
    pub requirement: String,
}

/// The manifest operations momo needs.
///
/// A manifest belongs to one Package: [`ManifestSync::locate`] is rooted at
/// `package_dir`. [`ManifestSync::render`] and [`ManifestSync::write`] read that
/// manifest, but an ecosystem whose version is inherited writes the shared root
/// the Package resolves to — outside `package_dir` — and reports that path in
/// its [`ManifestWrite`]. Every method is side-effect-free except
/// [`ManifestSync::write`].
pub trait ManifestSync {
    /// The manifest to sync for the Package rooted at `package_dir`.
    ///
    /// `override_path` is the configured manifest path, relative to
    /// `package_dir`; when set it is used as-is. Otherwise the ecosystem's
    /// conventional manifest is discovered in `package_dir`. `None` when no
    /// manifest is present — nothing to sync, which is not an error.
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf>;

    /// Render `manifest` with `edits` applied, without writing it.
    ///
    /// The read-only half of [`ManifestSync::write`]: the manifest is read and
    /// parsed and the resulting [`ManifestPlan`] is returned. A manifest that
    /// holds no version momo owns, or that already carries the version, appears
    /// in [`ManifestPlan::skipped`] rather than being dropped.
    ///
    /// A range edit is independent of the version: a manifest momo cannot sync
    /// a version into can still have a range widened, and the two compose into
    /// one [`ManifestWrite`] when they target the same file.
    ///
    /// [`ManifestError::InheritedVersion`] when the manifest inherits its
    /// version from a workspace root that momo cannot resolve: momo must not
    /// silently skip it.
    fn render(&self, manifest: &Path, edits: &ManifestEdits)
    -> Result<ManifestPlan, ManifestError>;

    /// Write `edits` into `manifest`, preserving the file's formatting.
    ///
    /// Only version scalars and dependency requirements change: comments, key
    /// order, whitespace and the file's newline style (LF or CRLF, including
    /// whether it ends in a newline) survive byte-for-byte. Every path in the
    /// plan is written; an inherited version may place one at the shared
    /// workspace root rather than at `manifest`.
    fn write(&self, manifest: &Path, edits: &ManifestEdits) -> Result<ManifestPlan, ManifestError>;

    /// The dependencies `manifest` declares, each with its version requirement.
    ///
    /// Read-only, and deliberately partial: a dependency declared without a
    /// version — a `path`- or `git`-only entry — constrains nothing momo could
    /// break, so it is omitted rather than reported as satisfied. A dependency
    /// whose requirement is inherited from a shared workspace root
    /// (`workspace = true`) is resolved to the root's requirement.
    fn dependencies(&self, manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError>;

    /// Whether `version` satisfies `requirement`.
    ///
    /// `None` when the requirement's syntax is not one this ecosystem can
    /// evaluate. A caller must not treat that as satisfied: momo reports an
    /// edge it could not read as *not checked*, never as satisfied, so this
    /// never returns `Some(true)` for a range it does not understand.
    fn satisfied(&self, requirement: &str, version: &Version) -> Option<bool>;

    /// Widen `requirement` so it admits `target`, or `None` when the
    /// requirement's syntax is not one this ecosystem rewrites.
    ///
    /// A caller must not fall back to a guess: `None` means the range is left
    /// exactly as the project wrote it, and the edge is reported as unsatisfied
    /// rather than silently rewritten into something momo invented. The result,
    /// when there is one, always admits `target` — a rewrite that excluded the
    /// version it was widened for would be a semver lie.
    fn widen(&self, requirement: &str, target: &Version) -> Option<String>;
}

/// The domain errors a caller must be able to match on.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The manifest could not be read.
    #[error("reading `{path}`")]
    Read {
        /// The path that could not be read.
        path: PathBuf,
        /// Why the read failed.
        #[source]
        source: std::io::Error,
    },

    /// The manifest could not be written.
    #[error("writing `{path}`")]
    Write {
        /// The path that could not be written.
        path: PathBuf,
        /// Why the write failed.
        #[source]
        source: std::io::Error,
    },

    /// The manifest could not be parsed.
    ///
    /// The source is opaque to the domain: the adapter owns the format — TOML
    /// today, JSON or YAML tomorrow — and reports its own parser's failure here
    /// rather than leaking a format-specific error type through the port.
    #[error("`{path}` could not be parsed")]
    Parse {
        /// The path that could not be parsed.
        path: PathBuf,
        /// Why the parse failed, owned by the adapter.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },

    /// The manifest says something momo cannot sync without contradicting it.
    ///
    /// A semantic refusal rather than a syntax one: the manifest parses, but
    /// the version it declares cannot be written — a version that is both
    /// static and dynamic, for instance.
    #[error("`{path}` conflicts: {message}")]
    Conflict {
        /// The manifest that refuses to be synced.
        path: PathBuf,
        /// What the manifest contradicts.
        message: String,
    },

    /// The manifest inherits its version from a workspace root that is not in
    /// the same document, so momo cannot resolve the version to write.
    #[error("`{manifest}` inherits its version from a workspace root momo cannot see")]
    InheritedVersion {
        /// The manifest whose version is inherited.
        manifest: PathBuf,
    },
}

impl_coded! {
    ManifestError {
        ManifestError::Read { .. } => ("momo.error.unclassified"),
        ManifestError::Write { .. } => ("momo.error.unclassified"),
        ManifestError::Parse { path, .. } => (
            "momo.manifest.unparseable",
            fix = format!("fix the syntax in `{}`", path.display()),
        ),
        ManifestError::Conflict { path, .. } => (
            "momo.manifest.conflict",
            fix = format!("resolve the contradiction in `{}`", path.display()),
        ),
        ManifestError::InheritedVersion { manifest } => (
            "momo.manifest.inherited-version",
            fix = format!(
                "pin an explicit `version` in `{}`, or keep the workspace root in the repo",
                manifest.display(),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skip_reason_reads_as_its_cause() {
        assert_eq!(SkipReason::NoVersion.to_string(), "no version momo owns");
        assert_eq!(SkipReason::AlreadyCurrent.to_string(), "already current");
        assert_eq!(
            SkipReason::Dynamic("hatch-vcs reads it from the VCS".to_owned()).to_string(),
            "dynamic version (hatch-vcs reads it from the VCS)"
        );
    }

    #[test]
    fn a_parse_error_names_the_path_and_keeps_its_source_opaque() {
        let err = ManifestError::Parse {
            path: PathBuf::from("Cargo.toml"),
            source: Box::new(std::io::Error::other("unexpected `=`")),
        };

        assert!(err.to_string().contains("Cargo.toml"), "{err}");
        assert!(
            Error::source(&err).is_some(),
            "the format detail was dropped"
        );
    }

    #[test]
    fn a_conflict_names_the_path_and_the_contradiction() {
        let err = ManifestError::Conflict {
            path: PathBuf::from("pyproject.toml"),
            message: "the version is both static and dynamic".to_owned(),
        };

        let text = err.to_string();
        assert!(text.contains("pyproject.toml"), "{text}");
        assert!(text.contains("both static and dynamic"), "{text}");
    }
}
