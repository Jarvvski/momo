# 30: Spawned VCS and mise commands have no timeout and no non-interactive env

Type: bug
Status: resolved
Severity: medium
Area: adapters/vcs + adapters/taskenv
Verified: no — reported by an agent; the spawn sites are as cited

## Symptom

Every external command is run with `Command::output()`, which blocks until the
child exits, with no timeout and no non-interactive environment. A held jj
workspace lock, a credential prompt, or a mise trust prompt can hang momo
forever with no diagnostic. In the unattended release job this is a silent stall.

## Evidence

`src/adapters/vcs/process.rs:36-43`:

```rust
let output = Command::new(program)
    .args(args)
    .current_dir(cwd)
    .output()
```

`src/adapters/taskenv/graph.rs:24-31` spawns `mise` the same way.

## Trigger

Another process holds the jj workspace lock; or a future/edited command needs
credentials. `git tag -a -m` supplies its message so it will not open an editor,
and a non-tty disables the pager — so today the realistic hang is jj lock
contention, but nothing guards it.

## Suggested direction

Set a non-interactive environment (`GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=true`,
`GIT_PAGER=cat` / `--no-pager`) and/or bound the wait with a timeout, surfacing a
clear error on expiry.

## Open questions for triage

- Is a timeout needed, or is the non-interactive env enough for the realistic
  cases? A timeout needs a sensible default and a way to configure it.

## Triage

**Verdict:** Confirmed as cited: `SystemRunner::run`
(`src/adapters/vcs/process.rs:36-43`) and `graph.rs:24-31` spawn with
`Command::output()`, no env and no timeout. But the ticket bundles two changes of
different readiness: the realistic hang it names (jj workspace-lock contention) is
fixed only by a timeout, which env vars do not address.

**Corrections:** The non-interactive env is cheap, safe and fully specified; the
timeout is a policy decision (a default duration, a config key, a new `VcsError`
variant, and possibly a `wait-timeout` dependency) with no obvious answer. Scope
them apart.

**Scope:** Split. (30a) non-interactive environment → this ticket,
`ready-for-agent`. (30b) command timeout → spin out a separate ticket and let the
owner decide the policy.

**Implementation sketch (30a):** Build the child `Command` through one small helper
so the env is testable. In it, set `GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=true`,
`GIT_SEQUENCE_EDITOR=true`, `GIT_PAGER=cat`, `GIT_OPTIONAL_LOCKS=0`, and a mise
non-prompt equivalent; use the same helper in `graph.rs`. Test: assert the built
`Command`'s environment contains the vars (a `Command` exposes them via
`get_envs()`), so no real process is needed.

## Resolution

Implemented **30a only**. A new `src/adapters/spawn.rs` exposes
`pub(crate) fn command(program, args, cwd) -> Command`, which applies a
`NON_INTERACTIVE` environment: `GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=true`,
`GIT_SEQUENCE_EDITOR=true`, `GIT_PAGER=cat`, `GIT_OPTIONAL_LOCKS=0` and
`MISE_YES=1` (mise's non-prompt equivalent, confirmed against mise's docs and
`--yes`). Both `SystemRunner::run` (`vcs/process.rs`) and `discover`
(`taskenv/graph.rs`) now build their child through it, so there is one spawn site
per module and one place the environment is defined.

Tests (no process spawned): `a_command_carries_the_non_interactive_environment`
asserts every var via `Command::get_envs()`, and
`a_command_carries_the_program_arguments_and_directory` pins the program, args
and cwd.

**No timeout was added** — that is ticket 48's policy decision (30b), which
remains `ready-for-human`.

