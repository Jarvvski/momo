# 09: Make `status` the blessed path; pin zero-config

Type: task
Status: resolved
Points: 13
Bucket: Opinionated posture
Severity: low
Area: cli/output + docs + tests

## Problem

momo has no single obvious entry verb. The intended path — inspect → derive →
generate → report — is expressed only as four commands and a `git commit` the user
assembles (`docs/ci.md:111-121`). hk sells "one set of steps"; Cargo gives
`build/test/run`; momo gives nine verbs and a README that opens with `momo init`
(which is *optional*). The bug is discoverability.

## Evidence

- `src/cli/commands.rs:202-204`: `release --dry-run` **errors** when clean, so
  `status` is the only always-valid entry verb.
- `src/cli/output.rs:61` (`StatusPackage.current` exists); `:153-177` (the human
  renderer omits it).
- `src/cli/config.rs:21-28` (`project_root` finds `.git`/`.jj`); config defaults
  apply; VCS detection reads tags; `Cargo.toml` is discovered — zero-config already
  works for the golden case.

## Proposal

**Pick `status` as the blessed verb — no new verb.** It is the only always-valid
entry verb and the same command the CI gate uses (`status --check`), so the human
path and the CI path converge. It needs **no new surface**, which respects the
freeze (ticket 01).

- **Reject `momo plan`** — a near-duplicate of `status` that fights the freeze.
- **Reject `momo ship`** — it would mutate, violating the report-only stance.
- **A blessed command must be read-only.** That resolves the tension with
  report-only: the "one command" is a report; the "one set of steps" is a
  documented sequence.

**Zero-config as a contract, not a claim.** The golden path is: single package, git
or jj, changelog mode, **zero hand-written config**. State the boundary as an
opinion: monorepo discovery (needs `mise.toml` + `monorepo_root`), fragments mode,
and a non-`CHANGELOG.md`/non-`v` convention all require config.

**"One set of steps" (hk register).** momo stays primitive; the *sequence* is the
opinion, expressed two ways: a numbered README path, and optionally a generated
composite `momo:release` mise task — the mise-native "one set of steps", where the
*task* commits and pushes and momo never does.

## Surface

- **No new verb.** The surface stays at nine.
- `status` human output gains a `current:` line (the one thing the human view omits
  but the JSON has).
- `README.md:44-68`: lead with a numbered path and a one-line zero-config promise.
- `docs/usage.md`: a "The path" section stating the sequence and the zero-config
  boundary.
- Optional: `momo install` writes a composite `momo:release` wrapper encoding the
  sequence.

## Implementation sketch

- `src/cli/output.rs` (`Status::human`, lines 153-177): print `    current: {v}`
  (`—` when `None`) before `    → next: {v}`; update snapshots.
- New `tests/golden_path.rs`: a bare `git init` repo with **no `momo.toml`**, run
  `momo add`/`status`/`release --dry-run`/`release`/`tag`; assert versions and that
  no hand-written config was needed. Makes zero-config a contract.
- `README.md` restructure; `docs/usage.md` new section.
- Optional composite task: `src/adapters/taskenv/mise.rs` `plan` gains a
  `momo:release` wrapper whose body is the documented sequence; bump the install
  test. Keep commit/push lines clearly editable.

## Scoring

Points **13** = Value 3×2 + Ease 5 + Novelty 2. Cheap (mostly docs + one output line
+ a test), not novel (uv/Biome do zero-config), moderate value.

## Risks

- **"Too much magic" if `status` prints next-step instructions.** Report facts
  (`current`, `next`, changes) only — never a tutorial or a suggested command.
- **Human-output change breaks scripts that grep it.** It is the human format by
  definition; scripts use `--format json`. Note it in the changelog entry.
- **The composite task bakes in a commit message and a push.** Leave those lines
  obviously editable, or ship the sequence as documentation only.
- `status --check`'s inverted sense is a real wart but a documented contract;
  changing it is a separate, breaking proposal.

## Related

- 01 (freeze — no new verb), 11 (state the opinions), `docs/ci.md:111`.
