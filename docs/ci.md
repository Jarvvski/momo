# CI

momo is designed to be driven by a script. It never reads `CI_*` or `GITHUB_*` —
it reports and exits, and CI decides what to do about it.

The reporting verbs (`status`, `release`, `candidate`, `tag`) honour the output
flags below. `break`/`add`/`fix`, `init` and `install` reject them, because those
verbs produce no structured report.

| Flag | Effect |
|---|---|
| `--format <human\|json>` | Rendering (default `human`) |
| `--version-only` | Print only the computed version; wins over `--format` |
| `--output <PATH>` | Write the same content to a file; stdout is left empty |

## The `status --check` exit-code contract

`momo status --check` **exits non-zero when there is nothing unreleased**, and
zero when there is. It still renders the requested report (human or JSON) to
stdout; the error goes to stderr. Use it to skip the rest of a release job when
there is nothing to mint:

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

Note the inverted sense: `--check` asks *"is there anything to do?"*, not *"is the
tree clean?"*. `0` means there **is** something to release.

## JSON shapes

Every reporting verb emits **one** JSON document per run, whatever the number
of packages. The multi-package report is an envelope: a run-wide flag plus a
top-level `packages` array, which is always present — even for a single-package
repo. This means `--output <PATH>` writes the whole report once, rather than
truncating to the last package.

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

`unreleased` is `true` when at least one package has an unreleased change — the
same gate as `--check`. `current` is the package's last release (or cut
candidate), or `null` when it has never released. `next` is the version a release
would mint, or the current version when nothing is unreleased (then `changes` is
empty). `packages` is always an array, even for a single-package repo.

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

`dry_run` is the run-wide flag. Each entry in `packages` is one released package,
named by its `name` (the same identity `status` reports). `version_file` is the
version file the run wrote. `manifest` is the ecosystem manifest whose version
scalar the run synced, or `null` when the package has none. `consumed` lists the
fragments the run deleted (empty for `candidate`). Under `--dry-run` the paths
are what the run *would* have written — including the manifest, which the dry run
parses but does not write.

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

`tag` is the package's namespaced tag name, prefix included; `name` identifies
which package it names.

## A release job

```bash
if ! momo status --check >/dev/null 2>&1; then
  echo "nothing to release"; exit 0
fi
version="$(momo status --version-only)"
momo release
# commit, then:
momo tag
```
