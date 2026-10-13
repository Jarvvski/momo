<p align="center">
  <strong>momo</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0-or--later-blue"></a>
</p>

---

momo turns a project's accumulated changes into semver releases, a changelog,
and VCS tags. It is a small, standalone Rust CLI.

- **Monorepo-aware** — the *package* is the unit of versioning.
- **VCS-optional** — works with git, jj, or no VCS at all.
- **Task-environment integration** — `mise` in v0.
- **Not CI-specific** — it never reads `CI_*` or `GITHUB_*`; it only reports and
  exits, and CI decides what to do with that.

## Install

From a checkout:

```bash
cargo install --path crates/momo-cli --locked   # installs the `momo` binary
```

The workspace is driven by [mise](https://mise.jdx.dev) too:

```bash
mise install            # pin the Rust toolchain
mise run build          # cargo build --release --workspace
mise run test           # cargo test --workspace
mise run fmt            # cargo fmt --all
mise run lint           # cargo clippy --workspace --all-targets -- -D warnings
mise run check          # fmt + lint + build + test (the pre-land gate)
mise run install        # cargo install --path crates/momo-cli --locked --force
```

Run it straight from the checkout with `cargo run -p momo-cli -- <args>` or
`mise run run`.

## Quick start

In a project with no config, record a change and release it:

```console
$ momo add "Add a shiny thing"
wrote /repo/CHANGELOG.md

$ momo fix "Fix a nasty bug"
wrote /repo/CHANGELOG.md

$ momo status
Unreleased

  proj
    Added
      - Add a shiny thing
    Fixed
      - Fix a nasty bug
    → next: 0.1.0

$ momo release
released 0.1.0
wrote /repo/CHANGELOG.md
wrote /repo/.momo/version
```

`release` mints the version and writes the changelog and version file; it does
**not** commit and does **not** tag. With a git or jj repository present, commit
the result and then tag it:

```console
$ momo tag
tagged v0.1.0
```

## Command surface

| Command | Alias | Purpose |
|---|---|---|
| `momo break "<desc>"` | `b` | Record a breaking change |
| `momo add "<desc>"` | `a` | Record an added change |
| `momo fix "<desc>"` | `f` | Record a fixed change |
| `momo status` | | Show Unreleased changes and the version they would mint |
| `momo release` | | Mint a version from the Unreleased changes |
| `momo candidate` | `rc` | Cut or increment a release candidate |
| `momo tag` | | Create the VCS tag for the current version |
| `momo init` | | Scaffold `momo.toml`, optionally with a start version |
| `momo install [env]` | | Install momo into a task environment |

Each change verb takes a single positional `<TEXT>` — one change per call.

Global flags:

| Flag | Effect |
|---|---|
| `--dry-run` | Show what would happen and change nothing |
| `--format <human\|json>` | Output format for structured commands (default `human`) |
| `--version-only` | Print only the computed version |
| `--output <PATH>` | Write output to a file instead of stdout |
| `-p, --package <PKG>` | Target a package (default: the one containing the cwd) |
| `--all` | Operate on every package |

`--version-only` wins over `--format`. `--all` and `--package` cannot be
combined. `--format`, `--version-only` and `--output` are honoured by the
reporting verbs (`status`, `release`, `candidate`, `tag`); on `break`/`add`/`fix`,
`init` and `install` they are rejected, because those verbs produce no structured
report.

## Kinds and the semver rules

Every Change has exactly one **Kind**: `break`, `add`, or `fix`. The Kind of the
Unreleased changes decides the Bump, and when a release mixes Kinds the
**highest** one wins (`break` > `add` > `fix`).

| Current MAJOR | `break` | `add` | `fix` |
|---|---|---|---|
| `0` | MINOR | MINOR | PATCH |
| `≥ 1` | MAJOR | MINOR | PATCH |

At MAJOR 0 a breaking change still bumps MINOR — momo is in the zero-aware
pre-1.0 camp. The baseline is `0.0.0`, so the first release follows the same
rules: `add`/`break` → `0.1.0`, a `fix`-only release → `0.0.1`. To start from a
different version, run `momo init --version <x.y.z>` (or write `.momo/version`, or
create the tag) before the first release.

## Recording models

Set `recording` in `momo.toml` (default `changelog`):

- **`changelog`** — changes accumulate under `## Unreleased` in the changelog.
  This section is the source of Unreleased changes.
- **`fragments`** — each change is its own file,
  `.momo/changes/<kind>-<slug>.md`, holding just the entry text. In a monorepo
  the store is namespaced by package slug. Fragments are plain files, so they can
  be hand- or CI-authored. The changelog then holds only releases.

`release` **consumes** fragments (deletes them); `candidate` does **not** — a
candidate shares them with the eventual release.

## Releases and release candidates

- **`momo release`** mints the version from the Unreleased changes, writes the
  changelog and `.momo/version`, syncs the package manifest's version scalar, and
  consumes fragments. It errors when there is nothing Unreleased.
- **`momo candidate`** (alias `rc`) bumps exactly as `release` would, then
  appends `-rc.N`. It writes the same files but leaves fragments in place, and it
  advances `.momo/version` so the candidate can be tagged. The counter is scoped
  to the target version and restarts at `1` when the target changes.
- **`momo tag`** creates the VCS tag for the version `.momo/version` holds — the
  one the last `release` or `candidate` minted. Run it after committing.
  - git → annotated tag; jj → lightweight tag.
  - The VCS is detected as `.jj` first, else `.git`.
  - It refuses to overwrite an existing tag; with no VCS it errors
    (`no VCS detected — nothing to tag`).

The current version comes from the latest VCS tag under the tag prefix when a
VCS is present; with no VCS it comes from `.momo/version`. `status` and
`--dry-run` never write.

## Configuration

`momo.toml` at the repo root is optional. Write commands create it lazily on
first use; `momo init` writes it (and nothing else) up front and refuses to
overwrite an existing file, unless given `--version <x.y.z>`, which also records
that version in `.momo/version`. Read-only commands (`status`, `--dry-run`) never
create it.

`momo init` writes exactly this:

```toml
# momo configuration.
# Precedence: MOMO_* environment variables > this file > built-in defaults.
recording = "changelog"
changelog = "CHANGELOG.md"
changes_dir = ".momo/changes"
tag_prefix = "v"
tag_kind = "auto"
```

| Key | Default | Meaning |
|---|---|---|
| `recording` | `"changelog"` | `"changelog"` or `"fragments"` |
| `changelog` | `"CHANGELOG.md"` | Changelog path, relative to the package root |
| `changes_dir` | `".momo/changes"` | Fragment store, relative to the repo root |
| `tag_prefix` | `"v"` | Prefix on the tag version |
| `tag_kind` | `"auto"` | `"auto"`, `"lightweight"`, or `"annotated"` |
| `remote` | *(unset)* | Override the changelog footer's remote; supplies one with no VCS |
| `task_env` | *(unset)* | Recorded by `momo install` |
| `rc_format` | `"-rc.{n}"` | Release-candidate suffix template; `{n}` is the number |

`task_env` is written by `momo install`; `remote` and `rc_format` are optional.
`remote` overrides the remote derived from the VCS and supplies one when there is
no VCS; with neither, the changelog omits its reference-link footer. Per-package
overrides are keyed by ecosystem identity:

```toml
[packages."cargo:momo"]
identity = "cargo:renamed"   # override the identity
slug = "alpha"               # override the ref/path-safe slug
manifest = "crates/momo/Cargo.toml"  # override the discovered manifest
```

### Precedence

Configuration is layered, lowest to highest:

**`MOMO_*` environment variables > `momo.toml` > built-in defaults.**

Each layer sets only the keys it actually sets, so a sparse file leaves
everything else at its default.

The environment variables are `MOMO_RECORDING`, `MOMO_CHANGELOG`,
`MOMO_CHANGES_DIR`, `MOMO_TAG_PREFIX`, `MOMO_TAG_KIND`, `MOMO_REMOTE`,
`MOMO_TASK_ENV`, and `MOMO_RC_FORMAT`. Unrelated `MOMO_*` variables are ignored.

```console
$ MOMO_TAG_PREFIX="rel-" momo status
```

## Monorepos

A single-package repo is a monorepo with one package. A repo with a mise project
graph is a monorepo of packages; discovery runs `mise tasks graph --json`, which
requires `monorepo_root = true` in the mise configuration.

Discovery is attempted whenever momo detects a mise config (`mise.toml` or
`.mise/`) at or above the current directory, and momo tells the outcomes apart by
reading that configuration — never by parsing mise's error message, which is not
a stable interface:

- **Not a monorepo** (no `monorepo_root = true`): the graph's refusal is
  expected, so momo silently treats the repo as a single package named after its
  directory.
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
- `release` with no `-p` releases every package that has Unreleased changes.

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

## Task-environment integration (mise)

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

So `mise run momo:status` runs the pinned CLI — once that version is published
as a GitHub release. `install` is idempotent; a wrapper that pins a different
version is left alone and reported as stale, and `--update` refreshes it.
`--uninstall` removes what install wrote and clears the recorded `task_env`.
With no task environment and no explicit name, `install` warns and exits zero.

Release assets are built for **Linux and macOS only** — four Unix targets
(`x86_64`/`aarch64` × `apple-darwin`/`unknown-linux-gnu`). There is no Windows
build, so the pinned wrappers resolve only on those platforms.

## CI

momo is designed to be driven by a script. `status` reports structured data and
can gate on whether there is anything to release.

- `momo status --format json` emits a stable JSON document (see below).
- `momo status --version-only` prints just the computed version.
- `momo status --output <PATH>` writes the same content to a file (stdout is
  left empty).
- `momo release --version-only`, `momo candidate --version-only`,
  `momo tag --format json` and friends work the same way.

### The `status --check` exit-code contract

`momo status --check` **exits non-zero when there is nothing Unreleased**, and
zero when there is something. It still renders the requested report (human or
JSON) to stdout; the error goes to stderr. Use it to skip the rest of a release
job when there is nothing to mint:

```console
$ momo status --check
Unreleased

  proj
    Added
      - Add a shiny thing
    → next: 0.1.0
$ echo $?
0

$ momo release >/dev/null
$ momo status --check
Unreleased

  proj
    → next: 0.1.0
error: nothing unreleased
$ echo $?
1
```

Note the inverted sense: `--check` is a "is there anything to do?" gate, not a
"is the tree clean?" check. `0` means *there is something to release*.

### JSON shapes

`status` (`--format json`):

```json
{
  "unreleased": true,
  "packages": [
    {
      "name": "momo",
      "current": "0.0.0",
      "next": "0.1.0",
      "changes": [
        {
          "kind": "add",
          "text": "Add a shiny thing"
        }
      ]
    }
  ]
}
```

`unreleased` is `true` when at least one package has an Unreleased Change — the
same gate as `--check`. `next` is the version a release would mint, or the
current version when nothing is Unreleased (then `changes` is empty).

`release` and `candidate` (`--format json`):

```json
{
  "version": "0.1.0",
  "changelog": "/repo/CHANGELOG.md",
  "version_file": "/repo/.momo/version",
  "consumed": [
    "/repo/.momo/changes/add-a-shiny-thing.md"
  ],
  "dry_run": false
}
```

`version_file` is the version file the run wrote; `consumed` lists the fragments
the run deleted (empty for `candidate`). Under `--dry-run` the paths are what the
run *would* have written.

`tag` (`--format json`):

```json
{
  "tag": "v0.1.0",
  "version": "0.1.0",
  "dry_run": false
}
```

## Not in v0

- Task environments beyond `mise` (gradle, bun).
- Manifest sync beyond Rust: `release` rewrites the version scalar in a
  `Cargo.toml`, format-preserving, and warns when a neighbouring lockfile may be
  stale — but never rewrites the lockfile and never runs `cargo`. Node, Gradle
  and Python are later adapters behind the same port.
- Monorepo discovery independent of `mise` — momo reads mise's project graph.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
