# mise integration content

Type: grilling
Status: resolved
Blocked by: 08, 16

## Question

Exactly what does `momo install mise` write into a consumer repo?

Build on `docs/mise-task-distribution.md` (Shape B): self-contained task files
under `mise-tasks/momo/`, each declaring the CLI via
`#MISE tools={"github:Jarvvski/momo" = "…"}`, and **no `mise.toml` edit** (so no
trust prompt, no experimental flag, no clobbering the consumer's tasks).

Settle:

- The **task set** — e.g. `momo:release`, `momo:status`, `momo:break`/`add`/`fix`,
  `momo:rc`, `momo:tag`? Which subset is useful as tasks?
- Task **names and namespace**.
- Where the **pinned version** comes from (the running momo version).
- **Idempotency**, `--update`, and `uninstall`.
- Whether the tasks **wrap `momo …`** or reimplement anything (they should wrap).

## Answer

Settled (all recommendations accepted):

1. **Task set: one thin wrapper per verb** — `momo:status`, `momo:release`,
   `momo:rc`, `momo:tag`, `momo:add`, `momo:fix`, `momo:break`. Each just
   `exec momo <verb> "$@"`.
2. **Placement: one set at the repo root** (`mise-tasks/momo/`), delegating
   per-package routing to momo itself (`-p` / `--all`) — no per-package
   duplication.
3. **Version pin = the running momo version**, via
   `#MISE tools={"github:Jarvvski/momo" = "<version>"}` on each task, so the tasks
   always pull the CLI that wrote them.
4. **Lifecycle:** idempotent (no-op when unchanged); `--update` refreshes the
   tasks to the running version; `uninstall` removes `mise-tasks/momo/`. Per
   `docs/mise-task-distribution.md` (Shape B) — **no `mise.toml` edit**, so no
   trust prompt and no clobbering the consumer's tasks.
