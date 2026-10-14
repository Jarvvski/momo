# 42: No test covers multi-package JSON or `--output`

Type: bug
Status: resolved
Blocked by: 03
Severity: medium
Area: tests
Verified: no — reported by an agent; every `tests/ci_output.rs` case is single-package

## Symptom

`tests/ci_output.rs` only exercises single-package repos, which is why the
multi-package `--format json` / `--output` bug (ticket 03) is invisible to the
suite. The JSON shapes are a documented contract in `docs/ci.md`.

## Evidence

`tests/ci_output.rs` — every case builds a one-package repo. The per-package
`emit` loop lives at `src/cli/commands.rs:182-186`, `:222`, `:255`.

## Trigger

Any change to the emit loop ships without a failing test.

## Suggested direction

Add a two-package `--format json` case asserting the output parses as one JSON
document, and a two-package `--output <file>` case asserting both packages are
present.

## Open questions for triage

- What is the intended JSON shape for multiple packages? The test pins the
  decision, so it should be written after the shape is agreed (ticket 03).

## Triage

**Verdict.** Confirmed: every `tests/ci_output.rs` case builds a single package via
`project()`; the per-package `emit` calls are `src/cli/commands.rs:183` (release),
`:221` (candidate), `:255` (tag). The multi-package output bug (03) is therefore
invisible. **`ready-for-agent`, blocked by 03.**

**Corrections.**
- Be precise: multi-package `status` *is* covered (unit:
  `output.rs::status_reports_every_package`; integration:
  `monorepo.rs::status_reports_each_package_and_its_next_version`). The gap is
  multi-package **JSON** and **`--output`** for `release`/`candidate`/`tag`. Say
  that, not "no multi-package coverage".
- `status` is single-`emit` by construction (`commands.rs:150`), which is why it is
  unaffected; the test should target the verbs that emit per package.

**Scope.** Companion to **03**, not a duplicate: 03 is the fix, 42 the regression
test. Write it test-first inside 03's PR; the JSON shape must be settled there
first (`docs/ci.md` is a contract).

**Implementation sketch.**
- A mise-gated two-package fixture (reuse `monorepo.rs::monorepo`), then:
  - `momo --all release --format json` → assert stdout parses as exactly one JSON
    document listing both `cargo:a` and `cargo:b` (shape per 03).
  - `momo --all release --output out.json` → assert both packages present in the
    file.
- Use the ticket-41 `require_tool("mise")` helper.

## Resolution

Resolved with ticket 03; the regression tests landed in the same change set.

`tests/monorepo.rs` gained two mise-gated two-package cases over the shared
`monorepo()` fixture:

- `a_multi_package_release_json_is_one_document` — `--all release --format json`
  parses via `serde_json::from_str`, which rejects trailing content, so N
  concatenated documents fail; the single object's `packages` array names
  `cargo:a` and `cargo:b`, both at `0.1.0`.
- `a_multi_package_release_output_file_holds_every_package` — `--all release
  --format json --output <file>` leaves stdout empty and writes one document
  naming both packages.

`src/cli/output.rs` additionally pins the multi-package envelope shape for
`release` and `tag` at the unit level.
