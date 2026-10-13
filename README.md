<p align="center">
  <img src="https://raw.githubusercontent.com/Jarvvski/momo/main/assets/momo.png" alt="momo" width="200">
</p>

<h1 align="center">momo</h1>

<p align="center">
  <a href="https://github.com/Jarvvski/momo/blob/main/LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue"></a>
  <a href="https://github.com/Jarvvski/momo/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Jarvvski/momo/actions/workflows/ci.yml/badge.svg"></a>
</p>

momo turns a project's accumulated changes into semver releases, a changelog and
VCS tags. It is a small, standalone Rust CLI.

Record a change with one of three verbs — `break`, `add`, `fix` — and momo works
out the version, writes the changelog, and tags the release.

- **Monorepo-aware** — the *package* is the unit of versioning.
- **VCS-optional** — git, jj, or no VCS at all.
- **Task-environment integration** — `mise`.
- **Not CI-specific** — it never reads `CI_*`/`GITHUB_*`; it reports, and CI
  decides what to do about it.

## Install

```bash
cargo install momo-cli          # from crates.io; installs the `momo` binary
```

Or let a task environment manage it:

```bash
mise use github:Jarvvski/momo@0.2.1
```

From a checkout:

```bash
cargo install --path . --locked
```

## Quick start

```console
$ momo init                        # optional — commands create config lazily
$ momo add "Add a shiny thing"
$ momo fix "Fix a nasty bug"

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

$ momo tag                         # after committing
tagged v0.1.0
```

`release` mints the version and writes the changelog and version record. It does
**not** commit and does **not** tag — you commit the result, then `momo tag`
creates the VCS tag.

## Kinds and the semver rules

Every change has exactly one **Kind**, and the Kind of the unreleased changes
decides the bump. When a release mixes Kinds the **highest** wins
(`break` > `add` > `fix`).

| Current MAJOR | `break` | `add` | `fix` |
|---|---|---|---|
| `0` | MINOR | MINOR | PATCH |
| `≥ 1` | MAJOR | MINOR | PATCH |

At MAJOR 0 a breaking change still bumps MINOR — momo is in the zero-aware
pre-1.0 camp. The baseline is `0.0.0`, so the first release follows the same
rules. To start elsewhere, run `momo init --version <x.y.z>`.

## Commands

| Command | Alias | Purpose |
|---|---|---|
| `momo break "<desc>"` | `b` | Record a breaking change |
| `momo add "<desc>"` | `a` | Record an added change |
| `momo fix "<desc>"` | `f` | Record a fixed change |
| `momo status` | | Show unreleased changes and the version they would mint |
| `momo release` | | Mint a version from the unreleased changes |
| `momo candidate` | `rc` | Cut or increment a release candidate |
| `momo tag` | | Create the VCS tag for the current version |
| `momo init` | | Scaffold `momo.toml`, optionally with a start version |
| `momo install [env]` | | Install momo into a task environment |

Global flags:

| Flag | Effect |
|---|---|
| `--dry-run` | Show what would happen and change nothing |
| `--format <human\|json>` | Output format for reporting commands |
| `--version-only` | Print only the computed version |
| `--output <PATH>` | Write output to a file instead of stdout |
| `-p, --package <PKG>` | Target a package (default: the one containing the cwd) |
| `--all` | Operate on every package |

## Documentation

- **[Usage](https://github.com/Jarvvski/momo/blob/main/docs/usage.md)** — recording
  models, releases and release candidates, tagging, monorepos, task-environment
  install.
- **[Configuration](https://github.com/Jarvvski/momo/blob/main/docs/configuration.md)**
  — every `momo.toml` key, the precedence order, and the `MOMO_*` environment
  variables.
- **[CI](https://github.com/Jarvvski/momo/blob/main/docs/ci.md)** — the JSON
  shapes, `status --check`, and `--output`.
- **[Contributing](https://github.com/Jarvvski/momo/blob/main/docs/contributing.md)**
  — building, testing, and the landing flow.

## License

GPL-3.0-or-later. See
[`LICENSE`](https://github.com/Jarvvski/momo/blob/main/LICENSE).
