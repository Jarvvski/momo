# 10: jdx CLI citizenship — `version`, help examples, `--json`, thesis

Type: task
Status: ready-for-agent
Points: 13
Bucket: jdx ecosystem fit
Severity: low
Area: cli + docs

## Problem

momo's wedge is the mise/jdx ecosystem, but the cheap ergonomic signals every
jdx-shaped CLI is expected to have are missing. `momo version` errors, help has no
usage examples (the most-referenced bit), and there is no `--json` shorthand. The
README does not state the thesis.

## Evidence

- `--version`/`-V` work (`tests/smoke.rs`); `momo version` → "unrecognized
  subcommand".
- `src/cli/mod.rs:24-125` (good descriptions, **no examples**).
- `src/cli/output.rs`, `tests/ci_output.rs` (the pinned `--format json` contract).
- `src/cli/commands.rs:37` — `VERBS` is used **both** to assert every command has a
  task wrapper *and* as the list `momo install` writes. Adding `version`/
  `completions` breaks that test or installs unwanted wrappers.

## Proposal

- **Split `VERBS` → `INSTALLABLE_VERBS`** (the nine verbs) separate from the command
  surface. This is the load-bearing refactor for tickets 10 and 16.
- **Add `Command::Version`** (prints `momo <version>`), excluded from
  `INSTALLABLE_VERBS`. Keep `-V`. Add `-v`=version **only if** momo commits to never
  having `--verbose` — otherwise reserve it.
- **Add `after_help` examples** on `Cli` and each verb (top-level: the quick-start
  block; per-verb: one realistic invocation).
- **Add a global `--json` shorthand** for `--format json`; add it to
  `reject_output_flags` (`commands.rs:78-95`) and the rejection tests, or it
  silently no-ops on `add`/`init`/`install`.
- **Reject `--csv`, report colour and tables.** `status` is a grouped changelog,
  not a tabular listing; `--format json` is a deliberate pinned contract. This is
  where jdx-citizenship *should* lose to short-but-sweet.
- **Adopt the thesis + positioning paragraph** (below).
- **Defer:** a docs site, `usage` (KDL), a first-class GitHub Action.

## Surface

- CLI: `+ Command::Version`, `+ --json` global, `+ after_help` examples.
- `README.md`: new thesis + positioning; move `mise use github:Jarvvski/momo` to
  the top of Install; add `version` to the Commands table.
- `docs/ci.md`: document exit codes (`0` ok / `1` runtime / `2` usage) and the
  `--json` shorthand.

**Thesis:** *momo — the release half of your mise workflow.*

**Positioning paragraph:**

> momo is the release step for mise-first polyglot monorepos. Pair it with mise and
> it discovers your packages from `mise tasks graph`, installs its own `momo:*`
> tasks with `momo install`, and turns recorded changes into semver versions, a
> changelog and VCS tags — `mise use github:Jarvvski/momo`, then `momo add "…"` and
> `momo release`. It is VCS-optional (git, jj, or none) and CI-agnostic: it never
> reads `CI_*`/`GITHUB_*`, never commits, and never pushes — it reports, and your CI
> decides what to do about it.

## Implementation sketch

| Change | Files |
|---|---|
| `INSTALLABLE_VERBS` split | `src/cli/commands.rs:37`, `tests/cli.rs:11,24` |
| `version` verb | `src/cli/mod.rs`, `src/cli/commands.rs`, `tests/smoke.rs` |
| Help examples | `src/cli/mod.rs` (`after_help`) |
| `--json` shorthand | `src/cli/mod.rs`, `src/cli/output.rs:44-53`, `reject_output_flags` |
| Thesis + README | `README.md`, `docs/ci.md` |

## Scoring

Points **13** = Value 3×2 + Ease 5 + Novelty 2. Cheap; the `INSTALLABLE_VERBS` split
is a prerequisite for 16.

## Risks

- **`VERBS` coupling** — split it first, or the install test breaks.
- **`--json` vs the pinned contract** — it must join the rejection list.
- **`-v` reservation** is a one-way door; state it.
- **Docs drift** — keep `docs/*.md` canonical if a site is ever added.

## Related

- 01 (freeze), 05 (attestations), 16 (completions, blocked by this).
