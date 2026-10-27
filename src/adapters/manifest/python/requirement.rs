//! Reading and evaluating a Python requirement's version specifier.
//!
//! Two grammars reach momo. A PEP 508 requirement string (`acme>=1.2,<2`, read
//! from `[project].dependencies`) carries the package name and its specifier;
//! Poetry's `[tool.poetry.dependencies]` carries the specifier alone
//! (`acme = "^1.2"`). Both are evaluated here: the PEP 440 operators (`==`,
//! `!=`, `>=`, `<=`, `>`, `<`, `~=`, `===`) and Poetry's caret, tilde and
//! wildcard constraints, over [`Pep440Version`]'s PEP 440 ordering.
//!
//! A form with no meaning momo can evaluate — a marker-only requirement, a
//! direct URL, a local version, a union — is refused (`None`) rather than
//! guessed at, and `widen` rewrites only a single-term form, re-anchored at the
//! target and verified to admit it.

use semver::Version;

use super::pep440::{self, Pep440Version};

/// A PEP 508 requirement string split into the parts momo needs.
pub(crate) struct Requirement {
    /// The package name, as written.
    pub(crate) name: String,
    /// The version specifier, verbatim, e.g. `>=1.2,<2`.
    pub(crate) specifier: String,
    /// The text before the specifier (name, extras and spacing), so a rewritten
    /// requirement keeps everything but the specifier.
    pub(crate) prefix: String,
    /// The text after the specifier, an environment marker included.
    pub(crate) suffix: String,
}

/// Parse a PEP 508 requirement string.
///
/// `None` when there is no version specifier to read — a bare name, or a direct
/// URL reference (`acme @ https://…`) — because such a requirement constrains
/// nothing a Release could break. An environment marker is kept in
/// [`Requirement::suffix`] and *not* evaluated: momo is not a resolver, so the
/// requirement is reported regardless of the marker.
pub(crate) fn parse_requirement(input: &str) -> Option<Requirement> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    // A marker starts at the first `;`; it is not part of the specifier.
    let requirement = match text.find(';') {
        Some(at) => &text[..at],
        None => text,
    };

    let bytes = requirement.as_bytes();
    let mut index = 0;
    while index < bytes.len() && is_name_byte(bytes[index]) {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    let name = requirement[..index].to_owned();
    if bytes.get(index) == Some(&b'[') {
        index += requirement[index..].find(']')? + 1;
    }

    let after = &requirement[index..];
    let trimmed = after.trim_start();
    let leading = after.len() - trimmed.len();
    if trimmed.is_empty() || trimmed.starts_with('@') {
        return None;
    }
    let specifier_start = index + leading;
    let specifier_end = specifier_start + trimmed.trim_end().len();

    Some(Requirement {
        name,
        specifier: requirement[specifier_start..specifier_end].to_owned(),
        prefix: text[..specifier_start].to_owned(),
        suffix: text[specifier_end..].to_owned(),
    })
}

/// Whether `byte` can appear in a PEP 508 distribution name.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

/// Whether `version` satisfies the constraint `specifier`.
///
/// `None` when the constraint is not one momo evaluates. PEP 440's rule that a
/// pre-release is excluded unless a specifier names one is honoured, so a
/// Release Candidate target does not silently satisfy a stable range.
pub(crate) fn satisfied(specifier: &str, version: &Version) -> Option<bool> {
    let specifiers = parse_specifiers(specifier)?;
    let candidate = Pep440Version::from_semver(version)?;
    Some(matches(&specifiers, &candidate))
}

/// `specifier` re-anchored at `target`, or `None` for a form momo will not
/// rewrite.
///
/// Only a single-term form is rewritten, preserving its operator. A conjunction
/// (`>=1.2,<2`), a union, a wildcard and an upper bound are refused: re-anchoring
/// one could narrow the range rather than widen it. The result is verified to
/// admit `target` before it is returned.
pub(crate) fn widen(specifier: &str, target: &Version) -> Option<String> {
    let specifiers = parse_specifiers(specifier)?;
    let [only] = specifiers.as_slice() else {
        return None;
    };
    let Specifier::Bound {
        op,
        wildcard: false,
        ..
    } = only
    else {
        return None;
    };
    // The PEP 440 spelling of the target; a local version has no specifier form
    // momo will write.
    let rendered = pep440::render(target).ok()?;
    if rendered.contains('+') {
        return None;
    }
    let widened = match op {
        Op::Caret => format!("^{rendered}"),
        Op::Tilde => format!("~{rendered}"),
        Op::Compatible => format!("~={rendered}"),
        Op::GtEq => format!(">={rendered}"),
        Op::Exact => rendered,
        Op::Eq => format!("=={rendered}"),
        _ => return None,
    };
    match satisfied(&widened, target) {
        Some(true) => Some(widened),
        _ => None,
    }
}

/// Whether every specifier in `specifiers` admits `candidate`.
fn matches(specifiers: &[Specifier], candidate: &Pep440Version) -> bool {
    if candidate.is_prerelease()
        && !specifiers.iter().any(|specifier| match specifier {
            Specifier::Bound { version, .. } => version.is_prerelease(),
            Specifier::Any | Specifier::Arbitrary(_) => false,
        })
    {
        return false;
    }
    specifiers
        .iter()
        .all(|specifier| specifier.matches(candidate))
}

/// One version specifier, a comma-separated set of which is AND-ed.
enum Specifier {
    /// `*` — admits everything.
    Any,
    /// An operator and the version it applies to.
    Bound {
        /// The operator.
        op: Op,
        /// The version operand.
        version: Pep440Version,
        /// Whether the operand was written as a prefix (`==1.2.*`).
        wildcard: bool,
    },
    /// `===` — an arbitrary string equality, the operand kept verbatim.
    Arbitrary(String),
}

impl Specifier {
    /// Whether `candidate` satisfies this specifier.
    fn matches(&self, candidate: &Pep440Version) -> bool {
        match self {
            Specifier::Any => true,
            Specifier::Arbitrary(operand) => candidate.canonical().eq_ignore_ascii_case(operand),
            Specifier::Bound {
                op,
                version,
                wildcard,
            } => match op {
                Op::Exact | Op::Eq => {
                    if *wildcard {
                        prefix(candidate, version, version.release().len())
                    } else {
                        candidate == version
                    }
                }
                Op::NotEq => {
                    if *wildcard {
                        !prefix(candidate, version, version.release().len())
                    } else {
                        candidate != version
                    }
                }
                Op::Gt => candidate > version,
                Op::GtEq => candidate >= version,
                Op::Lt => candidate < version,
                Op::LtEq => candidate <= version,
                // `~=X.Y.Z` is `>=X.Y.Z, ==X.Y.*`.
                Op::Compatible => {
                    candidate >= version
                        && prefix(
                            candidate,
                            version,
                            version.release().len().saturating_sub(1),
                        )
                }
                Op::Caret => candidate >= version && candidate < &caret_upper(version),
                Op::Tilde => candidate >= version && candidate < &tilde_upper(version),
            },
        }
    }
}

/// A specifier's operator.
#[derive(Clone, Copy)]
enum Op {
    /// `^` — Poetry's caret.
    Caret,
    /// `~` — Poetry's tilde.
    Tilde,
    /// `~=` — PEP 440's compatible release.
    Compatible,
    /// `==`.
    Eq,
    /// `!=`.
    NotEq,
    /// `>`.
    Gt,
    /// `>=`.
    GtEq,
    /// `<`.
    Lt,
    /// `<=`.
    LtEq,
    /// A bare version, or `=`.
    Exact,
}

/// Parse a comma-separated specifier set.
fn parse_specifiers(text: &str) -> Option<Vec<Specifier>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "*" {
        return Some(vec![Specifier::Any]);
    }
    let mut specifiers = Vec::new();
    for clause in trimmed.split(',') {
        specifiers.push(parse_specifier(clause.trim())?);
    }
    Some(specifiers)
}

/// Parse one specifier.
fn parse_specifier(clause: &str) -> Option<Specifier> {
    if clause.is_empty() {
        return None;
    }
    if clause == "*" {
        return Some(Specifier::Any);
    }
    let (op, rest) = split_operator(clause);
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if op == "===" {
        return Some(Specifier::Arbitrary(rest.to_owned()));
    }

    let wildcard = rest.ends_with(".*");
    let version_text = if wildcard {
        &rest[..rest.len() - 2]
    } else {
        rest
    };
    let version = Pep440Version::parse(version_text)?;
    // A local version is not compared, so a specifier naming one is refused.
    if version.has_local() {
        return None;
    }
    let op = op_of(op)?;
    // A wildcard is meaningful only for an equality.
    if wildcard && !matches!(op, Op::Eq | Op::Exact | Op::NotEq) {
        return None;
    }
    Some(Specifier::Bound {
        op,
        version,
        wildcard,
    })
}

/// Split a specifier's operator prefix, longest first.
fn split_operator(clause: &str) -> (&str, &str) {
    const OPERATORS: [&str; 11] = ["===", "~=", "==", "!=", ">=", "<=", ">", "<", "^", "~", "="];
    for op in OPERATORS {
        if let Some(rest) = clause.strip_prefix(op) {
            return (op, rest);
        }
    }
    ("", clause)
}

/// The [`Op`] an operator string names.
fn op_of(op: &str) -> Option<Op> {
    Some(match op {
        "" | "=" => Op::Exact,
        "==" => Op::Eq,
        "!=" => Op::NotEq,
        ">=" => Op::GtEq,
        ">" => Op::Gt,
        "<=" => Op::LtEq,
        "<" => Op::Lt,
        "~=" => Op::Compatible,
        "^" => Op::Caret,
        "~" => Op::Tilde,
        _ => return None,
    })
}

/// Whether `candidate`'s release starts with `operand`'s, over `len` segments.
///
/// A missing segment reads as `0`, so PEP 440's insignificant trailing zeros do
/// not defeat a prefix match.
fn prefix(candidate: &Pep440Version, operand: &Pep440Version, len: usize) -> bool {
    (0..len).all(|index| {
        candidate.release().get(index).copied().unwrap_or(0)
            == operand.release().get(index).copied().unwrap_or(0)
    })
}

/// The exclusive upper bound of a caret constraint, e.g. `^1.2.3` → `2`.
///
/// An omitted segment is not an explicit zero. A release of length 1 (`^0`) has
/// no minor, so it breaks at the next major; length 2 (`^0.0`) has no patch, so
/// it breaks at the next minor; only a full `^0.0.0` breaks at the next patch.
/// This mirrors poetry-core's `next_breaking`.
fn caret_upper(version: &Pep440Version) -> Pep440Version {
    let release = version.release();
    let major = release.first().copied().unwrap_or(0);
    let minor = release.get(1).copied();
    if major > 0 || minor.is_none() {
        return Pep440Version::from_release(vec![major + 1]);
    }
    let minor = minor.unwrap_or(0);
    let patch = release.get(2).copied();
    if minor > 0 || patch.is_none() {
        return Pep440Version::from_release(vec![0, minor + 1]);
    }
    let patch = patch.unwrap_or(0);
    Pep440Version::from_release(vec![0, 0, patch + 1])
}

/// The exclusive upper bound of a tilde constraint, e.g. `~1.2.3` → `1.3`.
fn tilde_upper(version: &Pep440Version) -> Pep440Version {
    let release = version.release();
    if release.len() <= 1 {
        return Pep440Version::from_release(vec![release.first().copied().unwrap_or(0) + 1]);
    }
    Pep440Version::from_release(vec![release[0], release[1] + 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    fn satisfies(specifier: &str, target: &str) -> Option<bool> {
        satisfied(specifier, &version(target))
    }

    #[test]
    fn a_requirement_string_is_split_into_its_parts() {
        let parsed = parse_requirement("acme[extra]>=1.2,<2 ; python_version < \"3.9\"")
            .expect("a requirement");

        assert_eq!(parsed.name, "acme");
        assert_eq!(parsed.specifier, ">=1.2,<2");
        assert_eq!(parsed.prefix, "acme[extra]");
        assert_eq!(parsed.suffix, " ; python_version < \"3.9\"");
    }

    #[test]
    fn a_requirement_without_a_specifier_is_refused() {
        assert!(parse_requirement("acme").is_none());
        assert!(parse_requirement("acme @ https://example.com/acme.whl").is_none());
        assert!(parse_requirement("").is_none());
    }

    #[test]
    fn the_pep_440_operators_apply_their_meaning() {
        assert_eq!(satisfies(">=1.2", "1.2.0"), Some(true));
        assert_eq!(satisfies(">=1.2", "1.1.9"), Some(false));
        assert_eq!(satisfies(">1.2", "1.2.0"), Some(false));
        assert_eq!(satisfies("<=1.2", "1.2.0"), Some(true));
        assert_eq!(satisfies("<1.2", "1.2.0"), Some(false));
        assert_eq!(satisfies("==1.2.3", "1.2.3"), Some(true));
        assert_eq!(satisfies("==1.2.3", "1.2.4"), Some(false));
        assert_eq!(satisfies("!=1.2.3", "1.2.4"), Some(true));
        assert_eq!(satisfies("!=1.2.3", "1.2.3"), Some(false));
        assert_eq!(satisfies("===1.2.3", "1.2.3"), Some(true));
    }

    #[test]
    fn an_exact_match_ignores_insignificant_trailing_zeros() {
        // PEP 440 reads `1.2` and `1.2.0` as the same version, so an exact
        // match against either admits the other, and `!=` rejects it.
        assert_eq!(satisfies("==1.2", "1.2.0"), Some(true));
        assert_eq!(satisfies("==1.2.0", "1.2.0"), Some(true));
        assert_eq!(satisfies("!=1.2", "1.2.0"), Some(false));
        assert_eq!(satisfies("1.2", "1.2.0"), Some(true));
    }

    #[test]
    fn a_compatible_release_admits_the_same_prefix() {
        assert_eq!(satisfies("~=1.4.2", "1.4.9"), Some(true));
        assert_eq!(satisfies("~=1.4.2", "1.5.0"), Some(false));
        assert_eq!(satisfies("~=1.4", "1.9.0"), Some(true));
        assert_eq!(satisfies("~=1.4", "2.0.0"), Some(false));
    }

    #[test]
    fn a_conjunction_requires_every_specifier() {
        assert_eq!(satisfies(">=1.2,<2", "1.5.0"), Some(true));
        assert_eq!(satisfies(">=1.2,<2", "2.0.0"), Some(false));
    }

    #[test]
    fn a_pep_440_prefix_match_ignores_trailing_zeros() {
        assert_eq!(satisfies("==1.2.*", "1.2.9"), Some(true));
        assert_eq!(satisfies("==1.2.*", "1.3.0"), Some(false));
        assert_eq!(satisfies("==1.*", "1.9.9"), Some(true));
        assert_eq!(satisfies("!=1.2.*", "1.2.9"), Some(false));
        assert_eq!(satisfies("!=1.2.*", "1.3.0"), Some(true));
    }

    #[test]
    fn poetrys_caret_and_tilde_are_understood() {
        assert_eq!(satisfies("^1.2.3", "1.9.0"), Some(true));
        assert_eq!(satisfies("^1.2.3", "2.0.0"), Some(false));
        assert_eq!(satisfies("^0.2.3", "0.2.9"), Some(true));
        assert_eq!(satisfies("^0.2.3", "0.3.0"), Some(false));
        assert_eq!(satisfies("^0.0.3", "0.0.4"), Some(false));
        assert_eq!(satisfies("~1.2.3", "1.2.9"), Some(true));
        assert_eq!(satisfies("~1.2.3", "1.3.0"), Some(false));
        assert_eq!(satisfies("~1", "1.9.9"), Some(true));
        assert_eq!(satisfies("~1", "2.0.0"), Some(false));
        assert_eq!(satisfies("1.2.*", "1.2.9"), Some(true));
        assert_eq!(satisfies("*", "9.9.9"), Some(true));
    }

    #[test]
    fn a_caret_on_a_zero_major_breaks_at_the_written_precision() {
        // `^0` omits the minor, so it breaks at the next major, not the next
        // patch; `^0.0` omits the patch, so it breaks at the next minor.
        assert_eq!(satisfies("^0", "0.9.9"), Some(true));
        assert_eq!(satisfies("^0", "1.0.0"), Some(false));
        assert_eq!(satisfies("^0.0", "0.0.9"), Some(true));
        assert_eq!(satisfies("^0.0", "0.1.0"), Some(false));
        // A fully written `^0.0.0` still breaks at the next patch.
        assert_eq!(satisfies("^0.0.0", "0.0.1"), Some(false));
    }

    #[test]
    fn a_prerelease_is_excluded_unless_a_specifier_names_one() {
        // The target is a semver Version; `2.0.0-rc.1` renders to `2.0.0rc1`.
        assert_eq!(satisfies(">=1.0", "2.0.0-rc.1"), Some(false));
        assert_eq!(satisfies(">=1.0rc1", "2.0.0-rc.1"), Some(true));
        assert_eq!(satisfies("==2.0.0rc1", "2.0.0-rc.1"), Some(true));
    }

    #[test]
    fn a_form_with_no_pep_440_meaning_is_refused() {
        assert_eq!(satisfies("", "1.0.0"), None);
        assert_eq!(satisfies(">=1.0,", "1.0.0"), None);
        assert_eq!(satisfies(">=1.0 || <2.0", "1.0.0"), None);
        assert_eq!(satisfies("==1.2.3+local", "1.2.3"), None);
        assert_eq!(satisfies(">=1.0.*", "1.0.0"), None);
    }

    #[test]
    fn a_widened_specifier_preserves_its_operator() {
        let target = version("2.0.0");
        for (specifier, expected) in [
            ("^1.2.3", "^2.0.0"),
            ("~1.2", "~2.0.0"),
            ("~=1.4", "~=2.0.0"),
            (">=1.0", ">=2.0.0"),
            ("==1.2.3", "==2.0.0"),
            ("1.2.3", "2.0.0"),
        ] {
            assert_eq!(
                widen(specifier, &target).as_deref(),
                Some(expected),
                "`{specifier}` widened wrongly"
            );
        }
    }

    #[test]
    fn an_unwritable_specifier_is_refused() {
        let target = version("2.0.0");
        for specifier in [
            "*",
            "!=1.2.3",
            "<2.0.0",
            "<=2.0.0",
            ">1.0.0",
            ">=1.0,<3.0",
            "==1.2.*",
            "===1.2.3",
            "not a specifier",
            "",
        ] {
            assert_eq!(
                widen(specifier, &target),
                None,
                "`{specifier}` was rewritten instead of refused"
            );
        }
    }

    #[test]
    fn a_widened_specifier_always_admits_its_target() {
        let target = version("2.0.0");
        for specifier in ["^1.2.3", "~1.2", "~=1.4", ">=1.0", "==1.2.3", "1.2.3"] {
            let widened = widen(specifier, &target).expect("a widen");
            assert_eq!(
                satisfied(&widened, &target),
                Some(true),
                "`{widened}` does not admit {target}"
            );
        }
    }
}
