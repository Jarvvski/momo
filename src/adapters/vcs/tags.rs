//! Pure helpers that map between tag names and versions.
//!
//! Both adapters feed the output of their tag listing through [`latest_version`],
//! so the "which tag is latest" rule lives in one place and is unit-tested
//! without a process.

use semver::Version;

use crate::domain::vcs::VcsError;

/// The tag name for `version` under `prefix`: `v1.2.3` or `cargo-momo-v1.2.3`.
pub(crate) fn tag_name(prefix: &str, version: &Version) -> String {
    format!("{prefix}{version}")
}

/// The tag name for `version` under `prefix`, or an error when the computed
/// name is not one git will accept as a tag.
///
/// The name is validated as a whole: the version (and, in a monorepo, the slug)
/// contributes characters a check of `prefix` alone could not see.
pub(crate) fn checked_tag_name(prefix: &str, version: &Version) -> Result<String, VcsError> {
    let name = tag_name(prefix, version);
    if is_valid_ref_name(&name) {
        Ok(name)
    } else {
        Err(VcsError::InvalidTagName { name })
    }
}

/// Whether `name` is usable as the final component of a `refs/tags/<name>` ref.
///
/// Mirrors `git check-ref-format` for a tag: no control characters, space, or
/// `~ ^ : ? * [ \`; no `..`, `@{`, `//`, or leading/trailing `/`; no trailing
/// `.` or `.lock`. A leading `-` is additionally rejected even though the full
/// ref accepts it, because the name is passed to `git tag` as an argument and
/// git would parse `-1.2.3` as an option.
pub(crate) fn is_valid_ref_name(name: &str) -> bool {
    if name.is_empty()
        || name.starts_with('-')
        || name.starts_with('/')
        || name.ends_with('/')
        || name.ends_with('.')
        || name.ends_with(".lock")
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
    {
        return false;
    }
    !name.chars().any(|c| {
        c.is_ascii_control() || c == ' ' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
    })
}

/// Parse `tag` as `<prefix><version>`, or `None` when it does not match.
pub(crate) fn parse_tag(tag: &str, prefix: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(prefix)?).ok()
}

/// The highest version among `tags` that matches `prefix`.
///
/// Ordering is semver precedence, not lexicographic and not build metadata:
/// `v0.10.0` outranks `v0.9.0`, and on a precedence tie the tag without build
/// metadata wins, so `v1.0.0` outranks `v1.0.0+meta`.
pub(crate) fn latest_version<'a>(
    tags: impl IntoIterator<Item = &'a str>,
    prefix: &str,
) -> Option<Version> {
    tags.into_iter()
        .filter_map(|tag| parse_tag(tag, prefix))
        .max_by(|a, b| a.cmp_precedence(b).then_with(|| b.build.cmp(&a.build)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_by_semver_not_lexicographically() {
        let tags = ["v0.9.0", "v0.10.0", "v0.1.0"];
        assert_eq!(latest_version(tags, "v"), Some(Version::new(0, 10, 0)));
    }

    #[test]
    fn ignores_tags_that_do_not_match_the_prefix() {
        let tags = ["v1.0.0", "other-v9.9.9", "v1.0.0-rc.1"];
        assert_eq!(latest_version(tags, "v"), Some(Version::new(1, 0, 0)));
    }

    #[test]
    fn ignores_tags_that_are_not_versions() {
        let tags = ["nightly", "v1.0", "v2.0.0"];
        assert_eq!(latest_version(tags, "v"), Some(Version::new(2, 0, 0)));
    }

    #[test]
    fn reports_none_when_nothing_matches() {
        assert_eq!(latest_version(["nightly"], "v"), None);
    }

    #[test]
    fn a_precedence_tie_prefers_the_tag_without_build_metadata() {
        // `Version`'s derived `Ord` ranks `1.0.0+meta` above `1.0.0`; semver
        // precedence treats them as equal, and momo prefers the plain tag.
        let metadata_last = ["v1.0.0", "v1.0.0+meta"];
        assert_eq!(
            latest_version(metadata_last, "v"),
            Some(Version::new(1, 0, 0))
        );

        let metadata_first = ["v1.0.0+meta", "v1.0.0"];
        assert_eq!(
            latest_version(metadata_first, "v"),
            Some(Version::new(1, 0, 0))
        );
    }

    #[test]
    fn build_metadata_does_not_outrank_a_higher_precedence_version() {
        let tags = ["v1.0.0+meta", "v1.0.1"];
        assert_eq!(latest_version(tags, "v"), Some(Version::new(1, 0, 1)));
    }

    #[test]
    fn checked_tag_name_accepts_prefixes_git_accepts() {
        assert_eq!(
            checked_tag_name("v", &Version::new(1, 2, 3)).expect("a valid tag"),
            "v1.2.3"
        );
        assert_eq!(
            checked_tag_name("cargo-momo-v", &Version::new(1, 2, 3)).expect("a valid tag"),
            "cargo-momo-v1.2.3"
        );
    }

    #[test]
    fn checked_tag_name_rejects_a_name_git_would_misread() {
        // A leading `-` is passed to `git tag` as an option.
        assert!(matches!(
            checked_tag_name("-", &Version::new(1, 2, 3)),
            Err(VcsError::InvalidTagName { name }) if name == "-1.2.3"
        ));
        // A space is not a valid ref character.
        assert!(matches!(
            checked_tag_name("v 1", &Version::new(2, 3, 0)),
            Err(VcsError::InvalidTagName { .. })
        ));
    }

    #[test]
    fn valid_ref_names_mirror_git_check_ref_format() {
        for name in [
            "v1.2.3",
            "cargo-momo-v1.2.3",
            "v1.0.0-rc.1",
            "release/v1.2.3",
        ] {
            assert!(is_valid_ref_name(name), "{name} should be valid");
        }
        for name in [
            "", "-1.2.3", "v 1", "v1..2", "v1.lock", "v1.", "/v1", "v1/", "v1//2", "v1~2", "v1^2",
            "v1:2", "v1?2", "v1*2", "v1[2]", "v1\\2", "v1@{2}", "v1\t2",
        ] {
            assert!(!is_valid_ref_name(name), "{name} should be rejected");
        }
    }
}
