# 43: Most `MOMO_*` environment keys have no test

Type: bug
Status: resolved
Severity: medium
Area: tests
Verified: no — reported by an agent; the `env_remove` lists are as cited

## Symptom

`from_env` parses eight variables (`src/domain/config.rs:294-322`). Tests only
set `MOMO_RECORDING`, `MOMO_TAG_PREFIX` and `MOMO_REMOTE`. The other five —
`MOMO_CHANGELOG`, `MOMO_CHANGES_DIR`, `MOMO_TAG_KIND`, `MOMO_TASK_ENV`,
`MOMO_RC_FORMAT` — appear only inside `env_remove([…])` lists in every integration
test. So the whole env layer for those keys is unverified, and two of them
(`tag_kind`, `task_env`) are dead config anyway (tickets 21, 22).

## Evidence

`src/domain/config.rs:531`, `:539`, `:561` set three keys; the rest are never set.

## Trigger

`MOMO_RC_FORMAT` or `MOMO_CHANGELOG` could be broken entirely and the suite would
pass.

## Suggested direction

A table-driven test that sets each `MOMO_*` var and asserts it beats the file and
the default, plus one end-to-end `MOMO_RC_FORMAT` candidate run.

## Open questions for triage

- Do this before or after the dead-config decision (tickets 21, 22)? Testing a key
  that is about to be removed is wasted work.

## Triage

**Verdict.** Confirmed: `from_env` (`src/domain/config.rs:294-322`) parses eight
keys; integration tests set only `MOMO_RECORDING` (`cli.rs:240`),
`MOMO_TAG_PREFIX` (`cli.rs:148`) and `MOMO_REMOTE` (`release.rs:389`), and the unit
tests add nothing beyond those three. Untested: `MOMO_CHANGELOG`,
`MOMO_CHANGES_DIR`, `MOMO_TAG_KIND`, `MOMO_TASK_ENV`, `MOMO_RC_FORMAT`.
**`ready-for-agent`**, scoped.

**Corrections.**
- `MOMO_TAG_KIND` and `MOMO_TASK_ENV` are inert today (tickets 21/22): a test can
  only prove the env→config plumbing, not behaviour. Testing them before 21/22
  decide "implement vs remove" is likely wasted work.
- `MOMO_RC_FORMAT` *does* drive behaviour (`cli/release.rs:163,257`), so it
  deserves a true end-to-end run.

**Scope.** Keep separate; partial dependency on **21/22** (batch 4). Test the live
keys now; leave the two inert keys to 21/22.

**Implementation sketch.**
- `tests/env.rs` (or extend `tests/cli.rs`): a table of
  `(var, value, observable)`:
  - `MOMO_CHANGELOG=HISTORY.md` → `add` writes `HISTORY.md`, not `CHANGELOG.md`.
  - `MOMO_CHANGES_DIR=.changes` + `recording=fragments` → `add` writes under
    `.changes/`.
  - `MOMO_RC_FORMAT=-rc{n}` → `candidate` prints `0.1.0-rc1` (end-to-end).
  - Each also with the file set to a different value, to prove env > file > default.
- Defer `MOMO_TAG_KIND`/`MOMO_TASK_ENV` to 21/22; if kept, an `init --dry-run`
  echo is the only honest assertion.

## Resolution

Added `tests/env.rs`: one table per live key (`MOMO_CHANGELOG`,
`MOMO_CHANGES_DIR`, `MOMO_RC_FORMAT`) driving the real binary through default →
file → env and proving `env > file > default`. `MOMO_RC_FORMAT` has a true
end-to-end `candidate --version-only` case. `MOMO_TAG_KIND` and `MOMO_TASK_ENV`
are deliberately not covered — they are inert until tickets 21/22 decide
implement-or-remove.
