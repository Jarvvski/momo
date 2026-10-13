//! Pure helpers that map between tag names and versions.
//!
//! Both adapters feed the output of their tag listing through [`latest_version`],
//! so the "which tag is latest" rule lives in one place and is unit-tested
//! without a process.

use semver::Version;

/// The tag name for `version` under `prefix`: `v1.2.3` or `cargo-momo-v1.2.3`.
pub(crate) fn tag_name(prefix: &str, version: &Version) -> String {
    format!("{prefix}{version}")
}

/// Parse `tag` as `<prefix><version>`, or `None` when it does not match.
pub(crate) fn parse_tag(tag: &str, prefix: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(prefix)?).ok()
}

/// The highest version among `tags` that matches `prefix`.
///
/// Ordering is semver's, not lexicographic: `v0.10.0` outranks `v0.9.0`.
pub(crate) fn latest_version<'a>(
    tags: impl IntoIterator<Item = &'a str>,
    prefix: &str,
) -> Option<Version> {
    tags.into_iter()
        .filter_map(|tag| parse_tag(tag, prefix))
        .max()
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
}
