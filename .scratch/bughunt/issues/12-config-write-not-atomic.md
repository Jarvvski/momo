# 12: `momo.toml` is written non-atomically

Type: bug
Status: resolved
Severity: low
Area: cli/config, cli/commands
Verified: yes — read at the cited lines

## Symptom

`momo.toml` is written with a bare `fs::write` (truncate then write) while every other state file goes through `cli::atomic`. A crash or `ENOSPC` mid-write leaves a truncated TOML, and every later command fails on it until the user repairs it by hand.

## Evidence

`src/cli/config.rs:86` (`record_task_env`), `src/cli/commands.rs:598` (`init`) and `commands.rs:723` (`ensure_config`) use `fs::write`. The changelog, ledger and release artefacts use `atomic::write`.

## Suggested direction

Route all three through `crate::cli::atomic::write`. No behaviour change on success; a torn file becomes impossible. There is no test that can catch the race directly, so pin the call site by construction (a small helper) rather than by a fault-injection test.
