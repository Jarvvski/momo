# Migrating an existing project to momo

You already have tags, a changelog, and maybe release pages. This guide gets momo
versioning that project without rewriting its history.

The short version: **adoption is mostly additive.** momo reads your existing tags
for the current version, preserves your changelog, and never touches your
existing releases. There is no import step. But three things behave in ways worth
knowing in advance, and they are the reason this page exists — the bump only reads
three headings, the changelog footer is regenerated, and momo cannot see a GitHub
release.

- [What momo adopts, and what it leaves alone](#what-momo-adopts-and-what-it-leaves-alone)
- [1. Look before you write](#1-look-before-you-write)
- [2. Match your existing tags](#2-match-your-existing-tags)
- [3. Your changelog: kept vs. changed](#3-your-changelog-kept-vs-changed)
- [4. The first release](#4-the-first-release)
- [5. Release pages and CI](#5-release-pages-and-ci)
- [6. Configuration](#6-configuration)
- [Monorepos](#monorepos)
- [Worked example](#worked-example)
- [When it disagrees with you](#when-it-disagrees-with-you)

## What momo adopts, and what it leaves alone

| Artefact | What momo does |
|---|---|
| Your current version | **Reads the latest tag.** Nothing to import or declare. |
| Existing tags | Never moved, rewritten, or deleted. An existing tag is never overwritten. |
| Changelog history | **Preserved byte-for-byte**, except the parts momo owns (see [§3](#3-your-changelog-kept-vs-changed)). |
| GitHub / GitLab releases | **Never read, never written.** momo is tag-driven. |
| `.momo/version` | Created for you by your first `release`. You do not need to create it. |
| `momo.toml` | Optional. Write commands create it lazily; read-only commands never do. |
| Your git history | Untouched. momo never commits and never pushes. |

## 1. Look before you write

Every check below is read-only — `status` and `--dry-run` never write anything, and
never even create `momo.toml`:

```console
$ momo status
Unreleased

  widget
    → next: 1.4.2

$ momo status --format json | jq '.packages[] | {name, current, next}'
{
  "name": "widget",
  "current": "1.4.2",
  "next": "1.4.2"
}
```

`current` is the version momo read from your tags. Check that it matches your last
real release, because everything downstream depends on it:

- `"current": null` means momo found no tag it recognises — see [§2](#2-match-your-existing-tags).
- `next` equal to `current` is correct when nothing is unreleased.
- If `next` is higher than you expect, your changelog already has counted entries —
  see [§3](#3-your-changelog-kept-vs-changed).

To see exactly what a release would write, without writing it:

```console
$ momo release --dry-run
would release 1.5.0
would write /repo/CHANGELOG.md
would write /repo/.momo/version
would write /repo/Cargo.toml
```

## 2. Match your existing tags

momo takes the current version from the **latest tag under your configured
prefix**, so the prefix is the one thing that has to match. A tag is parsed as
`<tag_prefix><semver>`; anything else is silently ignored rather than erroring,
which is convenient for stray tags and dangerous for a mismatched prefix.

| Your tags | What to set |
|---|---|
| `v1.2.3` | Nothing — `tag_prefix = "v"` is the default. |
| `1.2.3` (no prefix) | `tag_prefix = ""` |
| `release-1.2.3` | `tag_prefix = "release-"` |
| `widget-v1.2.3` | `tag_prefix = "widget-v"` |

A mismatch does not warn — the tags simply stop counting, and the package looks
like it has never released:

```console
$ git tag --list
v1.2.3
$ momo status --format json | jq '.packages[0].current'
"1.2.3"

$ printf 'tag_prefix = "widget-v"\n' > momo.toml
$ momo status --format json | jq '.packages[0] | {current, next}'
{
  "current": null,
  "next": "0.0.0"
}
```

That `null` is the symptom to watch for: a release from there would mint `0.1.0`,
not `1.3.0`.

A few consequences worth internalising:

- **GitHub releases are not a version source.** If your releases were created
  without tags, momo sees nothing. Create the tags (any commit will do — momo
  reads tag *names*, not their targets).
- **Pre-releases are recognised and skipped as a base.** A repo mid-`-rc` adopts
  cleanly: `1.2.0-rc.2` is a tag momo knows, but the next release bumps from
  `1.1.0`, the last stable.
- **Do not hand-write `.momo/version`.** It is the *pending-release ledger*, not a
  version source: `status` and `release` read tags, and `tag` reads the ledger.
  `tag` refuses a ledger older than your newest tag, so a stale hand-written file
  blocks tagging until the next release overwrites it.
- **`momo init --version <x.y.z>` is for a VCS-less project.** With a repository
  present the tags already answer the question, so seeding the ledger is at best
  a no-op and at worst the stale-ledger case above.
- **You cannot retro-tag with momo.** `momo tag` only tags what the ledger holds,
  so it will not recreate your existing release tags. Keep them as they are.

## 3. Your changelog: kept vs. changed

Your existing `CHANGELOG.md` needs no preparation. Recording creates the
`## [Unreleased]` section if it is missing, and a missing file is created from
scratch on first write. A changelog under another name is one key away:

```toml
changelog = "CHANGES.md"
```

### The bump only reads three headings

This is the first surprise. momo interprets `- ` bullets under **`### Breaking`**,
**`### Added`** and **`### Fixed`** — and nothing else. Every other heading is
preserved exactly and simply does not count toward the version:

```console
$ cat CHANGELOG.md
# Changelog

## [Unreleased]

### Added

- Hand-written added thing

### Changed

- Hand-written changed thing

### Fixed

- Hand-written fixed thing

$ momo status
Unreleased

  widget
    Added
      - Hand-written added thing
    Fixed
      - Hand-written fixed thing
    → next: 0.1.0
```

Note what happened: the two recognised bullets **are** counted — your accumulated
`### Added` and `### Fixed` entries drive the first release, so there is usually
nothing to do. The `### Changed` bullet is still in the file, untouched, but it did
not affect the version and is not listed.

The practical consequences:

- **Keep a Changelog users:** `### Added` and `### Fixed` map straight across.
  `### Changed`, `### Deprecated`, `### Removed` and `### Security` are preserved
  but ignored — a breaking change filed under `### Changed` will not bump MAJOR.
- **Do not double up.** If a hand-written `- Add a thing` already sits under
  `### Added` and you also run `momo add "Add a thing"`, you get two bullets.
  Either leave the hand-written entries alone (they count) or replace them.
- **Back-filling is optional.** To make an ignored entry count, re-record it with
  `momo break` / `momo add` / `momo fix` and delete the hand-written original.
  Only worth doing if the first release's number matters to you; otherwise let the
  entries ride along and start recording with momo from now on.

### The footer is regenerated on your first release

This is the second surprise, and the one that produces a surprising diff. momo
owns the `[Unreleased]` and `[x.y.z]` reference-link definitions: on a release it
drops them and rebuilds them from your remote and tag prefix. Every other
definition — the prose links a Keep a Changelog footer carries — is preserved.

```console
$ cat CHANGELOG.md     # before
...
[Unreleased]: https://github.com/acme/widget/compare/v1.4.2...HEAD
[1.4.2]: https://github.com/acme/widget/releases/v1.4.2
[keep a changelog]: https://keepachangelog.com/en/1.1.0/

$ momo release
released 1.5.0

$ cat CHANGELOG.md     # after
...
[keep a changelog]: https://keepachangelog.com/en/1.1.0/
[1.5.0]: https://github.com/acme/widget/compare/v1.4.2...v1.5.0
[1.4.2]: https://github.com/acme/widget/releases/tag/v1.4.2
```

Three things to notice: the `[1.4.2]` URL was **rewritten** to momo's shape
(`/releases/tag/`), the `[Unreleased]` link is gone because the section was
consumed, and the prose link survived but moved to the top of the block.

If your remote is not recognisable — or is not GitHub, GitLab, or a
GitHub-shaped forge — set `remote` explicitly, or leave it unset to skip the
footer entirely:

```toml
remote = "https://git.example.com/acme/widget"
```

A footer is only treated as a footer when a blank line separates it from the body,
which the Keep a Changelog convention already does.

### Other changelog rules

- **Release headings** are `## [<version>] - <YYYY-MM-DD>`, dated in **UTC** so a
  release is reproducible across machines and runners.
- **`## [Unreleased]` is matched case-insensitively** (`## [unreleased]` works).
- **More than one Unreleased heading is refused**, naming the line, because momo
  cannot tell which one to record into. Clean these up first.
- **CRLF is preserved.** A CRLF changelog round-trips, including the lines momo
  writes.
- **Fragments mode** (`recording = "fragments"`) is an alternative: changes live
  as files under `.momo/changes/` and the changelog holds only releases. You can
  switch later; the two models do not need to agree on history.

## 4. The first release

The ceremony is the same as any other release, and `--dry-run` shows the diff
first:

```console
$ momo release --dry-run          # inspect
$ momo release
$ git commit -am "release 1.5.0"  # or: jj commit
$ momo tag
tagged v1.5.0
```

`release` mints the version, writes the changelog and the version ledger, and
syncs the `Cargo.toml` version scalar. It does **not** commit and does **not**
tag — that split is what lets the tag land on the commit that carries the
changelog.

`momo tag` anchors to `HEAD` on git and `@-` on jj. If you tag before committing,
pass `--rev @` (jj) or the commit-ish (git) explicitly; the anchor is never
inferred from the working copy.

## 5. Release pages and CI

momo creates the **tag**. It never creates a GitHub release and never reads one —
your existing release pages are untouched, and keeping them going is your CI's
job. The changelog section momo just wrote is the body:

```bash
version="$(momo release --version-only)"
git add --all
git commit -m "release $version"
momo tag
git push --follow-tags
momo notes --output notes.md
gh release create "v$version" --title "v$version" --notes-file notes.md
```

`momo notes` prints the section verbatim, so the release page and the changelog
cannot drift.

For a complete, copy-pasteable GitHub Actions workflow — including the tags-shallow-clone
trap, the inverted `status --check` gate, concurrency, and the token a protected
branch needs — see **[CI](ci.md#a-github-actions-workflow)**.

## 6. Configuration

`momo.toml` is optional and created lazily. Two things to know if you already have
one:

- **An unrecognised key is a hard error.** A misspelled or removed key fails
  loudly instead of silently taking a default, so an old config makes `momo status`
  fail until it is fixed.
- **`tag_kind` was removed.** It selected between annotated and lightweight tags;
  the adapter decides now. Delete the key if you have it.

The keys most likely to matter when adopting:

| Key | Why you would set it |
|---|---|
| `tag_prefix` | Your tags are not `v`-prefixed. |
| `changelog` | Your changelog is not `CHANGELOG.md`. |
| `remote` | Your forge is not GitHub/GitLab, or you have no remote. |
| `command_timeout` | A very large repo where a `git` read legitimately takes over 60s. |

`MOMO_*` environment variables override the file, and the file overrides the
built-in defaults. See **[Configuration](configuration.md)** for the full list.

## Monorepos

A monorepo is the harder migration, because momo versions **each package
independently** while a monorepo often has one version and one tag for everything.

### How momo sees a monorepo

Discovery runs `mise tasks graph --json`, which needs `monorepo_root = true` in
your mise configuration. Without it, momo treats the repo as a single package
named after its directory — and warns and falls back to the same thing if the
graph cannot be read.

Each package carries an ecosystem identity (`cargo:momo`, `node:@acme/ui`) and a
derived slug (`cargo-momo`) which namespaces three things:

| Namespaced by slug | Single package | Monorepo |
|---|---|---|
| Tags | `v1.2.3` | `cargo-momo-v1.2.3` |
| Fragment store | `.momo/changes/` | `.momo/changes/cargo-momo/` |
| Version ledger | `.momo/version` | `crates/momo/.momo/version` |

### The single-tag problem

If your monorepo is tagged `v1.2.3` for the whole tree, that tag means nothing to
any individual package: each one looks never-released, and the first release from
each would mint `0.1.0`. There are two honest ways out.

**Adopt as a single package.** Do not declare `monorepo_root = true`. The repo
keeps one version, one tag and one changelog, and momo versions it as a whole.
Cheapest, and correct if your packages really do move together.

**Go per-package, and seed a tag for each.** Give each package a tag naming its
current version. momo reads tag *names*, not their targets, so the commit you
point at only matters for the compare links in your footer:

```console
$ git tag -a cargo-momo-v1.2.3 <commit> -m "Release 1.2.3"
$ git tag -a node-ui-v0.4.1   <commit> -m "Release 0.4.1"
$ git push --tags
```

Take each version from the package's own `Cargo.toml` or its changelog section.
Once the tags exist, `momo status` reports a `current` per package and the first
release bumps from there rather than from zero.

### One changelog per package

momo resolves the changelog as `<package dir>/<changelog>`, and there is no
per-package override for the path — so a monorepo needs one changelog **file per
package**, all with the same name:

```
repo/
├── CHANGELOG.md              ← the root crate's, if the root is a package
└── crates/
    ├── momo/CHANGELOG.md
    └── ui/CHANGELOG.md
```

A single root `CHANGELOG.md` shared by every package is not supported; split it,
or adopt as a single package.

### Targeting

Recording verbs default to the package containing the current directory;
aggregate verbs (`status`, `release`, `tag`, `notes`) default to every package.
`-p` scopes to one and `--all` covers them all:

```console
$ momo -p cargo:momo add "Add a thing"
$ momo --all release
```

`release` with no `-p` releases every package that has unreleased changes, in one
run. There is no cascading: releasing a package does not bump its dependents.

### Slugs are worth pinning

A slug is derived from the identity unless you set it, and it namespaces your tags
and fragments. Renaming a package's identity moves that namespace, so pin the slug
when adopting if you want the tag names to stay put:

```toml
[packages."cargo:momo"]
slug = "cargo-momo"
```

## Worked example

A single-crate repo, tagged `v1.4.2`, with a Keep a Changelog `CHANGELOG.md` and
GitHub releases. The `[1.4.2]` footer link uses the wrong URL shape, and the
Unreleased section has one entry under a heading momo does not read.

```console
$ momo status
Unreleased

  widget
    Added
      - A new thing
    → next: 1.5.0
```

The tag was read (so the bump is `1.4.2` → `1.5.0`, not `0.1.0`), and the
hand-written `### Added` entry counted. Nothing to prepare.

```console
$ momo release
released 1.5.0
wrote /repo/CHANGELOG.md
wrote /repo/.momo/version
wrote /repo/Cargo.toml

$ git commit -am "release 1.5.0"
$ momo tag
tagged v1.5.0

$ momo notes --output notes.md && cat notes.md
### Added

- A new thing
```

The only unexpected diff is the footer, regenerated as described in
[§3](#the-footer-is-regenerated-on-your-first-release). From here, `momo add`,
`momo fix` and `momo break` drive the next version, and CI can gate on
`momo status --check`.

## When it disagrees with you

| Symptom | Cause |
|---|---|
| `"current": null` but you have releases | The tags are missing, or `tag_prefix` does not match them. Check with `git tag --list`. |
| A release would mint `0.1.0` on a mature project | Same as above — momo sees no recognised tag. |
| `next` is lower than your changelog implies | The entries sit under `### Changed`/`### Removed`/`### Security`, which are preserved but not interpreted. Re-record them with `momo break`/`add`/`fix`. |
| `next` is *higher* than expected | A hand-written `### Added`/`### Fixed` entry is being counted alongside your new ones — check for duplicates. |
| `tag` refuses, mentioning a version older than the newest tag | A stale `.momo/version`. Run `momo release` to advance it, or delete it if it was hand-written. |
| momo refuses the changelog, naming a line | More than one `## [Unreleased]` heading. Remove the duplicate. |
| `error: reading momo.toml: unknown key` | A stale or misspelled config key (`tag_kind` is the common one). |
| My footer links changed on the first release | Expected — momo owns `[Unreleased]` and `[x.y.z]` and rebuilds them. See [§3](#the-footer-is-regenerated-on-your-first-release). |
| `momo tag` says there is nothing to tag | `tag` tags what the ledger holds, which only `release`/`candidate` write. Run a release first. |

## Upgrading an existing momo install

If the project already used an earlier momo, two changes will bite:

- **Unknown config keys now error**, and `tag_kind` is gone.
- **`.momo/version` is a ledger, not a version source.** `status` and `release`
  read tags; `tag` reads the ledger and refuses a stale one.

Both are documented in the release notes for the versions that introduced them.
