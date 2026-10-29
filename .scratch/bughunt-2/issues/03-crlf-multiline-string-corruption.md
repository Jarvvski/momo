# 03: Writing a CRLF manifest corrupts multi-line strings

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest/toml (shared by Cargo and Python)
Verified: yes — reproduced with `toml_edit` 0.25 in a scratch crate

## Symptom

Rewriting the version scalar of a CRLF `Cargo.toml` or `pyproject.toml` corrupts
every multi-line string in the file: each line inside the string gains a second
carriage return (`\r\r\n`), so the string's contents change on a write that
should only touch the version.

## Evidence

`src/adapters/manifest/toml.rs:77` does `rendered.replace('\n', "\r\n")`.
`toml_edit` renders the document's structure with LF but preserves a multi-line
string's raw `\r\n` verbatim, so the blanket replace doubles the `\r` inside the
string. Reproduced in a scratch crate against `toml_edit` 0.25:

```
in : [package]\r\nname = "acme"\r\nversion = "0.1.0"\r\ndescription = """\r\nline one\r\nline two\r\n"""\r\n
out: …description = """\r\r\nline one\r\r\nline two\r\r\n"""\r\n
```

`src/adapters/manifest/cargo.rs:551`, `src/adapters/manifest/python/mod.rs:91`
and `:224` all route through this helper. The existing CRLF tests
(`cargo.rs:903`, `python/mod.rs:719`, `roundtrip.rs:130`) have no multi-line
string, so none catch it.

## Fix direction

Convert only bare LF, not the LF of an existing CRLF. Normalising first is
enough: `rendered.replace("\r\n", "\n").replace('\n', "\r\n")` is the identity on
`\r\n` sequences while still promoting a bare `\n` to `\r\n`. Add a regression
test with a CRLF manifest containing a multi-line basic string and a multi-line
literal string, asserting the string bytes survive and only the version changes.
