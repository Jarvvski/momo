# 10: Property-test the changelog preserve model

Type: task
Status: ready-for-agent
Severity: medium
Area: domain/changelog
Verified: yes — `src/domain/changelog.rs` is the largest module (1511 lines); ADR 0003 makes preservation the contract; the hardening wave found a cluster of changelog-loss bugs.

## Symptom

The preserve model ("everything else passes through byte-for-byte") is covered by example-based tests only. The invariant that matters — a release never loses or corrupts foreign content — is exactly the kind a generator finds counterexamples for.

## Suggested direction

Add `proptest` as a dev-dependency and a `tests/changelog_properties.rs` integration test. Keep `src/domain/changelog.rs` itself untouched so ticket 15's split is not pre-empted. Invariants to encode:

- `Changelog::parse` → `render` → `parse` is stable (the second render equals the first).
- Releasing never drops a line momo does not own — generate documents with random foreign content (`*` bullets, HTML comments, `### Changed` sections, CRLF, user-authored footer links) and assert every foreign line survives.
- `record` then `unreleased` round-trips the recorded text.

Generate arbitrary (including malformed) documents and assert `parse` either succeeds or fails with a `ChangelogError` that names a line — never panics.

## Related

- 15, Group C.
