# 05: Two Packages in one directory share a Changelog and a version ledger

Type: bug
Status: ready-for-human
Severity: medium
Area: domain/workspace, domain/package
Verified: yes — reproduced against real mise (two projects at one root)

## Symptom

When mise reports two projects at the same root — a Cargo package and a Node package both at the repo root, say — `momo add` from that directory silently records into the last one, and both Packages resolve to the same `CHANGELOG.md` and `.momo/version`. Nothing reports the collision.

## Evidence

- `Workspace::select`'s CWD default filters by containment and takes `max_by_key(dir.components().count())` (`src/domain/workspace.rs:224-232`). Two Packages at one directory match with equal depth, and `Iterator::max_by_key` returns the last maximum — no ambiguity error, unlike the `-p` path (`workspace.rs:202-220`).
- `Package::changelog` and `version_file` are `dir(repo_root).join(…)` (`src/domain/package.rs:121-123`, `:138-140`), so same directory means same files.
- `check_unique_slugs` (`workspace.rs:157-177`) groups by slug only, and the two identities have distinct slugs, so it cannot see it.

## Trigger

A repo whose root is both a Cargo package and a pnpm workspace root; mise emits `cargo:momo-tools -> .` and `node:root-tools -> .`. `momo add` from the root records into `node:root-tools` only; both Packages read and write the same changelog and ledger.

## Suggested direction

This needs a ruling. The conservative fix, consistent with the duplicate-slug check, is to refuse: extend the uniqueness check to the Package *directory*, or make the CWD default report `SelectionError::Ambiguous` when the deepest match is not unique. The alternative is per-package namespacing of the changelog and version file. Recommend refuse-and-name, matching the existing slug guard; it fails loudly instead of silently picking a package.
