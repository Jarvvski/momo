//! The Release lifecycle: minting a Version, promoting the Changelog, consuming
//! Fragments, and creating the VCS tag.
//!
//! Everything here is orchestration, per Package. The pure decisions — the Bump,
//! the Changelog text, the reference links — live in `momo-core`; the file work
//! goes through [`crate::recording`] and [`FileVersionSource`], and the
//! repository is touched only through the injected [`Vcs`].

mod manifest;
mod prepare;
mod tag;

#[cfg(test)]
mod tests;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use semver::Version;

use crate::domain::changelog::Changelog;
use crate::domain::config::Config;
use crate::domain::manifest::ManifestPlan;
use crate::domain::package::Package;

pub use manifest::reject_shared_manifest_conflicts;
pub use prepare::{Resolved, assemble, commit, prepare_candidate, resolve_release};
pub use tag::tag;

/// What a `release` or `candidate` run did, or would do under `--dry-run`.
#[derive(Debug)]
pub struct Outcome {
    /// The Version minted.
    pub version: Version,
    /// The Changelog the run wrote.
    pub changelog: PathBuf,
    /// The version file the run wrote. Always present: both `release` and
    /// `candidate` advance it, so the run can be tagged.
    pub version_file: PathBuf,
    /// The Fragments a Release consumed.
    pub consumed: Vec<PathBuf>,
    /// The ecosystem manifest plan the run synced, when it found one.
    pub manifest: Option<ManifestPlan>,
}

/// A tag [`tag`] created, or would create.
#[derive(Debug)]
pub struct Tagged {
    /// The Version the tag names.
    pub version: Version,
    /// The full tag name, prefix included.
    pub name: String,
}

/// A Release resolved but not yet written.
///
/// Everything a Release would change, rendered and ready to stage. Resolving
/// without writing lets a `--all` run validate every Package together — in
/// particular, that no two Packages would write one shared manifest to
/// different versions — before anything is mutated.
pub struct Prepared {
    /// What the run would report.
    outcome: Outcome,
    /// Each artefact: the path it targets and its rendered contents.
    writes: Vec<(PathBuf, String)>,
}

impl Prepared {
    /// The Version the run would mint.
    pub(crate) fn version(&self) -> &Version {
        &self.outcome.version
    }

    /// Every manifest this run would write, with the text it would hold.
    ///
    /// A plan may hold several writes, and an inherited version's write targets
    /// the shared workspace root rather than the Package's own manifest, so the
    /// conflict check compares each of these paths.
    fn manifest_writes(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.outcome
            .manifest
            .iter()
            .flat_map(|plan| plan.writes.iter())
            .map(|write| (write.path.as_path(), write.text.as_str()))
    }
}

/// The Changelog on disk, or a new one when the file is missing.
pub(crate) fn read_changelog(
    repo_root: &Path,
    config: &Config,
    package: &Package,
) -> Result<Changelog> {
    let path = package.changelog(repo_root, config);
    match fs::read_to_string(&path) {
        Ok(text) => Changelog::parse(&text).with_context(|| format!("reading {}", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Changelog::empty()),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}
