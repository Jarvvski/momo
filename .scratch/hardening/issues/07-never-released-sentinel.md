# 07: `0.0.0` is both the "never released" sentinel and a legal version

Type: bug
Status: ready-for-agent
Severity: low
Area: domain/version
Verified: yes — read at the cited lines

## Symptom

The version sources model "this package has never released" as the magic value
`0.0.0`, which is also a perfectly legal release version. Nothing downstream can
tell the two apart.

## Evidence

`src/domain/version.rs:165-172`:

```rust
impl VersionSource for TagVersionSource<'_> {
    fn current(&self) -> Result<Version, VersionError> {
        Ok(self.vcs.latest_tag(self.prefix)?
            .unwrap_or_else(|| Version::new(0, 0, 0)))
    }
}
```

`FileVersionSource` does the same. `tag` works around it by checking
`file.exists()` (`src/cli/release.rs:115`), but `status` cannot: a package that
has genuinely released `0.0.0` is indistinguishable from one that has never
released.

## Trigger

A repo tagged `v0.0.0` with nothing unreleased reports the same `current` as a
repo with no tags at all.

## Suggested direction

Model absence explicitly — `Option<Version>`, or a dedicated bootstrap state on
the version source — rather than a sentinel that collides with a real version.

## Open questions for triage

- Does any user-visible behaviour actually change if this is modelled properly, or
  is it purely a modelling cleanliness fix? If the latter, it may belong with the
  refactor ticket 36 rather than standing alone.

## Triage

**Verdict:** `ready-for-human` — **subsumed by 36; merge and fix there.** The
collision is real but the user-visible impact is small, so it is a modelling fix
that belongs with the 36 refactor rather than a standalone change.

**Corrections:** the claim is accurate. Nuance: `status`/`release`'s *next*
version is identical whether `0.0.0` was really released or is the bootstrap
baseline (both use `0.0.0` as the bump base), so only the displayed `current` and
`tag`'s `exists()` workaround can differ. The fix — model absence as
`Option<Version>` or a bootstrap state on the source — is the same shape as 36's
"one source of truth" change.

**Scope:** subsumed by 36 (with 06).

## Decision (owner, 2026-10-14)

**Subsumed by ticket 36** (pending-release ledger). Implement there.
