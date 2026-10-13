//! Deriving path- and ref-safe slugs from human text.

/// The longest slug momo writes, in characters.
const MAX_LEN: usize = 60;

/// The slug used when `text` yields nothing path-safe.
const FALLBACK: &str = "change";

/// Derive a path-safe slug from `text`.
///
/// Lowercases ASCII, keeps alphanumerics, collapses every other run to a single
/// `-`, trims the ends, and truncates to [`MAX_LEN`]. Text with nothing
/// alphanumeric — for example only punctuation — becomes [`FALLBACK`].
pub(crate) fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.push(ch.to_ascii_lowercase());
        } else {
            separator = true;
        }
    }
    if slug.is_empty() {
        return FALLBACK.to_owned();
    }
    slug.truncate(MAX_LEN);
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_alphanumerics_and_hyphenates_the_rest() {
        assert_eq!(slugify("Fix the bug!"), "fix-the-bug");
        assert_eq!(
            slugify("`momo status --format json`"),
            "momo-status-format-json"
        );
        assert_eq!(slugify("CamelCase HTTP 2"), "camelcase-http-2");
    }

    #[test]
    fn collapses_and_trims_separators() {
        assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
        assert_eq!(slugify("--leading and trailing--"), "leading-and-trailing");
    }

    #[test]
    fn falls_back_when_nothing_is_path_safe() {
        assert_eq!(slugify("!!!"), "change");
        assert_eq!(slugify(""), "change");
    }

    #[test]
    fn truncates_without_a_trailing_separator() {
        let long = "a".repeat(MAX_LEN + 10);
        assert_eq!(slugify(&long).chars().count(), MAX_LEN);

        let words = format!("{} more", "word ".repeat(MAX_LEN));
        let slug = slugify(&words);
        assert!(slug.chars().count() <= MAX_LEN);
        assert!(!slug.ends_with('-'));
    }
}
