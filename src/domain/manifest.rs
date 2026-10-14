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
//! write is deliberately conservative: only version scalars change, and a
//! lockfile the ecosystem maintains is *reported* rather than rewritten —
//! refreshing it is the package manager's job, never momo's.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use semver::Version;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPlan {
    /// The manifests to write, with their new text.
    pub writes: Vec<ManifestWrite>,
    /// The lockfiles the sync left stale for the package manager to refresh.
    ///
    /// Reported, never rewritten: refreshing a lockfile is the package
    /// manager's job, and a format-preserving editor per ecosystem would cost
    /// far more than it buys.
    pub lockfiles: Vec<PathBuf>,
    /// The manifests considered but not written, each with why.
    pub skipped: Vec<(PathBuf, SkipReason)>,
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

    /// Render `manifest` with `version` set, without writing it.
    ///
    /// The read-only half of [`ManifestSync::write`]: the manifest is read and
    /// parsed and the resulting [`ManifestPlan`] is returned. A manifest that
    /// holds no version momo owns, or that already carries `version`, appears
    /// in [`ManifestPlan::skipped`] rather than being dropped.
    ///
    /// [`ManifestError::InheritedVersion`] when the manifest inherits its
    /// version from a workspace root that momo cannot resolve: momo must not
    /// silently skip it.
    fn render(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError>;

    /// Write `version` into `manifest`, preserving the file's formatting.
    ///
    /// Only version scalars change: comments, key order, whitespace and the
    /// file's newline style (LF or CRLF, including whether it ends in a newline)
    /// survive byte-for-byte. Every path in the plan is written; an inherited
    /// version may place one at the shared workspace root rather than at
    /// `manifest`.
    fn write(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError>;
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
