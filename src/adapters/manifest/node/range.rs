//! Evaluating and widening npm semver ranges.
//!
//! npm's range grammar is not Cargo's: it has `||` unions, hyphen ranges, `x`
//! wildcards and a caret that treats `0.x` specially. momo needs only two
//! questions answered — does a Version satisfy a declared range, and can that
//! range be re-anchored to admit it — so this is a small hand-rolled evaluator
//! rather than a new dependency. It desugars every form it recognises into a
//! union of conjunctions of simple comparators, exactly as `node-semver` does,
//! and refuses (`None`) anything it does not recognise rather than guess.
//!
//! A requirement that names a *protocol* rather than a range (`workspace:`,
//! `file:`, `link:`, `git:`/`github:`, an `npm:` alias, a tarball URL) is the
//! caller's to omit: no npm range contains a `:`, so [`satisfied`] simply
//! refuses one.

use std::cmp::Ordering;

use semver::{Prerelease, Version};

/// Whether `version` satisfies `requirement`, or `None` when the range is not
/// one this evaluator understands.
pub(crate) fn satisfied(requirement: &str, version: &Version) -> Option<bool> {
    parse_range(requirement).map(|range| range.matches(version))
}

/// `requirement` re-anchored at `target`, or `None` for a form momo will not
/// rewrite.
///
/// Only a single-term form is rewritten, preserving its operator: a caret, a
/// tilde, an exact `=x.y.z` or bare `x.y.z`, or a `>=` lower bound. A union, a
/// hyphen range, a wildcard and an upper bound are refused — re-anchoring one
/// could narrow the range rather than widen it, and a wrong rewrite is a semver
/// lie. The result is verified to admit `target` before it is returned, so momo
/// never writes a requirement that excludes the version it was widened for.
pub(crate) fn widen(requirement: &str, target: &Version) -> Option<String> {
    let trimmed = requirement.trim();
    if trimmed.is_empty()
        || trimmed.contains("||")
        || trimmed.contains(char::is_whitespace)
        || trimmed.contains(" - ")
    {
        return None;
    }
    let (op, rest) = split_operator(trimmed);
    let partial = parse_partial(rest)?;
    let widened = match op {
        // A bare partial (`1`, `1.2`) is an X-range, not an exact version; only
        // a fully specified `x.y.z` is exact, and only that is safe to re-anchor.
        "" | "=" => {
            if partial.x_patch {
                return None;
            }
            let eq = if op == "=" { "=" } else { "" };
            format!("{eq}{target}")
        }
        "^" => format!("^{target}"),
        "~" | "~>" => format!("~{target}"),
        ">=" => format!(">={target}"),
        // `>`, `<`, `<=`, `*` and anything unrecognised: an upper bound cannot
        // be widened by re-anchoring it, and `*` already admits everything.
        _ => return None,
    };
    match satisfied(&widened, target) {
        Some(true) => Some(widened),
        _ => None,
    }
}

/// A range: a union of comparator sets. A version satisfies the range when it
/// satisfies any one set.
struct Range {
    /// The `||` alternatives, each a conjunction of comparators.
    sets: Vec<Vec<Comparator>>,
}

impl Range {
    /// Whether `version` satisfies any alternative.
    fn matches(&self, version: &Version) -> bool {
        self.sets.iter().any(|set| matches_set(set, version))
    }
}

/// One comparator: an operator and the version it bounds.
#[derive(Clone)]
struct Comparator {
    /// The bound's operator.
    op: Op,
    /// The version the operator applies to.
    version: Version,
}

/// A comparator's operator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    /// `<`.
    Lt,
    /// `<=`.
    Lte,
    /// `>`.
    Gt,
    /// `>=`.
    Gte,
    /// `=` — an exact match.
    Eq,
}

impl Comparator {
    /// Whether `version` satisfies this bound.
    fn matches(&self, version: &Version) -> bool {
        match self.op {
            Op::Lt => cmp(version, &self.version) == Ordering::Less,
            Op::Lte => cmp(version, &self.version) != Ordering::Greater,
            Op::Gt => cmp(version, &self.version) == Ordering::Greater,
            Op::Gte => cmp(version, &self.version) != Ordering::Less,
            Op::Eq => cmp(version, &self.version) == Ordering::Equal,
        }
    }
}

/// Whether `version` satisfies every comparator in `set`.
///
/// node-semver's rule for prereleases applies: a version carrying one satisfies
/// a set only when a comparator in that set carries a prerelease on the *same*
/// `major.minor.patch`, so `^1.0.0` admits `1.2.3-rc.1` but not `2.0.0-rc.1`.
fn matches_set(set: &[Comparator], version: &Version) -> bool {
    if !set.iter().all(|comparator| comparator.matches(version)) {
        return false;
    }
    if version.pre.is_empty() {
        return true;
    }
    set.iter().any(|comparator| {
        !comparator.version.pre.is_empty()
            && comparator.version.major == version.major
            && comparator.version.minor == version.minor
            && comparator.version.patch == version.patch
    })
}

/// Compare two versions as npm does: build metadata is ignored.
fn cmp(a: &Version, b: &Version) -> Ordering {
    (a.major, a.minor, a.patch)
        .cmp(&(b.major, b.minor, b.patch))
        .then_with(|| a.pre.cmp(&b.pre))
}

/// Parse a full range into its `||` alternatives.
fn parse_range(input: &str) -> Option<Range> {
    let mut sets = Vec::new();
    for alternative in input.split("||") {
        sets.push(parse_set(alternative.trim())?);
    }
    Some(Range { sets })
}

/// Parse one `||` alternative: a hyphen range, or a space-separated conjunction
/// of comparators.
fn parse_set(alternative: &str) -> Option<Vec<Comparator>> {
    if alternative.is_empty() {
        // An empty alternative (or an empty range) is `*`.
        return Some(Vec::new());
    }
    if let Some((from, to)) = split_hyphen(alternative) {
        return Some(hyphen(&parse_partial(from)?, &parse_partial(to)?));
    }
    let mut comparators = Vec::new();
    for token in alternative.split_whitespace() {
        comparators.extend(parse_token(token)?);
    }
    Some(comparators)
}

/// Split a hyphen range `from - to` when it has exactly one ` - `.
fn split_hyphen(alternative: &str) -> Option<(&str, &str)> {
    let (from, to) = alternative.split_once(" - ")?;
    let (from, to) = (from.trim(), to.trim());
    if from.is_empty() || to.is_empty() || to.contains(" - ") {
        return None;
    }
    Some((from, to))
}

/// The operator prefix of a comparator token, longest first.
fn split_operator(token: &str) -> (&str, &str) {
    const OPERATORS: [&str; 8] = [">=", "<=", "~>", ">", "<", "=", "~", "^"];
    for op in OPERATORS {
        if let Some(rest) = token.strip_prefix(op) {
            return (op, rest.trim());
        }
    }
    ("", token)
}

/// Parse one comparator token into its desugared comparators.
fn parse_token(token: &str) -> Option<Vec<Comparator>> {
    let (op, rest) = split_operator(token);
    let partial = parse_partial(rest)?;
    Some(desugar(op, &partial))
}

/// Desugar one operator and partial version into simple comparators.
fn desugar(op: &str, partial: &Partial) -> Vec<Comparator> {
    match op {
        "^" => caret(partial),
        "~" | "~>" => tilde(partial),
        _ => xrange(op, partial),
    }
}

/// Desugar a bare, `=`, `>=`, `>`, `<=` or `<` comparator, applying npm's
/// X-range rules when the version has a wildcard segment.
fn xrange(op: &str, partial: &Partial) -> Vec<Comparator> {
    // `=1.2` is an X-range, not an exact match.
    let op = if op == "=" && partial.x_patch { "" } else { op };

    if partial.x_major {
        // `*` admits everything; `>*`/`<*` admit nothing.
        return if op == ">" || op == "<" {
            vec![bound("", 0, 0, 0, "0")]
        } else {
            Vec::new()
        };
    }

    if !op.is_empty() && partial.x_patch {
        let mut major = partial.version.major;
        let mut minor = partial.version.minor;
        let op = match op {
            // `>1` is `>=2.0.0`; `>1.2` is `>=1.3.0`.
            ">" => {
                if partial.x_minor {
                    major += 1;
                    minor = 0;
                } else {
                    minor += 1;
                }
                ">="
            }
            // `<=1` is `<2.0.0`; `<=1.2` is `<1.3.0`.
            "<=" => {
                if partial.x_minor {
                    major += 1;
                } else {
                    minor += 1;
                }
                "<"
            }
            other => other,
        };
        let pre = if op == "<" { "0" } else { "" };
        return vec![bound(op, major, minor, 0, pre)];
    }

    if partial.x_minor {
        return vec![
            bound(">=", partial.version.major, 0, 0, ""),
            bound("<", partial.version.major + 1, 0, 0, "0"),
        ];
    }
    if partial.x_patch {
        return vec![
            bound(">=", partial.version.major, partial.version.minor, 0, ""),
            bound(
                "<",
                partial.version.major,
                partial.version.minor + 1,
                0,
                "0",
            ),
        ];
    }

    vec![Comparator {
        op: op_of(op),
        version: partial.version.clone(),
    }]
}

/// Desugar a caret range, preserving npm's special handling of `0.x`.
fn caret(partial: &Partial) -> Vec<Comparator> {
    if partial.x_major {
        return Vec::new();
    }
    let (major, minor) = (partial.version.major, partial.version.minor);
    if partial.x_minor {
        return vec![
            bound(">=", major, 0, 0, ""),
            bound("<", major + 1, 0, 0, "0"),
        ];
    }
    if partial.x_patch {
        if major == 0 {
            return vec![
                bound(">=", 0, minor, 0, ""),
                bound("<", 0, minor + 1, 0, "0"),
            ];
        }
        return vec![
            bound(">=", major, minor, 0, ""),
            bound("<", major + 1, 0, 0, "0"),
        ];
    }
    let patch = partial.version.patch;
    let pre = partial.version.pre.as_str();
    if major == 0 && minor == 0 {
        return vec![
            bound(">=", 0, 0, patch, pre),
            bound("<", 0, 0, patch + 1, "0"),
        ];
    }
    if major == 0 {
        return vec![
            bound(">=", 0, minor, patch, pre),
            bound("<", 0, minor + 1, 0, "0"),
        ];
    }
    vec![
        bound(">=", major, minor, patch, pre),
        bound("<", major + 1, 0, 0, "0"),
    ]
}

/// Desugar a tilde range.
fn tilde(partial: &Partial) -> Vec<Comparator> {
    if partial.x_major {
        return Vec::new();
    }
    let (major, minor) = (partial.version.major, partial.version.minor);
    if partial.x_minor {
        return vec![
            bound(">=", major, 0, 0, ""),
            bound("<", major + 1, 0, 0, "0"),
        ];
    }
    if partial.x_patch {
        return vec![
            bound(">=", major, minor, 0, ""),
            bound("<", major, minor + 1, 0, "0"),
        ];
    }
    vec![
        bound(
            ">=",
            major,
            minor,
            partial.version.patch,
            partial.version.pre.as_str(),
        ),
        bound("<", major, minor + 1, 0, "0"),
    ]
}

/// Desugar a hyphen range into its lower and upper bounds.
fn hyphen(from: &Partial, to: &Partial) -> Vec<Comparator> {
    let mut comparators = Vec::new();
    if !from.x_major {
        if from.x_minor {
            comparators.push(bound(">=", from.version.major, 0, 0, ""));
        } else if from.x_patch {
            comparators.push(bound(">=", from.version.major, from.version.minor, 0, ""));
        } else {
            comparators.push(bound(
                ">=",
                from.version.major,
                from.version.minor,
                from.version.patch,
                from.version.pre.as_str(),
            ));
        }
    }
    if !to.x_major {
        if to.x_minor {
            comparators.push(bound("<", to.version.major + 1, 0, 0, "0"));
        } else if to.x_patch {
            comparators.push(bound("<", to.version.major, to.version.minor + 1, 0, "0"));
        } else {
            comparators.push(bound(
                "<=",
                to.version.major,
                to.version.minor,
                to.version.patch,
                to.version.pre.as_str(),
            ));
        }
    }
    comparators
}

/// A comparator built from its parts; `pre` is a prerelease or `""`.
fn bound(op: &str, major: u64, minor: u64, patch: u64, pre: &str) -> Comparator {
    let mut version = Version::new(major, minor, patch);
    if !pre.is_empty() {
        version.pre = Prerelease::new(pre).expect("a literal or already-parsed prerelease");
    }
    Comparator {
        op: op_of(op),
        version,
    }
}

/// The [`Op`] an operator string names; a bare comparator is exact.
fn op_of(op: &str) -> Op {
    match op {
        "<" => Op::Lt,
        "<=" => Op::Lte,
        ">" => Op::Gt,
        ">=" => Op::Gte,
        _ => Op::Eq,
    }
}

/// A partially specified version: any segment may be a wildcard or missing.
struct Partial {
    /// The version with every wildcard segment filled as `0`.
    version: Version,
    /// Whether the major segment is a wildcard (`*`, `x`, `X`).
    x_major: bool,
    /// Whether the major or minor segment is a wildcard.
    x_minor: bool,
    /// Whether any segment is a wildcard or missing.
    x_patch: bool,
}

/// Parse a partial version, e.g. `1.2.3`, `1.2`, `1.x`, `*`, `1.2.3-rc.1`.
fn parse_partial(text: &str) -> Option<Partial> {
    let (main, build) = match text.split_once('+') {
        Some((main, build)) => (main, build),
        None => (text, ""),
    };
    let (release, pre) = match main.split_once('-') {
        Some((release, pre)) => (release, pre),
        None => (main, ""),
    };

    let mut numbers = [0u64; 3];
    let mut wildcards = [false; 3];
    let mut seen = 0;
    for (index, segment) in release.split('.').enumerate() {
        if index >= 3 {
            return None;
        }
        if segment.is_empty() {
            return None;
        }
        if matches!(segment, "x" | "X" | "*") {
            wildcards[index] = true;
        } else {
            numbers[index] = segment.parse().ok()?;
        }
        seen = index + 1;
    }
    // A segment the version omits is a wildcard, as node-semver reads it.
    for wildcard in wildcards.iter_mut().skip(seen) {
        *wildcard = true;
    }

    let mut version = Version::new(numbers[0], numbers[1], numbers[2]);
    if !pre.is_empty() {
        version.pre = Prerelease::new(pre).ok()?;
    }
    if !build.is_empty() {
        version.build = semver::BuildMetadata::new(build).ok()?;
    }

    let x_major = wildcards[0];
    let x_minor = x_major || wildcards[1];
    let x_patch = x_minor || wildcards[2];
    Some(Partial {
        version,
        x_major,
        x_minor,
        x_patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    fn satisfies(requirement: &str, target: &str) -> Option<bool> {
        satisfied(requirement, &version(target))
    }

    #[test]
    fn a_caret_admits_the_versions_below_its_next_major() {
        assert_eq!(satisfies("^1.2.3", "1.2.3"), Some(true));
        assert_eq!(satisfies("^1.2.3", "1.9.9"), Some(true));
        assert_eq!(satisfies("^1.2.3", "1.2.2"), Some(false));
        assert_eq!(satisfies("^1.2.3", "2.0.0"), Some(false));
    }

    #[test]
    fn a_caret_on_a_zero_major_stops_at_the_next_minor() {
        assert_eq!(satisfies("^0.2.3", "0.2.9"), Some(true));
        assert_eq!(satisfies("^0.2.3", "0.3.0"), Some(false));
        assert_eq!(satisfies("^0.0.3", "0.0.3"), Some(true));
        assert_eq!(satisfies("^0.0.3", "0.0.4"), Some(false));
        assert_eq!(satisfies("^0.0", "0.0.9"), Some(true));
        assert_eq!(satisfies("^0.0", "0.1.0"), Some(false));
        assert_eq!(satisfies("^0", "0.9.9"), Some(true));
        assert_eq!(satisfies("^0", "1.0.0"), Some(false));
    }

    #[test]
    fn a_tilde_admits_only_the_same_minor() {
        assert_eq!(satisfies("~1.2.3", "1.2.9"), Some(true));
        assert_eq!(satisfies("~1.2.3", "1.3.0"), Some(false));
        assert_eq!(satisfies("~1.2", "1.2.0"), Some(true));
        assert_eq!(satisfies("~1", "1.9.0"), Some(true));
        assert_eq!(satisfies("~1", "2.0.0"), Some(false));
        assert_eq!(satisfies("~>1.2", "1.2.5"), Some(true));
    }

    #[test]
    fn an_exact_or_bare_version_matches_only_itself() {
        assert_eq!(satisfies("1.2.3", "1.2.3"), Some(true));
        assert_eq!(satisfies("1.2.3", "1.2.4"), Some(false));
        assert_eq!(satisfies("=1.2.3", "1.2.3"), Some(true));
        assert_eq!(satisfies("=1.2.3", "1.2.4"), Some(false));
    }

    #[test]
    fn a_wildcard_partial_is_an_x_range() {
        assert_eq!(satisfies("1", "1.9.9"), Some(true));
        assert_eq!(satisfies("1", "2.0.0"), Some(false));
        assert_eq!(satisfies("1.2", "1.2.9"), Some(true));
        assert_eq!(satisfies("1.2", "1.3.0"), Some(false));
        assert_eq!(satisfies("1.2.x", "1.2.9"), Some(true));
        assert_eq!(satisfies("1.x", "1.9.9"), Some(true));
        assert_eq!(satisfies("*", "9.9.9"), Some(true));
        assert_eq!(satisfies("x", "0.0.0"), Some(true));
        assert_eq!(satisfies("=1.2", "1.2.9"), Some(true));
        assert_eq!(satisfies("=1.2", "1.3.0"), Some(false));
    }

    #[test]
    fn comparators_apply_their_operator() {
        assert_eq!(satisfies(">=1.0.0", "1.0.0"), Some(true));
        assert_eq!(satisfies(">=1.0.0", "0.9.9"), Some(false));
        assert_eq!(satisfies(">1.0.0", "1.0.1"), Some(true));
        assert_eq!(satisfies(">1.0.0", "1.0.0"), Some(false));
        assert_eq!(satisfies("<=1.0.0", "1.0.0"), Some(true));
        assert_eq!(satisfies("<1.0.0", "1.0.0"), Some(false));
        assert_eq!(satisfies(">=1.2", "1.2.0"), Some(true));
        assert_eq!(satisfies(">1.2", "1.3.0"), Some(true));
        assert_eq!(satisfies(">1.2", "1.2.9"), Some(false));
        assert_eq!(satisfies("<=1.2", "1.3.0"), Some(false));
        assert_eq!(satisfies("<=1.2", "1.2.9"), Some(true));
    }

    #[test]
    fn a_conjunction_requires_every_comparator() {
        assert_eq!(satisfies(">=1.0.0 <2.0.0", "1.5.0"), Some(true));
        assert_eq!(satisfies(">=1.0.0 <2.0.0", "2.0.0"), Some(false));
    }

    #[test]
    fn a_hyphen_range_is_inclusive_of_its_bounds() {
        assert_eq!(satisfies("1.2.3 - 2.3.4", "1.2.3"), Some(true));
        assert_eq!(satisfies("1.2.3 - 2.3.4", "2.3.4"), Some(true));
        assert_eq!(satisfies("1.2.3 - 2.3.4", "2.3.5"), Some(false));
        assert_eq!(satisfies("1.2 - 2.3.4", "1.2.0"), Some(true));
        assert_eq!(satisfies("1.2.3 - 2.3", "2.3.9"), Some(true));
        assert_eq!(satisfies("1.2.3 - 2.3", "2.4.0"), Some(false));
        assert_eq!(satisfies("1.2.3 - 2", "2.9.9"), Some(true));
        assert_eq!(satisfies("1.2.3 - 2", "3.0.0"), Some(false));
    }

    #[test]
    fn a_union_admits_either_alternative() {
        assert_eq!(satisfies("^1.0.0 || ^3.0.0", "1.5.0"), Some(true));
        assert_eq!(satisfies("^1.0.0 || ^3.0.0", "3.5.0"), Some(true));
        assert_eq!(satisfies("^1.0.0 || ^3.0.0", "2.0.0"), Some(false));
    }

    #[test]
    fn a_prerelease_only_matches_a_range_that_names_one() {
        // `^1.0.0` does not admit a prerelease of a different tuple...
        assert_eq!(satisfies("^1.0.0", "2.0.0-rc.1"), Some(false));
        // ...but a range whose comparator carries the same tuple does.
        assert_eq!(satisfies(">=1.2.3-rc.1", "1.2.3-rc.2"), Some(true));
        assert_eq!(satisfies(">=1.2.3-rc.1", "1.2.4-rc.1"), Some(false));
        assert_eq!(satisfies("*", "1.0.0-rc.1"), Some(false));
    }

    #[test]
    fn an_unrecognised_range_is_never_reported_as_satisfied() {
        assert_eq!(satisfies("not a range", "1.0.0"), None);
        assert_eq!(satisfies("workspace:*", "1.0.0"), None);
        assert_eq!(satisfies("npm:other@^1.0.0", "1.0.0"), None);
        assert_eq!(satisfies("", "1.0.0"), Some(true));
    }

    #[test]
    fn a_widened_requirement_preserves_its_operator() {
        let target = version("2.0.0");
        for (requirement, expected) in [
            ("^1.0.0", "^2.0.0"),
            ("~1.2", "~2.0.0"),
            ("=1.2.3", "=2.0.0"),
            ("1.2.3", "2.0.0"),
            (">=1.0.0", ">=2.0.0"),
        ] {
            assert_eq!(
                widen(requirement, &target).as_deref(),
                Some(expected),
                "`{requirement}` widened wrongly"
            );
        }
    }

    #[test]
    fn an_unwritable_requirement_is_refused() {
        let target = version("2.0.0");
        for requirement in [
            "*",
            "1.x",
            "1.2",
            "=1.2",
            ">1.0.0",
            "<2.0.0",
            "<=2.0.0",
            ">=1.0.0 <3.0.0",
            "1.2.3 - 2.0.0",
            "^1.0.0 || ^2.0.0",
            "not a range",
            "",
        ] {
            assert_eq!(
                widen(requirement, &target),
                None,
                "`{requirement}` was rewritten instead of refused"
            );
        }
    }

    #[test]
    fn a_widened_requirement_always_admits_its_target() {
        let target = version("2.0.0");
        for requirement in ["^1.0.0", "~1.2", "=1.2.3", "1.2.3", ">=1.0.0", "^1.2"] {
            let widened = widen(requirement, &target).expect("a widen");
            assert_eq!(
                satisfied(&widened, &target),
                Some(true),
                "`{widened}` does not admit {target}"
            );
        }
    }
}
