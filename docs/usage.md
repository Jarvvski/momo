# Usage

Deeper notes on how momo behaves. For the command list and the semver rules, see
the [README](../README.md).

- [Recording models](#recording-models)
- [Releases and release candidates](#releases-and-release-candidates)
- [Tagging](#tagging)
- [Monorepos](#monorepos)
- [Task-environment integration](#task-environment-integration)
- [Where the version comes from](#where-the-version-comes-from)
- [Not yet supported](#not-yet-supported)

## Recording models

Set `recording` in `momo.toml` (default `changelog`):

- **`changelog`** — changes accumulate under `## Unreleased` in the changelog,
  and that section is the source of unreleased changes. momo edits only the spans
  it owns — the Unreleased body, its own footer links and the release headings it
  inserts — so the rest of the document (comments, `*` bullets, `### Changed`
  sections, user footer definitions, CRLF) passes through untouched.
- **`fragments`** — each change is its own file,
  `.momo/changes/<kind>-<slug>.md`, holding just the entry text. Fragments are
  plain files, so they can be hand- or CI-authored. The changelog then holds only
  releases.

In a monorepo the fragment store is namespaced by package slug.

`release` **consumes** fragments (deletes them). `candidate` does **not** — a
candidate shares them with the eventual release.

## Releases and release candidates

- **`momo release`** mints the version from the unreleased changes, writes the
  changelog and `.momo/version`, syncs the package manifest's version scalar, and
  consumes fragments. It errors when there is nothing unreleased.
- **`momo candidate`** (alias `rc`) bumps exactly as `release` would, then appends
  `-rc.N`. It writes the same files but leaves fragments in place, and it advances
  `.momo/version` so the candidate can be tagged. The counter is scoped to the
  target version and restarts at `1` when the target changes.
- **`momo tag`** creates the VCS tag for the version `.momo/version` holds — the
  one the last `release` or `candidate` minted. It refuses a ledger older than the
  newest tag, because that version is behind the release already tagged.

### Cargo workspaces

A member with `version.workspace = true` takes its version from the root's
`[workspace.package].version` — one scalar shared by every member. `release`
resolves the root by walking up to the manifest declaring `[workspace]` and
rewrites that scalar there, leaving the member's own manifest alone. Because the
scalar is shared, `--all` writes it only when every affected member resolves to
the same target; otherwise the whole run fails rather than letting the last
writer win. A root that is both a package and a workspace keeps its own
`[package].version` — that is the root crate's version, distinct from the default
its members inherit.

Neither `release` nor `candidate` commits or tags. The ceremony is split so the
tag lands on the commit that contains the changelog:

```console
$ momo release
$ git commit -am "release 0.1.0"     # or: jj commit
$ momo tag
```

## Tagging

- git → an **annotated** tag on `HEAD`, the committed release.
- jj → a **lightweight** tag (all jj supports), anchored to `@-`, the parent of
  the working-copy commit. The documented flow is release → commit → tag, so the
  release is the commit `@-` names — after `jj commit`, or after a pull-request
  merge where `@` sits above `main`.
- `--rev <REV>` overrides the anchor: a commit-ish for git, a revset resolving
  to exactly one commit for jj. Tag an **uncommitted** release (the files still
  in `@`) with `--rev @`. The anchor is never inferred from the working copy.
- The VCS is detected as `.jj` first, else `.git`.
- An existing tag is never overwritten.
- With no VCS, `tag` errors: `no VCS detected — nothing to tag`.

## Monorepos

A single-package repo is a monorepo with one package. A repo with a mise project
graph is a monorepo of packages; discovery runs `mise tasks graph --json`, which
requires `monorepo_root = true` in the mise configuration.

Discovery is attempted whenever momo detects a mise config (`mise.toml` or
`.mise/`) at or above the current directory. momo tells the outcomes apart by
reading that configuration — never by parsing mise's error message, which is not
a stable interface:

- **Not a monorepo** (no `monorepo_root = true`): the graph's refusal is expected,
  so momo silently treats the repo as a single package named after its directory.
- **A monorepo whose graph cannot be read** (`monorepo_root = true`, but the
  command failed): momo warns and falls back to that same single package.
- **`-p`/`--all` on a monorepo whose graph cannot be read** fails outright: the
  request for packages cannot be honoured.

A package carries its ecosystem identity (`cargo:momo`, `node:@acme/ui`) and a
derived, ref/path-safe slug (`cargo-momo`). The slug namespaces tags
(`cargo-momo-v0.1.0`), the per-package changelog, and the fragment store.

Targeting:

- Recording verbs (`break`/`add`/`fix`) default to the package containing the
  current directory.
- Aggregate verbs (`status`, `release`, `tag`) default to every package.
- `-p, --package <PKG>` scopes a command to one package; `<PKG>` matches its
  identity, slug, or bare name (`momo` matches `cargo:momo`).
- `--all` covers every package.
- `release` with no `-p` releases every package that has unreleased changes.

```console
$ momo -p cargo:a add "Add a thing"
$ momo status
Unreleased

  cargo:a
    Added
      - Add a thing
    → next: 0.1.0

  cargo:b
    → next: 0.0.0

$ momo --all release
```

There is no cascading: releasing a package does not bump its dependents.

## Task-environment integration

`momo install` auto-detects a mise project at or above the current directory
(`mise.toml` or `.mise/`), or takes an explicit environment name
(`momo install mise`). It writes one thin `momo:*` task wrapper per verb under
`mise-tasks/momo/` and edits no `mise.toml`:

```console
$ momo install
installed momo 0.1.0: 9 task(s) in /repo/mise-tasks/momo

$ mise tasks ls
momo:add        Run momo add
momo:break      Run momo break
momo:candidate  Run momo candidate
momo:fix        Run momo fix
momo:init       Run momo init
momo:install    Run momo install
momo:release    Run momo release
momo:status     Run momo status
momo:tag        Run momo tag
```

Each wrapper is self-contained and pins the running CLI in its `#MISE` header:

```bash
#!/usr/bin/env bash
# generated by momo 0.1.0 - refresh with: momo install --update
#MISE description="Run momo status"
#MISE tools={ "github:Jarvvski/momo" = "0.1.0" }
momo status "$@"
```

So `mise run momo:status` runs the pinned CLI — once that version is published as
a GitHub release. `install` is idempotent; a wrapper pinning a different version
is left alone and reported as stale, and `--update` refreshes it. `--uninstall`
removes what install wrote and clears the recorded `task_env`. With no task
environment and no explicit name, `install` warns and exits zero.

Release assets are built for **Linux and macOS only** — four Unix targets
(`x86_64`/`aarch64` × `apple-darwin`/`unknown-linux-gnu`). There is no Windows
build, so the pinned wrappers resolve only on those platforms.

## Where the version comes from

A package's version has one authority per question:

- **`status` and `release`** read the **latest VCS tag** under the tag prefix.
  With no VCS they fall back to `.momo/version`, so a project outside a repository
  still versions. The VCS becomes authoritative as soon as it exists.
- **`tag`** reads **`.momo/version`**, the pending-release ledger. The version it
  must tag is by definition one no tag records yet, so the tags cannot supply it.

`.momo/version` is written by `release`/`candidate` as the record of the last
*minted* version — a Release or a Release Candidate. It is **not** a version
source for `status`/`release`; those never read it while a VCS is present.

A release target is always recomputed from the **last release** — the newest
stable tag — whether or not a Release Candidate was cut. If a change recorded
after a Candidate moves the target, `release`/`candidate` refuse and ask for a
fresh Candidate rather than shipping a version the bump policy says is too low:

```console
$ momo add "Add a thing"
$ momo candidate            # 1.1.0-rc.1
$ momo break "Break a thing"
$ momo release
error: the cut candidate `1.1.0-rc.1` is stale: the changes now imply `2.0.0`; cut a new candidate for `2.0.0`
```

A package that has never released has **no** version, not version `0.0.0`:
`status --format json` reports `"current": null`, and a genuine `0.0.0` release is
reported as `"current": "0.0.0"`. `status` and `--dry-run` never write.

## Not yet supported

- Task environments beyond `mise` (gradle, bun).
- Manifest sync beyond Rust: `release` rewrites the version scalar in a
  `Cargo.toml`, format-preserving, and warns when a neighbouring lockfile may be
  stale — but never rewrites the lockfile and never runs `cargo`. Node, Gradle and
  Python are later adapters behind the same port.
- Monorepo discovery independent of `mise` — momo reads mise's project graph.
