# 15: Split the >1000-line modules

Type: refactor
Status: resolved
Severity: low
Area: structure
Verified: yes — `src/domain/changelog.rs` 1511, `src/cli/release.rs` 1415, `src/cli/output.rs` 1118, `src/domain/config.rs` 1008 lines.

## Symptom

Four modules are past the point where a newcomer can hold them in view. They are cohesive today, so this is not a bug — it is a navigability cost that grows.

## Suggested direction

Split one at a time, each its own PR, starting with `domain/changelog.rs` (model / parse / render / footer submodules under `domain/changelog/`). Keep public paths stable with `pub use`.

**Do not start until tickets 10 and 12 have landed** — they touch `changelog.rs` and `output.rs` respectively, and splitting underneath them would conflict by construction.

## Resolution

All four modules were split, one PR each:

- `domain/changelog.rs` → parse/render/footer/tests (#76)
- `cli/release.rs` → prepare/manifest/tag/tests (#79)
- `domain/config.rs` → keys/env/paths/tests (#78)
- `cli/output.rs` → tests moved to `output/tests.rs` (#81), after the serde migration (#80) had already shrunk it

## Related

- 10, 12.
