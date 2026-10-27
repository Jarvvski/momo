# Errors

Every error momo can hand you carries a **code**: a short, namespaced string
like `momo.version.stale-candidate` that is stable enough to grep, document and
allow-list in CI. The code names the *what*; the message says what went wrong
and the `fix` says what to do about it.

An error with no code of its own — an internal failure momo cannot classify —
reports `momo.error.unclassified`. momo never invents a code for a condition a
user cannot act on, so a code in this list always has a fix worth trying.

## Human output

A failed run writes one line to **stderr**, prefixed with its code, and an
indented `fix:` line when there is something to do:

```console
$ momo release
error[momo.version.stale-candidate]: the cut candidate `1.1.0-rc.1` is stale: the changes now imply `2.0.0`
  fix: cut a new candidate for `2.0.0` (`momo candidate`)
```

## JSON output

Under `--format json` the same error is one document on **stdout**, so a machine
consumer can branch on `code` instead of parsing prose. The exit code is still
non-zero, and `--output` is never written on failure.

```json
{
  "error": {
    "code": "momo.version.stale-candidate",
    "message": "the cut candidate `1.1.0-rc.1` is stale: the changes now imply `2.0.0`",
    "fix": "cut a new candidate for `2.0.0` (`momo candidate`)",
    "url": "https://github.com/Jarvvski/momo/blob/main/docs/errors.md#momoversionstale-candidate",
    "context": ["validating the configuration"]
  }
}
```

`context` is the outer chain, outermost first — the `.context(...)` layers that
say *why* momo was doing this. It is empty for a top-level refusal.

## The codes

Each section below names the message momo renders (with the variable parts in
angle brackets) and the fix it suggests.

### `momo.change.empty-text`

A `break`/`add`/`fix` was given no text once whitespace was collapsed.

Message: `a change needs some text`. Fix: give the change some text.

### `momo.change.unknown-kind`

A change Kind name was not `break`, `add` or `fix`.

Message: `unknown kind <value>; expected one of: break, add, fix`. Fix: use
`break`, `add` or `fix`.

### `momo.changelog.duplicate-unreleased`

The Changelog has more than one `## [Unreleased]` heading, so momo cannot tell
which section to record into or promote.

Message: `line <n>: the changelog has more than one Unreleased section; momo
cannot tell which one to use`. Fix: remove the duplicate `## [Unreleased]`
heading.

### `momo.changelog.unversioned-section`

The Unreleased body holds bullets under a Keep-a-Changelog heading momo cannot
read, so a mint would silently under-version them.

Message: `<heading> has <n> entries momo cannot version (line <n>)`. Fix: move
them under `### Breaking`/`### Added`/`### Fixed`, or — if they are not
versioned changes — out of a Keep-a-Changelog heading momo reads.

### `momo.config.empty-path`

A path-valued key in `momo.toml` was empty.

Message: `<key> must not be empty`. Fix: give the key a path.

### `momo.config.invalid-path`

A path-valued key was absolute or escaped upward with `..`.

Message: `<key> must be a relative path without ..: <value>`. Fix: use a
relative path inside the project.

### `momo.config.invalid-rc-format`

`rc_format` is not a usable Release Candidate template.

Message: `invalid value <format> for rc_format`. Fix: use a template with
exactly one `{n}`, e.g. `-rc.{n}`.

### `momo.config.invalid-remote`

The `remote` key is not a URL momo recognises.

Message: `invalid value <value> for <key>; expected a remote URL`. Fix: set it to
a URL like `https://github.com/owner/repo`.

### `momo.config.invalid-seconds`

A duration-valued key is not a positive whole number of seconds.

Message: `<key> must be a positive whole number of seconds, got <value>`. Fix:
set it to a positive whole number of seconds.

### `momo.config.invalid-toml`

`momo.toml` is not valid TOML.

Message: `invalid TOML: <detail>`. Fix: fix the TOML syntax in `momo.toml`.

### `momo.config.invalid-value`

A key has a value momo does not recognise.

Message: `invalid value <value> for <key>; expected one of: <expected>`. Fix: set
the key to one of the listed values.

### `momo.config.unknown-key`

`momo.toml` sets a key momo does not recognise. `tag_kind` is the
migration-critical one: it was removed, and the release notes for the version
that removed it say what replaced it.

Message: `unknown key <key>`. Fix: remove the key, or check the recognised keys
in the [configuration](configuration.md) docs.

### `momo.config.wrong-type`

A key has the wrong type.

Message: `<key> must be a <expected>`. Fix: set the key to the right type.

### `momo.error.unclassified`

momo could not classify the failure. The message is the root cause; the context
is the chain that led to it. If it looks actionable, it is a gap in momo's
codes — please report it.

### `momo.init.exists`

`momo init` refused to overwrite an existing file.

Message: `<path> already exists; refusing to overwrite`. Fix: remove or rename
the existing file.

### `momo.manifest.conflict`

A manifest parses but declares a version momo cannot write without contradicting
it — a version that is both static and dynamic, for instance.

Message: `<path> conflicts: <message>`. Fix: resolve the contradiction in the
manifest.

### `momo.manifest.inherited-version`

A manifest inherits its version from a workspace root that is not in the same
document, so momo cannot resolve the version to write.

Fix: pin an explicit `version` in the manifest, or keep the workspace root in
the repo.

### `momo.manifest.unparseable`

An ecosystem manifest could not be parsed.

Message: `<path> could not be parsed`. Fix: fix the syntax in the manifest.

### `momo.manifest.shared-conflict`

Two Packages would write one shared manifest — a Cargo workspace's
`[workspace.package].version` — to different contents. The last writer would win
and the earlier ones would be silently wrong, so the whole run fails before
anything is written.

Fix: release the packages in separate runs.

### `momo.recording.no-fragment-name`

No free Fragment file name could be found for a Change.

Message: `no free Fragment name under <dir>`. Fix: give the change a different
description.

### `momo.recording.unrecognised-fragment`

A file in the changes directory is not a recognised Fragment, so momo cannot
tell what kind of change it records.

Message: `<path> is not a Fragment: expected <kind>-<slug>.md with <kind> one
of: break, add, fix`. Fix: rename it to `<kind>-<slug>.md`, or move it out of
the changes directory.

### `momo.release.nothing-unreleased`

There is nothing Unreleased to release. `status --check` exits with this code
when it is the CI skip gate; `release` and `candidate` raise it when every
selected Package is already released.

Message: `nothing unreleased` (the `--check` gate) or `nothing unreleased to
release` (a minting run). Fix: record a change with `momo add`, `momo fix` or
`momo break`.

### `momo.selection.ambiguous`

A package name matched more than one Package.

Message: `<query> matches more than one package: <candidates>`. Fix: pass a
fully-qualified package name.

### `momo.selection.conflicting`

`--all` and `--package` were both given.

Message: `--all` and `--package` cannot be combined. Fix: pass either `--all` or
`-p <package>`, not both.

### `momo.selection.not-in-package`

The current directory is not inside any Package.

Message: `no package contains <cwd>; pass -p <package> or --all`. Fix: run from
inside a package, or pass `-p <package>`/`--all`.

### `momo.selection.unknown`

No Package matches the requested name.

Message: `no package matches <query>`. Fix: check the package name with
`momo status --all`.

### `momo.tag.no-vcs`

`momo tag` was run where no `.jj` or `.git` repository exists.

Message: `no VCS detected — nothing to tag`. Fix: initialise a repository (`jj
git init` or `git init`).

### `momo.tag.nothing-to-tag`

`tag` tags what the ledger holds, and only `release`/`candidate` write it.

Message: `nothing to tag: run momo release first`. Fix: run `momo release`
first.

### `momo.tag.stale-version`

The version file names a Version older than the newest tag: a later Version is
already tagged, so tagging this one would publish a release behind it.

Message: `the version file holds <version>, which is older than the newest tag
<latest>; run momo release to advance it before tagging`. Fix: run `momo
release` to advance the version, or delete the version file if it was
hand-written.

### `momo.taskenv.unknown`

The requested task environment is not one momo supports.

Message: `unknown task environment <name>; expected: mise`. Fix: use `mise`.

### `momo.usage.unsupported-flag`

An output flag (`--format`, `--version-only`, `--output`) was passed to a verb
that reports nothing structured.

Message: `<flag> is not supported by momo <verb>`. Fix: drop the flag, or use a
reporting verb (`status`, `release`, `candidate`, `tag`).

### `momo.vcs.invalid-tag-name`

The computed tag name is not one git accepts as a ref.

Message: `<name> is not a valid git tag name; check the package's tag prefix`.
Fix: fix the package's `tag_prefix`.

### `momo.vcs.no-repository`

No `.jj` or `.git` entry was found at or above the starting path.

Message: `no VCS detected at or above <path>`. Fix: initialise a repository at
that path (`jj git init` or `git init`).

### `momo.vcs.tag-exists`

A tag with the requested name already exists; momo refuses to overwrite it.

Message: `tag <tag> already exists`. Fix: delete the tag, or bump the version
with `momo release`.

### `momo.version.invalid`

A version file does not contain a semver version.

Message: `<path> does not contain a valid version: <detail>`. Fix: write a semver
version (e.g. `1.2.3`) into the file.

### `momo.version.stale-candidate`

A cut Release Candidate no longer matches the recomputed Release target: a
`break` landed after the Candidate was cut, so promoting it would release the
wrong version.

Message: `the cut candidate <candidate> is stale: the changes now imply
<target>; cut a new candidate for <target>`. Fix: cut a new candidate for the
target (`momo candidate`).

### `momo.workspace.discovery-failed`

The monorepo graph could not be read while `-p`/`--all` needed it.

Message: `package discovery failed (<detail>); cannot resolve -p/--all`. Fix:
run `mise tasks graph` to see why discovery failed.

### `momo.workspace.duplicate-slug`

Two Packages derive the same slug, so they would share a tag prefix and a
Fragment store.

Message: `packages <identities> share the slug <slug>; give one a distinct slug
override`. Fix: give one package a distinct `slug` override.
