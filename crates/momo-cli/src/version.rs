//! Reading the current Version from momo's own state file.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use momo_core::version::{Version, VersionError, VersionSource};

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
