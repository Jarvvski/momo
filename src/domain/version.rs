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
    ///
    /// `None` when the component being incremented is already at `u64::MAX`: the
    /// overflow has to surface as an error, never wrap to `0`.
    pub fn apply(self, current: &Version) -> Option<Version> {
        let (major, minor, patch) = match self {
            Bump::Patch => (current.major, current.minor, current.patch.checked_add(1)?),
            Bump::Minor => (current.major, current.minor.checked_add(1)?, 0),
            Bump::Major => (current.major.checked_add(1)?, 0, 0),
        };
        Some(Version::new(major, minor, patch))
    }

    /// The component this Bump increments, for naming an overflow.
    fn incremented(self, current: &Version) -> u64 {
        match self {
            Bump::Patch => current.patch,
            Bump::Minor => current.minor,
            Bump::Major => current.major,
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
/// `Ok(None)` when there are no Changes — there is nothing to release. `Err`
/// when incrementing `current` would overflow a component.
pub fn next_version(
    changes: &[Change],
    current: &Version,
) -> Result<Option<Version>, VersionError> {
    match bump(changes.iter().map(|change| change.kind), current) {
        None => Ok(None),
        Some(bump) => bump.apply(current).map(Some).ok_or(VersionError::Overflow {
            value: bump.incremented(current),
        }),
    }
}

/// The Version a Release of `changes` would mint at `current`.
///
/// Like [`next_version`], except that when `current` is a pre-release — an RC
/// already cut for this Version — the Release reuses that target and drops the
/// suffix rather than bumping again. `Ok(None)` when there are no Changes; `Err`
/// when incrementing `current` would overflow a component.
pub fn release_version(
    changes: &[Change],
    current: &Version,
) -> Result<Option<Version>, VersionError> {
    if changes.is_empty() {
        return Ok(None);
    }
    if current.pre.is_empty() {
        next_version(changes, current)
    } else {
        Ok(Some(Version::new(
            current.major,
            current.minor,
            current.patch,
        )))
    }
}

/// The Release Candidate Version for `target` numbered `n`.
///
/// `format` is the suffix template — [`DEFAULT_RC_FORMAT`] by default — with
/// `{n}` replaced by `n` and a leading `-` (the separator) stripped.
pub fn candidate_version(target: &Version, n: u64, format: &str) -> Result<Version, semver::Error> {
    Ok(Version {
        major: target.major,
        minor: target.minor,
        patch: target.patch,
        pre: candidate_prerelease(n, format)?,
        build: semver::BuildMetadata::EMPTY,
    })
}

/// The pre-release a candidate template renders for `n`.
fn candidate_prerelease(n: u64, format: &str) -> Result<semver::Prerelease, semver::Error> {
    let suffix = format.replace("{n}", &n.to_string());
    semver::Prerelease::new(suffix.trim_start_matches('-'))
}

/// The fixed text either side of the single `{n}` in an RC template.
///
/// The leading `-` separator is dropped from the prefix, matching
/// [`candidate_version`]. `None` unless the template holds exactly one `{n}`.
fn rc_parts(format: &str) -> Option<(&str, &str)> {
    let (prefix, suffix) = format.split_once("{n}")?;
    if suffix.contains("{n}") {
        return None;
    }
    Some((prefix.trim_start_matches('-'), suffix))
}

/// Check that `format` is a usable Release Candidate template.
///
/// It must contain exactly one `{n}` and render to a valid semver pre-release. A
/// template that fails either check would mint the same candidate for every
/// number, so the configuration is rejected before any Release runs.
pub fn validate_rc_format(format: &str) -> Result<(), RcFormatError> {
    if rc_parts(format).is_none() {
        return Err(RcFormatError::Marker {
            format: format.to_owned(),
        });
    }
    candidate_prerelease(1, format)
        .map(|_| ())
        .map_err(|source| RcFormatError::Invalid {
            format: format.to_owned(),
            source,
        })
}

/// The next Release Candidate number for `target`.
///
/// Numbers are scoped to the target Version: the result is one past the highest
/// number among `recorded`, restarting at `1` when the target changes. `format`
/// is the suffix template the recorded candidates use — the digits between its
/// fixed prefix and suffix — so candidates under a different template are
/// ignored. `Err` when the counter is already at `u64::MAX`.
pub fn next_candidate<'a>(
    target: &Version,
    format: &str,
    recorded: impl IntoIterator<Item = &'a Version>,
) -> Result<u64, VersionError> {
    let Some((prefix, suffix)) = rc_parts(format) else {
        return Ok(1);
    };
    let highest = recorded
        .into_iter()
        .filter(|version| same_release(version, target))
        .filter_map(|version| candidate_number(&version.pre, prefix, suffix))
        .max();
    match highest {
        Some(n) => n.checked_add(1).ok_or(VersionError::Overflow { value: n }),
        None => Ok(1),
    }
}

/// The candidate number `pre` encodes under `(prefix, suffix)`, if any.
fn candidate_number(pre: &semver::Prerelease, prefix: &str, suffix: &str) -> Option<u64> {
    let digits = pre.as_str().strip_prefix(prefix)?.strip_suffix(suffix)?;
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u64>().ok()
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

/// The error produced when a Release Candidate template is not usable.
#[derive(Debug, thiserror::Error)]
pub enum RcFormatError {
    /// The template does not contain exactly one `{n}`.
    #[error("`{format}` must contain exactly one `{{n}}`")]
    Marker {
        /// The offending template.
        format: String,
    },
    /// The template does not render to a valid semver pre-release.
    #[error("`{format}` does not render to a valid pre-release")]
    Invalid {
        /// The offending template.
        format: String,
        /// The underlying parse error.
        #[source]
        source: semver::Error,
    },
}

/// Errors that can occur resolving a Version.
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
    /// Incrementing a version component or candidate number would overflow `u64`.
    #[error("cannot increment `{value}`: it is already at the maximum")]
    Overflow {
        /// The value already at `u64::MAX`.
        value: u64,
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
            next_version(&changes, &version(0, 0, 0)).expect("a bump that fits"),
            Some(version(0, 1, 0))
        );
        assert_eq!(
            next_version(&changes, &version(1, 2, 3)).expect("a bump that fits"),
            Some(version(2, 0, 0))
        );
    }

    #[test]
    fn a_release_of_one_kind_mints_the_expected_version() {
        let fix = [change(Kind::Fix)];
        assert_eq!(
            next_version(&fix, &version(0, 0, 0)).expect("a bump that fits"),
            Some(version(0, 0, 1))
        );
        assert_eq!(
            next_version(&fix, &version(1, 2, 3)).expect("a bump that fits"),
            Some(version(1, 2, 4))
        );

        let add = [change(Kind::Add)];
        assert_eq!(
            next_version(&add, &version(0, 0, 0)).expect("a bump that fits"),
            Some(version(0, 1, 0))
        );
        assert_eq!(
            next_version(&add, &version(1, 2, 3)).expect("a bump that fits"),
            Some(version(1, 3, 0))
        );
    }

    #[test]
    fn nothing_unreleased_has_no_next_version() {
        assert_eq!(bump([], &version(1, 2, 3)), None);
        assert_eq!(
            next_version(&[], &version(1, 2, 3)).expect("no changes is not an error"),
            None
        );
    }

    #[test]
    fn applying_a_bump_zeroes_the_components_below_it() {
        let current = version(1, 2, 3);
        assert_eq!(
            Bump::Patch.apply(&current).expect("a bump that fits"),
            version(1, 2, 4)
        );
        assert_eq!(
            Bump::Minor.apply(&current).expect("a bump that fits"),
            version(1, 3, 0)
        );
        assert_eq!(
            Bump::Major.apply(&current).expect("a bump that fits"),
            version(2, 0, 0)
        );
    }

    #[test]
    fn a_bump_at_u64_max_overflows_instead_of_wrapping() {
        let max = u64::MAX;
        assert_eq!(Bump::Patch.apply(&version(1, 2, max)), None);
        assert_eq!(Bump::Minor.apply(&version(1, max, 3)), None);
        assert_eq!(Bump::Major.apply(&version(max, 2, 3)), None);

        let fix = [change(Kind::Fix)];
        let err =
            next_version(&fix, &version(1, 2, max)).expect_err("the patch component overflows");
        assert!(err.to_string().contains(&max.to_string()), "{err}");

        let add = [change(Kind::Add)];
        assert!(next_version(&add, &version(1, max, 3)).is_err());

        let breaking = [change(Kind::Break)];
        assert!(release_version(&breaking, &version(max, 2, 3)).is_err());
    }

    #[test]
    fn a_release_from_a_candidate_reuses_the_target_and_drops_the_suffix() {
        let changes = [change(Kind::Add)];
        let candidate = Version::parse("0.1.0-rc.2").expect("a valid version");
        assert_eq!(
            release_version(&changes, &candidate).expect("a bump that fits"),
            Some(version(0, 1, 0))
        );
    }

    #[test]
    fn a_release_from_a_stable_version_bumps_as_usual() {
        let changes = [change(Kind::Add)];
        assert_eq!(
            release_version(&changes, &version(0, 1, 0)).expect("a bump that fits"),
            Some(version(0, 2, 0))
        );
    }

    #[test]
    fn a_release_of_nothing_has_no_version() {
        assert_eq!(
            release_version(&[], &version(1, 2, 3)).expect("no changes is not an error"),
            None
        );
        assert_eq!(
            release_version(&[], &Version::parse("1.2.3-rc.1").expect("a valid version"))
                .expect("no changes is not an error"),
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
            next_candidate(&version(0, 1, 0), DEFAULT_RC_FORMAT, &recorded)
                .expect("the counter fits"),
            3
        );
        assert_eq!(
            next_candidate(&version(0, 2, 0), DEFAULT_RC_FORMAT, &recorded)
                .expect("the counter fits"),
            2
        );
        // A target that has never been cut restarts at 1...
        assert_eq!(
            next_candidate(&version(0, 3, 0), DEFAULT_RC_FORMAT, &recorded)
                .expect("the counter fits"),
            1
        );
        // ...and so does one with no recorded candidates at all.
        assert_eq!(
            next_candidate(&version(1, 0, 0), DEFAULT_RC_FORMAT, &recorded)
                .expect("the counter fits"),
            1
        );
    }

    #[test]
    fn candidate_numbers_ignore_a_different_template() {
        let recorded = [Version::parse("0.1.0-beta.4").expect("a valid version")];
        assert_eq!(
            next_candidate(&version(0, 1, 0), DEFAULT_RC_FORMAT, &recorded)
                .expect("the counter fits"),
            1
        );
        assert_eq!(
            next_candidate(&version(0, 1, 0), "-beta.{n}", &recorded).expect("the counter fits"),
            5
        );
    }

    #[test]
    fn candidate_numbers_advance_with_text_after_the_marker() {
        let recorded = [
            Version::parse("0.1.0-1.rc").expect("a valid version"),
            Version::parse("0.1.0-2.rc").expect("a valid version"),
        ];
        assert_eq!(
            next_candidate(&version(0, 1, 0), "-{n}.rc", &recorded).expect("the counter fits"),
            3
        );
        // A pre-release that only matches the prefix is not a candidate number.
        let unrelated = [Version::parse("0.1.0-1.beta").expect("a valid version")];
        assert_eq!(
            next_candidate(&version(0, 1, 0), "-{n}.rc", &unrelated).expect("the counter fits"),
            1
        );
    }

    #[test]
    fn a_candidate_counter_at_u64_max_overflows_instead_of_wrapping() {
        let recorded = [Version::parse("0.1.0-rc.18446744073709551615").expect("a valid version")];
        let err = next_candidate(&version(0, 1, 0), DEFAULT_RC_FORMAT, &recorded)
            .expect_err("the counter overflows");
        assert!(err.to_string().contains("18446744073709551615"), "{err}");
    }

    #[test]
    fn a_template_without_exactly_one_marker_is_rejected() {
        for format in ["-rc", "-rc.{n}.{n}", "{n}{n}"] {
            assert!(
                validate_rc_format(format).is_err(),
                "`{format}` should be rejected"
            );
        }
    }

    #[test]
    fn a_template_that_is_not_a_valid_prerelease_is_rejected() {
        let err = validate_rc_format("-rc_{n}").expect_err("an underscore is not a prerelease");
        assert!(err.to_string().contains("-rc_{n}"), "{err}");
    }

    #[test]
    fn the_default_and_a_suffixed_template_are_accepted() {
        for format in [DEFAULT_RC_FORMAT, "-beta.{n}", "-{n}.rc", "-{n}"] {
            assert!(
                validate_rc_format(format).is_ok(),
                "`{format}` should be accepted"
            );
        }
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
            _rev: Option<&str>,
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
