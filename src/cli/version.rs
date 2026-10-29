//! Reading and writing a Package's Version history, and choosing its source.
//!
//! The VCS is authoritative when a repository is present — its tags record every
//! Release. With no VCS, momo keeps its own state in `.momo/version`.
//!
//! `.momo/version` is the **pending-release ledger**: the record of the last
//! version momo *minted* (a Release or a Release Candidate), written by
//! `release`/`candidate`. It is not a version source for `status`/`release`,
//! which read the tags; it is what `tag` reads, because the version it must tag
//! is by definition one no tag records yet. With no VCS there are no tags, so
//! the ledger also serves as the fallback version source.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::cli::atomic;
use crate::domain::changelog::Changelog;
use crate::domain::vcs::Vcs;
use crate::domain::version::{TagVersionSource, Version, VersionError, VersionSource};

/// A [`VersionSource`] backed by `.momo/version`, the pending-release ledger.
///
/// A missing file means the Package has never minted a version, so both the
/// current version and the last release report `None`.
pub struct FileVersionSource {
    path: PathBuf,
}

impl FileVersionSource {
    /// Read the ledger at `path`.
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    /// Write `version` to the ledger, creating `.momo/` as needed.
    pub fn write(&self, version: &Version) -> Result<(), VersionError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| VersionError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        atomic::write(&self.path, &format!("{version}\n")).map_err(|source| VersionError::Write {
            path: self.path.clone(),
            source,
        })
    }

    /// The Version the ledger holds, or `None` when there is no ledger.
    fn read(&self) -> Result<Option<Version>, VersionError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => {
                text.trim()
                    .parse::<Version>()
                    .map(Some)
                    .map_err(|source| VersionError::Invalid {
                        path: self.path.clone(),
                        source,
                    })
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(source) => Err(VersionError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

impl VersionSource for FileVersionSource {
    fn current(&self) -> Result<Option<Version>, VersionError> {
        self.read()
    }

    /// The ledger's Version when it is a Release, else `None`.
    ///
    /// A ledger holding a Candidate does not name a Release, so it cannot be the
    /// base a bump applies to; the caller falls back to the Changelog's newest
    /// Release.
    fn last_release(&self) -> Result<Option<Version>, VersionError> {
        Ok(self.read()?.filter(|version| version.pre.is_empty()))
    }
}

/// The authoritative Version source for the Package whose ledger is
/// `version_file`.
///
/// The latest VCS tag under `tag_prefix` when a repository is present, else the
/// Package's `.momo/version` ledger.
pub fn source<'a>(
    version_file: &Path,
    tag_prefix: &'a str,
    vcs: Option<&'a dyn Vcs>,
) -> Box<dyn VersionSource + 'a> {
    match vcs {
        Some(vcs) => Box::new(TagVersionSource::new(vcs, tag_prefix)),
        None => Box::new(FileVersionSource::new(version_file)),
    }
}

/// The base the next Release bumps from: the Package's last Release.
///
/// The version source supplies it from the tags. When it cannot — a VCS-less
/// Package whose ledger holds a Candidate rather than a Release — the newest
/// Release the Changelog records is the base. With neither, the Package has
/// never released, so the bootstrap baseline `0.0.0` is the base.
pub fn release_base(
    source: &dyn VersionSource,
    changelog: &Changelog,
) -> Result<Version, VersionError> {
    Ok(source
        .last_release()?
        .or_else(|| changelog.last_release())
        .unwrap_or_else(|| Version::new(0, 0, 0)))
}
