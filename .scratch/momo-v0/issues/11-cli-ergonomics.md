# CLI ergonomics

Type: prototype
Status: open

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
