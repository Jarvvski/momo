//! The reference-link footer helpers: recognising the links momo owns and
//! splitting the trailing definitions off the document body.

use semver::Version;

/// Whether `line` is a Markdown reference-link definition.
fn is_reference_link(line: &str) -> bool {
    line.starts_with('[') && line.contains("]: ")
}

/// The label of a reference-link definition, e.g. `Unreleased` in
/// `[Unreleased]: https://…`.
fn link_label(line: &str) -> Option<&str> {
    line.strip_prefix('[')?
        .split_once("]: ")
        .map(|(label, _)| label)
}

/// Whether `line` is a reference link momo owns and regenerates.
///
/// momo owns the `[Unreleased]` link and the `[x.y.z]` version links; every
/// other definition in the footer is the user's and is preserved.
pub(super) fn is_owned_link(line: &str) -> bool {
    link_label(line).is_some_and(|label| {
        label.eq_ignore_ascii_case("unreleased") || Version::parse(label).is_ok()
    })
}

/// Split trailing reference links off `lines`.
///
/// The footer is the block of `[label]: url` lines at the end of the document,
/// separated from the body by a blank line. Without one the footer is empty.
pub(super) fn split_footer<'a, 'b>(lines: &'a [&'b str]) -> (&'a [&'b str], &'a [&'b str]) {
    let end = lines
        .iter()
        .rposition(|line| !line.is_empty())
        .map_or(0, |index| index + 1);
    let start = lines[..end]
        .iter()
        .rposition(|line| !is_reference_link(line))
        .map_or(0, |index| index + 1);
    let separated = start > 0 && lines[start - 1].is_empty();
    if start < end && separated {
        (&lines[..start], &lines[start..end])
    } else {
        (lines, &[])
    }
}

/// Own a slice of borrowed lines.
pub(super) fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}
