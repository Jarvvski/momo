//! The Kind and the Change: what a recorded unit of work is.

use std::fmt;
use std::str::FromStr;

use crate::domain::slug::slugify;

/// The classification of a Change — one of `break`, `add`, `fix`.
///
/// The variants are ordered from lowest to highest severity, so the highest
/// Kind in a Release is `kinds.into_iter().max()`: `break` > `add` > `fix`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A bug fix; bumps PATCH.
    Fix,
    /// An addition; bumps MINOR.
    Add,
    /// A breaking change; bumps MINOR at MAJOR 0 and MAJOR at MAJOR ≥ 1.
    Break,
}

impl Kind {
    /// Every Kind, highest severity first — the order of Changelog sections.
    pub const ALL: [Kind; 3] = [Kind::Break, Kind::Add, Kind::Fix];

    /// The Changelog section heading for this Kind.
    pub fn heading(self) -> &'static str {
        match self {
            Kind::Break => "Breaking",
            Kind::Add => "Added",
            Kind::Fix => "Fixed",
        }
    }

    /// The Kind a Changelog section heading names, if any.
    pub fn from_heading(heading: &str) -> Option<Self> {
        match heading.trim() {
            "Breaking" => Some(Kind::Break),
            "Added" => Some(Kind::Add),
            "Fixed" => Some(Kind::Fix),
            _ => None,
        }
    }
}

impl fmt::Display for Kind {
    /// The verb that names this Kind.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Break => "break",
            Kind::Add => "add",
            Kind::Fix => "fix",
        })
    }
}

/// The error produced when a Kind name is not `break`, `add`, or `fix`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown kind `{value}`; expected one of: break, add, fix")]
pub struct UnknownKind {
    /// The unrecognised name.
    pub value: String,
}

impl FromStr for Kind {
    type Err = UnknownKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "break" => Ok(Kind::Break),
            "add" => Ok(Kind::Add),
            "fix" => Ok(Kind::Fix),
            other => Err(UnknownKind {
                value: other.to_owned(),
            }),
        }
    }
}

/// One recorded unit of work, classified by exactly one Kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The Kind classifying this Change.
    pub kind: Kind,
    /// The Change's text, as recorded.
    pub text: String,
}

impl Change {
    /// Record a Change of `kind` with `text`.
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }

    /// The Fragment file name for this Change: `<kind>-<slug>.md`.
    pub fn fragment_file_name(&self) -> String {
        format!("{}-{}.md", self.kind, slugify(&self.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_ordered_by_severity() {
        assert!(Kind::Break > Kind::Add);
        assert!(Kind::Add > Kind::Fix);
        assert_eq!(
            [Kind::Fix, Kind::Break, Kind::Add].into_iter().max(),
            Some(Kind::Break)
        );
    }

    #[test]
    fn kind_names_round_trip() {
        for kind in Kind::ALL {
            assert_eq!(kind.to_string().parse::<Kind>(), Ok(kind));
        }
        assert_eq!(
            "nonsense".parse::<Kind>(),
            Err(UnknownKind {
                value: "nonsense".to_owned()
            })
        );
    }

    #[test]
    fn headings_map_to_their_kind() {
        assert_eq!(Kind::from_heading("Breaking"), Some(Kind::Break));
        assert_eq!(Kind::from_heading("Added"), Some(Kind::Add));
        assert_eq!(Kind::from_heading("Fixed"), Some(Kind::Fix));
        assert_eq!(Kind::from_heading("Changed"), None);
    }

    #[test]
    fn fragment_file_name_encodes_kind_and_slug() {
        let change = Change::new(Kind::Add, "`momo status --format json` for CI");
        assert_eq!(
            change.fragment_file_name(),
            "add-momo-status-format-json-for-ci.md"
        );
    }
}
