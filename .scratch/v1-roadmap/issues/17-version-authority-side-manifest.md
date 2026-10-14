# 17: Promote `.momo/version` to a version authority

Type: task
Status: wontfix
Points: — (rejected)
Bucket: Ecosystem manifest adapters
Severity: low
Area: domain/version + cli/version
Verified: yes — the write-only nature of the port is read at the cited lines

## Question

Should momo adopt release-please's side-manifest semantics — a committed
`.momo/manifest.json` mapping identity → last-released version, read as the current
version — so "no adapter is blocked by a version it cannot read"?

## Answer

**No.** The premise is a misconception for momo, and the change fights the model.

**momo's manifest adapters are write-only.** `ManifestSync::render` is handed the
target `Version` (`src/domain/manifest.rs:63-67`); the *current* version always
comes from `TagVersionSource`/`FileVersionSource` (`src/cli/version.rs:94-103`),
never from a manifest. So **no adapter is blocked by a version it cannot read** —
release-please needs a side manifest because it *reads* the manifest to learn the
current version; momo never does.

The promotion is rejected on four further grounds:

1. **Tags are the release artefact** — what consumers resolve and CI reads. A side
   file is a shadow.
2. **A committed ledger is a merge-conflict magnet and a second source of truth.**
   The codebase already treats ledger-vs-tag drift as an error (`tag.rs:42-49`
   refuses a ledger older than the newest tag) — proof the team's model makes tags
   truth.
3. **It breaks adoption.** `docs/migrating.md:22` promises "reads the latest tag,
   nothing to import". A fresh adopter with existing tags would have no side file
   and look never-released.
4. **It is a vocabulary break.** `GLOSSARY.md:21` and `docs/usage.md:130` define the
   ledger precisely as *not* a version source, and `src/cli/version.rs:6-11` says so
   in code.

## What actually delivers the need

The real gap is **visibility**, not authority: when an adapter *skips* (dynamic
version), the user cannot tell "momo correctly did nothing" from "momo found no
manifest". That is fixed by ticket 14's `SkipReason`, surfaced by tickets 04 and 08 —
no new version source.

If release-please semantics were ever genuinely wanted, the correct shape is **one**
committed `.momo/manifest.json` read *in addition to* tags with tags winning on
disagreement — because a monorepo needs one atomically-reviewable file, not
per-package ledgers. Recommend against; no demand, and it fights the model.

## Related

- 14 (the `SkipReason` that delivers the real need), 04, 08.
- `GLOSSARY.md:21`, `docs/usage.md:130`, `src/cli/version.rs:6-11`.
