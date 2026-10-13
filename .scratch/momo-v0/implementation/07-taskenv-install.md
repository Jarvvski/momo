# 07: Task-environment install (mise)

**What to build:** A `TaskEnvironment` port in `momo-core` and a mise
implementation in `momo-taskenv`. `momo install` auto-detects mise
(`mise.toml`/`.mise/`) or takes an explicit override, then writes one thin
`momo:*` task wrapper per verb at the repo root, each pinning the running CLI via
`#MISE tools={…}`. It is idempotent and supports `--update` and `--uninstall`.
When no task environment is found it is a graceful no-op plus a warning. It never
edits `mise.toml`.

**Blocked by:** 01

**Spec:** `.scratch/momo-v0/spec.md` §10 · playbook: `docs/mise-task-distribution.md`

**Status:** ready-for-agent

- [ ] `momo install` in a mise repo writes the `momo:*` task wrappers and leaves `mise.toml` byte-identical.
- [ ] Running `momo install` twice changes nothing the second time.
- [ ] `--update` refreshes the pinned version; `--uninstall` removes what install wrote.
- [ ] With no task environment present, `install` warns and exits zero.
- [ ] The chosen environment is recorded in config as `task_env`.
