# mise integration content

Type: grilling
Status: open
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
