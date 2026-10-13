# 44: Tests pin exact output whitespace and clap internals

Type: bug
Status: resolved
Severity: low
Area: tests
Verified: no — reported by an agent; the assertions are as cited

## Symptom

Several tests assert on exact human-readable strings, so a cosmetic change fails
tests unrelated to behaviour, and one depends on clap's help rendering.

## Evidence

- `tests/recording.rs:121`: `assert_eq!(stdout.lines().last(), Some("    → next: 0.1.0"))`
- `tests/monorepo.rs:125`: `assert!(stdout.contains("  cargo:a"))`
- `tests/cli.rs:70-74`: `assert!(stdout.contains(&format!("[alias: {alias}]")))`

Also `tests/release.rs:49-51` compares a `now_utc()` date computed in the test
against the binary's own `now_utc()` stamp (`src/cli/commands.rs:310-312`), which
flakes if the run straddles midnight UTC.

## Suggested direction

Assert on parsed/normalised fields (or the JSON shape) rather than exact column
counts; for the date, assert the shape and allow `today()`/`today()-1`, or inject
the date.

## Open questions for triage

- Some exact-output assertions are valuable (they pin the human contract). The
  question is which — the indentation of `→ next:` is arguably part of the UX.

## Triage

**Verdict.** Confirmed verbatim: `tests/recording.rs:121`,
`tests/monorepo.rs:125`, `tests/cli.rs:70-74`, and the `now_utc()`-vs-binary flake
at `tests/release.rs:49-51` (the test's `today()` against
`src/cli/commands.rs:310-312`). **`ready-for-agent`.**

**Corrections.** Split the class:
- *Deliberate* contract assertions — the `→ next:` indentation and the
  `[alias: …]` help text — pin the human UX and are mostly worth keeping. The
  `[alias: …]` one is coupled to clap's rendering and should be flagged as such,
  not "fixed".
- *Incidental* ones — the midnight-UTC date compare — are a genuine flake and
  should change.

**Scope.** Keep separate. Its date fix overlaps **45**; land them together (45
hoists `today()`). Not a duplicate.

**Implementation sketch.**
- Date (`release.rs`): assert the heading matches `## [0.1.0] - <date>` where
  `<date>` is today-or-yesterday UTC (or parse the heading and allow ±1 day),
  instead of recomputing `now_utc()` and comparing exactly.
- Keep the UX assertions; where the command can emit JSON, prefer asserting on the
  parsed shape over the human column counts.
- `cli.rs` alias assertion: keep as a clap smoke check, but optionally assert the
  alias tokens without the `[alias: …]` wrapper to reduce coupling.

## Resolution

Fixed the genuine flake: `tests/release.rs` no longer recomputes `now_utc()` and
compares exactly. It reads the `## [0.1.0] - <date>` heading and asserts the date
is today or yesterday in UTC (`common::today_or_yesterday`), so a run straddling
midnight cannot fail. The deliberate contract assertions — the `→ next:`
indentation in `tests/recording.rs` and the clap-rendered `[alias: …]` in
`tests/cli.rs` — are kept and now commented as intentional UX/clap contracts
rather than "fixed".
