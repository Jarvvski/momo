# momo

momo is a versioning CLI: it turns a project's accumulated changes into semver
releases, a changelog, and VCS tags.

## Language

**Change**:
One recorded unit of work, classified by exactly one Kind.
_Avoid_: entry, item, commit

**Kind**:
The classification of a Change — one of `break`, `add`, `fix`. (The original
word was "noun"; `break`/`add`/`fix` are verbs, so "Kind" is preferred.)
_Avoid_: noun, type, category

**Bump**:
The semver increment a set of Changes implies, given the current version.
_Avoid_: increment, delta

**Unreleased**:
The Changes recorded since the last Release — the raw material a Release is
minted from.
_Avoid_: pending, staged, draft

**Release**:
A minted, immutable version of the project, formed from the Unreleased Changes
and stamped with a timestamp.
_Avoid_: publish, ship

**Release Candidate (RC)**:
A pre-Release version, marked with an incrementing suffix, cut before the final
Release.
_Avoid_: prerelease, beta

**Changelog**:
The durable record of Releases and their Changes, grouped by Kind, newest first.
_Avoid_: release notes, history

**Task Environment**:
An external task runner momo integrates into (e.g. `mise`), so a project can run
momo's commands through its own tooling.
_Avoid_: build system, task runner
