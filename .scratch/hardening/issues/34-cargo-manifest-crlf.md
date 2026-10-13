# 34: `Cargo.toml` rewrites are not byte-preserving for CRLF or a missing final newline

Type: bug
Status: ready-for-agent
Severity: low
Area: adapters/manifest
Verified: no — reported by an agent, which compiled probes against the cached `toml_edit`

## Symptom

The module doc claims "Comments, key order, and whitespace survive byte-for-byte".
`toml_edit` normalises on render: a CRLF manifest is rewritten entirely in LF, and
a file without a trailing newline gains one. Bumping a version in such a file
produces a whole-file diff.

## Evidence

`src/adapters/manifest/cargo.rs:5-6` (the claim) and `:102` `Ok(Some(doc.to_string()))`.
The port doc makes the same promise (`src/domain/manifest.rs:46-47`).

## Trigger

A CRLF `Cargo.toml`, then `momo release` — every line ending changes.

## Suggested direction

Either document the normalisation, or preserve the source's newline style and
trailing byte (detect and reapply, or re-emit only the changed scalar's span).

## Open questions for triage

- Same question as ticket 13 (changelog CRLF). If CRLF is out of scope for a
  Linux/macOS-only tool, this is a docs fix rather than a code fix.

## Triage

**Verdict:** Confirmed — verified directly. Against the project's pinned
`toml_edit` 0.25.17, parsing a CRLF manifest and calling `to_string()` renders the
whole document in LF **even with no edit**, and a manifest without a trailing
newline gains one. So the "byte-for-byte" claim in `cargo.rs:5-6` and
`domain/manifest.rs:46-47` is false for CRLF and missing-trailing-newline files.

**Corrections:** The claim is real, not theoretical. (`Value::span()`, which would
allow true byte-preserving splicing, returns `None` for a `DocumentMut` in this
version, so reapply the source's line endings rather than splice.)

**Scope:** Same class as ticket 13 (changelog CRLF) — rule both the same way. In
scope: momo is Linux/macOS-only, but CRLF files do appear there (Windows
contributors, legacy checkouts), and a release tool that rewrites every line
ending of a file is a genuine, if low, defect against its own documented
contract. Prefer preserving; at minimum stop claiming byte-for-byte.

**Implementation sketch:** In `set_version` (`cargo.rs:72-103`), after
`doc.to_string()`: if the source `text` is consistently CRLF (contains `\r\n` and
no bare `\n`), convert the rendered `\n` back to `\r\n`; if `text` did not end in
`\n`, drop the rendered trailing newline. Tests: a CRLF manifest round-trips CRLF;
a no-trailing-newline manifest keeps none; the LF cases are unchanged.
