# 22: `task_env` is write-only configuration

Type: bug
Status: ready-for-agent
Severity: low
Area: domain/config + cli/commands
Verified: yes — grepped every reference in `src/`

## Symptom

`task_env` is recorded by `install` and listed in `docs/configuration.md`, but
nothing reads it back. Environment selection is done by filesystem scanning, so a
user cannot point momo at a non-adjacent or non-standard task environment.

## Evidence

`src/domain/config.rs:157` `pub task_env: Option<String>,` — read only by config
plumbing (`:173`, `:211`, `:238`, `:316`, `:338`) and `with_task_env` (`:350`).
`install` (`src/cli/commands.rs:353-392`) selects the environment via
`taskenv::detect(&root)` and never consults `config.task_env`.

## Trigger

`task_env = "mise"` with a `mise.toml` that `detect` does not recognise (see
ticket 32) — momo still reports "no task environment detected".

## Suggested direction

Either consume `task_env` in `install` (prefer the configured environment over
detection), or drop it from the config surface and docs.

## Open questions for triage

- Is a configured environment override a wanted feature (per
  `issues/08-taskenv-selection.md`, which reserved "an explicit override"), or was
  it superseded by auto-detection?

## Triage

**Verdict:** Confirmed write-only. `task_env` is written by
`config::record_task_env` (`commands.rs:389`, `cli/config.rs:59-86`) and read
only by config plumbing (`config.rs:157,173,211-212,238-239,281-282,316,338`);
`install` (`commands.rs:353-392`) selects the environment from the `env`
argument or `taskenv::detect(&root)` and never consults `config.task_env`. It
does not even load the config.

**Ruling: implement, do not remove.** v0 ticket 08 explicitly stores the choice
"so `install --update` / `uninstall` know what to refresh", and the explicit
override is already the settled UX (`named()` at `detect.rs:22` backs
`momo install <env>`). Consuming it closes the loop the design intended and
gives `--update`/`--uninstall` a path when detection fails (ticket 32). This is
weaker than 21: the docs row only says "Recorded by `momo install`", which is
true, so the lie is smaller and the severity lower.

**Corrections:** The ticket's trigger depends on ticket 32, but the feature
stands on its own — the recorded choice should win over detection regardless of
whether detection is later widened.

**Scope:** Standalone; relates to 32 (detection) but is not a duplicate.

**Implementation sketch:** In `src/cli/commands.rs::install`, prefer the
recorded environment over detection when no CLI `env` is given:

```rust
let environment = match env {
    Some(name) => crate::adapters::taskenv::named(name, &root)?,
    None => {
        let config = config::load(&root)?;
        match config.task_env.as_deref() {
            Some(name) => crate::adapters::taskenv::named(name, &root)?,
            None => match crate::adapters::taskenv::detect(&root) {
                Some(environment) => environment,
                None => { /* existing warn-and-return */ }
            },
        }
    }
};
```

This block is shared by `--update` and `--uninstall`, so both pick up the
recorded choice. An unknown recorded name should error (via `named`'s
`TaskEnvError::Unknown`) rather than fall through to detection. Tests: record
`task_env = "mise"` with no `mise.toml` on disk and assert `install --uninstall`
still resolves the environment; assert an explicit CLI `env` still beats the
config.
