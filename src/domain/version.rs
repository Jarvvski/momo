//! Versions, the Bump policy, and the seam that supplies a Package's version.

use std::path::PathBuf;

use crate::domain::change::{Change, Kind};
use crate::domain::vcs::{Vcs, VcsError};

/// A semver version — the domain's [`Version`] type.
pub use semver::Version;

/// The suffix momo appends to a target Version to form a Release Candidate.
///
/// `{n}` is replaced by the candidate number. Configurable as `rc_format`.
pub const DEFAULT_RC_FORMAT: &str = "-rc.{n}";

/// The semver increment a Release implies.
///
/// Ordered from smallest to largest, so the strongest of several Bumps is
/// `bumps.into_iter().max()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bump {
    /// Increment the patch component.
    Patch,
    /// Increment the minor component.
    Minor,
    /// Increment the major component.
    Major,
}

impl Bump {
    /// Apply this Bump to `current`, zeroing the components below it.
    pub fn apply(self, current: &Version) -> Version {
        match self {
            Bump::Patch => Version::new(current.major, current.minor, current.patch + 1),
            Bump::Minor => Version::new(current.major, current.minor + 1, 0),
            Bump::Major => Version::new(current.major + 1, 0, 0),
        }
    }
}

impl Kind {
    /// The Bump a Change of this Kind implies at `current`.
    ///
    /// At MAJOR 0 a `break` and an `add` both bump MINOR; at MAJOR ≥ 1 a `break`
    /// bumps MAJOR. `add` is always MINOR and `fix` always PATCH.
    pub fn bump(self, current: &Version) -> Bump {
        match self {
            Kind::Fix => Bump::Patch,
            Kind::Add => Bump::Minor,
            Kind::Break if current.major == 0 => Bump::Minor,
            Kind::Break => Bump::Major,
        }
    }
}

/// The Bump a Release containing `kinds` implies at `current`.
///
/// The highest Kind wins; `None` when there are no Kinds.
pub fn bump(kinds: impl IntoIterator<Item = Kind>, current: &Version) -> Option<Bump> {
    kinds.into_iter().max().map(|kind| kind.bump(current))
}

/// The next Version a Release of `changes` would mint at `current`.
///
/// `None` when there are no Changes — there is nothing to release.
pub fn next_version(changes: &[Change], current: &Version) -> Option<Version> {
    bump(changes.iter().map(|change| change.kind), current).map(|bump| bump.apply(current))
}

/// The Version a Release of `changes` would mint at `current`.
///
/// Like [`next_version`], except that when `current` is a pre-release — an RC
/// already cut for this Version — the Release reuses that target and drops the
/// suffix rather than bumping again. `None` when there are no Changes.
pub fn release_version(changes: &[Change], current: &Version) -> Option<Version> {
    if changes.is_empty() {
        return None;
    }
    if current.pre.is_empty() {
        next_version(changes, current)
    } else {
        Some(Version::new(current.major, current.minor, current.patch))
    }
}

/// The Release Candidate Version for `target` numbered `n`.
///
/// `format` is the suffix template — [`DEFAULT_RC_FORMAT`] by default — with
/// `{n}` replaced by `n` and a leading `-` (the separator) stripped.
pub fn candidate_version(target: &Version, n: u64, format: &str) -> Result<Version, semver::Error> {
    let suffix = format.replace("{n}", &n.to_string());
    let pre = semver::Prerelease::new(suffix.trim_start_matches('-'))?;
    Ok(Version {
        major: target.major,
        minor: target.minor,
        patch: target.patch,
        pre,
        build: semver::BuildMetadata::EMPTY,
    })
}

/// The next Release Candidate number for `target`.
///
/// Numbers are scoped to the target Version: the result is one past the highest
/// number among `recorded`, restarting at `1` when the target changes. `format`
/// is the suffix template the recorded candidates use, so candidates under a
/// different template are ignored.
pub fn next_candidate<'a>(
    target: &Version,
    format: &str,
    recorded: impl IntoIterator<Item = &'a Version>,
) -> u64 {
    let Some((identifier, _)) = format.trim_start_matches('-').split_once("{n}") else {
        return 1;
    };
    recorded
        .into_iter()
        .filter(|version| same_release(version, target))
        .filter_map(|version| {
            version
                .pre
                .as_str()
                .strip_prefix(identifier)?
                .parse::<u64>()
                .ok()
        })
        .max()
        .map_or(1, |n| n + 1)
}

/// Whether `version` has the same `major.minor.patch` as `target`.
fn same_release(version: &Version, target: &Version) -> bool {
    version.major == target.major && version.minor == target.minor && version.patch == target.patch
}

/// The seam that supplies a Package's current Version.
///
/// The VCS is authoritative when present ([`TagVersionSource`]); the file-backed
/// implementation in the CLI is the fallback for a project with no VCS.
pub trait VersionSource {
    /// The current Version.
    ///
    /// A Package that has never released reports the bootstrap baseline
    /// `0.0.0`.
    fn current(&self) -> Result<Version, VersionError>;
}

/// A [`VersionSource`] backed by a repository's tags.
///
/// The current Version is the highest tag named `<prefix><version>`; with no
/// such tag the Package has never released, so this reports the bootstrap
/// baseline `0.0.0`.
pub struct TagVersionSource<'a> {
    vcs: &'a dyn Vcs,
    prefix: &'a str,
}

impl<'a> TagVersionSource<'a> {
    /// Read the current Version from `vcs`'s tags under `prefix`.
    pub fn new(vcs: &'a dyn Vcs, prefix: &'a str) -> Self {
        Self { vcs, prefix }
    }
}

impl VersionSource for TagVersionSource<'_> {
    fn current(&self) -> Result<Version, VersionError> {
        Ok(self
            .vcs
            .latest_tag(self.prefix)?
            .unwrap_or_else(|| Version::new(0, 0, 0)))
    }
}

/// Errors that can occur reading the current Version.
#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    /// The version file could not be read.
    #[error("reading `{path}`")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The version file did not hold a semver version.
    #[error("`{path}` does not contain a valid version: {source}")]
    Invalid {
        /// The file holding the bad version.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: semver::Error,
    },
    /// The version file could not be written.
    #[error("writing `{path}`")]
    Write {
        /// The file that could not be written.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The repository could not report its latest tag.
    #[error("reading the latest tag: {0}")]
    Vcs(#[from] VcsError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn change(kind: Kind) -> Change {
        Change::new(kind, "a change")
    }

    #[test]
    fn at_bootstrap_break_and_add_bump_minor_and_fix_bumps_patch() {
        let current = version(0, 0, 0);
        assert_eq!(Kind::Break.bump(&current), Bump::Minor);
        assert_eq!(Kind::Add.bump(&current), Bump::Minor);
        assert_eq!(Kind::Fix.bump(&current), Bump::Patch);
    }

    #[test]
    fn below_major_one_break_still_bumps_minor() {
        let current = version(0, 4, 2);
        assert_eq!(Kind::Break.bump(&current), Bump::Minor);
        assert_eq!(Kind::Add.bump(&current), Bump::Minor);
        assert_eq!(Kind::Fix.bump(&current), Bump::Patch);
    }

    #[test]
    fn at_or_above_major_one_break_bumps_major() {
        let current = version(1, 2, 3);
        assert_eq!(Kind::Break.bump(&current), Bump::Major);
        assert_eq!(Kind::Add.bump(&current), Bump::Minor);
        assert_eq!(Kind::Fix.bump(&current), Bump::Patch);
    }

    #[test]
    fn the_highest_kind_wins_in_a_release() {
        let changes = [change(Kind::Fix), change(Kind::Add), change(Kind::Break)];

        assert_eq!(
            bump(changes.iter().map(|c| c.kind), &version(0, 0, 0)),
            Some(Bump::Minor)
        );
        assert_eq!(
            bump(changes.iter().map(|c| c.kind), &version(1, 2, 3)),
            Some(Bump::Major)
        );

        assert_eq!(
            next_version(&changes, &version(0, 0, 0)),
            Some(version(0, 1, 0))
        );
        assert_eq!(
            next_version(&changes, &version(1, 2, 3)),
            Some(version(2, 0, 0))
        );
    }

    #[test]
    fn a_release_of_one_kind_mints_the_expected_version() {
        let fix = [change(Kind::Fix)];
        assert_eq!(
            next_version(&fix, &version(0, 0, 0)),
            Some(version(0, 0, 1))
        );
        assert_eq!(
            next_version(&fix, &version(1, 2, 3)),
            Some(version(1, 2, 4))
        );

        let add = [change(Kind::Add)];
        assert_eq!(
            next_version(&add, &version(0, 0, 0)),
            Some(version(0, 1, 0))
        );
        assert_eq!(
            next_version(&add, &version(1, 2, 3)),
            Some(version(1, 3, 0))
        );
    }

    #[test]
    fn nothing_unreleased_has_no_next_version() {
        assert_eq!(bump([], &version(1, 2, 3)), None);
        assert_eq!(next_version(&[], &version(1, 2, 3)), None);
    }

    #[test]
    fn applying_a_bump_zeroes_the_components_below_it() {
        let current = version(1, 2, 3);
        assert_eq!(Bump::Patch.apply(&current), version(1, 2, 4));
        assert_eq!(Bump::Minor.apply(&current), version(1, 3, 0));
        assert_eq!(Bump::Major.apply(&current), version(2, 0, 0));
    }

    #[test]
    fn a_release_from_a_candidate_reuses_the_target_and_drops_the_suffix() {
        let changes = [change(Kind::Add)];
        let candidate = Version::parse("0.1.0-rc.2").expect("a valid version");
        assert_eq!(
            release_version(&changes, &candidate),
            Some(version(0, 1, 0))
        );
    }

    #[test]
    fn a_release_from_a_stable_version_bumps_as_usual() {
        let changes = [change(Kind::Add)];
        assert_eq!(
            release_version(&changes, &version(0, 1, 0)),
            Some(version(0, 2, 0))
        );
    }

    #[test]
    fn a_release_of_nothing_has_no_version() {
        assert_eq!(release_version(&[], &version(1, 2, 3)), None);
        assert_eq!(
            release_version(&[], &Version::parse("1.2.3-rc.1").expect("a valid version")),
            None
        );
    }

    #[test]
    fn a_candidate_appends_the_number_to_the_target() {
        let target = version(1, 2, 0);
        assert_eq!(
            candidate_version(&target, 1, DEFAULT_RC_FORMAT).expect("a valid candidate"),
            Version::parse("1.2.0-rc.1").expect("a valid version")
        );
        assert_eq!(
            candidate_version(&target, 2, DEFAULT_RC_FORMAT).expect("a valid candidate"),
            Version::parse("1.2.0-rc.2").expect("a valid version")
        );
    }

    #[test]
    fn a_candidate_honours_a_custom_format() {
        let target = version(1, 2, 0);
        assert_eq!(
            candidate_version(&target, 3, "-beta.{n}").expect("a valid candidate"),
            Version::parse("1.2.0-beta.3").expect("a valid version")
        );
    }

    #[test]
    fn candidate_numbers_increment_per_target_and_restart_on_a_new_target() {
        let recorded = [
            Version::parse("0.1.0-rc.1").expect("a valid version"),
            Version::parse("0.1.0-rc.2").expect("a valid version"),
            Version::parse("0.2.0-rc.1").expect("a valid version"),
            version(0, 3, 0),
        ];

        assert_eq!(
            next_candidate(&version(0, 1, 0), DEFAULT_RC_FORMAT, &recorded),
            3
        );
        assert_eq!(
            next_candidate(&version(0, 2, 0), DEFAULT_RC_FORMAT, &recorded),
            2
        );
        // A target that has never been cut restarts at 1...
        assert_eq!(
            next_candidate(&version(0, 3, 0), DEFAULT_RC_FORMAT, &recorded),
            1
        );
        // ...and so does one with no recorded candidates at all.
        assert_eq!(
            next_candidate(&version(1, 0, 0), DEFAULT_RC_FORMAT, &recorded),
            1
        );
    }

    #[test]
    fn candidate_numbers_ignore_a_different_template() {
        let recorded = [Version::parse("0.1.0-beta.4").expect("a valid version")];
        assert_eq!(
            next_candidate(&version(0, 1, 0), DEFAULT_RC_FORMAT, &recorded),
            1
        );
        assert_eq!(next_candidate(&version(0, 1, 0), "-beta.{n}", &recorded), 5);
    }

    /// A [`Vcs`] that reports a fixed latest tag, for the version-source tests.
    struct FakeVcs(Option<Version>);

    impl Vcs for FakeVcs {
        fn kind(&self) -> crate::domain::vcs::VcsKind {
            crate::domain::vcs::VcsKind::Git
        }

        fn root(&self) -> &std::path::Path {
            std::path::Path::new("/repo")
        }

        fn latest_tag(&self, _prefix: &str) -> Result<Option<Version>, VcsError> {
            Ok(self.0.clone())
        }

        fn remote(&self) -> Result<Option<String>, VcsError> {
            Ok(None)
        }

        fn create_tag(
            &self,
            _prefix: &str,
            _version: &Version,
            _message: &str,
        ) -> Result<(), VcsError> {
            Ok(())
        }

        fn is_dirty(&self) -> Result<bool, VcsError> {
            Ok(false)
        }
    }

    #[test]
    fn the_tag_source_reports_the_latest_tag_or_the_baseline() {
        let tagged = FakeVcs(Some(version(1, 2, 3)));
        assert_eq!(
            TagVersionSource::new(&tagged, "v").current().unwrap(),
            version(1, 2, 3)
        );

        let untagged = FakeVcs(None);
        assert_eq!(
            TagVersionSource::new(&untagged, "v").current().unwrap(),
            version(0, 0, 0)
        );
    }
}
