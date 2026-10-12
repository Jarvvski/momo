# Distributing mise tasks (and tool pins) to third-party repos

A playbook for the recurring question: **how does a tool give an arbitrary
repo its `mise` tasks — and ideally a pinned copy of the tool itself — with
minimal consumer setup?** Written generically so it applies beyond momo; momo's
`install` command is the worked example.

Facts below are version-stamped where behaviour is new or undocumented. The
primary sources are the [mise docs](https://mise.jdx.dev) and the
[`jdx/mise`](https://github.com/jdx/mise) source; the version-gated claims were
confirmed by running mise directly (see [Verification log](#8-verification-log)).

---

## 1. TL;DR and decision tree

**A mise plugin cannot carry tasks.** If you want tasks to reach a consumer,
either they copy self-contained task files into their repo, or they reference a
remote include. For *arbitrary third-party repos*, prefer **self-contained task
files** (Shape B): they need no consumer configuration, no trust prompt, and no
experimental flag.

```
Need to ship mise tasks to a consumer repo?
│
├─ Via a mise plugin?                      → NO. Plugins install tools/env/packages only.
│
├─ Consumer edits their mise.toml?
│  ├─ Remote include (task_config.includes) → works, but: experimental flag + trust + replaces defaults.
│  └─ Inline [tasks.*] tables               → worst update story; destructive merge.
│
└─ No consumer mise.toml edit?              → YES: self-contained task files (recommended).
                                              Pin the tool inside each task's #MISE header.
```

**Recommended (Shape B):** the tool's `install` command writes executable task
files under `mise-tasks/<tool>/`, each declaring the tool as a pinned dependency
in its `#MISE tools={…}` header. No `mise.toml` is touched.

---

## 2. Mechanism catalog

| Mechanism | What it does | Verdict for third-party distribution |
|---|---|---|
| **mise plugin** (Tool/Backend/Environment/Package-manager/asdf) | Installs a tool; sets env/PATH; manages packages | ❌ **Cannot carry tasks.** No task hook, no manifest key. |
| **`task_config.includes`** — `git::` / `oci::` | Pulls task files from a remote git repo or OCI artifact | ⚠️ Works, but needs `experimental = true`, flips config to "needs trust", and **replaces** the default task dirs. |
| **File tasks** (`mise-tasks/`, `.mise/tasks/`, …) | Executable scripts + `.toml` files discovered on disk | ✅ **Recommended.** Additive, ungated, no trust needed for static scripts. |
| **Config-level `include`** | Merges another config file (local/`git::`/`oci::`) | ❌ Cannot carry `[tasks]`/`task_config`/`task_templates` (explicitly rejected). Useful for `[tools]`/`[env]`/`[hooks]` only. |
| **Per-task `file =`** | One task's script fetched from a URL/`git::` | ⚠️ Single task, not a set. The only place a plain `https://` task source is allowed. |

---

## 3. Hard constraints (verified, version-stamped)

These are the non-obvious facts that decide the design. Each cost real
investigation; treat them as load-bearing.

1. **A plugin cannot expose tasks.** Plugin types are Tool, Backend,
   Environment, Package manager, and legacy asdf — none task-related.
   `src/plugins/mod.rs` contains no task references, the plugin manifest schema
   (`schema/mise.plugin.json`) errors on unknown keys, and the internal
   `TaskFileProvider` trait is `pub(crate)` with a hardcoded provider list (not
   a third-party extension point). Maintainer position (discussion
   [#10006](https://github.com/jdx/mise/discussions/10006)): *"Tasks can be
   resolved from config because they are part of the project config model.
   Plugins are more like executable extensions installed into the user's mise
   plugin directory."*

2. **Remote `git::`/`oci::` task includes require `experimental = true`**
   (mise ≥ **2026.10.5**, PR #14114). Without it, mise **silently skips** the
   include with a `WARN`. This is **not in the docs** and **not listed by
   `mise settings ls`** — verified empirically. This is the single biggest
   reason to avoid Shape A for third-party repos: you cannot ask every consumer
   to enable an experimental mode.

3. **`task_config.includes` *replaces* the default task directories.** Defaults,
   in precedence order: `mise-tasks`, `.mise-tasks`, `.mise/tasks`,
   `.config/mise/tasks`, `mise/tasks`. Setting `includes` drops them unless you
   re-list them explicitly.

4. **Adding `[task_config]` flips a config into "needs trust."** The
   trust-exempt whitelist is only `min_version`, plain `[tools]` version
   strings, and `[tasks]` without `secrets`. Everything else requires
   `mise trust`; non-interactive contexts (CI, editors) **fail** rather than
   prompt.

5. **`mise tasks add` auto-trusts the file it writes — but cannot write `tools`
   pins.** The `--file` mode writes an executable script, yet the `tools` field
   is a commented-out TODO in the source with no `--tools` flag. So a tool that
   needs pinned dependencies in its tasks must **write the files itself**.

6. **`serde` + `toml` destroys a user's file.** Parse→serialize drops comments,
   formatting, and key order. Any edit to a user's `mise.toml` must use a
   round-trip editor — **`toml_edit::DocumentMut`** (what mise and hk use
   internally).

---

## 4. Recommended pattern (Shape B)

Write self-contained task files; let the tool pin travel inside the task.

```
consumer-repo/
└── mise-tasks/
    └── momo/
        ├── lint      → task `momo:lint`
        └── fmt       → task `momo:fmt`
```

```bash
#!/usr/bin/env bash
#MISE description="Lint with momo"
#MISE tools={"github:you/momo"="1.2.3"}   # the CLI pin travels INSIDE the task
momo lint "$@"
```

Why this wins:

- **No `mise.toml` edit** → no trust prompt, no `includes`-replaces-defaults
  landmine, no experimental flag.
- **Additive** — coexists with the consumer's existing tasks.
- **Self-contained** — works offline and in CI with no clone/pull.
- **Namespaced** — the `momo/` directory makes the task `momo:lint`, so it
  cannot collide with a consumer task, and the consumer can override it
  individually.

The cost: tasks are **copied**, so the tool's `install` command owns
versioning and updates (mise offers no drift detection for local task files).

---

## 5. Implementation mechanics

### Writing a task file

- **Header syntax.** Each `#MISE` line is parsed as TOML. The marker regex is
  `^(?:#|//|::)\s*(?:MISE|\[MISE\]) (.*)$` — so `#MISE`, `# MISE`, `// MISE`,
  `# [MISE]` all work. Disable a line by changing the keyword (e.g. `# NOMISE`).
- **`tools` is a valid header key**, with each value a version string or a
  `{ version = "…", <opts> }` map. This is how the pinned CLI travels.
- **Excluded header keys:** `run`, `run_windows`, `file`, `vars`, `timeout`,
  and the sandbox keys. Unknown keys warn (not error) and are ignored.
- **Multi-line values are allowed** as long as every line keeps the prefix.

### Discovery and naming

- **Directory precedence** (later wins on collision): `mise-tasks`,
  `.mise-tasks`, `.mise/tasks`, `.config/mise/tasks`, `mise/tasks`. Scanned
  recursively; dot-prefixed files/dirs are skipped. `mise-tasks/` is the
  conventional, always-valid target (`mise tasks add --file` falls back to it).
- **Name derivation:** the path below the task dir becomes the name, `/` → `:`.
  `mise-tasks/momo/lint` → `momo:lint`. Known script extensions (`.sh`, `.py`,
  `.js`, `.ps1`, …) are stripped; a file named `_default` takes its directory's
  name.

### Cross-platform

- **Unix:** the file **must be executable** (`chmod +x`); mise lists only
  executable files as tasks.
- **Windows:** no execute bit. A file is a task if its extension is in
  `windows_executable_extensions` (`exe bat cmd com ps1 vbs`) **or** it starts
  with a shebang. Ship a `build.ps1` alongside `build.sh` if Windows matters.

### Versioning, idempotency, updates

- **Stamp each generated file** with a marker comment (e.g. `# generated by
  momo 1.2.3`) and record the generating version where mise won't choke: the
  **`[_]` table is explicitly ignored by mise** — ideal for `[_].momo_version`.
- **Idempotent:** re-running `install` should no-op when files are unchanged.
- **Refuse to clobber:** error if a target file exists unless `--force`
  (biome-style), or merge-if-absent (hk-style).
- **Provide `--update`** to refresh files to the running version, and an
  **`uninstall`** that removes the tool's directory.

### If you must edit `mise.toml`

Use `toml_edit::DocumentMut` (comment/format/order-preserving). For a list like
`task_config.includes`, append with dedup — `mise config set
task_config.includes '<url>' --append --type list` is the idempotent form (the
default **replaces** the whole list, and `config set` bails if the project file
doesn't exist). Write to a temp file and rename; read-modify-write is not atomic.

---

## 6. Precedents and sources

- **`hk init --mise` (jdx/hk) — the blueprint.** mise's own author's tool edits
  `mise.toml` in place with `toml_edit::DocumentMut`; *"Existing mise.toml files
  are preserved"*; it is idempotent ("mise.toml already contains hk
  configuration"); `--force` is scoped to its own file and never resets
  `mise.toml`; it inspects `task_config.includes` before inserting to avoid
  duplicates. Notably it does **not** warn users about `mise trust`.
  ([hk.jdx.dev/mise_integration.html](https://hk.jdx.dev/mise_integration.html),
  `jdx/hk` `src/cli/init/mod.rs`.)
- **mise cookbook — "Project scaffolding tasks"** — the official recipe: a
  global file task with `#MISE dir="{{cwd}}"` that runs `mise tasks add` +
  `mise use`. Its own caveat: *"`mise tasks add` replaces a task with the same
  name and `mise use` replaces the existing versions."*
  ([mise-cookbook/presets.html](https://mise.jdx.dev/mise-cookbook/presets.html).)
- **`iwamot/mise-tasks` / `montblu/mise-tasks`** — shared task repos consumed
  via a one-line `git::` include, with each task declaring its own tools via
  `#MISE tools={…}`.
- **`pre-commit install`** — embeds `CURRENT_HASH`/`PRIOR_HASHES` markers to
  identify and migrate its own generated artifact, and moves existing hooks to
  `.legacy` (migration mode) rather than clobbering.
- **`biome init`** — refuse-to-clobber (`create_new`) plus a `$schema`
  version marker; `biome migrate` for updates.
- **Tool pinning for a Rust CLI:** prefer the **`github:`** backend (prebuilt
  binary; `mise.lock` records URL + checksum) over `cargo:` (needs Rust, often
  compiles, lock records version only). **`ubi:` is deprecated** (removal
  2027.1.0) — use `github:`.

---

## 7. Pitfalls checklist

Before shipping a task-distribution mechanism, confirm:

- [ ] You are **not** trying to ship tasks through a plugin — that is impossible.
- [ ] If using remote includes: consumer sets `experimental = true`; you re-list
      any default task dirs; you handle the trust flip.
- [ ] Task files are `chmod +x` (Unix) and carry a Windows extension or shebang.
- [ ] Tasks are **namespaced** (`tool:task`) so they cannot clobber consumer tasks.
- [ ] `install` is **idempotent** and **refuses to clobber** without `--force`.
- [ ] Generated files carry a **version marker**; `--update` and `uninstall` exist.
- [ ] Any `mise.toml` edit uses **`toml_edit::DocumentMut`**, not `serde`+`toml`.
- [ ] The tool pin uses the **`github:`** backend (not deprecated `ubi:`), so it
      locks with a URL + checksum.
- [ ] If you edit `mise.toml` directly (rather than via `mise tasks add`), you
      **surface a `mise trust` hint** — no surveyed tool does, and consumers in
      CI will fail without it.

---

## 8. Verification log

The version-gated claims above were confirmed against mise **2026.10.8**
(macos-arm64) in a scratch directory.

**Shape A is gated.** With:

```toml
[task_config]
includes = ["git::https://github.com/iwamot/mise-tasks.git//.mise/tasks?ref=v2.7.3"]
```

`mise tasks ls` produced:

```
mise WARN  skipping task include `git::…?ref=v2.7.3`: including tasks from a
`git::` source is experimental. Enable it with `mise settings experimental=true`
```

Adding `[settings] experimental = true` made the remote tasks load. `mise
settings ls` does **not** list `experimental`.

**Shape B is ungated.** A file at `mise-tasks/momo/lint` with a `#MISE
description` and `#MISE tools={…}` header was listed immediately as `momo:lint`
with **no experimental flag and no trust prompt** (`mise trust --show` reported
"No trusted config files found").

---

## Dead ends (don't re-investigate)

- No `mise tasks include` subcommand ([#4792](https://github.com/jdx/mise/discussions/4792)).
- No mise JSON **write** API; `mise mcp` is read/run-only.
- `[task_templates]` cannot be shipped remotely (config-file-only; excluded
  from both config-level `include` and task-file includes).
- `mise sync` (vendoring repo dependencies with a lockfile) is an unmerged
  proposal ([#10861](https://github.com/jdx/mise/discussions/10861)) — the only
  future feature that would rival Shape B natively.
