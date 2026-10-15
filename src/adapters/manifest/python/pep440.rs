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

/// A PEP 440 version, as much of one as momo needs to evaluate a specifier.
///
/// momo never reads a version from `pyproject.toml`; it parses the operands of
/// a requirement's specifiers and compares them with the PEP 440 rendering of a
/// semver target. The comparison follows PEP 440's ordering key — epoch,
/// release (trailing zeros insignificant), then pre, post, dev and local — so
/// `~=1.2` and `==1.2.*` mean what `pip` means by them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pep440Version {
    /// The epoch (`N!`), `0` when absent.
    epoch: u64,
    /// The release segments, e.g. `[1, 2, 0]`.
    release: Vec<u64>,
    /// A pre-release kind and number, e.g. `rc1`.
    pre: Option<(PreKind, u64)>,
    /// A post-release number, e.g. `post1`.
    post: Option<u64>,
    /// A development-release number, e.g. `dev1`.
    dev: Option<u64>,
    /// A local version, lowercased.
    local: Option<String>,
}

/// A PEP 440 pre-release kind, ordered as PEP 440 orders them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PreKind {
    /// `a`/`alpha`.
    Alpha,
    /// `b`/`beta`.
    Beta,
    /// `rc`/`c`/`pre`/`preview`.
    Rc,
}

impl PreKind {
    /// The canonical spelling PEP 440 normalises this kind to.
    fn as_str(self) -> &'static str {
        match self {
            PreKind::Alpha => "a",
            PreKind::Beta => "b",
            PreKind::Rc => "rc",
        }
    }
}

impl Pep440Version {
    /// The release segments, e.g. `[1, 2, 0]`.
    pub(crate) fn release(&self) -> &[u64] {
        &self.release
    }

    /// Whether this is a pre-release or a development release.
    pub(crate) fn is_prerelease(&self) -> bool {
        self.pre.is_some() || self.dev.is_some()
    }

    /// Whether this carries a local version, which momo does not compare.
    pub(crate) fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// The canonical PEP 440 spelling, used for `===`'s string comparison.
    pub(crate) fn canonical(&self) -> String {
        let mut text = String::new();
        if self.epoch != 0 {
            text.push_str(&self.epoch.to_string());
            text.push('!');
        }
        let release: Vec<String> = self.release.iter().map(u64::to_string).collect();
        text.push_str(&release.join("."));
        if let Some((kind, number)) = self.pre {
            text.push_str(kind.as_str());
            text.push_str(&number.to_string());
        }
        if let Some(number) = self.post {
            text.push_str(".post");
            text.push_str(&number.to_string());
        }
        if let Some(number) = self.dev {
            text.push_str(".dev");
            text.push_str(&number.to_string());
        }
        if let Some(local) = &self.local {
            text.push('+');
            text.push_str(local);
        }
        text
    }

    /// A version with just a release segment, for the bounds a specifier builds.
    pub(crate) fn from_release(release: Vec<u64>) -> Self {
        Self {
            epoch: 0,
            release,
            pre: None,
            post: None,
            dev: None,
            local: None,
        }
    }

    /// The PEP 440 form of a semver [`Version`], with build metadata dropped.
    ///
    /// semver build metadata is not part of precedence and PEP 440 treats a
    /// local version specially, so it is discarded rather than compared.
    pub(crate) fn from_semver(version: &Version) -> Option<Self> {
        let rendered = render(version).ok()?;
        let public = rendered.split('+').next().unwrap_or(&rendered);
        Self::parse(public)
    }

    /// Parse a PEP 440 version string, or `None` when it is not one.
    ///
    /// The grammar supported is the one momo meets in a requirement: an epoch,
    /// a release, an optional pre-release, post-release and development
    /// release, and an optional local version. Anything else is refused.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        let lowered = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
        if lowered.is_empty() {
            return None;
        }

        let (epoch, rest) = match lowered.split_once('!') {
            Some((epoch, rest)) => (epoch.parse().ok()?, rest),
            None => (0, lowered),
        };
        let (public, local) = match rest.split_once('+') {
            Some((public, local)) => (public, Some(local.to_ascii_lowercase())),
            None => (rest, None),
        };
        if local.as_deref() == Some("") {
            return None;
        }

        // The release is every digit and separating dot before the first letter.
        let release_end = public
            .find(|character: char| character.is_ascii_alphabetic())
            .unwrap_or(public.len());
        let (release_text, mut suffix) = public.split_at(release_end);
        let release_text = release_text.trim_end_matches(['-', '_', '.']);
        let mut release = Vec::new();
        for segment in release_text.split('.') {
            release.push(segment.parse().ok()?);
        }
        if release.is_empty() {
            return None;
        }

        let mut pre = None;
        if let Some(((kind, number), rest)) = take_pre(suffix) {
            pre = Some((kind, number));
            suffix = rest;
        }
        let mut post = None;
        if let Some((number, rest)) = take_marker(suffix, &["post", "rev", "r"]) {
            post = Some(number);
            suffix = rest;
        }
        let mut dev = None;
        if let Some((number, rest)) = take_marker(suffix, &["dev"]) {
            dev = Some(number);
            suffix = rest;
        }
        if !suffix.is_empty() {
            return None;
        }

        Some(Self {
            epoch,
            release,
            pre,
            post,
            dev,
            local,
        })
    }
}

/// The pre-release marker `text` starts with, and the rest after it.
fn take_pre(text: &str) -> Option<((PreKind, u64), &str)> {
    const KINDS: [(&str, PreKind); 8] = [
        ("alpha", PreKind::Alpha),
        ("beta", PreKind::Beta),
        ("preview", PreKind::Rc),
        ("pre", PreKind::Rc),
        ("rc", PreKind::Rc),
        ("c", PreKind::Rc),
        ("a", PreKind::Alpha),
        ("b", PreKind::Beta),
    ];
    let rest = text.strip_prefix(['-', '_', '.']).unwrap_or(text);
    for (name, kind) in KINDS {
        let Some(after) = strip_ci(rest, name) else {
            continue;
        };
        let after = after.strip_prefix(['-', '_', '.']).unwrap_or(after);
        let (number, after) = take_number(after);
        return Some(((kind, number.unwrap_or(0)), after));
    }
    None
}

/// The post/dev marker `text` starts with, and the rest after it.
fn take_marker<'a>(text: &'a str, names: &[&str]) -> Option<(u64, &'a str)> {
    let rest = text.strip_prefix(['-', '_', '.']).unwrap_or(text);
    for name in names {
        let Some(after) = strip_ci(rest, name) else {
            continue;
        };
        let after = after.strip_prefix(['-', '_', '.']).unwrap_or(after);
        let (number, after) = take_number(after);
        return Some((number.unwrap_or(0), after));
    }
    None
}

/// `text` with an ASCII-case-insensitive `prefix` removed.
fn strip_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// The leading decimal number of `text`, if any, and the rest.
fn take_number(text: &str) -> (Option<u64>, &str) {
    let digits = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    match text[..digits].parse() {
        Ok(number) => (Some(number), &text[digits..]),
        Err(_) => (None, text),
    }
}

/// PEP 440's ordering key, applied segment by segment.
impl Ord for Pep440Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.epoch
            .cmp(&other.epoch)
            .then_with(|| cmp_release(&self.release, &other.release))
            .then_with(|| cmp_pre(self.pre, other.pre))
            // A missing post-release is below any post-release...
            .then_with(|| cmp_option(self.post, other.post, Ordering::Less))
            // ...while a missing dev-release is above any dev-release.
            .then_with(|| cmp_option(self.dev, other.dev, Ordering::Greater))
            .then_with(|| self.local.cmp(&other.local))
    }
}

impl PartialOrd for Pep440Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Compare two release segments, ignoring trailing zeros.
fn cmp_release(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let a = trim_zeros(a);
    let b = trim_zeros(b);
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// `release` without its trailing zeros.
fn trim_zeros(release: &[u64]) -> &[u64] {
    let end = release
        .iter()
        .rposition(|segment| *segment != 0)
        .map_or(0, |index| index + 1);
    &release[..end]
}

/// Compare two pre-releases; an absent one is below any present one.
fn cmp_pre(a: Option<(PreKind, u64)>, b: Option<(PreKind, u64)>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left.cmp(&right),
    }
}

/// Compare two optional numbers, `missing` ordering an absent one.
fn cmp_option(a: Option<u64>, b: Option<u64>, missing: std::cmp::Ordering) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => missing.reverse(),
        (None, Some(_)) => missing,
        (Some(left), Some(right)) => left.cmp(&right),
    }
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
