# Distribute mise tasks as self-contained files, not a plugin or remote include

momo needs to give arbitrary third-party repos its mise tasks plus a pinned
copy of the momo CLI. A mise plugin cannot carry tasks — plugins only install
tools/env/packages (verified in the plugin interface and manifest schema) — and
a remote `task_config.includes` entry now requires the consumer to set the
undocumented `experimental = true` flag (mise ≥ 2026.10.5). We therefore ship
tasks as self-contained executable files under `mise-tasks/momo/`, each
declaring the CLI as a pinned tool in its `#MISE tools={…}` header, written by a
`momo install` command that edits no `mise.toml`. This needs no consumer
configuration, no trust prompt, and no experimental flag. The trade-off: tasks
are copied, so updates require re-running `momo install --update`.

## Considered Options

- **mise plugin** — rejected: cannot carry tasks (only tools/env/packages).
- **`task_config.includes` (git::/oci::)** — rejected: needs `experimental = true`,
  *replaces* the default task directories, and flips the config into "needs trust".
- **`[tasks.*]` merged into `mise.toml`** — rejected: destructive to a user's
  hand-formatted file and the worst update story.

## Consequences

- `momo install` owns versioning/drift (mise offers none): version marker + `--update`.
- Tasks are namespaced `momo:*` to avoid collisions with consumer tasks.
