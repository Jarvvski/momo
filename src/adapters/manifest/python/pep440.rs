//! Rendering a semver [`Version`] as a PEP 440 version string.
//!
//! momo reads versions from tags and never from `pyproject.toml`, so only one
//! direction is needed: the semver the tag carries, rendered the way Python's
//! packaging tools spell it. The tag keeps its semver spelling (`v1.2.0-rc.1`)
//! while the manifest holds the PEP 440 form (`1.2.0rc1`) — a PEP 440 parser
//! rejects `1.2.0-rc.1`, and PyPI normalises what the manifest holds.
//!
//! A pre-release whose kind has no PEP 440 spelling — anything but alpha, beta
//! or a release candidate — is [`Pep440Error::Unrepresentable`], never silently
//! dropped: writing a version that says less than the tag would be a wrong
//! release.

use semver::{Prerelease, Version};

/// Render `version` as a PEP 440 version.
///
/// Build metadata becomes a PEP 440 local version (`1.2.3+abc`), lowercased as
/// PEP 440 normalises it. `Err` when a pre-release has no PEP 440 form.
pub fn render(version: &Version) -> Result<String, Pep440Error> {
    let mut rendered = format!("{}.{}.{}", version.major, version.minor, version.patch);
    if !version.pre.is_empty() {
        rendered.push_str(&prerelease(&version.pre)?);
    }
    if !version.build.is_empty() {
        rendered.push('+');
        rendered.push_str(&version.build.as_str().to_ascii_lowercase());
    }
    Ok(rendered)
}

/// The PEP 440 pre-release segment for `pre`, e.g. `rc1`, `b2`, `a0`.
///
/// semver spells a pre-release as dot-separated identifiers (`rc.1`); PEP 440
/// spells it as one kind and one number (`rc1`). The kind's first segment names
/// it, with an optional glued number (`rc1` as well as `rc.1`); a missing number
/// is `0` (`-alpha` becomes `a0`). More than one number, or a non-numeric
/// second segment, has no PEP 440 form.
fn prerelease(pre: &Prerelease) -> Result<String, Pep440Error> {
    let text = pre.as_str();
    let mut segments = text.split('.');
    let head = segments.next().unwrap_or_default();
    let (kind, glued) = classify(head).ok_or_else(|| Pep440Error::Unrepresentable {
        prerelease: text.to_owned(),
    })?;

    let mut number = glued;
    for segment in segments {
        if number.is_some() {
            return Err(Pep440Error::Unrepresentable {
                prerelease: text.to_owned(),
            });
        }
        number = Some(
            segment
                .parse::<u64>()
                .map_err(|_| Pep440Error::Unrepresentable {
                    prerelease: text.to_owned(),
                })?,
        );
    }
    Ok(format!("{kind}{}", number.unwrap_or(0)))
}

/// The PEP 440 kind and glued number for a pre-release's first segment.
///
/// `alpha`/`a`, `beta`/`b`, and `rc`/`c`/`pre`/`preview` are the aliases PEP 440
/// normalises; the first segment may carry the number glued on (`rc1`). `None`
/// when the segment names no kind.
fn classify(head: &str) -> Option<(&'static str, Option<u64>)> {
    const ALIASES: [(&str, &str); 8] = [
        ("alpha", "a"),
        ("beta", "b"),
        ("preview", "rc"),
        ("pre", "rc"),
        ("rc", "rc"),
        ("c", "rc"),
        ("a", "a"),
        ("b", "b"),
    ];
    ALIASES.iter().find_map(|(alias, kind)| {
        if head == *alias {
            return Some((*kind, None));
        }
        head.strip_prefix(alias)
            .filter(|digits| !digits.is_empty())
            .and_then(|digits| digits.parse::<u64>().ok())
            .map(|number| (*kind, Some(number)))
    })
}

/// Why a semver pre-release cannot be written as a PEP 440 version.
#[derive(Debug, thiserror::Error)]
pub enum Pep440Error {
    /// The pre-release's kind has no PEP 440 spelling.
    #[error("the pre-release `{prerelease}` has no PEP 440 form")]
    Unrepresentable {
        /// The offending semver pre-release.
        prerelease: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    fn rendered(text: &str) -> String {
        render(&version(text)).expect("a representable version")
    }

    #[test]
    fn a_stable_version_is_unchanged() {
        assert_eq!(rendered("1.2.3"), "1.2.3");
        assert_eq!(rendered("0.0.0"), "0.0.0");
    }

    #[test]
    fn a_release_candidate_becomes_rc_then_its_number() {
        assert_eq!(rendered("1.2.0-rc.1"), "1.2.0rc1");
        assert_eq!(rendered("1.2.0-rc.2"), "1.2.0rc2");
    }

    #[test]
    fn beta_and_alpha_become_b_and_a() {
        assert_eq!(rendered("1.2.0-beta.2"), "1.2.0b2");
        assert_eq!(rendered("1.2.0-alpha.1"), "1.2.0a1");
    }

    #[test]
    fn a_pre_release_without_a_number_is_zero() {
        assert_eq!(rendered("1.2.0-alpha"), "1.2.0a0");
        assert_eq!(rendered("1.2.0-beta"), "1.2.0b0");
        assert_eq!(rendered("1.2.0-rc"), "1.2.0rc0");
    }

    #[test]
    fn a_glued_number_is_understood() {
        assert_eq!(rendered("1.2.0-rc1"), "1.2.0rc1");
        assert_eq!(rendered("1.2.0-beta2"), "1.2.0b2");
        assert_eq!(rendered("1.2.0-alpha1"), "1.2.0a1");
    }

    #[test]
    fn the_pep_440_aliases_are_normalised() {
        assert_eq!(rendered("1.2.0-c.1"), "1.2.0rc1");
        assert_eq!(rendered("1.2.0-pre.1"), "1.2.0rc1");
        assert_eq!(rendered("1.2.0-preview.1"), "1.2.0rc1");
        assert_eq!(rendered("1.2.0-a.1"), "1.2.0a1");
        assert_eq!(rendered("1.2.0-b.1"), "1.2.0b1");
    }

    #[test]
    fn build_metadata_becomes_a_lowercased_local_version() {
        assert_eq!(rendered("1.2.3+Build.1"), "1.2.3+build.1");
        assert_eq!(rendered("1.2.0-rc.1+Build"), "1.2.0rc1+build");
    }

    #[test]
    fn an_unrepresentable_pre_release_errors() {
        for text in ["1.2.0-dev.1", "1.2.0-nightly", "1.2.0-rc.1.2", "1.2.0-rc.x"] {
            let err = render(&version(text)).expect_err("no PEP 440 form");
            assert!(err.to_string().contains("PEP 440"), "{err}");
        }
    }
}
