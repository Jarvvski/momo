# momo v0 — spec

> The consolidated v0 design. Every decision below was resolved by a ticket in
> `issues/`; the map is `map.md`; vocabulary is `GLOSSARY.md`. **Status: locked** —
> a builder can implement v0 with nothing left to decide.

## 1. What momo is

A short, standalone Rust CLI that turns a project's accumulated changes into semver
releases, a changelog, and VCS tags. It runs standalone (`cargo install`) or
integrates into a task environment (`mise` in v0).

## 2. Scope (v0)

- Single binary; Rust, edition 2024; `unsafe_code = "forbid"`.
- **Monorepo-aware**: the *package* is the unit of versioning.
- **VCS-optional**: works with git, jj, or no VCS at all.
- Task-environment integration via an internal `TaskEnvironment` trait — `mise` only.
- Manifest version-sync via an internal `ManifestSync` trait — Rust `Cargo.toml` only.
- Standalone use needs neither a task environment nor a VCS.

## 3. Command surface

| Command | Purpose |
|---|---|
| `momo break\|add\|fix "<desc>"` | Record a change (aliases `b` / `a` / `f`) |
| `momo status` | Show unreleased changes + the version they'd mint |
| `momo release` | Mint a version from the unreleased changes |
| `momo candidate` | Cut / increment a release candidate (alias `rc`) |
| `momo tag` | Create the VCS tag for the current version |
| `momo init` | Scaffold `momo.toml` |
| `momo install [env]` | Install momo into a task environment |
| `--dry-run` | Global: show what would happen, change nothing |

## 4. Semver rules

- **MAJOR 0:** `break` and `add` bump MINOR; `fix` bumps PATCH.
- **MAJOR ≥ 1:** `break` bumps MAJOR; `add` bumps MINOR; `fix` bumps PATCH.
- **Multiple kinds in one release:** the **highest** kind wins (`break` > `add` >
  `fix`).
- **Bootstrap:** baseline `0.0.0`; the first release follows the rules (`add`/
  `break` → `0.1.0`; `fix`-only → `0.0.1`). `momo init --version <x.y.z>` sets an
  explicit start by writing `.momo/version`.
- This is the **zero-aware** pre-1.0 camp (matches release-plz; see `research/versioning-precedents.md`).

## 5. Version source

- **Primary:** the latest VCS tag.
- **Tag names:** `<slug>-v<version>` in a monorepo; plain `v<version>` for a single
  package.
- **VCS-optional:** with no VCS, the source is `.momo/version` (a momo-managed file
  written on every release).
- The VCS is **authoritative when present**; `.momo/version` is the fallback.
- Reading is side-effect-free; `status` / `--dry-run` never write.

## 6. Recording model & changelog

- **Selection:** `recording = "changelog" | "fragments"` in `momo.toml`; default
  `changelog`.
- **Fragments:** `.momo/changes/<package>/<kind>-<slug>.md`; the body is the entry
  text (the Kind is in the filename — no front-matter). Plain files, so they can be
  hand- or CI-authored.
- **Changelog:** one `CHANGELOG.md` **per package**:
  - Sections: `Breaking` / `Added` / `Fixed`.
  - Release heading: `## [<version>] - YYYY-MM-DD`.
  - Entry: `- <text> (#issue)`.
  - RCs get their own heading.
  - **Reference-link footer** derived from the VCS remote, overridable with the
    `remote` config key; omitted when neither is available. The `[Unreleased]`
    link is emitted only while the document has an Unreleased section.
  - In **changelog mode** the `## Unreleased` section is the source; in **fragment
    mode** the changelog holds only releases and `status` reports unreleased.
  - **Preservation.** The document is kept verbatim; momo edits only the spans it
    owns — the Unreleased body, its own footer lines (`[Unreleased]`, `[x.y.z]`)
    and the release headings it inserts. CRLF, `*` bullets, HTML comments,
    `### Changed` headings and user-authored footer definitions pass through
    byte-for-byte. Recording appends to the Unreleased body; releasing moves the
    whole body into the new dated section. A document momo cannot edit without
    touching foreign content — more than one Unreleased heading — is refused,
    naming the line (ADR 0003). The three-Kind vocabulary is unchanged:
    unrecognised headings are preserved, never interpreted, so they do not affect
    the bump.
- `release` **consumes** fragments (deletes them); `candidate` does **not** (they
  are shared with the final release).
- Prototype: `prototypes/changelog-format/`.

## 7. Release & release candidates

- **`momo release`** mints the version from the unreleased changes and writes the
  changelog (+ manifests). It does **not** commit and does **not** tag.
- **`momo candidate`** (alias `rc`) computes the bump exactly as `release` would,
  then appends **`-rc.N`** (dotted; scoped per target version; restarts at `1` if
  the target changes). Same file writes as `release`, but leaves fragments in place.
- **`momo tag`** creates the VCS tag for the current version — run after committing.
- `release` / `candidate` with **nothing unreleased** → error.

## 8. Tagging

- **git:** annotated (`git tag -a`, message = release title) at `HEAD`, the
  committed release.
- **jj:** lightweight (`jj tag set`) — jj's only option. **Anchor:** `@` when the
  working copy has changes, else its parent `@-`. jj defaults to `@`, which is
  empty after `jj commit` or above `main` after a merge; tagging it would publish
  a dangling commit.
- Detect **`.jj` first, else `.git`**.
- Refuse to overwrite an existing tag.
- **No VCS → `tag` errors clearly** ("no VCS detected — nothing to tag").

## 9. Monorepo model

- **Unit:** the package (a single-package repo is a monorepo with one package).
- **Discovery:** `mise tasks graph --json` (requires `monorepo_root = true`); momo's
  own manifest discovery is deferred.
- **Identity:** the ecosystem ID (`cargo:momo`, `node:@acme/ui`) with a derived
  ref/path-safe **slug** (`cargo-momo`), **config-overridable**.
- **Targeting:** cwd-relative default; `-p/--package` to override; `--all` for
  every package. `release` with no `-p` releases every package with unreleased
  changes; `tag` with no `-p` tags every package, symmetric with `release`.
- **No cascading** to dependents in v0.

## 10. Task-environment integration (`mise`)

- **`momo install [env]`** auto-detects (`mise.toml` / `.mise/`) with an explicit
  override. **No TUI in v0.**
- **No environment → graceful no-op + warning** (momo works standalone).
- Writes **one thin `momo:*` task wrapper per verb** at the repo root
  (`mise-tasks/momo/`), each pinning the running CLI via
  `#MISE tools={"github:Jarvvski/momo" = "<version>"}`. **No `mise.toml` edit.**
- Idempotent; `--update` refreshes to the running version; `--uninstall` removes
  the directory.
- Records `task_env` in config.
- See `docs/mise-task-distribution.md` (Shape B).

## 11. Version-file sync

- **Minimal v0:** a `ManifestSync` trait + **Rust `Cargo.toml`** (Node/Gradle/
  Python are later impls with no core change).
- **Auto-discover** per package; config-overridable; skip gracefully when none.
- **Format-preserving** (`toml_edit`) — touch only the version scalar, never
  reserialize.
- **Lockfiles** (`Cargo.lock`) are left to the package manager: momo **warns** they
  may be stale.
- A **release step** only; `status` / `--dry-run` never write.

## 12. Config

- **`momo.toml`** at the repo root (TOML).
- **Precedence:** `MOMO_*` env vars > `momo.toml` > built-in defaults.
- **Optional**, with **lazy creation** — each write command creates what it needs on
  first use; `momo init` writes only `momo.toml` (plus `.momo/version` when given
  `--version`).
- Keys: `recording`, `changelog` path, `changes_dir`, `tag_prefix`,
  `remote`, per-package overrides (identity/slug/manifest), `task_env`,
  `rc_format` (optional).

## 13. CLI & CI behaviour

- Change nouns take a positional `<TEXT>`, **one change per call**.
- **Global `--dry-run`**; **flags-only** (no interactive prompts).
- `candidate` (alias `rc`); change-noun aliases `a` / `f` / `b`.
- `status`: human by default (per-package, ending in `→ next: <version>`),
  `--format json`, `--version-only`, and `--check` (exits **non-zero when there is
  nothing unreleased**). *The `--check` exit-code contract must be documented.*
- `--output <path>` writes output to a file for CI.
- `--format`, `--version-only` and `--output` are honoured by the reporting verbs
  (`status`, `release`, `candidate`, `tag`); on `break`/`add`/`fix`, `init` and
  `install` they are rejected with a clear error naming the flag.
- **VCS-agnostic** — no `GITHUB_*` / `CI_*` sniffing.
- `status` / `--dry-run` are read-only; `release` / `candidate` / `tag` / `install`
  mutate.
- Prototype: `prototypes/cli/`.

## 14. Repo & module shape

- **One crate**: package `momo-cli` — a library plus a thin `momo` binary.
  (Amended from the original five-crate virtual workspace; see
  `docs/adr/0002-ship-as-a-single-crate.md`.)
- Modules:
  - `domain` — Kind/Change, bump policy, version, changelog, config, the
    Package/Workspace model, and the **ports** (`Vcs`, `TaskEnvironment`,
    `ManifestSync`). Pure: no I/O, no clap. Public.
  - `adapters::vcs` — git + jj adapters. Public.
  - `adapters::taskenv` — mise adapter. Public.
  - `adapters::manifest` — manifest adapters (`Cargo.toml`). Public.
  - `cli` — the clap tree + handlers; wires the adapters. Private, reached
    through `momo_cli::run`.

## 15. Dependencies

`clap` (derive) · `toml_edit` · `semver` · `time` · `anyhow` + `thiserror`; shell out
to `git`/`jj` (no `git2`). `inquire` only if a picker is ever needed. Dev:
`tempfile`, `assert_cmd`, `predicates`. (See `research/rust-crates.md`.)

## 16. Remaining builder notes

- Confirm the exact failure messages for: nothing unreleased, tag already exists,
  no VCS on `tag`, no task environment on `install`.
- The `--check` exit code needs a documented contract (ticket 07).
