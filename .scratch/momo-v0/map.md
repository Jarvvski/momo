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

## Not yet specified

- The semver edge-case table (`0.0.x`; multiple kinds in one release; whether an
  `add` can be breaking pre-1.0) — sharpens once 01 and 05 resolve.
- Failure modes and their messages (release with nothing unreleased, tag already
  exists, dirty tree, no VCS detected) — graduates after 06.
- Whether momo owns a "Keep a Changelog" heading set or a bespoke one.
- How `momo install` records which environment it installed into, for idempotent
  re-install — graduates after 08/09.

## Out of scope

- Third-party plugin/extension loading for task environments (gradle/bun are
  future *implementations* of the internal trait, not an external API).
- The gradle and bun/npm task-environment implementations themselves.
- Publishing beyond tag creation (registry publishing, CI workflow authoring).
- Any momo scope beyond versioning / changelog / tagging.
