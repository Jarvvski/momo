# 20: Lazy config creation bakes transient environment overrides into `momo.toml`

Type: bug
Status: ready-for-agent
Severity: medium
Area: cli/commands
Verified: yes — read at the cited lines

## Symptom

`ensure_config` writes the *env-merged* configuration to `momo.toml` when the file
is missing. A one-off `MOMO_*` variable used for a single command is therefore
persisted into the committed file, inverting the documented precedence
(env > file > defaults).

## Evidence

`src/cli/commands.rs:108` (inside `record`, i.e. `break`/`add`/`fix`):

```rust
if !cli.dry_run {
    ensure_config(&root, &config)?;
}
```

`src/cli/commands.rs:446-452`:

```rust
fn ensure_config(root: &Path, config: &Config) -> Result<()> {
    let path = root.join(CONFIG_FILE);
    if path.exists() {
        return Ok(());
    }
    fs::write(&path, config.to_string())...   // `config` is the env-merged result
}
```

`config` comes from `config::load` (`src/cli/config.rs:34-51`), which resolves
file then env.

## Trigger

In a repo with no `momo.toml`:

```
MOMO_TAG_PREFIX=rel- momo add "x"
```

writes `tag_prefix = "rel-"` into the newly created `momo.toml`. The override now
outlives the environment.

## Suggested direction

When lazily creating the file, persist only the defaults (or the file layer),
never the environment layer.

## Open questions for triage

- `momo init` deliberately records the resolved config. Is that in scope to change
  too, or is `init` (an explicit act) different from the incidental creation here?

## Triage

**Verdict:** Confirmed. `ensure_config` (`commands.rs:446-452`) writes
`config.to_string()`, and `config` comes from `config::load`
(`cli/config.rs:34-51`), which is the env-merged result. So `MOMO_TAG_PREFIX=rel-
momo add "x"` in a repo with no `momo.toml` persists `tag_prefix = "rel-"`,
inverting the documented precedence (env > file) for every later run.

**Corrections — the `init` question:** `momo init` is **not** deliberately
different; it has the same defect. It also writes `config.to_string()` from the
env-merged `config::load` (`commands.rs:320-338`). The docs do not authorise
env-baking: `docs/configuration.md:8` says "momo init writes exactly this:" and
shows the default values, and the v0 spec §12 makes the same env > file promise.
The only thing suggesting otherwise is `init`'s own doc comment ("Writes the
effective configuration", `commands.rs:317`) — a code comment, not the spec. So
the fix applies to **both** `ensure_config` and `init`: persist the file/default
layer, never the environment layer. (If the owner *wants* `init` to snapshot the
resolved config, that is a spec change and the docs must say so; as written they
promise defaults.)

**Scope:** Standalone; grouped with 19 and 23 as config integrity (PR D). Note
`init`'s env-baking is also untested.

**Implementation sketch:**

- `ensure_config`: write `Config::default().to_string()` (the file layer is
  empty when the file is absent). Drop the now-unused `config` parameter and
  update the call site in `record` (`commands.rs:108`).
- `init`: write `Config::default().to_string()`; keep the `--version` handling.
  The `config::load` call existed only to feed the write (init bails on an
  existing file at `commands.rs:334-336`, so the file layer is always empty
  here) — remove it.
- Tests (integration, `tests/`): with no `momo.toml` and `MOMO_TAG_PREFIX=rel-`,
  run `momo add x` and assert the created file holds `tag_prefix = "v"`, not
  `rel-`; same for `MOMO_RECORDING=fragments momo init`.
