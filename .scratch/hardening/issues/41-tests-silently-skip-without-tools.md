# 41: jj and mise integration tests silently pass when the tool is missing

Type: bug
Status: ready-for-agent
Severity: medium
Area: tests
Verified: no — reported by an agent; the skip sites are as cited

## Symptom

Several integration tests early-return with an "skipping" message when `jj` or
`mise` is not on `PATH`. A `return` is indistinguishable from success, so the
strongest regression coverage — the jj tag anchor, monorepo tagging, install
wrappers — reports green while testing nothing on any machine or CI image lacking
the tool.

## Evidence

`tests/release.rs:231`: `if !have("jj") { eprintln!("skipping…"); return; }`
`tests/monorepo.rs:101`: `fn require(program) -> bool` with the same early return.

`mise.toml` pins `jj = "0.46.0"`, so CI does run them — but nothing enforces that
the tests *ran* rather than skipped.

## Trigger

A contributor (or a future CI image) without `jj`/`mise`: `cargo test` is green and
the tag-anchor regression is unverified.

## Suggested direction

Gate on an environment flag (e.g. `MOMO_REQUIRE_TOOLS=1`, set in CI) that turns a
missing tool into a hard failure, keeping the skip only for local runs. Or assert
that at least one tool-dependent test executed.

## Open questions for triage

- Env flag, or make the tools a hard test requirement and document it? The
  env-flag route keeps local `cargo test` frictionless.

## Triage

**Verdict.** Confirmed. The skip sites early-return: `tests/release.rs:231`,
`tests/monorepo.rs:101` (`require`), and additionally `tests/ci_output.rs:272`. A
`return` is indistinguishable from a pass. CI (`.github/workflows/ci.yml`) runs
`mise run check` under `jdx/mise-action`, with `jj` pinned in `mise.toml`, so CI
has both tools **today** — the defect is that nothing *enforces* they ran, so a
future image or a contributor without the tool gets a false green.
**`ready-for-agent`.**

**Corrections.**
- Evidence is incomplete: add `tests/ci_output.rs:272` (jj skip). `mise` is
  supplied by `mise-action`, so "a CI image lacking the tool" is a regression risk,
  not a current gap — state it that way.
- `tests/monorepo.rs` skips on **`mise`** too (the graph tests), not only `jj`.

**Scope.** Keep separate from 42/43/44: this is harness integrity, not coverage or
brittleness. Can ship in the same "test hardening" PR (roll-up PR G) as separate
commits.

**Implementation sketch.**
- A shared helper (per test binary, or `tests/common/mod.rs`)
  `require_tool(name) -> bool`: `have(name)` → true; else if
  `std::env::var_os("MOMO_REQUIRE_TOOLS").is_some()` →
  `panic!("{name} is required by MOMO_REQUIRE_TOOLS but is not on PATH")`; else
  eprintln-skip + false.
- Set `MOMO_REQUIRE_TOOLS: "1"` on the `check` job in `ci.yml`.
- Replace the ad-hoc `have`/`require` sites in `release.rs`, `monorepo.rs`,
  `ci_output.rs`.
- Add one test asserting the flag turns a missing tool into a failure.
