# Precedents: how established versioning/changelog tools model the problem

Type: research (ticket `.scratch/momo-v0/issues/13-precedents.md`)
Status: findings
Date: 2026-10-12

## Question

Survey how established tools record changes, bump versions (especially pre-1.0),
handle release candidates, format changelogs, format tags, commit/push, and
report dry-run/CI output — so momo can borrow proven choices.

Tools covered: `changesets`, `towncrier`, `release-plz`, `cargo-release`, `svu`,
`semantic-release`, `git-cliff`, `scriv`.

Method: primary sources only (official docs and source repositories); every claim
is cited inline. No `git` commands were run.

## The one distinction momo must hold: how intent is *recorded*

There are three recording models in the wild. momo's `break|add|fix` explicitly
sits in the first two (it writes intent to a file); the third derives intent from
the VCS log and is the thing momo is *not*.

| Model | What is recorded | Tools |
|---|---|---|
| **A. Fragment files** — one file per change, consumed at release | A small file per change, named/timestamped, with a kind + body | **changesets**, **towncrier**, **scriv** |
| **B. Single changelog file** — one `CHANGELOG.md`, an `Unreleased` section edited in place | The changelog file itself is the accumulator | **cargo-release** (via `pre-release-replacements`), **release-plz** (regenerates `CHANGELOG.md`), **git-cliff** (`--prepend`), **semantic-release** (`@semantic-release/changelog`), **svu** (no changelog at all) |
| **C. Commit-message parsing** — no separate record | Conventional commits in the VCS history | **release-plz**, **cargo-release** (`changes` step), **svu**, **semantic-release**, **git-cliff** |

Notes on the table:

- Fragments and commit-parsing are not mutually exclusive: **release-plz** and
  **git-cliff** *record* via commits and *emit* a single changelog file. The
  fragment tools are the only ones where a human writes a per-change file.
- **towncrier** and **scriv** are pure changelog engines: they have **no bump
  rules, no RC handling, and create no tags**. They are precedents for the
  fragment format, not for semver/tagging.
- momo must support **A and B**; the ticket says so, and the map records it as
  settled.

## At-a-glance comparison

| Tool | Recording | Pre-1.0 bump | RC format | Changelog | Tag | Commits/pushes? | Dry-run / CI |
|---|---|---|---|---|---|---|---|
| **changesets** | Fragments (`.changeset/*.md`, YAML front matter) | **No special-case**: `major` on `0.x` → `1.0.0` | `pre enter <tag>` → `X.Y.Z-<tag>.N`; `pre.json` state | per-package `CHANGELOG.md`; pluggable generator | `pkg@X.Y.Z` / `vX.Y.Z` (single pkg) | **No** by default (`commit:false`); user commits & pushes | `status`/`publish-plan` JSON; split `version`/`publish` |
| **towncrier** | Fragments (`newsfragments/<issue>.<type>`) | n/a (no bumps) | n/a | `NEWS.rst`/`NEWS.md`, template + sections | n/a | `git rm` fragments + `git add` news; **does not commit** | `--draft` (stdout), `--yes`, `check` |
| **release-plz** | Commit messages (conventional) | **Cargo rules**: `feat`→patch, breaking→minor at `0.x` | no RC cmd; version is whatever is in `Cargo.toml`; `git_release_type="auto"` | `CHANGELOG.md`, Keep a Changelog (git-cliff engine) | `v{{version}}` / `{{pkg}}-v{{version}}` | `update` no commit; `release-pr` opens PR; `release` publishes+tags | `release --dry-run`; GitHub Action / GitLab CI |
| **cargo-release** | none (unopinionated); optional commit list | **No special-case**: explicit level; `major` on `0.x` → `1.0.0` | `rc`/`beta`/`alpha`: `1.0.0→1.0.1-rc.1`, `-rc.1→-rc.2`; `release` strips it | not managed (hook/replacements) | `{{prefix}}v{{version}}`, **annotated** by default | **Yes**: commits bump + **pushes** by default | **dry-run is the default**; `-x/--execute` to act |
| **svu** | Commit messages | `--v0` keeps `0.x` (breaking→minor); default → `1.0.0` | `prerelease` cmd: `<name>.<N>` (e.g. `rc.1`) | none | prints `vX.Y.Z` (prefix `v`); does **not** create tags | No — pure stdout | `--json`; script/CI oriented |
| **semantic-release** | Commit messages (Angular) | **No special-case**: `semver.inc`; breaking on `0.x` → `1.0.0`; first release `1.0.0` | branch pre-release → `X.Y.Z-<pre>.1` | notes via `release-notes-generator`; file needs `@semantic-release/changelog` | `tagFormat` default `v${version}`, **lightweight** | **No** commit by default (`@semantic-release/git` optional); pushes tag | `--dry-run` prints next version + notes; `--ci`/`--no-ci` |
| **git-cliff** | Commit messages | `breaking_always_bump_major` default **true** (0→1); `features_always_bump_minor` default true | no RC cmd; tag prefixes supported (`-beta.1→-beta.2`) | Tera templates; Keep-a-Changelog-style default; `-o`/`--prepend` | does **not** create tags (`--tag` labels only) | No | `--bumped-version`; `--context` JSON |
| **scriv** | Fragments (`changelog.d/`) | n/a (no bumps) | n/a | `CHANGELOG.rst`/`.md`, categories, insert marker | n/a | `--add` git-adds; **does not commit** | `scriv print` (stdout); `github-release --dry-run` |

---

## changesets (JS)

- **Recording — fragments.** Each change is a Markdown file in `.changeset/`
  with YAML front matter listing packages and their bump types
  (`major`/`minor`/`patch`); the Markdown body is the changelog summary. Files
  are created by `changeset add`, get random human-readable names, and are
  committed to the branch. On `version`, **all changeset files are removed** so a
  changeset is consumed exactly once.
  ([changesets.dev/faq](https://changesets.dev/faq);
  [docs/adding-a-changeset.md](https://raw.githubusercontent.com/changesets/changesets/main/docs/adding-a-changeset.md))
- **Bump rules.** Explicit and intent-based, *not* derived from commits: "you
  directly specify a `major`, `minor`, or `patch` bump." Multiple changesets for
  a package flatten to the **highest** bump (two minors + one patch → minor).
  ([technical-decisions](https://changesets.dev/guide/technical-decisions))
- **Pre-1.0.** **No special-casing.** `incrementVersion` calls
  `semver.inc(oldVersion, release.type)` directly, so a `major` changeset on
  `0.x` yields `1.0.0`.
  ([increment.ts](https://raw.githubusercontent.com/changesets/changesets/main/packages/assemble-release-plan/src/increment.ts);
  cf. open issue [#1228](https://github.com/changesets/changesets/issues/1228)
  asking for `0.x`-friendly behaviour.)
- **Release candidates.** A global "prerelease mode": `pre enter <tag>` writes
  `.changeset/pre.json`; `version` then appends `-<tag>.N` (e.g. `2.1.0-beta.0`)
  and **increments N** on each run. During pre mode changesets move to
  `.changeset/pre/` and are **retained** for the stable release; `pre exit` then
  strips the suffix. Snapshot releases use `0.0.0-<tag>-<datetime>`.
  ([prereleases](https://changesets.dev/guide/prereleases);
  [config-file-options](https://raw.githubusercontent.com/changesets/changesets/main/docs/config-file-options.md))
- **Changelog.** Per-package `CHANGELOG.md`. Generator is pluggable
  (`@changesets/cli/changelog` default; `changelog-git`, `changelog-github`
  available).
- **Tag format.** `pkg-name@X.X.X` in a monorepo, `vX.X.X` in a single-package
  repo. Created at `publish` (or standalone `git-tag`). The docs do not state
  annotated vs lightweight.
  ([versioning-and-publishing](https://changesets.dev/guide/versioning-and-publishing))
- **Commit/push.** Default `commit: false` — "we do not commit the files, and
  leave it to the user"; the tree is left dirty for the user to `git add`/commit.
  Pushing tags is the user's job (`git push --follow-tags`). ([config-file-options](https://raw.githubusercontent.com/changesets/changesets/main/docs/config-file-options.md))
- **Dry-run/CI.** `status` (exit 1 if changes exist without changesets; JSON via
  `--output`), `publish-plan` (JSON for CI), and the deliberate split of
  `version` (file changes only) from `publish` (npm + tags).
  ([cli](https://changesets.dev/guide/cli))

## towncrier (Python)

- **Recording — fragments.** "news fragments" in a directory
  (`newsfragments/`), named `<issue>.<type>[.ext]`; types default to
  `feature`, `bugfix`, `doc`, `removal`, `misc` and are configurable. Created by
  hand or `towncrier create`. ([index](https://towncrier.readthedocs.io/en/stable/index.html);
  [tutorial](https://towncrier.readthedocs.io/en/stable/tutorial.html))
- **Bump rules — none.** towncrier "does not read Git history" and does not bump
  versions; the version is supplied via `--version` or config. It is a news-file
  renderer only. ([index](https://towncrier.readthedocs.io/en/stable/index.html);
  [configuration](https://towncrier.readthedocs.io/en/stable/configuration.html))
- **RC handling — none.**
- **Changelog.** `NEWS.rst` (default) or `NEWS.md`; template-driven, sectioned by
  fragment type; content above a magic `start_string` marker is preserved.
  ([configuration](https://towncrier.readthedocs.io/en/stable/configuration.html))
- **Tag format — none.**
- **Commit/push.** `build` **removes** processed fragments with `git rm` and
  `git add`s the news file, but "leaves committing the changes up to the user."
  ([tutorial](https://towncrier.readthedocs.io/en/stable/tutorial.html))
- **Dry-run/CI.** `--draft` renders fragments to stdout without writing;
  `--yes` for automation; `check` fails a branch that adds no fragment; honours
  `SOURCE_DATE_EPOCH` for reproducible builds.
  ([cli](https://towncrier.readthedocs.io/en/stable/cli.html))

## release-plz (Rust)

- **Recording — commit messages.** Versions and changelogs are derived by
  "analyzing your git history"; conventional commits assumed.
  ([usage](https://release-plz.dev/docs/usage))
- **Bump rules.** `fix:`→patch, `feat:`→minor, `<prefix>!:` / breaking→major;
  non-conventional commits default to patch.
  ([changelog/format](https://raw.githubusercontent.com/release-plz/release-plz/main/website/docs/changelog/format.md))
- **Pre-1.0.** Follows **Cargo SemVer rules**, not the "0.x minor is additive"
  reading: `features_always_increment_minor` defaults to **false**, so in `0.x` a
  feature bumps **patch** and a breaking change bumps **minor** (`0.x →
  0.(x+1)`). The docs explicitly warn the opposite setting "violates the Cargo
  SemVer rules because the transition from `0.x` to `0.(x+1)` is used for
  breaking changes."
  ([config](https://release-plz.dev/docs/config);
  [semver-check](https://release-plz.dev/docs/semver-check))
  This is exactly momo's rule (`break`→MINOR and `add`→MINOR at MAJOR 0).
- **RC handling.** No dedicated RC command: release-plz versions whatever is in
  `Cargo.toml` (there is a `set-version` helper). `git_release_type = "auto"`
  marks a SemVer-prerelease version as a GitHub/Gitea **pre-release**.
  ([config](https://release-plz.dev/docs/config))
- **Changelog.** `CHANGELOG.md` per package, **Keep a Changelog** format by
  default, generated by git-cliff as a library; `[changelog]` config section
  controls header/body/parsers.
  ([changelog](https://release-plz.dev/docs/changelog);
  [format](https://raw.githubusercontent.com/release-plz/release-plz/main/website/docs/changelog/format.md))
- **Tag format.** `v{{version}}` for a single crate; `{{package}}-v{{version}}`
  for a multi-package workspace.
  ([config](https://release-plz.dev/docs/config))
- **Commit/push.** `update` edits locally "without committing any change";
  `release-pr` opens a PR; `release` publishes + tags. `allow_dirty` defaults to
  **false** (errors on a dirty tree).
  ([usage](https://release-plz.dev/docs/usage);
  [config](https://release-plz.dev/docs/config))
- **Dry-run/CI.** CI-first (GitHub Action / GitLab CI). `release-plz release
  --dry-run` exists; the release PR body is the primary human-facing output.
  ([faq](https://release-plz.dev/docs/faq))

## cargo-release (Rust)

- **Recording — none for the changelog.** "cargo release is unopinionated in its
  support for CHANGELOGs"; you either run a hook (e.g. git-cliff) or edit a
  Keep-a-Changelog file in place with `pre-release-replacements`. The `changes`
  step only *prints* commits since the last tag.
  ([faq](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/faq.md))
- **Bump rules.** Explicit `LEVEL|VERSION` argument (`major`, `minor`, `patch`,
  `release`, `rc`, `beta`, `alpha`, or an exact version). No commit parsing.
  ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))
- **Pre-1.0.** **No special-case** — levels are applied literally: `major` on
  `0.x` → `1.0.0`; `release` strips any pre-release extension.
  ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))
- **RC handling.** First-class levels: `alpha`/`beta`/`rc` add/increment a
  pre-release — `1.0.0 → 1.0.1-rc.1`, `1.0.1-alpha → 1.0.1-rc.1`, `1.0.1-rc.1 →
  1.0.1-rc.2`; the `release` level removes the pre-release extension
  (`0.1.0-alpha.1 → 0.1.0`). Note the dotted `-rc.N` form.
  ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))
- **Changelog.** Not managed (see above).
- **Tag format.** `tag-name` default `"{{prefix}}v{{version}}"`; `tag-prefix`
  defaults to `""` at the repo root and `"{{crate_name}}-"` otherwise. Annotated
  by default: `tag-message` defaults to `"chore: Release"`, and "set to blank for
  lightweight tags."
  ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))
- **Commit/push.** **Commits and pushes by default**: `consolidate-commits =
  true`, `pre-release-commit-message = "chore: Release"`, `push = true`,
  `push-remote = "origin"`. ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))
- **Dry-run/CI.** **Dry-run is the default**; `-x/--execute` actually performs
  the release. `--no-confirm` skips the interactive version preview for CI.
  ([reference](https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md))

## svu (Go)

- **Recording — commit messages.** `svu next` walks `git log` and maps commit
  messages to a bump. ([README](https://github.com/caarlos0/svu))
- **Bump rules.** `chore:`→none, `fix:`→patch, `feat:`→minor, `!` or
  `BREAKING CHANGE:`→major; `--always` falls back to patch when nothing matches.
  ([README](https://github.com/caarlos0/svu);
  [internal/svu/svu.go](https://raw.githubusercontent.com/caarlos0/svu/main/internal/svu/svu.go))
- **Pre-1.0.** `--v0` / `v0: true` "prevent[s] major version increments if
  current version is still v0" — a breaking change becomes a **minor** bump while
  at `0.x`. Default (without `--v0`) a breaking change on `0.x` → `1.0.0`.
  ([main.go](https://raw.githubusercontent.com/caarlos0/svu/main/main.go);
  [svu.go](https://raw.githubusercontent.com/caarlos0/svu/main/internal/svu/svu.go))
- **RC handling.** `svu prerelease` increments the pre-release build portion;
  `--prerelease <suffix>` sets it; the composed form is `<name>.<N>` (e.g.
  `rc.1`, `rc.2`). ([main.go](https://raw.githubusercontent.com/caarlos0/svu/main/main.go);
  [svu.go](https://raw.githubusercontent.com/caarlos0/svu/main/internal/svu/svu.go))
- **Changelog — none.**
- **Tag format.** Output prefix defaults to `v` (`--tag.prefix`); `--tag.output`
  decouples the printed prefix; `--tag.mode` (`all`/`current`) and
  `--tag.pattern` select tags. It only **prints** the version — it does not
  create tags (the README suggests `alias gtn='git tag $(svu next)'`).
  ([README](https://github.com/caarlos0/svu))
- **Commit/push.** None — pure stdout, by design a script helper.
- **Dry-run/CI.** `--json` structured output; `--verbose` logging; built for
  release scripts and CI. ([main.go](https://raw.githubusercontent.com/caarlos0/svu/main/main.go))

## semantic-release (JS)

- **Recording — commit messages.** Uses Angular commit conventions by default;
  "removes the immediate connection between human emotions and version numbers."
  ([README](https://raw.githubusercontent.com/semantic-release/semantic-release/master/README.md))
- **Bump rules.** breaking→major, `feat`→minor, `fix`/`perf`→patch,
  `revert`→patch; highest matching rule wins; unmatched types (e.g. `chore`,
  `docs`, `test`) trigger no release.
  ([commit-analyzer README](https://raw.githubusercontent.com/semantic-release/commit-analyzer/master/README.md);
  [default-release-rules.js](https://raw.githubusercontent.com/semantic-release/commit-analyzer/master/lib/default-release-rules.js))
- **Pre-1.0.** **No special-case.** `getNextVersion` does
  `semver.inc(lastRelease.version, type)`, so a breaking change on `0.x` →
  `1.0.0`; with no prior release the version is `FIRST_RELEASE = "1.0.0"`.
  ([get-next-version.js](https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/get-next-version.js);
  [constants.js](https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/definitions/constants.js))
- **RC handling.** Branch-driven pre-releases: `beta`/`alpha` branches publish
  `X.Y.Z-<prerelease>.1` (`FIRSTPRERELEASE = "1"`), with the npm dist-tag set to
  the branch channel. Default branch config lists `beta` and `alpha` as
  prerelease branches.
  ([get-next-version.js](https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/get-next-version.js);
  [configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration))
- **Changelog.** Release notes come from `@semantic-release/release-notes-generator`;
  writing a `CHANGELOG.md` file requires the separate
  `@semantic-release/changelog` plugin.
  ([configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration))
- **Tag format.** `tagFormat` default `v${version}`; created with
  `git tag <tagName> <ref>` — i.e. **lightweight** (no `-a`/`-m`) — and pushed
  with `git push --tags`.
  ([configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration);
  [lib/git.js](https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/git.js))
- **Commit/push.** Does **not** commit by default; committing release assets
  requires `@semantic-release/git`, whose own README warns against it. Core
  creates and pushes the tag.
  ([@semantic-release/git](https://raw.githubusercontent.com/semantic-release/git/master/README.md))
- **Dry-run/CI.** `--dry-run` "prints the next version and release notes to the
  console" and skips prepare/publish/addChannel/success/fail; default dry-run is
  `true` outside CI; `--ci`/`--no-ci` and `--debug` control environment
  assumptions. ([configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration))

## git-cliff (Rust)

- **Recording — commit messages.** Generates a changelog from Git history using
  conventional commits and regex parsers. ([docs](https://git-cliff.org/docs/))
- **Bump rules.** `--bump`: `fix:`→patch, `feat:`→minor, `!`→major; `--bump
  [major|minor|patch]` forces a type.
  ([bump-version](https://git-cliff.org/docs/usage/bump-version))
- **Pre-1.0.** Configurable via `[bump]`: `breaking_always_bump_major` defaults
  to **true** (breaking at `0.x` → `1.0.0`); set **false** to keep a zero-based
  scheme (breaking at `0.x` → minor). `features_always_bump_minor` defaults to
  **true** (features always bump minor, even at `0.x`). Also `initial_tag`,
  `no_increment_regex`, `custom_{major,minor}_increment_regex`, `bump_type`.
  ([bump](https://git-cliff.org/docs/configuration/bump))
- **RC handling.** No dedicated RC command; tag prefixes are supported
  (`testing/v1.0.0-beta.1` can advance to `...-beta.2`), and the next version is
  checked against `tag_pattern`. ([bump-version](https://git-cliff.org/docs/usage/bump-version))
- **Changelog.** Tera templates; TOML/YAML config; the shipped default
  (`config/cliff.toml`) is a Keep-a-Changelog-style layout with emoji groups
  (Features / Bug Fixes / Documentation / …). Output with `-o CHANGELOG.md` or
  `--prepend CHANGELOG.md` (prepends unreleased entries).
  ([configuration](https://git-cliff.org/docs/configuration/);
  [default cliff.toml](https://raw.githubusercontent.com/orhun/git-cliff/main/config/cliff.toml);
  [examples](https://git-cliff.org/docs/usage/examples))
- **Tag format.** git-cliff **does not create tags**; `--tag <name>` only labels
  the changelog entry, and `--bumped-version` prints the computed next version.
  ([examples](https://git-cliff.org/docs/usage/examples);
  [bump-version](https://git-cliff.org/docs/usage/bump-version))
- **Commit/push.** None.
- **Dry-run/CI.** `--bumped-version` (stdout), `--context` (JSON),
  `--unreleased`. ([bump-version](https://git-cliff.org/docs/usage/bump-version))

## scriv (Python)

- **Recording — fragments.** "a directory of changelog fragments" in
  `changelog.d/`; filenames are date + author + branch; content is categorised
  bullets. Created by `scriv create`.
  ([index](https://scriv.readthedocs.io/en/latest/);
  [commands](https://scriv.readthedocs.io/en/latest/commands.html))
- **Bump rules — none.** The version is a config value (`version = literal: …
  : __version__` or `command: …`) used only to title the changelog entry.
  ([configuration](https://scriv.readthedocs.io/en/latest/configuration.html))
- **RC handling — none.** (`github-release` knows a `release.prerelease`
  boolean, but scriv does not mint RCs.)
  ([configuration](https://scriv.readthedocs.io/en/latest/configuration.html))
- **Changelog.** `CHANGELOG.rst`/`.md`; `scriv collect` assembles fragments into
  one entry and inserts it at a `scriv-insert-here` marker (or the top). Default
  categories: `Removed, Added, Changed, Deprecated, Fixed, Security` — the Keep a
  Changelog set.
  ([commands](https://scriv.readthedocs.io/en/latest/commands.html);
  [configuration](https://scriv.readthedocs.io/en/latest/configuration.html))
- **Tag format — none.** `scriv github-release` requires the entry's version to
  match an **existing** git tag; it does not create tags.
  ([commands](https://scriv.readthedocs.io/en/latest/commands.html))
- **Commit/push.** Collected fragments are deleted; `--add` (or git config
  `scriv.collect.add`) `git add`s the changelog and the deletions, but scriv
  **does not commit**. ([commands](https://scriv.readthedocs.io/en/latest/commands.html))
- **Dry-run/CI.** `scriv print` writes the pending entry to stdout; `collect`
  exits non-zero when there are no fragments; `github-release --dry-run`.
  ([commands](https://scriv.readthedocs.io/en/latest/commands.html))

---

## Findings that bear on momo's open tickets

- **Pre-1.0 is genuinely contested.** The literal-semver camp (changesets,
  cargo-release, semantic-release) lets a `major`/breaking change at `0.x` jump
  to `1.0.0`. The zero-aware camp keeps `0.x` and treats a breaking change as a
  **minor** bump: release-plz (default, citing Cargo's rules), svu (`--v0`), and
  git-cliff (`breaking_always_bump_major = false`). momo's spec already chooses
  the zero-aware camp (`break` and `add` both bump MINOR at MAJOR 0), matching
  release-plz's default and Cargo's semantics. Ticket 01 should name this
  precedent.
- **The dotted RC form is the norm.** cargo-release (`-rc.1` → `-rc.2`) and svu
  (`rc.1`) both use `<name>.<N>`; semantic-release uses
  `<prerelease>.1`. This supports momo ticket 05's lean toward `1.2.0-rc.1` over
  `-rc-1`. cargo-release's `release` level (strip the pre-release to finalise) is
  a clean model for momo's `release` after `rc`.
- **RC state and consumption.** changesets is the richest precedent: a `pre.json`
  state file, a per-package pre-version counter, fragments **retained** during
  pre mode and replayed into the stable release, and `pre exit` to strip the
  suffix. momo ticket 05 asks precisely the "consume vs share" question; the
  changesets answer is "share, then strip on exit".
- **Fragment mechanics worth copying.** towncrier and scriv show the durable
  patterns: a per-change file whose *filename* encodes identity (towncrier:
  `<issue>.<type>`; scriv: timestamp+author+branch), fragments deleted on
  collection, an "orphan/uncategorised" escape hatch, and a configurable
  fragments directory. Both avoid merge conflicts by construction — the stated
  reason momo wants fragments (ticket 02).
- **Single-file mechanics worth copying.** cargo-release's
  `pre-release-replacements` is a precise recipe for editing an `Unreleased`
  section in place: rename `Unreleased`→version, stamp the date, and re-insert a
  fresh `Unreleased` heading. That is a concrete model for momo's single-file
  recording mode (ticket 02/03).
- **Tag format consensus.** `v`-prefixed is near-universal: semantic-release
  `v${version}`, cargo-release `v{{version}}` (root), release-plz `v{{version}}`,
  changesets `vX.X.X` in single-package repos, svu prefix `v`, git-cliff default
  `v`-stripping in templates. Annotated vs lightweight splits: cargo-release
  **annotated** by default; semantic-release **lightweight**; changesets
  unspecified. momo ticket 06 must pick one (note momo is jj-first; ticket 12
  covers jj/git tag mechanics).
- **"Leave the tree dirty" is the majority default.** changesets
  (`commit:false`), towncrier, scriv, release-plz `update`, and semantic-release
  all leave committing to the user (semantic-release and cargo-release are the
  exceptions — cargo-release commits **and pushes** by default, the most
  aggressive of the set). momo ticket 06's side-effects decision should note that
  the conservative precedent is "write files, don't commit".
- **Dry-run conventions.** Two shapes recur: a read-only *preview* (towncrier
  `--draft`, scriv `print`, semantic-release `--dry-run`, cargo-release's
  default dry-run) and a structured *machine* output (changesets `status
  --output` / `publish-plan`, svu `--json`, git-cliff `--context`). momo ticket
  07 should offer both: human dry-run text plus a JSON mode for CI.
- **Changelog format.** "Keep a Changelog" is the de-facto default among the
  tools that own a changelog (release-plz default, git-cliff default template,
  scriv's default categories, cargo-release's worked example). momo ticket 03
  should treat Keep a Changelog as the baseline vocabulary.

## Sources

- changesets: <https://changesets.dev/faq>, <https://changesets.dev/guide/cli>,
  <https://changesets.dev/guide/prereleases>,
  <https://changesets.dev/guide/versioning-and-publishing>,
  <https://changesets.dev/guide/technical-decisions>,
  <https://raw.githubusercontent.com/changesets/changesets/main/docs/adding-a-changeset.md>,
  <https://raw.githubusercontent.com/changesets/changesets/main/docs/config-file-options.md>,
  <https://raw.githubusercontent.com/changesets/changesets/main/docs/prereleases.md>,
  <https://raw.githubusercontent.com/changesets/changesets/main/packages/assemble-release-plan/src/increment.ts>
- towncrier: <https://towncrier.readthedocs.io/en/stable/index.html>,
  <https://towncrier.readthedocs.io/en/stable/cli.html>,
  <https://towncrier.readthedocs.io/en/stable/configuration.html>,
  <https://towncrier.readthedocs.io/en/stable/tutorial.html>
- release-plz: <https://release-plz.dev/docs>,
  <https://release-plz.dev/docs/usage>, <https://release-plz.dev/docs/config>,
  <https://release-plz.dev/docs/changelog>, <https://release-plz.dev/docs/semver-check>,
  <https://release-plz.dev/docs/faq>,
  <https://raw.githubusercontent.com/release-plz/release-plz/main/website/docs/changelog/format.md>
- cargo-release: <https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/reference.md>,
  <https://raw.githubusercontent.com/crate-ci/cargo-release/master/docs/faq.md>
- svu: <https://github.com/caarlos0/svu>,
  <https://raw.githubusercontent.com/caarlos0/svu/main/main.go>,
  <https://raw.githubusercontent.com/caarlos0/svu/main/internal/svu/svu.go>
- semantic-release: <https://raw.githubusercontent.com/semantic-release/semantic-release/master/README.md>,
  <https://semantic-release.gitbook.io/semantic-release/usage/configuration>,
  <https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/get-next-version.js>,
  <https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/definitions/constants.js>,
  <https://raw.githubusercontent.com/semantic-release/semantic-release/master/lib/git.js>,
  <https://raw.githubusercontent.com/semantic-release/commit-analyzer/master/README.md>,
  <https://raw.githubusercontent.com/semantic-release/commit-analyzer/master/lib/default-release-rules.js>,
  <https://raw.githubusercontent.com/semantic-release/git/master/README.md>
- git-cliff: <https://git-cliff.org/docs/>,
  <https://git-cliff.org/docs/configuration/>, <https://git-cliff.org/docs/configuration/bump>,
  <https://git-cliff.org/docs/usage/bump-version>, <https://git-cliff.org/docs/usage/examples>,
  <https://raw.githubusercontent.com/orhun/git-cliff/main/config/cliff.toml>
- scriv: <https://scriv.readthedocs.io/en/latest/>,
  <https://scriv.readthedocs.io/en/latest/commands.html>,
  <https://scriv.readthedocs.io/en/latest/configuration.html>
