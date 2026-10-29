# CI

momo is designed to be driven by a script. It never reads `CI_*` or `GITHUB_*` — it reports and exits, and CI decides what to do about it.

The reporting verbs (`status`, `release`, `candidate`, `tag`) honour the output flags below. `break`/`add`/`fix`, `init` and `install` reject them, because those verbs produce no structured report.

| Flag | Effect |
|---|---|
| `--format <human\|json>` | Rendering (default `human`) |
| `--json` | Shorthand for `--format json`; clap rejects it together with `--format` |
| `--version-only` | Print only the computed version; wins over `--format` |
| `--output <PATH>` | Write the same content to a file; stdout is left empty |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The verb did what was asked. |
| `1` | Runtime failure: momo ran but could not do what was asked — nothing unreleased to release, no VCS to tag, an unreadable `momo.toml`, an output flag the verb does not support. The message goes to stderr. |
| `2` | Usage error: the command line itself is wrong — an unknown verb or flag, or two flags that conflict. clap reports it; momo never exits `2` on its own. |

`status --check` inverts the sense of `0` and `1` for the skip gate, which is the next section. A rejected output flag is a `1`, not a `2`: momo parsed the command line, then refused it.

## The `status --check` exit-code contract

`momo status --check` **exits non-zero when there is nothing unreleased**, and zero when there is. It still renders the requested report (human or JSON) to stdout; the error goes to stderr. Use it to skip the rest of a release job when there is nothing to mint:

```console
$ momo status --check
Unreleased

  proj
    Added
      - Add a shiny thing
    current: —
    → next: 0.1.0
$ echo $?
0

$ momo release >/dev/null
$ momo status --check
Unreleased

  proj
    current: 0.1.0
    → next: 0.1.0
error[momo.release.nothing-unreleased]: nothing unreleased
  fix: record a change with `momo add`, `momo fix` or `momo break`
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
      ],
      "unversioned": [],
      "cascade": []
    }
  ]
}
```

`unreleased` is `true` when at least one package has an unreleased change — the same gate as `--check`. `current` is the package's last release (or cut candidate), or `null` when it has never released. `next` is the version a release would mint, or the current version when nothing is unreleased (then `changes` is empty). `unversioned` lists the Unreleased sections under a Keep-a-Changelog heading momo cannot read — the condition that makes a `release` refuse; it is empty in the common case. `packages` is always an array, even for a single-package repo. `cascade` names the dependent edges the new version would leave unsatisfied, or could not check — see [Cascade](#cascade) below.

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
      "manifest": ["/repo/Cargo.toml"],
      "skipped": [],
      "consumed": [
        "/repo/.momo/changes/add-a-shiny-thing.md"
      ],
      "cascade": [
        {
          "dependent": "cargo:b",
          "dependency": "momo",
          "requirement": "^0.1.0",
          "target": "0.2.0",
          "satisfied": false
        }
      ],
      "ranges": [
        {
          "dependent": "cargo:b",
          "dependency": "momo",
          "requirement": "^0.1.0",
          "target": "0.2.0"
        }
      ]
    }
  ]
}
```

`dry_run` is the run-wide flag. Each entry in `packages` is one released package, named by its `name` (the same identity `status` reports). `version_file` is the version file the run wrote. `manifest` is the list of ecosystem manifests whose version scalar the run synced — empty when the package has none, and possibly more than one, since a member that inherits its version writes the shared workspace root rather than its own manifest. `skipped` names the manifests the run considered but did not write, each as `{"path": …, "reason": …}`: a reason is `no version momo owns`, `already current`, or `dynamic version (…)` for a version computed elsewhere (a `hatch-vcs` project). A skip is never silent — it is reported here and on stderr. `consumed` lists the fragments the run deleted (empty for `candidate`). `cascade` is the same array `status` reports, read against the version the run minted. `ranges` names the dependent requirements the run actually rewrote — see [Dependency ranges](#dependency-ranges) below; it is `[]` unless the run passed `--sync-ranges`. Under `--dry-run` the paths are what the run *would* have written — including the manifest, which the dry run parses but does not write.

A lockfile the package manager maintains (`Cargo.lock`, `pnpm-lock.yaml`) is never rewritten and never appears in the report; momo warns on stderr that it may be stale, naming a shared lockfile once however many packages resolve to it. For a Node lockfile the warning names the refresh command (`npm install --package-lock-only`, `pnpm install --lockfile-only`, `yarn install --mode update-lockfile`, `bun install --lockfile-only`); a Node lockfile lives at the workspace root, not beside the package, so it is found by walking up from the package.

### Cascade

`cascade` is a per-package array of the dependent edges a release would leave unsatisfied. It is **report-only**: momo never rewrites a dependent's range here.

```json
{
  "dependent": "cargo:b",
  "dependency": "cargo:a",
  "requirement": "^0.1.0",
  "target": "0.2.0",
  "satisfied": false
}
```

- `dependent` depends on `dependency` — the package being released — declaring `requirement`.
- `target` is the version the released package would mint.
- `satisfied` is `false` when momo read the range and the target does not match it, and `null` when it could not check the edge: a dependent whose provider has no manifest adapter (a `gem:` package), a requirement whose syntax momo does not recognise, or a manifest it could not read. It is **never `true`** — a satisfied edge is simply absent, so an empty array means either no dependents or every one satisfied.

Cargo, Node, Python (`uv`) and Go dependents are checked, each against its own grammar: Cargo's caret/`~`/`=`/`>=`/`*` forms, npm's semver ranges (caret, tilde, exact, `*`/`x`, comparators, hyphen ranges and `||` unions), PEP 440 and Poetry's caret/tilde constraints, and a Go pin — satisfied when the target keeps the pinned major and is at least it, since a new major is a different module path. A protocol entry that constrains nothing (`workspace:`, `file:`, `link:`, `git:`/`github:`, an `npm:` alias) is omitted rather than reported. The edges come from `mise tasks graph`, so a dependency mise does not infer is not reported at all — an empty array is not a promise that nothing will break.

### Dependency ranges

`release --sync-ranges` rewrites the ranges `cascade` reports, for dependents that are themselves in the release set. The rewrite is opt-in, so an un-opted-in run's JSON is unchanged but for the empty array.

```json
{
  "dependent": "cargo:b",
  "dependency": "cargo:a",
  "requirement": "^0.1.0",
  "target": "0.2.0"
}
```

- `requirement` is what the dependent declared before the rewrite; `target` is the version the requirement was widened to admit.
- The array is attached to the package whose release *caused* the rewrite — the `dependency` — and is empty unless the run passed `--sync-ranges`.
- Only dependents already in the release set are rewritten; the rest stay in `cascade`. Each ecosystem's adapter rewrites only the forms it understands, preserving the operator's form (`^0.1.0` → `^0.2.0` in Cargo, npm or Poetry, `>=1.2` → `>=2.0.0`), and a requirement momo cannot rewrite is left alone and reported in `cascade` — a Go pin is reported, never rewritten. A `candidate` never rewrites a range.

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

## Errors

A failed run is structured too. Every error carries a stable code — the full
catalog is [errors.md](errors.md) — and `--format json` renders it as one
document on **stdout**, with a non-zero exit code:

```json
{
  "error": {
    "code": "momo.version.stale-candidate",
    "message": "the cut candidate `1.1.0-rc.1` is stale: the changes now imply `2.0.0`",
    "fix": "cut a new candidate for `2.0.0` (`momo candidate`)",
    "url": "https://github.com/Jarvvski/momo/blob/main/docs/errors.md#momoversionstale-candidate",
    "context": []
  }
}
```

`context` holds the outer `.context(...)` chain, outermost first, and is empty
for a top-level refusal. Branch on `code`, never on the prose. Without
`--format json` the same error is `error[<code>]: <explanation>` on stderr, where
`<explanation>` is the whole error chain on one line — `<context>: <message>`,
outermost first — plus an indented `fix:` line. `--output` is never written on
failure.

One nuance: `status --check --format json` renders its report first and the error
document second, because the gate is evaluated after the report. A consumer
should read the report's `unreleased` field and treat the exit code as the gate.

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
gh release create "v$version" --title "v$version" --generate-notes
```

momo writes the files and creates the tag; committing, pushing and publishing are the job's own steps. `release --version-only` mints and writes exactly as `release` does — it only narrows what is printed — so the version names the commit it is about to land on.

The release page is the forge's job: `--generate-notes` has GitHub list the merged pull requests. momo's changelog is the record that lives in the repo, written from the Changes you recorded — the two are generated from their own sources and neither is derived from the other.

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
          gh release create "$tag" --title "$tag" --generate-notes
```

### What it has to get right

- **`fetch-depth: 0`.** momo takes the current version from the tags. A shallow clone has none, so every run looks like a first release and mints `0.1.0`.
- **momo never pushes.** `momo tag` creates the tag locally, like `git tag`; the job pushes it, and that push is what triggers a downstream tag-driven release pipeline.
- **The tag comes after the commit.** It anchors to `HEAD` (git) or `@-` (jj), so it names the commit carrying the changelog, not the one before it.
- **`git add --all`, not `-am`.** A first release creates files (`CHANGELOG.md`, `.momo/version`) that are not yet tracked, and fragments mode deletes files; only `--all` covers new, modified and removed together.
- **`--check` is inverted.** It exits non-zero when there is *nothing* to release. Read the other way round it skips every release.
- **A protected branch needs a token that can bypass it.** `github.token` is rejected when protection requires review or status checks; supply a PAT or GitHub App token as `RELEASE_TOKEN`. If CI should not push at all, have the job open a pull request with the release commit and tag after the merge instead — the same commands, one step later.
- **Concurrency.** Two runs seeing the same unreleased changes mint the same version, and the second `momo tag` then fails on the existing tag. The `concurrency` group serialises them.
- **Monorepos.** Every package with unreleased changes is released in one run and `momo tag --format json` reports one entry per package: loop over `.packages[]` rather than taking `[0]`, and note that the tag prefix is `<slug>-v` per package — except a `go:` module, which is prefixed by its directory so `go get` resolves it (`api/v1.3.0`).

### Installing momo

`jdx/mise-action` above installs whatever the repo pins, so a repo already using mise needs no extra step. Without mise, the simplest path is the installer — it detects the runner's target, verifies the published checksum, and installs to `$HOME/.local/bin`, which the job then adds to `PATH`:

```yaml
      - name: Install momo
        run: |
          curl -fsSL https://raw.githubusercontent.com/Jarvvski/momo/main/install.sh | sh
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
```

A release archive also carries a GitHub build-provenance attestation, tying it to the workflow and commit that built it. `gh` verifies the archive against the repository — the installer runs the same check when `gh` is installed and authenticated, so this is only needed to verify a download you fetched yourself:

```bash
gh attestation verify momo-<version>-x86_64-unknown-linux-gnu.tar.gz --repo Jarvvski/momo
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
