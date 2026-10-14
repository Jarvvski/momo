# 48: Command timeout policy for spawned VCS and mise processes (30b)

Type: hardening
Status: resolved
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

## Decision (owner, 2026-10-14)

**Add a timeout.** Bound every spawned command with a default (60s) and a config
key to raise it, plus a distinct error variant for expiry.

Pairs with ticket 39's reshaped error vocabulary — land them together so the
timeout variant is part of the new shape rather than bolted onto the old one.

## Resolution

Landed on `process-seam` (with ticket 39).

- `adapters::spawn::run` is now the single wait path: it spawns the child with
  piped stdout/stderr, drains both pipes on their own threads (so a full pipe
  buffer cannot be misread as a hang), polls `try_wait`, and stops the child once
  the timeout elapses, reporting `ProcessError::TimedOut`.
- The child leads its own process group (`CommandExt::process_group(0)` on Unix),
  so expiry stops everything it forked — a mise tool process, a `sh -c`
  grandchild — not just the direct child. `unsafe` is forbidden crate-wide, so
  the group signal goes through the `kill` binary (`kill -s KILL -- -<pgid>`);
  when that is unavailable the direct child is killed on its own.
- On expiry `run` does **not** join the pipe readers: a forked grandchild holds
  the write end, so joining would hold the call open for as long as the
  grandchild lives — the exact hang the timeout exists to bound. The readers end
  once the killed group closes the pipes.
- No new dependency: the wait is hand-rolled with `try_wait` + a 10 ms poll, so
  `Cargo.lock` is untouched.
- Default 60s, held as `domain::config::DEFAULT_COMMAND_TIMEOUT`. New config key
  `command_timeout` (whole seconds, positive) with `MOMO_COMMAND_TIMEOUT`; it
  flows into `SystemRunner` for git/jj and into `taskenv::discover` for mise.
  `docs/configuration.md` documents it.
- Expiry is a distinct variant: `VcsError::Timeout { timeout }` and
  `TaskEnvError::Timeout { timeout }`, mapped from `ProcessError::TimedOut`.

Tests: a **forking** child (`sh -c '(sleep 0.3; …) & sleep 30'`) is bounded
promptly and its forked grandchild is proven killed (it never writes its marker);
a second test drives `command_timeout` from a parsed `momo.toml` through
`SystemRunner` to prove the key reaches the runner.

Limitations that remain:

- If `kill` is not on `PATH`, only the direct child is stopped and a forked
  grandchild can linger (holding its pipe and any lock). The call still returns
  promptly, so the timeout is never defeated; the leftover process is the cost.
- Because the child leads its own group, an interactive Ctrl-C no longer
  propagates to it — `momo` dies and the child runs on. The timeout is now the
  bound for that case.
- The *success* path still joins the readers, so a command that exits while
  leaving a background grandchild holding the pipes would block there. git, jj
  and mise do not background work, so this is not reachable today.
- The open question about a half-done `tag set` is unchanged: a killed child may
  leave a partial operation, which the staged release transaction (ticket 37)
  does not cover for VCS mutations.
