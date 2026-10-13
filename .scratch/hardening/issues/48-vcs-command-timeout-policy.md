# 48: Command timeout policy for spawned VCS and mise processes (30b)

Type: hardening
Status: ready-for-human
Severity: medium
Area: adapters
Verified: yes — split out of ticket 30 during triage (batch 5)

## Symptom

`SystemRunner::run` and the mise graph spawn use `Command::output()`, which blocks
until the child exits. The realistic hang the parent ticket names — jj
workspace-lock contention — is fixed *only* by a timeout; a non-interactive
environment does not address it. In the unattended release job that is a silent
stall.

This is the human-owned slice of ticket 30; the non-interactive environment
(30a) is agent-ready and stays in ticket 30.

## Evidence

`src/adapters/vcs/process.rs:36-43` and `src/adapters/taskenv/graph.rs:24-31`
spawn with `Command::output()`, no timeout.

## Trigger

Another process holds the jj workspace lock. `momo release` blocks indefinitely.

## Suggested direction

Bound the wait and surface a clear error on expiry. This needs decisions, not
just code:

- a default duration (and whether it is configurable — a new config key?);
- a new `VcsError` / `TaskEnvError` variant for "timed out";
- whether to add a dependency (`wait-timeout`) or hand-roll the wait;
- whether a timeout is even the right answer for lock contention, versus detecting
  and reporting the lock.

## Open questions for triage

- Is a timeout in scope for v0.x, or is a clear "jj is locked" diagnostic enough?
- If a timeout is added, does killing the child leave a half-done VCS operation?
  (For read commands it is safe; for `tag set` it may not be.)

## Related

- 30 (parent; 30a is the agent-ready non-interactive env), 39 (port error
  vocabulary — the timeout error is one of the variants that must not carry
  process detail into the domain).
