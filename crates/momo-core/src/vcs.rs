//! The `Vcs` port: the version-control operations momo's domain depends on.
//!
//! Adapters live in `momo-vcs` (git and jj); the domain never shells out. A
//! caller injects whichever adapter fits — a real one in production, a fake in
//! tests — so the domain logic stays free of processes.

use std::path::{Path, PathBuf};

use semver::Version;

/// Which version-control system backs a repository.
///
/// Detection prefers [`VcsKind::Jj`] when a directory holds both `.jj` and
/// `.git`, because a jj repository is always git-backed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsKind {
    /// A git repository.
    Git,
    /// A jj repository, colocated or not.
    Jj,
}

/// The version-control operations momo needs.
///
/// Tag names are `<prefix><version>`, so a single-package repo passes `v` and a
/// monorepo package passes `<slug>-v`. The prefix is a parameter, never read
/// from config here.
///
/// Every read is side-effect-free from momo's point of view; [`Vcs::create_tag`]
/// is the only call that mutates the repository.
pub trait Vcs {
    /// The kind of repository this adapter speaks to.
    fn kind(&self) -> VcsKind;

    /// The repository root: the directory that holds `.git` or `.jj`.
    fn root(&self) -> &Path;

    /// The highest version among tags named `<prefix><version>`.
    ///
    /// Returns `Ok(None)` when no tag matches — there is no version yet.
    fn latest_tag(&self, prefix: &str) -> Result<Option<Version>, VcsError>;

    /// Create the tag `<prefix><version>` at the current revision.
    ///
    /// `message` is the Release title; git records it on the annotated tag, jj
    /// ignores it (jj tags are always lightweight).
    fn create_tag(&self, prefix: &str, version: &Version, message: &str) -> Result<(), VcsError>;

    /// Whether the working copy has changes that are not committed.
    fn is_dirty(&self) -> Result<bool, VcsError>;
}

/// The domain errors a caller must be able to match on.
#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    /// No `.jj` or `.git` entry was found at or above the starting path.
    #[error("no VCS detected at or above `{path}`")]
    NoRepository {
        /// The path the search started from.
        path: PathBuf,
    },

    /// A tag with the requested name already exists; momo refuses to overwrite it.
    #[error("tag `{tag}` already exists")]
    TagAlreadyExists {
        /// The full tag name, including the prefix.
        tag: String,
    },

    /// The VCS binary could not be started.
    #[error("failed to run `{program}`")]
    Spawn {
        /// The program that could not be started.
        program: String,
        /// Why the process could not be spawned.
        #[source]
        source: std::io::Error,
    },

    /// The VCS binary ran but exited non-zero.
    #[error("`{program} {args}` failed: {stderr}")]
    CommandFailed {
        /// The program that failed.
        program: String,
        /// The arguments it was given, space-separated.
        args: String,
        /// The trimmed standard error it produced.
        stderr: String,
    },
}
