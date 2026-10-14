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

The changelog module was split in #76 into `parse.rs`, `render.rs`, `footer.rs`, `tests.rs` and the existing `property_tests.rs`, with the root down from 1515 to 387 lines — a pure move, tests preserved.

**Remaining scope (follow-ups, one PR each as this ticket requires):** `cli/release.rs` (1415), `cli/output.rs` (1118) and `domain/config.rs` (1021) are still single files.

## Related

- 10, 12.
