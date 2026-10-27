# 09: A failed `momo add` still writes a generated `momo.toml`

Type: bug
Status: ready-for-agent
Severity: low
Area: cli/commands
Verified: yes — reproduced with the built binary

## Symptom

A `momo add` that fails leaves a freshly generated `momo.toml` behind — a mutation on a command that reported failure.

## Evidence

`commands.rs:145-147` calls `ensure_config(&root)` before the recording loop, and `ensure_config` writes the file (`commands.rs:717-725`). Any later failure — a duplicate Unreleased heading, an unwritable changelog — leaves the file.

## Trigger

```console
$ mkdir demo && cd demo
$ printf '# Changelog\n\n## [Unreleased]\n\n- a\n\n## [Unreleased]\n\n- b\n' > CHANGELOG.md
$ momo add "a thing"
error[momo.changelog.duplicate-unreleased]: … exit 1
$ ls
CHANGELOG.md momo.toml     # momo.toml was never there before
```

## Suggested direction

Create the config only once the change is committed to disk (or make a failed run remove a config it created). Compare `a_change_verb_rejects_empty_text`, which already pins "a rejected change created the config" as a bug for the argument-rejection path; this is the runtime-failure path.
