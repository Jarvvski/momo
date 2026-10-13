# momo v0 — wayfinder map

Type: map

**Status: complete.** Every ticket is resolved and `.scratch/momo-v0/spec.md` is
locked — nothing left to decide before implementation.

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
  - **Verb surface:** `break|add|fix`, `status`, `release`, `candidate` (alias
    `rc`), `tag`, `init`, `install <taskenv>`, and a global `--dry-run`. (The
    `candidate` rename and the short aliases were settled in ticket 11.)
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
- [Changelog format & entry shape](issues/03-changelog-format.md): `Breaking` /
  `Added` / `Fixed` sections; `## [x.y.z] - YYYY-MM-DD` headings; `Unreleased` only
  in changelog mode; entries `- text (#issue)`; RCs get headings; **reference
  links** derived from the VCS remote (config-overridable, omitted when unknown).
  Prototype: `prototypes/changelog-format/`.
- [Release-candidate semantics](issues/05-release-candidates.md): dotted
  `1.2.0-rc.1`, increment per target version (restart on target change); RCs
  **share** the unreleased fragments (consumed at the final release); `rc` bumps
  like `release`, then appends `-rc.N`.
- [CI / non-interactive output & dry-run shape](issues/07-ci-output.md): human
  output by default, `--format json` + `--version-only`; `--output <path>` for CI;
  `status --check` exits non-zero when nothing is unreleased (**must be
  documented**); momo stays VCS-agnostic.
- [Repo & module shape](issues/10-repo-shape.md): **virtual-workspace** split —
  `momo-core` (domain + `Vcs`/`TaskEnvironment` ports), `momo-vcs` (git/jj),
  `momo-taskenv` (mise), `momo-cli` (binary); ports in core, adapters separate,
  wired by the CLI.
- [CLI ergonomics](issues/11-cli-ergonomics.md): positional `<TEXT>` for the change
  nouns (one per call); global `--dry-run`; flags-only; **`candidate`** (alias
  `rc`) replaces `release --rc`; short aliases `a`/`f`/`b`; `status` prints
  per-package `→ next: <version>`. Prototype: `prototypes/cli/`.
- [VCS-optional operation](issues/17-vcs-optional.md): without a VCS the version
  source is `.momo/version`; `status`/`release`/`candidate` still work, `tag`
  errors clearly; the VCS becomes authoritative once present.
- [Version-file detection & sync](issues/18-version-file-sync.md): minimal v0 — a
  `ManifestSync` trait + **Rust `Cargo.toml`**; auto-discover per package,
  config-overridable; format-preserving writes (`toml_edit`); lockfiles left to the
  package manager (warn); a release step only.
- [Assemble the v0 spec](issues/15-assemble-spec.md): consolidated every decision
  into `spec.md` (§1–16); glossary consistent; map complete.

## Not yet specified

- Minor behaviours for the assembly ticket (15) to pin down: the
  multiple-kinds-in-one-release rule (highest kind wins?); `release` / `candidate`
  with nothing unreleased (error vs no-op); and the exact failure messages.

*(Cleared from fog as decisions landed: the "Keep a Changelog vs bespoke" heading
question → ticket 03; how `install` records its environment → tickets 08/09; the
changelog-vs-tags disagreement → ticket 17, "the VCS wins".)*

## Out of scope

- Third-party plugin/extension loading for task environments (gradle/bun are
  future *implementations* of the internal trait, not an external API).
- The gradle and bun/npm task-environment implementations themselves.
- Publishing beyond tag creation (registry publishing, CI workflow authoring).
- Any momo scope beyond versioning / changelog / tagging.
