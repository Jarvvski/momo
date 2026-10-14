# 12: Verify the JSON output contract per verb

Type: task
Status: resolved
Severity: low
Area: cli/output
Verified: yes — `src/cli/output.rs` hand-rolls JSON (`enum Json`, `escape_into`, `render`); tests parse some shapes via `serde_json` but not every verb.

## Symptom

`docs/ci.md` documents a JSON contract per reporting verb, but the serializer is hand-written and only some shapes are round-tripped through a real JSON parser in tests. An escaping or structure regression in an untested verb could ship.

## Suggested direction

Cheapest safe fix: add a test that renders **every** `Report` implementation (`Status`, `Release`, `Tag`, `Notes`) to JSON and parses it back with `serde_json`, asserting the documented keys and shape from `docs/ci.md` — including a value with a quote, a backslash and a newline, to exercise `escape_into`.

Optional, larger, owner call: replace the hand-rolled serializer with `serde`/`serde_json` derive structs, since `serde_json` is already a dependency. Do **not** do that here without a ruling — it is a rewrite of the reporting layer.

## Related

- Group E.

## Resolution

Landed the round-trip tests (#65). The optional direction was then taken: the hand-rolled serializer was replaced with `serde`/`serde_json` derive structs (#80), with `Status`/`StatusPackage` keeping hand-written `Serialize` impls for their computed fields. The `docs/ci.md` contract is byte-for-byte unchanged, verified by the exact-bytes tests and a new control-character escaping case.
