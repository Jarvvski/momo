# 03: Multi-package `--format json` and `--output` are broken

Type: bug
Status: ready-for-human
Severity: high
Area: cli/output
Verified: yes — read at the cited lines

## Symptom

`release`, `candidate` and `tag` emit their report once **per package**. With
`--all` (or the default target for these verbs) and more than one package:

- `--format json` prints N complete JSON documents back to back — not one valid
  document, so any `jq`/CI consumer breaks;
- `--output <path>` calls `fs::write` per package, truncating each time, so only
  the **last** package's report survives. The others are silently lost.

## Evidence

`src/cli/commands.rs:182-186` (same shape at `:222` and `:255`):

```rust
for package in packages {
    let outcome = release::release(...)?;
    match outcome {
        Some(outcome) => { report_release(cli, &outcome, Verb::Release)?; ... }
```

`src/cli/output.rs:31-40`:

```rust
pub fn emit(cli: &Cli, report: &impl Report) -> Result<()> {
    let text = render(cli, report);
    match &cli.output {
        Some(path) => fs::write(path, text).with_context(...),   // truncates
        None => { print!("{text}"); Ok(()) }
    }
}
```

`status` is unaffected because it builds one aggregated `Status` and calls `emit`
once (`src/cli/commands.rs:134-150`) — which is why no test catches this: every
`tests/ci_output.rs` case is single-package.

## Trigger

A two-package monorepo, both with unreleased changes:
`momo release --all --format json | jq .` fails to parse;
`momo release --all --output out.json` leaves only the second package's report.

## Suggested direction

Aggregate the per-package outcomes into a single report (a JSON array for the
multi-package shape) and call `emit` exactly once, as `status` does.

## Open questions for triage

- Does the aggregated JSON need a schema change, or is an array of the existing
  per-package objects acceptable? (Back-compat matters — this shape is a
  documented contract in `docs/ci.md`.)

## Triage

**Verdict:** ready-for-human — the bug is real and verified, but the fix forces a
JSON-shape decision on a documented contract (`docs/ci.md`); that is a product
call, not an unattended one.

**Corrections:** Accurate, and `tag` is affected too — the per-Package
`output::emit` sits in the loop at `src/cli/commands.rs:255`, not only in
`release`/`candidate` (`:183`, `:221`). The `--output` truncation and the
N-documents JSON share one root cause (emit once per Package), so they cannot be
fixed independently.

**Scope:** One ticket; do not split. The `--output` fix depends on the aggregate
content, which is the undecided shape. Ties to 42 (no multi-package JSON or
`--output` test exists).

**Human decision required:** pick the aggregate shape, then update `docs/ci.md`:
- (a) always wrap — `release`/`candidate` return
  `{ "releases": [ …per-package objects… ] }`, `tag` returns `{ "tags": [ … ] }`;
  a breaking change for single-package consumers.
- (b) single-package stays a bare object, multi-package returns an array —
  inconsistent typing; avoid.
- (c) a `status`-style envelope with a `packages` array — consistent with
  `status`, still breaking for single-package.
- Recommendation: (c) for cross-verb consistency with `status`, or (a) if a
  minimal conceptual change is preferred. Either is a breaking change to the
  documented shape; make it deliberately.

**Implementation sketch (after the shape is chosen):** in
`src/cli/commands.rs`, collect the per-Package outcomes into one aggregate
`Report` and call `output::emit` exactly once (mirroring `status`,
`:134-150`); add the aggregate type in `src/cli/output.rs`. `--output` then
writes once by construction. Cover it with the multi-package test from 42.
