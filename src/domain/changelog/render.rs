//! Rendering helpers for the preserved changelog model.

use semver::Version;
use time::Date;

use crate::domain::change::{Change, Kind};

use super::Section;

/// The `## [version] - date` section momo inserts, carrying `body`.
pub(super) fn release_section(version: &Version, date: Date, body: Vec<String>) -> Section {
    Section {
        heading: format!("## [{version}] - {date}"),
        body,
    }
}

/// The `### Kind` body for `changes`, highest Kind first, with a leading blank.
pub(super) fn body_from_changes(changes: &[Change]) -> Vec<String> {
    let mut body = Vec::new();
    for kind in Kind::ALL {
        let matching: Vec<&Change> = changes
            .iter()
            .filter(|change| change.kind == kind)
            .collect();
        if matching.is_empty() {
            continue;
        }
        blank(&mut body);
        body.push(format!("### {}", kind.heading()));
        blank(&mut body);
        body.extend(matching.iter().map(|change| format!("- {}", change.text)));
    }
    if !body.is_empty() {
        body.insert(0, String::new());
    }
    body
}

/// Push a blank line unless the buffer is empty or already ends with one.
pub(super) fn blank(lines: &mut Vec<String>) {
    if lines.last().is_some_and(|line| !line.is_empty()) {
        lines.push(String::new());
    }
}
