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

    /// The highest version among *release* tags named `<prefix><version>` — the
    /// highest matching tag that carries no pre-release suffix.
    ///
    /// `Ok(None)` when only Candidate tags (or none) exist. A Release bumps
    /// from the last Release, never from a Candidate cut since it, so the base
    /// is read separately from [`Vcs::latest_tag`].
    fn latest_release_tag(&self, prefix: &str) -> Result<Option<Version>, VcsError>;

    /// The URL of the repository's remote, when one is configured.
    ///
    /// `origin` is preferred; `Ok(None)` when no remote exists. A remote that
    /// exists but cannot be read is an error, not an absence. The URL feeds
    /// the Changelog's reference links, so it is returned verbatim and
    /// normalised by the caller.
    fn remote(&self) -> Result<Option<String>, VcsError>;

    /// Create the tag `<prefix><version>` at `rev`.
    ///
    /// `rev` is the revision to tag — a commit-ish for git, a revset that must
    /// resolve to exactly one commit for jj. `None` uses the adapter's default
    /// anchor: `HEAD` for git, `@-` for jj. The documented flow is
    /// release → commit → tag, so jj's committed release sits at `@-`; an
    /// uncommitted release names `Some("@")` explicitly.
    ///
    /// `message` is the Release title; git records it on the annotated tag, jj
    /// ignores it (jj tags are always lightweight). The computed name is
    /// validated as a git ref first, so an unusable one fails with
    /// [`VcsError::InvalidTagName`] rather than an opaque VCS error.
    fn create_tag(
        &self,
        rev: Option<&str>,
        prefix: &str,
        version: &Version,
        message: &str,
    ) -> Result<(), VcsError>;

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

    /// The computed tag name is not one git accepts as a ref.
    #[error("`{name}` is not a valid git tag name; check the package's tag prefix")]
    InvalidTagName {
        /// The full tag name, including the prefix.
        name: String,
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
