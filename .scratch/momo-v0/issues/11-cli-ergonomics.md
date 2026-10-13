# CLI ergonomics

Type: prototype
Status: resolved

## Question

Build a **rough CLI** (help text + a few stubbed commands) to react to, then
lock the ergonomics.

Cover:

- `momo break|add|fix "<desc>"` argument shape — positional vs `-m`, one change
  per call or many?
- Interactive prompting vs flags-only.
- `--dry-run` placement — global vs per-command.
- How `momo status` presents unreleased changes + the computed version.
- `momo rc` vs `momo release --rc` (both exist in the surface — clarify roles).
- `--help` / `--version` output.

## Answer

Settled (all recommendations accepted, plus aliases):

1. **`add|fix|break` take a positional `<TEXT>`**, one change per call.
2. **`--dry-run` is global**, usable on any command (before or after the
   subcommand).
3. **Flags-only in v0** — no interactive prompting (the only future prompt is the
   multi-environment install TUI).
4. **`momo candidate`** (canonical) mints/increments a release candidate, with
   **`rc` as a visible alias**; `release --rc` is **dropped**. The change nouns get
   short **visible aliases**: `a` → add, `f` → fix, `b` → break. Other commands are
   unaliased for now (avoid ambiguity; easy to add later).
5. **`status` presentation** as prototyped: per-package, grouped by kind, ending in
   `→ next: <version>`; `--format json` mirrors it structurally.

Prototype: [`../prototypes/cli/HELP.md`](../prototypes/cli/HELP.md).
