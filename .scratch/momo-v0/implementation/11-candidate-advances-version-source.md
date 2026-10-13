# 11: A Candidate advances the version source

**What to build:** `momo candidate` must write `.momo/version` exactly as
`momo release` does. Without it, `momo tag` immediately after a candidate has
nothing to name — it reports the last Release's version, or errors when there has
never been one — so an RC can never be tagged. That defeats the point of cutting
one: a release candidate exists to be built and tagged before the final Release.

This is a spec conformance fix. §5 says `.momo/version` is "written on every
release"; §7 says a Candidate makes "the same file writes as `release`"; and the
release-candidate decision repeats "the same file writes as `release`
(changelog + manifests)". Ticket 04 read that as "changelog only" and left the
version file unwritten.

The machinery is already in place: the release path reads the current version
through `release_version`, which drops an `-rc.N` suffix, so the final Release
reuses the Candidate's target rather than bumping again. Only the write was
missing.

**Blocked by:** 04

**Spec:** `.scratch/momo-v0/spec.md` §5, §7

**Status:** resolved

- [x] `momo candidate` writes `.momo/version` with the `-rc.N` version.
- [x] `momo tag` immediately after a candidate creates the tag for that candidate.
- [x] The final `momo release` reuses the candidate's target and drops the suffix.
- [x] `--dry-run` still writes nothing.
