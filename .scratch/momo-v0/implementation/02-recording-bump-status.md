# 02: Recording Changes, the Bump policy, and `status`

**What to build:** `break`/`add`/`fix` record a Change, and the `recording`
setting selects the store. In changelog mode a Change lands under
`## Unreleased`, grouped by Kind; in fragment mode it lands at
`.momo/changes/<kind>-<slug>.md`. `momo status` lists the Unreleased Changes and
the version they imply. Introduces the package-aware model — a single-package
repo is a one-Package monorepo — and a version-source seam whose only
implementation for now reads `.momo/version`, falling back to `0.0.0`.

The Bump rules: at MAJOR 0, `break` and `add` bump MINOR and `fix` bumps PATCH;
at MAJOR ≥ 1, `break` bumps MAJOR, `add` MINOR, `fix` PATCH. When a Release
contains several Kinds, the highest Kind wins.

**Blocked by:** 01

**Spec:** `.scratch/momo-v0/spec.md` §4, §5, §6, §9

**Status:** resolved

- [x] `momo add "text"`, `momo fix "text"` and `momo break "text"` each record exactly one Change.
- [x] With `recording = "fragments"` each Change becomes `.momo/changes/<kind>-<slug>.md`; with `changelog` it appears under `## Unreleased` in the right Kind section.
- [x] `momo status` prints the Unreleased Changes and ends with `→ next: <version>`.
- [x] The Bump policy is unit-tested across `0.0.0`, `0.x.y`, `1.x.y`, and a Release containing several Kinds.
- [x] `--dry-run` on the recording verbs writes nothing.
