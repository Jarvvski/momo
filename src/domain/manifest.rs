//! The `ManifestSync` port: writing a Release's Version into a Package's
//! ecosystem manifest.
//!
//! Adapters live in `momo-manifest` (`Cargo.toml` in v0); the domain never
//! touches the filesystem. A caller injects whichever adapter fits — a real one
//! in production, a fake in tests — so the domain logic stays free of I/O.
//!
//! Syncing is split into a read-only [`ManifestSync::locate`] and a
//! [`ManifestSync::write`], so a caller can report a `--dry-run` without
//! mutating anything.
//!
//! The write is deliberately conservative: only the version scalar changes, and
//! a lockfile the ecosystem maintains is *reported* rather than rewritten —
//! refreshing it is the package manager's job, never momo's.

use std::path::{Path, PathBuf};

use semver::Version;

/// What syncing a manifest produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUpdate {
    /// The manifest written.
    pub manifest: PathBuf,
    /// The lockfile left stale for the package manager to refresh, when the
    /// ecosystem keeps one next to the manifest.
    pub stale_lockfile: Option<PathBuf>,
}

/// The manifest operations momo needs.
///
/// A manifest belongs to one Package; both methods are rooted at
/// `package_dir`. Every method is side-effect-free except
/// [`ManifestSync::write`].
pub trait ManifestSync {
    /// The manifest to sync for the Package rooted at `package_dir`.
    ///
    /// `override_path` is the configured manifest path, relative to
    /// `package_dir`; when set it is used as-is. Otherwise the ecosystem's
    /// conventional manifest is discovered in `package_dir`. `None` when no
    /// manifest is present — nothing to sync, which is not an error.
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf>;

    /// Write `version` into `manifest`, preserving the file's formatting.
    ///
    /// Only the version scalar changes: comments, key order, whitespace and the
    /// file's newline style (LF or CRLF, including whether it ends in a newline)
    /// survive byte-for-byte. `Ok(None)` when the manifest holds no version momo
    /// owns, or already carries `version` — either way the Release carries on.
    fn write(
        &self,
        manifest: &Path,
        version: &Version,
    ) -> Result<Option<ManifestUpdate>, ManifestError>;
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

    /// The manifest is not valid TOML.
    #[error("`{path}` is not valid TOML: {source}")]
    Parse {
        /// The path that could not be parsed.
        path: PathBuf,
        /// Why the parse failed.
        #[source]
        source: toml_edit::TomlError,
    },
}
