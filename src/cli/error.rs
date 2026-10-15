//! The CLI's own errors: the ad-hoc refusals that belong to no domain type.
//!
//! Every one is something a user can act on, so every one carries a
//! [`Coded`](crate::domain::diagnostic::Coded) code. A domain error that
//! propagates untouched keeps its own code; these are the boundary's own
//! decisions — a rejected flag, an empty release, a tag with nothing to name.

use std::path::PathBuf;

use semver::Version;

use crate::domain::diagnostic::impl_coded;

/// An error the command surface raises on its own.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// An output flag was passed to a verb that reports nothing structured.
    #[error("`{flag}` is not supported by `momo {verb}`")]
    UnsupportedFlag {
        /// The rejected flag, e.g. `--format`.
        flag: &'static str,
        /// The verb it was passed to.
        verb: String,
    },

    /// `status --check` found nothing Unreleased.
    #[error("nothing unreleased")]
    NothingUnreleased,

    /// A minting run found nothing Unreleased to release.
    #[error("nothing unreleased to release")]
    NothingToRelease,

    /// `tag` was asked to run in a directory with no repository.
    #[error("no VCS detected — nothing to tag")]
    NoVcs,

    /// `tag` found no Version for the Package to name.
    #[error("nothing to tag: run `momo release` first")]
    NothingToTag,

    /// `init` refused to overwrite an existing file.
    #[error("{} already exists; refusing to overwrite", path.display())]
    InitExists {
        /// The file that already exists.
        path: PathBuf,
    },

    /// Monorepo discovery failed while `-p`/`--all` needed it.
    #[error("package discovery failed ({detail}); cannot resolve `-p`/`--all`")]
    DiscoveryFailed {
        /// The discovery failure, already rendered.
        detail: String,
    },

    /// The version file names a Version older than the newest tag.
    #[error(
        "the version file holds `{version}`, which is older than the newest tag `{latest}`; \
         run `momo release` to advance it before tagging"
    )]
    StaleLedger {
        /// The stale Version in the file.
        version: Version,
        /// The newest tag the repository holds.
        latest: Version,
    },

    /// A fragments-mode Package's Changelog holds an Unreleased section.
    #[error(
        "{} holds an Unreleased section, but this package records changes as fragments; \
         move those entries into {} as fragments, or set recording = \"changelog\"",
        path.display(),
        changes_dir.display(),
    )]
    UnversionedSection {
        /// The Changelog holding the section.
        path: PathBuf,
        /// The fragments directory the entries should move into.
        changes_dir: PathBuf,
    },

    /// Two Packages would write one shared manifest to different contents.
    #[error(
        "`{other}` and `{package}` both sync `{path}` with {detail}; a workspace version is shared, \
         so release them in separate runs"
    )]
    SharedManifestConflict {
        /// The Package that already claimed the manifest.
        other: String,
        /// The Package that collided with it.
        package: String,
        /// The shared manifest.
        path: PathBuf,
        /// How the two writes differ.
        detail: String,
    },

    /// No free Fragment file name could be found for a Change.
    #[error("no free Fragment name under {}", dir.display())]
    NoFragmentName {
        /// The fragments directory that is full.
        dir: PathBuf,
    },

    /// A file in the changes directory is not a recognised Fragment.
    #[error(
        "{} is not a Fragment: expected `<kind>-<slug>.md` with `<kind>` one of: \
         break, add, fix",
        path.display(),
    )]
    UnrecognisedFragment {
        /// The offending file.
        path: PathBuf,
    },
}

impl_coded! {
    CliError {
        CliError::UnsupportedFlag { flag, .. } => (
            "momo.usage.unsupported-flag",
            fix = format!(
                "drop `{flag}`, or use a reporting verb (`status`, `release`, `candidate`, `tag`)"
            ),
        ),
        CliError::NothingUnreleased => (
            "momo.release.nothing-unreleased",
            fix = "record a change with `momo add`, `momo fix` or `momo break`",
        ),
        CliError::NothingToRelease => (
            "momo.release.nothing-unreleased",
            fix = "record a change with `momo add`, `momo fix` or `momo break`",
        ),
        CliError::NoVcs => (
            "momo.tag.no-vcs",
            fix = "initialise a repository (`jj git init` or `git init`)",
        ),
        CliError::NothingToTag => ("momo.tag.nothing-to-tag", fix = "run `momo release` first"),
        CliError::InitExists { .. } => (
            "momo.init.exists",
            fix = "remove or rename the existing file",
        ),
        CliError::DiscoveryFailed { .. } => (
            "momo.workspace.discovery-failed",
            fix = "run `mise tasks graph` to see why discovery failed",
        ),
        CliError::StaleLedger { .. } => (
            "momo.tag.stale-version",
            fix = "run `momo release` to advance the version, or delete the version file if it was hand-written",
        ),
        CliError::UnversionedSection { .. } => (
            "momo.changelog.unversioned-section",
            fix = "move the entries into the fragments directory, or set `recording = \"changelog\"`",
        ),
        CliError::SharedManifestConflict { .. } => (
            "momo.manifest.shared-conflict",
            fix = "release the packages in separate runs",
        ),
        CliError::NoFragmentName { .. } => (
            "momo.recording.no-fragment-name",
            fix = "give the change a different description",
        ),
        CliError::UnrecognisedFragment { .. } => (
            "momo.recording.unrecognised-fragment",
            fix = "rename it to `<kind>-<slug>.md`, or move it out of the changes directory",
        ),
    }
}
