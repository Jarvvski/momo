# 32: mise detection recognises only two of the config names mise reads

Type: bug
Status: resolved
Severity: medium
Area: adapters/taskenv
Verified: no — reported by an agent; `detect.rs:36` and `monorepo.rs:14-26` read as cited

## Symptom

`detect` looks for `mise.toml` or a `.mise/` directory. `monorepo.rs` itself lists
eleven config names mise actually reads. A repo configured with `.mise.toml` (or
`mise/config.toml` without a `.mise/` dir) is never detected, so `workspace::load`
short-circuits to the single-package fallback and a `monorepo_root = true`
monorepo is silently treated as one package. `momo install` warns "no task
environment detected" and exits 0.

## Evidence

`src/adapters/taskenv/detect.rs:36`:

```rust
.find(|dir| dir.join("mise.toml").is_file() || dir.join(".mise").is_dir())
```

versus the name list in `src/adapters/taskenv/monorepo.rs:14-26` (`.mise.toml`,
`mise/config.toml`, `.config/mise.toml`, …).

## Trigger

A repo whose only mise config is `.mise.toml`. `momo -p <pkg> release` fails to
resolve the package; `momo install` does nothing.

## Suggested direction

Share one config-name list between `detect` and `monorepo` (and consider
`MISE_CONFIG_FILE` / `task_config`), or gate discovery on `is_monorepo` rather
than file presence.

## Open questions for triage

- Is `is_monorepo` the better gate? It would decouple "is mise present" from "does
  this repo have a package graph".

## Triage

**Verdict:** Confirmed. `find_root` (`detect.rs:36`) recognises only `mise.toml`
or a `.mise/` directory, while `monorepo.rs:14-26` lists the eleven names mise
actually reads. A repo whose only config is `.mise.toml` (or `mise/config.toml`
without a `.mise/` dir) is not detected, so `workspace::load`
(`workspace.rs:64-66`) returns the single-package fallback before it ever calls
`is_monorepo`, and a `monorepo_root = true` monorepo is silently treated as one
package. `momo install` warns and exits 0.

**Corrections:** The open question is answered no — do not gate discovery on
`is_monorepo`: detection also backs `momo install`, and `is_monorepo` reads config
*content*, not presence. Keep detection as the gate; fix its name list.

**Scope:** Single, medium. Keep.

**Implementation sketch:** Move the name list out of `monorepo.rs` into one shared
`pub(crate)` item (e.g. `CONFIG_FILES`) and use it in `detect::find_root`:
`CONFIG_FILES.iter().any(|n| dir.join(n).is_file()) || dir.join(".mise").is_dir()`.
Consider honouring `MISE_CONFIG_FILE`. Tests: only `.mise.toml` detects; only
`mise/config.toml` detects; a nested subdirectory finds the root.

## Resolution

The eleven-name list moved out of `monorepo.rs` into `pub(crate) const
CONFIG_FILES` in `src/adapters/taskenv/mod.rs`, and `detect::find_root` now uses
it: `CONFIG_FILES.iter().any(|name| dir.join(name).is_file()) ||
dir.join(".mise").is_dir()`. Detection stays the gate (it also backs `momo
install`), and the `.mise/` directory is still recognised on presence alone.

Tests: `detects_a_dot_mise_toml_file` (only `.mise.toml`), and
`detects_a_mise_config_toml_without_a_dot_mise_directory` (only
`mise/config.toml`, from a nested subdirectory) — both previously collapsed to
the single-package fallback.

Not done: `MISE_CONFIG_FILE` is still ignored. The ticket's sketch said "consider
honouring" it; it is an environment override rather than one of the names mise
reads, and adding env-dependence to detection widens the change beyond the name
list the ticket is about.

