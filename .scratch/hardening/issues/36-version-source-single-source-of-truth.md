# 36: One source of truth for a package's version

Type: refactor
Status: resolved
Severity: medium
Area: domain/version + cli
Verified: yes — the divergence is read at the cited lines

## Symptom

A package's current version has two sources: the VCS tag and `.momo/version`.
`release`/`candidate` write the file even when a VCS is present; `status`/`release`
read the tag; `tag` reads the file. They can disagree (tickets 06, 07).

The spec (`issues/01-version-source.md`) settles it — "the VCS is authoritative" —
but the implementation has a second, drift-prone store.

## Evidence

`src/domain/version.rs:136-172` declares the seam ("The VCS is authoritative when
present; the file-backed implementation in the CLI is the fallback for a project
with no VCS"), yet `release.rs:190-192` always writes the file and `release.rs:114`
always reads it for `tag`.

## Suggested direction

Decide the model and enforce it:
- **derived:** `.momo/version` is always regenerated from the tag, never read as a
  source; or
- **fallback-only:** the file is read only when there is no VCS, and `tag` routes
  through the same `version::source(..., vcs)` seam as `status`/`release`.

Either way, one function answers "what version is this package at?".

## Open questions for triage

- Which model? "Derived" is cleaner but means momo writes a file even when it does
  not need to. "Fallback-only" keeps the file for VCS-less repos but leaves it
  stale in VCS repos unless written.
- Does `momo tag` need `.momo/version` at all when a VCS exists?

## Related

- 06 (version-source divergence), 07 (never-released sentinel), 22 (task_env),
  37 (staged release).

## Triage

**Verdict:** `ready-for-human` — a design decision, and the **parent of 06 and
07**. `spec.md` §5 ("The VCS is authoritative when present; `.momo/version` is
the fallback") is incomplete: it never says where the *minted-but-untagged*
version lives, yet `momo tag` needs exactly that, and no tag can supply it. So
the "fallback-only" reading the spec suggests is not implementable for `tag`;
either `.momo/version` is redefined as a pending-release ledger (still written in
VCS repos, but never the source for `status`/`release`), or `tag` derives its
target elsewhere (e.g. the newest changelog heading). Pick one, then update
`spec.md`/`GLOSSARY.md` and fold in 06/07.

**Corrections:** the premise is only half right. `release.rs:190-192` writing the
file is not by itself a contradiction of "the VCS is authoritative" — that write
is what lets `tag` name the version just minted. The gap is that the spec never
models a pending release. Also note `tag` reads the file directly
(`release.rs:114`) while `commands.rs:254` reads it again through
`FileVersionSource` after `release::tag` — same source, but a second copy of the
rule.

**Scope:** parent; 06 and 07 are subsumed and should be closed into it. Not a
duplicate of 04 (RC-bump semantics), though 04's fix depends on where the base /
last release is tracked.

**Implementation sketch (design first):** choose the model, then
- if **pending-release ledger:** keep writing `.momo/version` on
  `release`/`candidate`; make `status`/`release` read only the tag source; rename
  the file's role in docs (a pending-release record, not a version source).
- if **derived:** stop writing `.momo/version` in a VCS repo; `tag` reads the
  newest released changelog heading (needs a `Changelog` → `tag` seam).
- either way: one function answers "what version is this package at?", and 06/07
  fall out of the same change.

## Decision (owner, 2026-10-14)

**Pending-release ledger.** `.momo/version` keeps being written on
`release`/`candidate`, and its documented role becomes the record of the last
minted release — not a version source. `status` and `release` read only the tag
source; `tag` reads the ledger, because the version it must tag is by definition
one no tag records yet. Document the split; tickets 06 and 07 fall out of the
same change.

## Resolution

Landed the pending-release ledger. `.momo/version` is documented as the record of
the last *minted* version — a Release or a Release Candidate — written by
`release`/`candidate` and read by `tag`; `status`/`release` read the tag source.
`docs/usage.md`, `docs/ci.md`, `GLOSSARY.md` and `spec.md` §5 record the split.

- `VersionSource` now answers two questions: `current()` (the newest tag, or the
  ledger with no VCS — an `Option<Version>`) and `last_release()` (the newest
  stable Release). The `Vcs` port gained `latest_release_tag`, which the git and
  jj adapters implement by filtering Candidate tags (`tags::latest_release`).
- `tag` reads the ledger through the same source and **refuses a ledger older
  than the newest tag**, so a stale `.momo/version` can no longer tag a version
  behind the newest one — the genuine defect from 06.
- `release::tag` returns the Version it names, so `commands::tag` no longer
  re-reads the file (the second copy of the rule noted in triage).

Tests: `status_reads_the_tag_while_tag_reads_the_ledger` and
`tag_refuses_a_ledger_older_than_the_newest_tag` (integration), plus the
version-source unit tests and `tags::latest_release` tests.
