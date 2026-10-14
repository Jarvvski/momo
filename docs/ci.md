# CI

momo is designed to be driven by a script. It never reads `CI_*` or `GITHUB_*` — it reports and exits, and CI decides what to do about it.

The reporting verbs (`status`, `release`, `candidate`, `tag`, `notes`) honour the output flags below. `break`/`add`/`fix`, `init` and `install` reject them, because those verbs produce no structured report.

| Flag | Effect |
|---|---|
| `--format <human\|json>` | Rendering (default `human`) |
| `--version-only` | Print only the computed version; wins over `--format` |
| `--output <PATH>` | Write the same content to a file; stdout is left empty |

## The `status --check` exit-code contract

`momo status --check` **exits non-zero when there is nothing unreleased**, and zero when there is. It still renders the requested report (human or JSON) to stdout; the error goes to stderr. Use it to skip the rest of a release job when there is nothing to mint:

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

Note the inverted sense: `--check` asks *"is there anything to do?"*, not *"is the tree clean?"*. `0` means there **is** something to release.

## JSON shapes

Every reporting verb emits **one** JSON document per run, whatever the number of packages. The multi-package report is an envelope: a run-wide flag plus a top-level `packages` array, which is always present — even for a single-package repo. This means `--output <PATH>` writes the whole report once, rather than truncating to the last package.

### `status --format json`

```json
{
  "unreleased": true,
  "packages": [
    {
      "name": "momo",
      "current": null,
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

`unreleased` is `true` when at least one package has an unreleased change — the same gate as `--check`. `current` is the package's last release (or cut candidate), or `null` when it has never released. `next` is the version a release would mint, or the current version when nothing is unreleased (then `changes` is empty). `packages` is always an array, even for a single-package repo.

### `release` and `candidate` (`--format json`)

```json
{
  "dry_run": false,
  "packages": [
    {
      "name": "momo",
      "version": "0.1.0",
      "changelog": "/repo/CHANGELOG.md",
      "version_file": "/repo/.momo/version",
      "manifest": "/repo/Cargo.toml",
      "consumed": [
        "/repo/.momo/changes/add-a-shiny-thing.md"
      ]
    }
  ]
}
```

`dry_run` is the run-wide flag. Each entry in `packages` is one released package, named by its `name` (the same identity `status` reports). `version_file` is the version file the run wrote. `manifest` is the ecosystem manifest whose version scalar the run synced, or `null` when the package has none. `consumed` lists the fragments the run deleted (empty for `candidate`). Under `--dry-run` the paths are what the run *would* have written — including the manifest, which the dry run parses but does not write.

### `tag` (`--format json`)

```json
{
  "dry_run": false,
  "packages": [
    {
      "name": "momo",
      "tag": "v0.1.0",
      "version": "0.1.0"
    }
  ]
}
```

`tag` is the package's namespaced tag name, prefix included; `name` identifies which package it names.

### `notes` (`--format json`)

```json
{
  "packages": [
    {
      "name": "momo",
      "version": "1.4.2",
      "notes": "### Added\n\n- Add a shiny thing"
    }
  ]
}
```

`notes` is the Changelog section for the described Version, **verbatim** — the same lines the changelog holds under `## [1.4.2]`, including content momo cannot interpret. There is no `dry_run`: `notes` reads only.

The Version is `--version` when given, else the pending-release ledger, so `momo notes` straight after a `release` describes what was just minted without naming it twice.

The human rendering is the section body alone when one Package is reported, and a `## <package> <version>` header per Package when several are — so a single-package `--output` is a release body ready to publish:

```console
$ momo notes --output notes.md
$ cat notes.md
### Added

- Add a shiny thing
```

## A release job

The shape, independent of the CI system:

```bash
if ! momo status --check >/dev/null 2>&1; then
  echo "nothing to release"; exit 0
fi
version="$(momo release --version-only)"   # mints, writes, prints the version
git add --all
git commit -m "release $version"
momo tag
git push --follow-tags
momo notes --output notes.md
gh release create "v$version" --title "v$version" --notes-file notes.md
```

momo writes the files and creates the tag; committing, pushing and publishing are the job's own steps. `release --version-only` mints and writes exactly as `release` does — it only narrows what is printed — so the version names the commit it is about to land on.

`momo notes` prints the section `release` just wrote, so the release body and the changelog never drift.

## A GitHub Actions workflow

```yaml
name: release

on:
  push:
    branches: [main]
  workflow_dispatch:

# One release at a time. Two runs racing would both see the same unreleased
# changes and mint the same version twice.
concurrency:
  group: release
  cancel-in-progress: false

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          # momo reads the current version from the tags, so a shallow clone
          # with none looks like a project that has never released.
          fetch-depth: 0
          # `github.token` cannot push past a protected branch. A PAT or GitHub
          # App token that may bypass protection can — see below.
          token: ${{ secrets.RELEASE_TOKEN || github.token }}

      - uses: jdx/mise-action@v2          # installs the repo's tools, momo included

      # `--check` is inverted: it *succeeds* when there is something to release.
      - name: Anything to release?
        id: gate
        run: |
          if momo status --check >/dev/null 2>&1; then
            echo "pending=true" >> "$GITHUB_OUTPUT"
          else
            echo "pending=false" >> "$GITHUB_OUTPUT"
          fi

      - name: Release
        if: steps.gate.outputs.pending == 'true'
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail

          version="$(momo release --version-only)"
          git config user.name  "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add --all
          git commit -m "release $version"

          # The tag names the committed release, so it is created after the
          # commit. `--follow-tags` pushes the commit and the annotated tag
          # together.
          momo tag --format json --output tag.json
          git push origin HEAD --follow-tags

          tag="$(jq -r '.packages[0].tag' tag.json)"
          momo notes --output notes.md
          gh release create "$tag" --title "$tag" --notes-file notes.md
```

### What it has to get right

- **`fetch-depth: 0`.** momo takes the current version from the tags. A shallow clone has none, so every run looks like a first release and mints `0.1.0`.
- **momo never pushes.** `momo tag` creates the tag locally, like `git tag`; the job pushes it, and that push is what triggers a downstream tag-driven release pipeline.
- **The tag comes after the commit.** It anchors to `HEAD` (git) or `@-` (jj), so it names the commit carrying the changelog, not the one before it.
- **`git add --all`, not `-am`.** A first release creates files (`CHANGELOG.md`, `.momo/version`) that are not yet tracked, and fragments mode deletes files; only `--all` covers new, modified and removed together.
- **`--check` is inverted.** It exits non-zero when there is *nothing* to release. Read the other way round it skips every release.
- **A protected branch needs a token that can bypass it.** `github.token` is rejected when protection requires review or status checks; supply a PAT or GitHub App token as `RELEASE_TOKEN`. If CI should not push at all, have the job open a pull request with the release commit and tag after the merge instead — the same commands, one step later.
- **Concurrency.** Two runs seeing the same unreleased changes mint the same version, and the second `momo tag` then fails on the existing tag. The `concurrency` group serialises them.
- **Monorepos.** Every package with unreleased changes is released in one run and `momo tag --format json` reports one entry per package: loop over `.packages[]` rather than taking `[0]`, and note that the tag prefix is `<slug>-v` per package.

### Installing momo

`jdx/mise-action` above installs whatever the repo pins, so a repo already using mise needs no extra step. Without mise, the simplest path is the installer — it detects the runner's target, verifies the published checksum, and installs to `$HOME/.local/bin`, which the job then adds to `PATH`:

```yaml
      - name: Install momo
        run: |
          curl -fsSL https://raw.githubusercontent.com/Jarvvski/momo/main/install.sh | sh
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
```

Or fetch a pinned release archive by hand and unpack it straight into `/usr/local/bin`:

```yaml
      - name: Install momo
        run: |
          tag="$(curl -sSL https://api.github.com/repos/Jarvvski/momo/releases/latest | jq -r .tag_name)"
          curl -sSL "https://github.com/Jarvvski/momo/releases/download/${tag}/momo-${tag#v}-x86_64-unknown-linux-gnu.tar.gz" \
            | sudo tar xz -C /usr/local/bin --strip-components=1
```

Release binaries are built for Linux and macOS only — `x86_64`/`aarch64` × `unknown-linux-gnu`/`apple-darwin` — so there is no Windows asset. momo is Unix-only: `cargo install momo-cli` compiles from source on Linux and macOS, and a Windows build fails at compile time (see the [platform note](../README.md)).
