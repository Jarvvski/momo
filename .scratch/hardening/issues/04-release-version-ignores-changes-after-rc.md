# 04: A pre-release current version suspends the "highest Kind wins" rule

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/version
Verified: yes — read at the cited lines

## Symptom

`release_version` returns the RC's `x.y.z` and ignores the change set entirely
whenever `current` carries a pre-release suffix. The change set is never compared
against the RC's target, so a change recorded *after* cutting an RC ships under a
version the bump policy says is too low.

## Evidence

`src/domain/version.rs:75-84`:

```rust
pub fn release_version(changes: &[Change], current: &Version) -> Option<Version> {
    if changes.is_empty() {
        return None;
    }
    if current.pre.is_empty() {
        next_version(changes, current)
    } else {
        Some(Version::new(current.major, current.minor, current.patch))
    }
}
```

The reuse is only correct while the RC target still dominates the implied bump.

## Trigger

`release_version` discards `changes` whenever `current` carries a pre-release
suffix, so any Kind that raises the implied Bump above the RC's target is lost.

- **MAJOR ≥ 1:** released `1.0.0`, cut `1.1.0-rc.1` from an `add`, then record a
  `break`. The change set now implies MAJOR (`2.0.0`), but `momo release` mints
  `1.1.0` (and `momo candidate` cuts `1.1.0-rc.2`, not `2.0.0-rc.1`).
- **MAJOR 0:** there is no such case. `break` and `add` both imply MINOR and the
  RC target was already the MINOR result, so no later Kind can move it. (The
  original report's `0.1.0-rc.1` + `break` → `0.2.0` example is wrong: it bumps
  from the RC's own components, double-counting the bump.)

The existing test only covers the RC-with-no-new-changes case (`version.rs:306`).

## Suggested direction

Recompute the target from the last *release* — the base the RC was cut from —
and reuse the RC's target only when it is at least that. The naive "take the
greater of the RC target and `next_version(changes, current)`" is **wrong**:
`next_version` bumps from the RC's own components, so for an unchanged change set
it returns the *next* bump above the target (`1.1.0-rc.1` + `{add}` → `1.2.0`)
and would double-bump. The base is what the code currently discards when it
overwrites `.momo/version` with the RC — see 36.

## Open questions for triage

- What *should* happen when a `break` lands after an RC — a new RC at the higher
  target, or a release at the higher target? The rule needs stating in the spec /
  `GLOSSARY.md`, not just the code.
- Should a hand-made non-RC pre-release (e.g. `1.2.3-alpha.1`) be treated as an RC
  of its target at all?

## Triage

**Verdict:** `ready-for-human` — the bug is real, but the fix is a semantics
decision, not a mechanical one. `issues/05-release-candidates.md` answers the
*outcome* ("if the target changes (a new kind moves the bump), the counter
restarts at `1`"; the final `release` reuses the changes and drops the suffix)
but not the *computation model*, and it never says what a direct `release`
(rather than a fresh `candidate`) mints after a new Kind lands. That rule belongs
in `spec.md` / `GLOSSARY.md`, as the ticket's own open question says.

**Corrections:**
- The MAJOR-0 trigger is wrong (see Trigger): at MAJOR 0 no Kind after an RC can
  move the target. The real bug needs MAJOR ≥ 1.
- The suggested direction is unsound as written — `next_version(changes,
  current)` double-counts. The comparison is against the last *release*, not the
  RC's components.
- `momo candidate` is affected too, not just `momo release`: both go through
  `release_version`, so a moved target never restarts the counter as the spec
  requires.

**Scope:** no split. Overlaps 36 (where the base / last release is tracked) and,
through it, 06 and 07. Should land with 36 or immediately after.

**Implementation sketch (once ruled):** give the version seam a way to report the
last *release* separately from a pending RC — e.g. `VersionSource` returns the
base and `release_version` recomputes `next_version(changes, base)` — instead of
stripping the suffix off `current`. Keep the RC's target only to drive the
candidate counter (`next_candidate`). Domain tests in `version.rs`:
`1.1.0-rc.1` + `{add, break}` → `2.0.0`; `1.1.0-rc.1` + `{add}` → `1.1.0`
(unchanged); `0.1.0-rc.1` + `{add, break}` → `0.1.0` (unchanged).

## Decision (owner, 2026-10-14)

**Recompute the target from the last release.** The release target is always
`next_version(changes, last_release)`, whether or not an RC exists. If a cut RC's
target disagrees with the recomputed one, fail and tell the user to cut a new RC
rather than shipping the stale target.

Depends on ticket 36's pending-release-ledger model for "the last release".
