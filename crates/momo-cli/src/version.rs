//! Reading and writing the current Version, and choosing its source.
//!
//! The VCS is authoritative when a repository is present — its tags record every
//! Release. With no VCS, momo keeps its own state in `.momo/version`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use momo_core::config::Config;
use momo_core::vcs::Vcs;
use momo_core::version::{TagVersionSource, Version, VersionError, VersionSource};

/// The momo-managed version file, relative to the project root.
pub const VERSION_FILE: &str = ".momo/version";

/// A [`VersionSource`] backed by `.momo/version`.
///
/// A missing file means the Package has never released, so the source reports
/// the bootstrap baseline `0.0.0`.
pub struct FileVersionSource {
    path: PathBuf,
}

impl FileVersionSource {
    /// Read the version file under `root`.
    pub fn new(root: &Path) -> Self {
        Self {
            path: root.join(VERSION_FILE),
        }
    }

    /// Write `version` to the version file, creating `.momo/` as needed.
    pub fn write(&self, version: &Version) -> Result<(), VersionError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| VersionError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        fs::write(&self.path, format!("{version}\n")).map_err(|source| VersionError::Write {
            path: self.path.clone(),
            source,
        })
    }

    /// Whether the version file exists — that is, whether momo has released.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

impl VersionSource for FileVersionSource {
    fn current(&self) -> Result<Version, VersionError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => text
                .trim()
                .parse::<Version>()
                .map_err(|source| VersionError::Invalid {
                    path: self.path.clone(),
                    source,
                }),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Version::new(0, 0, 0)),
            Err(source) => Err(VersionError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

/// The authoritative Version source for a project rooted at `root`.
///
/// The latest VCS tag when a repository is present, else `.momo/version`.
pub fn source<'a>(
    root: &Path,
    config: &'a Config,
    vcs: Option<&'a dyn Vcs>,
) -> Box<dyn VersionSource + 'a> {
    match vcs {
        Some(vcs) => Box::new(TagVersionSource::new(vcs, &config.tag_prefix)),
        None => Box::new(FileVersionSource::new(root)),
    }
}
