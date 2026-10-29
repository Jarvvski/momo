# 14: Discovery may resolve package roots against the wrong base

Type: bug
Status: resolved
Severity: medium
Area: cli/config, adapters/taskenv
Verified: no — reported by an investigator, not reproduced

## Symptom (unconfirmed)

When momo's project root differs from mise's monorepo root, `taskenv::graph::discover` passes momo's root to mise and treats mise's returned roots as relative to it, so every `Package::dir` is wrong.

## Evidence

`cli/config.rs::project_root` falls back to the cwd when there is no `.jj`/`.git`/`momo.toml`. `adapters/taskenv/graph.rs::discover` runs mise with that root and resolves the returned roots against it. The investigator reproduced a run from a subdirectory where `status` listed the identities but `add` failed with `not-in-package`.

## Next step

Reproduce: a repo with no VCS and no `momo.toml`, a mise monorepo config, run `momo status`/`add` from a subdirectory. ## Reproduction (2026-10-29)

Confirmed against real mise + momo. A repo with `monorepo_root = true`, a workspace `Cargo.toml`, `crates/a`, and no VCS and no `momo.toml`:

- from the root, `momo status` is correct;
- from `crates/a`, `momo add` fails `momo.selection.not-in-package`.

mise reports `root: "crates/a"` relative to the monorepo root regardless of cwd; `project_root` falls back to the cwd when no VCS/`momo.toml` marker is found, so `discover` resolves mise's roots against `crates/a` and `Package::dir` becomes `crates/a/crates/a`.

## Suggested direction

The Workspace root must be the mise monorepo root when one is detected. When no VCS or `momo.toml` marker is found, `project_root` should stop at the directory holding the mise configuration (the same ancestor `taskenv` detection finds) rather than the cwd, so discovery resolves mise's roots correctly. `momo.toml` is then also written at the true root.
