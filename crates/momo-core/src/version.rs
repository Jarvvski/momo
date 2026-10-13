//! Versions, the Bump policy, and the seam that supplies a Package's version.

use std::path::PathBuf;

use crate::change::{Change, Kind};

/// A semver version — the domain's [`Version`] type.
pub use semver::Version;

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

/// The seam that supplies a Package's current Version.
///
/// The VCS tag becomes the authoritative source in a later ticket; for now the
/// only implementation reads `.momo/version`.
pub trait VersionSource {
    /// The current Version.
    ///
    /// A Package that has never released reports the bootstrap baseline
    /// `0.0.0`.
    fn current(&self) -> Result<Version, VersionError>;
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
}
