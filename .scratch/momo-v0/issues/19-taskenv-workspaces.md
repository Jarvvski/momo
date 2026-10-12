# Task-runner workspace / monorepo models

Type: research
Status: resolved

## Question

How do `mise`, `gradle`, and npm/bun model **workspaces / monorepos**, so momo can
reuse the task environment's model rather than invent its own?

Specifically:

- Does **mise** have monorepo/workspace support? How does it discover subprojects,
  resolve per-directory configs, and run tasks across them? Is there a canonical
  way for a tool to read "what are the packages here" from mise?
- **Gradle** multi-project builds: `settings.gradle(.kts)`, `include`, subproject
  modules — how is the module graph declared?
- **npm/yarn/pnpm/bun** workspaces: the `workspaces` field, `pnpm-workspace.yaml`,
  etc.
- For each, what a versioning tool should read to enumerate packages.

Report exact file/field names and cite sources.

## Answer

Full findings: [`../research/taskenv-workspaces.md`](../research/taskenv-workspaces.md).

- **mise has first-class monorepo support.** Root `mise.toml` sets
  `monorepo_root = true` + `[monorepo].config_roots = [...]` (paths or
  single-level globs; auto-discovery is deprecated).
- **The seam momo should use: `mise tasks graph --json`** — a documented project
  graph with `id` (ecosystem-namespaced, e.g. `cargo:momo`, `node:@acme/ui`),
  `root`, `dependencies`, `metadata`, `provenance`. It unifies Cargo / uv / Go /
  Node and needs **no package manager installed**. Requires `monorepo_root = true`.
- **But the graph carries no versions** — versions still come from each
  ecosystem's manifest (`Cargo.toml`, `package.json`, Gradle `version`), or from
  momo's own config. This couples ticket 16 to ticket 18/20.
- **Gradle:** the module graph is `include(...)` in `settings.gradle(.kts)`
  (colon = nesting); enumeration's real interface is the Tooling API
  `GradleBuild.getProjects()` — `settings.gradle` is code, so regex is unreliable.
- **Node:** `workspaces` in the root `package.json` (array, or object
  `packages`); **`pnpm-workspace.yaml` wins** when both exist; versions live in
  each member's `package.json`.

Unblocks ticket 16 (monorepo model).
