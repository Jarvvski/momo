//! Parsing helpers for the preserved changelog model.

use semver::Version;

use crate::domain::change::{Change, Kind};

use super::{Ending, Section};

/// Whether `line` is a level-2 (`## `) heading.
pub(super) fn is_section(line: &str) -> bool {
    line.starts_with("## ")
}

/// Whether `line` heads the Unreleased section, in any accepted spelling.
pub(super) fn is_unreleased(line: &str) -> bool {
    line.strip_prefix("## ")
        .map(str::trim)
        .is_some_and(|heading| {
            heading.eq_ignore_ascii_case("[Unreleased]")
                || heading.eq_ignore_ascii_case("Unreleased")
        })
}

/// The line ending a document uses.
pub(super) fn detect_ending(text: &str) -> Ending {
    if text.contains("\r\n") {
        Ending::Crlf
    } else {
        Ending::Lf
    }
}

/// Drop trailing blank lines — the separator re-emitted on render.
pub(super) fn trim_trailing_blanks(lines: &mut Vec<String>) {
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
}

/// Drop blank lines from both ends of `lines`.
pub(super) fn trim_blank_edges(lines: &mut Vec<String>) {
    trim_trailing_blanks(lines);
    let leading = lines.iter().take_while(|line| line.is_empty()).count();
    lines.drain(..leading);
}

/// Whether `body` holds a non-blank line.
pub(super) fn has_content(body: &[String]) -> bool {
    body.iter().any(|line| !line.is_empty())
}

/// The Version a `## [x.y.z]` heading names, if any.
pub(super) fn heading_version(line: &str) -> Option<Version> {
    let rest = line.strip_prefix("## [")?;
    let (version, _) = rest.split_once(']')?;
    Version::parse(version).ok()
}

/// Parse the entries of an Unreleased body, grouped by their `###` heading.
pub(super) fn entries(body: &[String]) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut kind = None;
    for line in body {
        if let Some(heading) = line.strip_prefix("### ") {
            kind = Kind::from_heading(heading);
        } else if let Some(text) = line.strip_prefix("- ")
            && let Some(kind) = kind
        {
            changes.push(Change::new(kind, text.trim()));
        }
    }
    changes
}

/// Whether `line` is the `### Kind` heading for `kind`.
fn is_kind_heading(line: &str, kind: Kind) -> bool {
    line.strip_prefix("### ")
        .and_then(Kind::from_heading)
        .is_some_and(|heading| heading == kind)
}

/// The Kind's position in [`Kind::ALL`], highest severity first.
fn kind_rank(kind: Kind) -> usize {
    Kind::ALL
        .iter()
        .position(|&candidate| candidate == kind)
        .unwrap_or(Kind::ALL.len())
}

/// Where a new `### Kind` heading belongs among the body's recognised headings.
///
/// The insertion point is the first recognised heading below `kind` in severity,
/// so the three Kind headings keep [`Kind::ALL`]'s order. Foreign headings are
/// not anchors and are never reordered.
fn insertion_index(body: &[String], kind: Kind) -> usize {
    let wanted = kind_rank(kind);
    body.iter()
        .position(|line| {
            line.strip_prefix("### ")
                .and_then(Kind::from_heading)
                .is_some_and(|heading| kind_rank(heading) > wanted)
        })
        .unwrap_or(body.len())
}

/// Append `change`'s bullet to `section`, under its Kind heading.
pub(super) fn append_change(section: &mut Section, change: Change) {
    let bullet = format!("- {}", change.text);

    if let Some(at) = section
        .body
        .iter()
        .position(|line| is_kind_heading(line, change.kind))
    {
        let end = section.body[at + 1..]
            .iter()
            .position(|line| line.starts_with("### "))
            .map(|offset| at + 1 + offset)
            .unwrap_or(section.body.len());
        let insert_at = match section.body[at + 1..end]
            .iter()
            .rposition(|line| !line.is_empty())
        {
            Some(offset) => at + 1 + offset + 1,
            // A heading with no entries yet: leave a blank line beneath it.
            None => {
                section.body.insert(at + 1, String::new());
                at + 2
            }
        };
        section.body.insert(insert_at, bullet);
        return;
    }

    let at = insertion_index(&section.body, change.kind);
    let mut block = vec![
        format!("### {}", change.kind.heading()),
        String::new(),
        bullet,
    ];
    let need_prefix = if at == 0 {
        section.body.first().is_none_or(|line| !line.is_empty())
    } else {
        !section.body[at - 1].is_empty()
    };
    if need_prefix {
        block.insert(0, String::new());
    }
    if at < section.body.len() {
        block.push(String::new());
    }
    section.body.splice(at..at, block);
}
