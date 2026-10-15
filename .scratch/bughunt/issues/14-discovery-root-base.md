# 14: Discovery may resolve package roots against the wrong base

Type: bug
Status: needs-triage
Severity: medium
Area: cli/config, adapters/taskenv
Verified: no — reported by an investigator, not reproduced

## Symptom (unconfirmed)

When momo's project root differs from mise's monorepo root, `taskenv::graph::discover` passes momo's root to mise and treats mise's returned roots as relative to it, so every `Package::dir` is wrong.

## Evidence

`cli/config.rs::project_root` falls back to the cwd when there is no `.jj`/`.git`/`momo.toml`. `adapters/taskenv/graph.rs::discover` runs mise with that root and resolves the returned roots against it. The investigator reproduced a run from a subdirectory where `status` listed the identities but `add` failed with `not-in-package`.

## Next step

Reproduce: a repo with no VCS and no `momo.toml`, a mise monorepo config, run `momo status`/`add` from a subdirectory. Confirm whether `Package::dir` points at the wrong place, then promote to `ready-for-agent` or close.
