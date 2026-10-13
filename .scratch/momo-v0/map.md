# momo v0 — wayfinder map

Type: map

## Destination

A locked **v0 spec for `momo`**: a short, standalone Rust CLI that manages a
project's versioning through three change kinds (`break`/`add`/`fix`), keeps a
changelog, mints releases and release candidates, creates git/jj tags, offers a
dry mode for CI, and installs its integration into a task environment (`mise`
first; `gradle`/`bun` later). The spec lives at `.scratch/momo-v0/spec.md` and
is assembled as the decision tickets below resolve. The way is clear when a
builder can implement momo v0 with nothing left to decide.

## Notes

- **Effort:** define momo's scope. The repo is a bare stub (`src/main.rs` only
  prints its version); this is its first map.
- **Skills:** `grilling` + `domain-modeling` for decision tickets; `research`
  for research tickets; `prototype` for prototype tickets.
- **Settled during charting (no ticket):**
  - The tool **is `momo`** — this repo's binary.
  - **v0 extensibility:** an internal `TaskEnvironment` trait with `mise` as its
    only implementation; no third-party plugin loading.
  - **Verb surface:** `break|add|fix`, `status`, `release [--rc]`, `rc`, `tag`,
    `init`, `install <taskenv>`, and a global `--dry-run`.
  - **Recording model:** support **both** a single changelog file and fragment
    files (each suits different setups).
- **Scope grew during resolution** (ticket 01's answer): **monorepo-aware**
  versioning (16), **VCS-optional** operation (17), and (stretch) **manifest
  version-sync** (18) are now in scope — v0 is no longer "one version per repo".
  Monorepo-awareness is **owner-confirmed** for v0.
- **Standing preference:** "short but sweet" — the smallest thing that works;
  defer gradle/bun and plugin loading.
- **Prior art in-repo:** `docs/mise-task-distribution.md` — the playbook for
  shipping momo's tasks + a pinned CLI into a consumer repo. Ticket 09 builds on
  it.
- **Landing:** jj only; agents never push (`AGENTS.md`).

## Decisions so far

- [Rust crate & dependency landscape](issues/14-crate-landscape.md): `clap`
  (derive), `inquire` (trimmed), `toml_edit`, `semver` (+ own bump policy),
  `time`, shell out to `git`/`jj`, `anyhow` + `thiserror`; **no `git2`**.
- [jj & git tag mechanics](issues/12-jj-git-tags.md): `jj tag` is native but
  **lightweight-only**; detect `.jj` first; jj → `jj tag set`, git → `git tag -a`;
  annotated/signed in jj via `git -C "$(jj git root)"` + `jj git import`; both
  refuse to overwrite by default.
- [Precedents: existing versioning/changelog tools](issues/13-precedents.md):
  momo's zero-aware pre-1.0 rule matches release-plz; dotted `-rc.N` is the norm;
  changesets' `pre.json` is the RC share-then-strip model; `v`-prefix is
  near-universal; "leave the tree dirty" is the majority default; Keep a Changelog
  is the baseline.
- [Recording model: single changelog vs fragments, and how momo chooses](issues/02-recording-model.md):
  explicit `recording` config key (default `changelog`); fragments live at
  `.momo/changes/<kind>-<slug>.md` (Kind in the filename, no front-matter);
  `release` deletes consumed fragments.
- [Version source of truth & first-release bootstrap](issues/01-version-source.md):
  source is the **latest VCS tag** (VCS-optional → 17); **monorepo-aware** (model
  → 16); bootstrap at `0.0.0` with normal bumps (`init` may set a start version);
  manifest version-sync is a stretch (→ 18).
- [Task-runner workspace / monorepo models](issues/19-taskenv-workspaces.md):
  use **`mise tasks graph --json`** as the monorepo seam (ecosystem-namespaced
  project IDs + edges, no package manager needed) — but it carries **no versions**;
  Gradle enumerates via the Tooling API; Node via `workspaces` /
  `pnpm-workspace.yaml`.
- [Version-file formats & safe writing](issues/20-version-file-formats.md):
  per-ecosystem manifest locations + safe writes (`toml_edit`, `jsonc-parser`,
  line-oriented properties, XML splice); never reserialize; watch inheritance,
  dynamic versions, duplicates, and lockfiles.
- [Monorepo / workspace model](issues/16-monorepo-model.md): the **package** is the
  unit of versioning (per-package version/changelog/tag); discovery via
  `mise tasks graph --json`; fragments at `.momo/changes/<package>/…`; identity is
  the ecosystem ID + slug (config-overridable); tags `<slug>-v<version>`; targeting
  `cwd` / `-p` / `--all`; no cascade in v0.
- [Release side-effects & tag format](issues/06-release-side-effects.md):
  `release` writes the changelog + manifests but does **not** commit or tag;
  `tag` is a separate step run after committing; git tags are **annotated**, jj
  tags lightweight; release drives manifest sync (18).
- [momo config file & keys](issues/04-config-file.md): `momo.toml` at the repo root
  (TOML); precedence flags > `MOMO_*` env > file > defaults; config is optional
  with **lazy creation** (write commands create what they need; read-only ones
  don't); `init` writes only `momo.toml`.
- [Task-environment selection UX](issues/08-taskenv-selection.md): auto-detect
  (`mise.toml`) with an explicit override, **no TUI in v0**; no environment found →
  graceful **no-op + warning** (momo works standalone); the choice is stored as
  `task_env` in config.
- [mise integration content](issues/09-mise-integration.md): one thin task wrapper
  per verb (`momo:*`) at the repo root, delegating package routing to momo; each
  pins the running CLI via `#MISE tools={…}`; idempotent, `--update`, `uninstall`;
  **no `mise.toml` edit** (playbook Shape B).

## Not yet specified

- The semver edge-case table (`0.0.x`; multiple kinds in one release; whether an
  `add` can be breaking pre-1.0) — sharpens once 05 resolves.
- Failure modes and their messages (release with nothing unreleased, tag already
  exists, dirty tree) — graduates after 06.
- Whether momo owns a "Keep a Changelog" heading set or a bespoke one (ticket 03).
- How `momo install` records which environment it installed into, for idempotent
  re-install — graduates after 08/09.
- The **changelog-vs-tags disagreement** rule — surfaced by ticket 01; graduates
  once the VCS-optional source (17) is settled.

## Out of scope

- Third-party plugin/extension loading for task environments (gradle/bun are
  future *implementations* of the internal trait, not an external API).
- The gradle and bun/npm task-environment implementations themselves.
- Publishing beyond tag creation (registry publishing, CI workflow authoring).
- Any momo scope beyond versioning / changelog / tagging.
