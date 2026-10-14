# 06: `tag` and `status`/`release` derive the current version from different sources

Type: bug
Status: resolved
Severity: medium
Area: domain/version + cli
Verified: yes — read at the cited lines

## Symptom

`status` and `release` read the current version from the VCS tag when a repo
exists; `tag` unconditionally reads `.momo/version`. The two can disagree, and
`release`/`candidate` keep writing `.momo/version` even when a VCS is present, so
the file is a second source of truth that can fall behind the tags.

## Evidence

`src/cli/release.rs:114-119` (`tag`):

```rust
let file = FileVersionSource::new(&package.dir(repo_root));
if !file.exists() {
    return Ok(None);
}
let version = file.current()?;
```

`src/cli/commands.rs:140` (`status`) and `src/cli/release.rs:141` (`release`) both
go through `version::source(..., vcs)` instead.

## Trigger

A clone that has tag `v1.2.3` but no committed `.momo/version`: `status` reports
`current: 1.2.3`, `momo tag` says `nothing to tag`. Conversely a stale
`.momo/version` (`0.9.0`) makes `momo tag` attempt a tag for a version older than
the newest existing tag.

## Suggested direction

Route `tag` through the same `version::source(..., vcs)` seam as `status` and
`release`, or make `.momo/version` purely derived and always regenerated from the
tag. See the design ticket 36.

## Open questions for triage

- Is `.momo/version` meant to be authoritative, derived, or a VCS-less fallback
  only? `issues/01-version-source.md` says "the VCS is authoritative" — the code
  contradicts that for `tag`.

## Triage

**Verdict:** `ready-for-human` — **subsumed by 36; merge and fix there.** The
divergence is real but the fix is not "route `tag` through `version::source`":
`tag` does not want the authoritative *current* version, it wants the version the
last `release`/`candidate` *minted* and has not yet tagged. No tag records that
(a tag is what `tag` is about to create), so `tag` legitimately needs
`.momo/version` even in a VCS repo. What `.momo/version` *is* — a pending-release
ledger that also serves as the VCS-less source — is exactly the model 36 has to
settle.

**Corrections:**
- The first trigger is not a bug: a clone with tag `v1.2.3` and no pending
  `.momo/version` has nothing to tag, so `momo tag` → "nothing to tag" is
  correct.
- "the code contradicts [the VCS is authoritative]" is imprecise: the
  authoritative source answers "what is the current version?" for
  `status`/`release`; `tag` asks a different question, "what did we just mint?".
  The two reads are not the same.
- The genuine defect is the stale-file case (an old `.momo/version` makes `tag`
  attempt a tag behind the newest existing one); `tag` should at least refuse
  when the pending version is not newer than the latest tag.

**Scope:** duplicate of 36 (with 07). Keep this file as the concrete symptom; the
decision and the code change land under 36.

## Decision (owner, 2026-10-14)

**Subsumed by ticket 36** (pending-release ledger). Implement there.

## Resolution

Landed with 36. `tag` still reads `.momo/version` — the version it tags is one no
tag records yet, so the tags cannot supply it — but the ledger's role is now
documented as the pending-release ledger, and the reads are one rule: `tag` goes
through the same `VersionSource` seam (`FileVersionSource::current()`), and
`commands::tag` no longer re-reads the file.

The genuine defect is fixed: `tag` refuses when the ledger is older than the
newest tag, so a stale `.momo/version` can no longer tag a version behind the
newest existing one. The "a clone with tag `v1.2.3` and no ledger has nothing to
tag" case was already correct and is unchanged.
