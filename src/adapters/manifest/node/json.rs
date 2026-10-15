//! Locating and splicing a `package.json` version by byte span.
//!
//! momo rewrites one string literal — the top-level `"version"` value — so it
//! needs the byte span of that value and a splice, not a parse tree. Nothing is
//! re-rendered, so indentation, key order, the newline style and whether the
//! file ends in a newline survive byte-for-byte. A `serde_json` round-trip would
//! lose all of that, and a concrete syntax tree would cost a dependency.
//!
//! The scanner below is a small recursive-descent validator that also records
//! the span. It fails loudly on malformed input — a `JSONC` comment, a trailing
//! comma, a truncated document — so a `package.json` momo cannot read is
//! reported rather than silently half-edited.

use std::ops::Range;

/// Why a `package.json` could not be scanned.
#[derive(Debug, thiserror::Error)]
#[error("invalid JSON at byte {offset}: {message}")]
pub(crate) struct JsonError {
    /// The byte offset the scanner stopped at.
    offset: usize,
    /// What the scanner expected there.
    message: &'static str,
}

/// The byte span of the top-level `"version"` string value in `text`, quotes
/// included.
///
/// `None` when the top-level object has no `version` key, or its value is not a
/// string. A duplicate key follows `JSON.parse`: the last one wins, so
/// `{"version": 1, "version": "1.0.0"}` yields the string's span and
/// `{"version": "1.0.0", "version": 1}` yields `None`.
///
/// [`JsonError`] when `text` is not a single, well-formed JSON object.
pub(crate) fn version_span(text: &str) -> Result<Option<Range<usize>>, JsonError> {
    let mut scanner = Scanner::new(text);
    scanner.skip_ws();
    let span = scanner.object()?;
    scanner.skip_ws();
    if !scanner.at_end() {
        return Err(scanner.error("unexpected trailing content"));
    }
    Ok(span)
}

/// One declaration in a dependency table: the dependency's name, the byte span
/// of its string value (quotes included), and the value's text.
#[derive(Debug)]
pub(crate) struct Dependency {
    /// The dependency's name, as the table key spells it.
    pub(crate) name: String,
    /// The byte span of the value literal, quotes included.
    pub(crate) value: Range<usize>,
    /// The value's text, without its quotes.
    pub(crate) requirement: String,
}

/// Every dependency declaration in `text`'s `tables`, in document order.
///
/// Only the named top-level tables are read, and only their string values: a
/// nested object or a non-string value is not a requirement momo can read. The
/// whole document is validated, so a malformed `package.json` is reported
/// rather than half-read.
pub(crate) fn dependency_entries(
    text: &str,
    tables: &[&str],
) -> Result<Vec<Dependency>, JsonError> {
    let mut scanner = Scanner::new(text);
    scanner.skip_ws();
    scanner.expect(b'{', "expected a top-level object")?;
    scanner.skip_ws();
    let mut entries = Vec::new();
    if scanner.peek() == Some(b'}') {
        scanner.bump();
        return Ok(entries);
    }
    loop {
        scanner.skip_ws();
        let key = scanner.string()?;
        scanner.skip_ws();
        scanner.expect(b':', "expected `:`")?;
        scanner.skip_ws();
        if tables.contains(&inner(text, &key)) {
            scanner.dependency_object(&mut entries)?;
        } else {
            scanner.skip_value()?;
        }
        scanner.skip_ws();
        match scanner.peek() {
            Some(b',') => scanner.bump(),
            Some(b'}') => {
                scanner.bump();
                break;
            }
            _ => return Err(scanner.error("expected `,` or `}`")),
        }
    }
    scanner.skip_ws();
    if !scanner.at_end() {
        return Err(scanner.error("unexpected trailing content"));
    }
    Ok(entries)
}

/// `text` with the text between each span's quotes replaced by its string.
///
/// A span covers a whole string literal, quotes included, so only the bytes
/// between them change and the quotes survive. Spans are applied from the last
/// to the first, so an earlier span's offsets stay valid as a later one changes
/// length; they must be disjoint, which a top-level `version` value and the
/// dependency values always are.
pub(crate) fn splice(text: &str, edits: &mut [(Range<usize>, String)]) -> String {
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut updated = text.to_owned();
    for (span, value) in edits.iter() {
        updated.replace_range(span.start + 1..span.end - 1, value);
    }
    updated
}

/// The text between a string literal's quotes.
pub(crate) fn inner<'a>(text: &'a str, span: &Range<usize>) -> &'a str {
    &text[span.start + 1..span.end - 1]
}

/// A cursor over the raw bytes of a JSON document.
struct Scanner<'a> {
    /// The document being scanned.
    text: &'a str,
    /// The cursor's byte offset into `text`.
    pos: usize,
}

impl<'a> Scanner<'a> {
    /// A scanner at the start of `text`.
    fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    /// The byte at the cursor, or `None` at the end.
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    /// Advance the cursor by one byte.
    fn bump(&mut self) {
        self.pos += 1;
    }

    /// Whether the cursor is past the last byte.
    fn at_end(&self) -> bool {
        self.pos >= self.text.len()
    }

    /// Skip JSON whitespace.
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.bump();
        }
    }

    /// Consume `byte`, or fail with `expected`.
    fn expect(&mut self, byte: u8, expected: &'static str) -> Result<(), JsonError> {
        if self.peek() == Some(byte) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(expected))
        }
    }

    /// A [`JsonError`] at the cursor.
    fn error(&self, message: &'static str) -> JsonError {
        JsonError {
            offset: self.pos,
            message,
        }
    }

    /// Validate the object at the cursor, returning the span of its top-level
    /// `"version"` string value when it has one.
    ///
    /// Only the top level is inspected: a `"version"` nested inside another
    /// object or array is skipped by [`Scanner::skip_value`] and never recorded.
    fn object(&mut self) -> Result<Option<Range<usize>>, JsonError> {
        self.expect(b'{', "expected a top-level object")?;
        let mut version = None;
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(None);
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':', "expected `:`")?;
            self.skip_ws();
            if inner(self.text, &key) == "version" {
                if self.peek() == Some(b'"') {
                    version = Some(self.string()?);
                } else {
                    // A non-string `version` is not one momo owns, and it
                    // shadows any earlier duplicate, as `JSON.parse` does.
                    version = None;
                    self.skip_value()?;
                }
            } else {
                self.skip_value()?;
            }
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.bump(),
                Some(b'}') => {
                    self.bump();
                    return Ok(version);
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    /// Validate and consume one JSON value.
    fn skip_value(&mut self) -> Result<(), JsonError> {
        match self.peek() {
            Some(b'{') => self.skip_object(),
            Some(b'[') => self.skip_array(),
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal("true"),
            Some(b'f') => self.literal("false"),
            Some(b'n') => self.literal("null"),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }

    /// Validate and consume an object, recording no version span.
    fn skip_object(&mut self) -> Result<(), JsonError> {
        self.expect(b'{', "expected `{`")?;
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(());
        }
        loop {
            self.skip_ws();
            self.string()?;
            self.skip_ws();
            self.expect(b':', "expected `:`")?;
            self.skip_ws();
            self.skip_value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.bump(),
                Some(b'}') => {
                    self.bump();
                    return Ok(());
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    /// Validate and consume an object of dependency declarations, recording
    /// the span of each string value. A non-string value is a declaration momo
    /// cannot read, so it is validated and skipped.
    fn dependency_object(&mut self, entries: &mut Vec<Dependency>) -> Result<(), JsonError> {
        self.expect(b'{', "expected `{`")?;
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':', "expected `:`")?;
            self.skip_ws();
            let name = inner(self.text, &key).to_owned();
            if self.peek() == Some(b'"') {
                let value = self.string()?;
                let requirement = inner(self.text, &value).to_owned();
                entries.push(Dependency {
                    name,
                    value,
                    requirement,
                });
            } else {
                self.skip_value()?;
            }
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.bump(),
                Some(b'}') => {
                    self.bump();
                    return Ok(());
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    /// Validate and consume an array.
    fn skip_array(&mut self) -> Result<(), JsonError> {
        self.expect(b'[', "expected `[`")?;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(());
        }
        loop {
            self.skip_ws();
            self.skip_value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.bump(),
                Some(b']') => {
                    self.bump();
                    return Ok(());
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }

    /// Validate and consume a string, returning the span of the literal, quotes
    /// included.
    fn string(&mut self) -> Result<Range<usize>, JsonError> {
        let start = self.pos;
        self.expect(b'"', "expected a string")?;
        loop {
            match self.peek() {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.bump();
                    return Ok(start..self.pos);
                }
                Some(b'\\') => {
                    self.bump();
                    if self.peek().is_none() {
                        return Err(self.error("unterminated escape"));
                    }
                    self.bump();
                }
                Some(_) => self.bump(),
            }
        }
    }

    /// Consume the exact word `word`.
    fn literal(&mut self, word: &str) -> Result<(), JsonError> {
        let end = self.pos + word.len();
        if self.text.get(self.pos..end) == Some(word) {
            self.pos = end;
            Ok(())
        } else {
            Err(self.error("expected a literal"))
        }
    }

    /// Consume a number.
    ///
    /// Validation is loose — the locator only needs to know where a value ends,
    /// and the structural checks around it catch a number that runs into the
    /// next token.
    fn number(&mut self) -> Result<(), JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some(b'.') {
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump();
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        if self.pos == start {
            return Err(self.error("expected a number"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inner text of a located span, without its quotes.
    fn value<'a>(text: &'a str, span: &Range<usize>) -> &'a str {
        inner(text, span)
    }

    /// `text` with its top-level `version` set to `version`, or `None` when
    /// there is nothing to write.
    fn set_version(text: &str, version: &str) -> Result<Option<String>, JsonError> {
        let Some(span) = version_span(text)? else {
            return Ok(None);
        };
        if inner(text, &span) == version {
            return Ok(None);
        }
        Ok(Some(splice(text, &mut [(span, version.to_owned())])))
    }

    #[test]
    fn a_top_level_version_is_located_with_its_quotes() {
        let text = "{\"name\": \"ui\", \"version\": \"1.2.3\"}";
        let span = version_span(text).expect("valid JSON").expect("a version");

        assert_eq!(value(text, &span), "1.2.3");
        assert_eq!(&text[span], "\"1.2.3\"");
    }

    #[test]
    fn a_version_after_a_nested_object_is_found() {
        let text = "{\"scripts\": {\"version\": \"0.0.0\"}, \"version\": \"1.2.3\"}";
        let span = version_span(text).expect("valid JSON").expect("a version");

        assert_eq!(value(text, &span), "1.2.3");
    }

    #[test]
    fn a_nested_version_alone_is_not_the_top_level_one() {
        let text = "{\"dependencies\": {\"version\": \"1.2.3\"}}";
        assert_eq!(version_span(text).expect("valid JSON"), None);
    }

    #[test]
    fn a_version_inside_an_array_is_not_the_top_level_one() {
        let text = "{\"keywords\": [{\"version\": \"1.2.3\"}]}";
        assert_eq!(version_span(text).expect("valid JSON"), None);
    }

    #[test]
    fn a_missing_version_is_none() {
        assert_eq!(
            version_span("{\"name\": \"ui\"}").expect("valid JSON"),
            None
        );
    }

    #[test]
    fn an_empty_object_is_none() {
        assert_eq!(version_span("{}").expect("valid JSON"), None);
    }

    #[test]
    fn a_non_string_version_is_not_a_version_momo_owns() {
        assert_eq!(version_span("{\"version\": 3}").expect("valid JSON"), None);
        assert_eq!(
            version_span("{\"version\": null}").expect("valid JSON"),
            None
        );
        assert_eq!(
            version_span("{\"version\": {\"major\": 1}}").expect("valid JSON"),
            None
        );
    }

    #[test]
    fn a_duplicate_key_takes_the_last_one() {
        let text = "{\"version\": \"1.0.0\", \"version\": \"2.0.0\"}";
        let span = version_span(text).expect("valid JSON").expect("a version");
        assert_eq!(value(text, &span), "2.0.0");
    }

    #[test]
    fn a_duplicate_non_string_version_shadows_an_earlier_string() {
        let text = "{\"version\": \"1.0.0\", \"version\": 2}";
        assert_eq!(version_span(text).expect("valid JSON"), None);
    }

    #[test]
    fn a_duplicate_string_version_shadows_an_earlier_non_string() {
        let text = "{\"version\": 2, \"version\": \"2.0.0\"}";
        let span = version_span(text).expect("valid JSON").expect("a version");
        assert_eq!(value(text, &span), "2.0.0");
    }

    #[test]
    fn surrounding_whitespace_and_a_trailing_newline_are_tolerated() {
        let text = "\n\t{\n  \"version\" : \"1.2.3\"\n}\n";
        let span = version_span(text).expect("valid JSON").expect("a version");
        assert_eq!(value(text, &span), "1.2.3");
    }

    #[test]
    fn a_trailing_second_value_is_rejected() {
        let err = version_span("{\"version\": \"1.2.3\"} {}")
            .expect_err("two top-level values are not one document");
        assert!(err.to_string().contains("trailing content"), "{err}");
    }

    #[test]
    fn an_array_at_the_top_level_is_rejected() {
        let err = version_span("[\"version\"]").expect_err("the top level must be an object");
        assert!(err.to_string().contains("top-level object"), "{err}");
    }

    #[test]
    fn a_trailing_comma_is_rejected() {
        assert!(version_span("{\"version\": \"1.2.3\",}").is_err());
    }

    #[test]
    fn a_missing_colon_is_rejected() {
        assert!(version_span("{\"version\" \"1.2.3\"}").is_err());
    }

    #[test]
    fn a_truncated_document_is_rejected() {
        assert!(version_span("{\"version\": \"1.2.").is_err());
    }

    #[test]
    fn a_jsonc_comment_is_rejected() {
        // A comment is not JSON: the scanner fails loudly rather than skipping
        // it and editing a document it does not understand.
        assert!(version_span("{\n  // the version\n  \"version\": \"1.2.3\"\n}").is_err());
    }

    #[test]
    fn an_escaped_quote_inside_a_value_does_not_end_the_string() {
        let text = "{\"description\": \"a \\\"quoted\\\" word\", \"version\": \"1.2.3\"}";
        let span = version_span(text).expect("valid JSON").expect("a version");
        assert_eq!(value(text, &span), "1.2.3");
    }

    #[test]
    fn a_new_version_is_spliced_into_the_existing_span() {
        let text = "{\n  \"name\": \"ui\",\n  \"version\": \"1.2.3\"\n}\n";
        let updated = set_version(text, "2.0.0")
            .expect("valid JSON")
            .expect("a write");
        assert_eq!(
            updated,
            "{\n  \"name\": \"ui\",\n  \"version\": \"2.0.0\"\n}\n"
        );
    }

    #[test]
    fn an_already_current_version_is_not_rewritten() {
        let text = "{\"version\": \"1.2.3\"}";
        assert_eq!(set_version(text, "1.2.3").expect("valid JSON"), None);
    }

    #[test]
    fn a_document_without_a_version_writes_nothing() {
        assert_eq!(
            set_version("{\"name\": \"ui\"}", "1.2.3").expect("valid JSON"),
            None
        );
    }

    #[test]
    fn dependency_entries_read_every_dependency_table() {
        let text = "{\n  \"name\": \"ui\",\n  \"dependencies\": {\"a\": \"^1.0.0\"},\n  \
                    \"devDependencies\": {\"b\": \"~2.0.0\"},\n  \
                    \"peerDependencies\": {\"c\": \"3.0.0\"},\n  \
                    \"optionalDependencies\": {\"d\": \"*\"},\n  \
                    \"scripts\": {\"e\": \"^9.0.0\"}\n}\n";
        let entries = dependency_entries(
            text,
            &[
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ],
        )
        .expect("valid JSON");

        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
        assert_eq!(entries[0].requirement, "^1.0.0");
        assert_eq!(entries[3].requirement, "*");
    }

    #[test]
    fn a_non_string_dependency_is_skipped() {
        let text = "{\"dependencies\": {\"a\": {\"version\": \"^1.0.0\"}, \"b\": \"^2.0.0\"}}";
        let entries = dependency_entries(text, &["dependencies"]).expect("valid JSON");
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].name, "b");
    }

    #[test]
    fn a_splice_replaces_only_the_named_spans() {
        let text = "{\n  \"version\": \"1.0.0\",\n  \"dependencies\": {\"a\": \"^1.0.0\"}\n}\n";
        let mut edits = Vec::new();
        let version = version_span(text).expect("valid JSON").expect("a version");
        edits.push((version, "2.0.0".to_owned()));
        let entries = dependency_entries(text, &["dependencies"]).expect("valid JSON");
        edits.push((entries[0].value.clone(), "^2.0.0".to_owned()));

        assert_eq!(
            splice(text, &mut edits),
            "{\n  \"version\": \"2.0.0\",\n  \"dependencies\": {\"a\": \"^2.0.0\"}\n}\n"
        );
    }

    #[test]
    fn a_malformed_dependency_object_is_rejected() {
        let text = "{\"dependencies\": {\"a\": \"^1.0.0\",}}";
        assert!(dependency_entries(text, &["dependencies"]).is_err());
    }

    use proptest::prelude::*;

    /// A leaf JSON value, so a generated document has no nested `version` key
    /// for the locator to confuse with the top-level one.
    fn leaf() -> impl Strategy<Value = serde_json::Value> {
        prop_oneof![
            any::<i64>().prop_map(|number| serde_json::json!(number)),
            "[a-z0-9 ._-]{0,12}".prop_map(|text| serde_json::json!(text)),
            any::<bool>().prop_map(|flag| serde_json::json!(flag)),
            Just(serde_json::Value::Null),
        ]
    }

    /// A `package.json`-shaped document with a string `version` and arbitrary
    /// siblings, across both line endings and with or without a trailing
    /// newline.
    fn document() -> impl Strategy<Value = String> {
        (
            prop::collection::vec(("[a-z_]{1,8}", leaf()), 0..5),
            "[0-9]+\\.[0-9]+\\.[0-9]+",
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(siblings, version, crlf, trailing)| {
                let mut object = serde_json::Map::new();
                for (key, value) in siblings {
                    if key != "version" {
                        object.insert(key, value);
                    }
                }
                object.insert("version".to_owned(), serde_json::json!(version));
                let mut text = serde_json::to_string_pretty(&serde_json::Value::Object(object))
                    .expect("a serialisable document");
                if crlf {
                    text = text.replace('\n', "\r\n");
                }
                if trailing {
                    text.push_str(if crlf { "\r\n" } else { "\n" });
                }
                text
            })
    }

    proptest! {
        /// The splice changes only the located version span: every byte outside
        /// it survives, and the document still parses to the same value with
        /// only `version` updated.
        #[test]
        fn only_the_version_span_changes(text in document()) {
            let span = version_span(&text)
                .expect("generated JSON is valid")
                .expect("a generated document has a version");
            let target = "9.9.9";
            let Some(updated) = set_version(&text, target).expect("valid JSON") else {
                return Ok(());
            };

            // The bytes before the opening quote and from the closing quote on
            // are untouched; only the value between them changed.
            prop_assert_eq!(&updated[..span.start + 1], &text[..span.start + 1]);
            prop_assert_eq!(
                &updated[span.start + 1 + target.len()..],
                &text[span.end - 1..]
            );

            let before: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
            let after: serde_json::Value = serde_json::from_str(&updated).expect("valid JSON");
            let mut expected = before.clone();
            expected["version"] = serde_json::json!(target);
            prop_assert_eq!(after, expected);
        }
    }
}
