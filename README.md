<p align="center"> <img src="https://raw.githubusercontent.com/Jarvvski/momo/main/assets/momo.png" alt="momo" width="200"> </p>

<h1 align="center">momo</h1>

<p align="center"> <a href="https://github.com/Jarvvski/momo/blob/main/LICENSE-MIT"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue"></a> <a href="https://github.com/Jarvvski/momo/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Jarvvski/momo/actions/workflows/ci.yml/badge.svg"></a> </p>

**momo — the release half of your mise workflow.**

momo is the release step for mise-first polyglot monorepos. Pair it with mise and it discovers your packages from `mise tasks graph`, installs its own `momo:*` tasks with `momo install`, and turns recorded changes into semver versions, a changelog and VCS tags — `mise use github:Jarvvski/momo`, then `momo add "…"` and `momo release`. It is VCS-optional (git, jj, or none) and CI-agnostic: it never reads `CI_*`/`GITHUB_*`, never commits, and never pushes — it reports, and your CI decides what to do about it.

Record a change with one of three verbs — `break`, `add`, `fix` — and momo works out the version, writes the changelog, and tags the release.

- **Monorepo-aware** — the *package* is the unit of versioning.
- **VCS-optional** — git, jj, or no VCS at all.
- **Task-environment integration** — `mise`.
- **Not CI-specific** — it never reads `CI_*`/`GITHUB_*`; it reports, and CI decides what to do about it.

## Install

momo is Unix-only: it builds on Linux and macOS, and there is no Windows build.

Let a task environment manage it:

```bash
mise use github:Jarvvski/momo
```

Or with the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/Jarvvski/momo/main/install.sh | sh
```

The installer picks the release for your OS and architecture, verifies it against the published checksums, and drops the `momo` binary in `$HOME/.local/bin` (override with `--to <dir>`, pin a version with `--version <x.y.z>`).

The release workflow publishes a GitHub build-provenance attestation for each archive, tying it to the commit and workflow that built it. With the [GitHub CLI](https://cli.github.com) installed and authenticated, you can check a downloaded archive against this repository:

```bash
gh attestation verify momo-<version>-<target>.tar.gz --repo Jarvvski/momo
```

The installer runs the same check when `gh` is available, and always verifies the published checksum.

Or from crates.io:

```bash
cargo install momo-cli          # from crates.io; installs the `momo` binary
```

From a checkout:

```bash
cargo install --path . --locked
```

## Quick start

In a git repo with a `CHANGELOG.md`, momo needs no configuration. The path is:

```console
$ momo add "Add a shiny thing"        # 1. record a change
$ momo fix "Fix a nasty bug"

$ momo status                         # 2. inspect: current, next, and the changes between
Unreleased

  proj
    Added
      - Add a shiny thing
    Fixed
      - Fix a nasty bug
    current: —
    → next: 0.1.0

$ momo release                        # 3. mint the version and write the changelog
released 0.1.0

$ git commit -am "release 0.1.0"      # 4. commit the release
$ momo tag                            # 5. tag it
tagged v0.1.0
```

`release` mints the version and writes the changelog and version record. It does **not** commit and does **not** tag — you commit the result, then `momo tag` creates the VCS tag.

`momo init` is optional: it scaffolds `momo.toml` when you want to change a default, and write commands create it lazily otherwise.

## Kinds and the semver rules

Every change has exactly one **Kind**, and the Kind of the unreleased changes decides the bump. When a release mixes Kinds the **highest** wins (`break` > `add` > `fix`).

| Current MAJOR | `break` | `add` | `fix` |
|---|---|---|---|
| `0` | MINOR | MINOR | PATCH |
| `≥ 1` | MAJOR | MINOR | PATCH |

At MAJOR 0 a breaking change still bumps MINOR — momo is in the zero-aware pre-1.0 camp. The baseline is `0.0.0`, so the first release follows the same rules. To start elsewhere, run `momo init --version <x.y.z>`.

## Opinions

momo is opinionated where it matters and configurable where reality varies. The semver rules above are **asserted** — they are the product, tested and not configurable. Where a project's changelog lives, what its tags look like, and how changes are recorded are **defaulted**, with keys to override them.

Two deviations from convention are deliberate and owned out loud: a `fix` at `0.0.x` bumps PATCH (Cargo treats every `0.0.z` as breaking), and the breaking-change section is `Breaking` where Keep a Changelog says `Changed`. See **[Opinions](https://github.com/Jarvvski/momo/blob/main/docs/opinions.md)** for the full asserted-versus-defaulted set, and [ADR 0005](https://github.com/Jarvvski/momo/blob/main/docs/adr/0005-zero-aware-bump-policy.md) for the zero-aware policy.

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
| `momo version` | | Print the momo version |

Global flags:

| Flag | Effect |
|---|---|
| `--dry-run` | Show what would happen and change nothing |
| `--format <human\|json>` | Output format for reporting commands |
| `--json` | Shorthand for `--format json` |
| `--version-only` | Print only the computed version |
| `--output <PATH>` | Write output to a file instead of stdout |
| `-p, --package <PKG>` | Target a package (default: the one containing the cwd) |
| `--all` | Operate on every package |

## Documentation

- **[Opinions](https://github.com/Jarvvski/momo/blob/main/docs/opinions.md)** — what momo asserts, what it defaults, and the deviations it owns.
- **[Usage](https://github.com/Jarvvski/momo/blob/main/docs/usage.md)** — recording models, releases and release candidates, tagging, monorepos, task-environment install.
- **[Migrating](https://github.com/Jarvvski/momo/blob/main/docs/migrating.md)** — adopting momo on a project that already has tags, a changelog and releases.
- **[Configuration](https://github.com/Jarvvski/momo/blob/main/docs/configuration.md)** — every `momo.toml` key, the precedence order, and the `MOMO_*` environment variables.
- **[CI](https://github.com/Jarvvski/momo/blob/main/docs/ci.md)** — the JSON shapes, `status --check`, and `--output`.
- **[Contributing](https://github.com/Jarvvski/momo/blob/main/docs/contributing.md)** — building, testing, and the landing flow.

## License

Dual-licensed under either of [MIT](https://github.com/Jarvvski/momo/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/Jarvvski/momo/blob/main/LICENSE-APACHE), at your option. Contributions are accepted under the same terms.
