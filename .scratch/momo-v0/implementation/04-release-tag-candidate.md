# 04: `release`, `tag`, and `candidate`

**What to build:** The Release lifecycle. `momo release` mints the version from
the Unreleased Changes, promotes `## Unreleased` to `## [x.y.z] - YYYY-MM-DD`,
consumes the Fragments, writes the reference-link footer when a remote is known,
and updates `.momo/version`; once a VCS is present the tag becomes the
authoritative version source. It does not commit and does not tag. `momo tag`
creates the tag for the current version — annotated on git, lightweight on jj —
and errors clearly when no VCS is present. `momo candidate` (alias `rc`) Bumps
exactly as a Release would, then appends `-rc.N`, incrementing per target version
and sharing Fragments with the eventual Release.

**Blocked by:** 02, 03

**Spec:** `.scratch/momo-v0/spec.md` §6, §7, §8

**Status:** resolved

- [x] `momo release` writes the Changelog heading with the minted version and today's date, and removes the consumed Fragments.
- [x] `momo release` leaves the working copy otherwise untouched: no commit, no tag.
- [x] The next `momo release` takes its version source from the tag the previous `momo tag` created.
- [x] `momo candidate` produces `x.y.z-rc.1`, then `-rc.2`, and restarts at `1` when the target version changes.
- [x] `momo tag` errors clearly when no VCS is present.
- [x] `momo release` errors when there is nothing Unreleased.
