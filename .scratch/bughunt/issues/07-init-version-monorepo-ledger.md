# 07: `init --version` seeds a ledger monorepo Packages never read

Type: bug
Status: resolved
Severity: medium
Area: cli/commands, cli/version
Verified: yes — read at the cited lines

## Symptom

In a VCS-less monorepo, `momo init --version 5.0.0` writes the ledger at the repo root, but each Package reads its own directory's ledger, so the seeded version is silently ignored and the first release bumps from `0.0.0`.

## Evidence

`init` writes `FileVersionSource::new(&root)` (`src/cli/commands.rs:601-604`), where `root` is the repo root. A Package's source is rooted at its own directory (`src/cli/version.rs:94-103`), and its ledger path is `dir(repo_root).join(".momo/version")` (`src/domain/package.rs:138-140`). With a VCS present the ledger is unused, so the divergence only bites the VCS-less monorepo the flag is documented for (`docs/migrating.md`).

## Trigger

A VCS-less mise monorepo (`monorepo_root = true`, `crates/a`), then `momo init --version 5.0.0`; `momo status --format json` reports `cargo:a`'s `current` as `null`, not `5.0.0`.

## Suggested decision

Seed every selected Package's ledger, or seed the repo-root ledger only for the single-package case and fail (or warn) when a monorepo has more than one Package. ## Decision (2026-10-29)

**Seed every selected Package.** `init --version` writes the version into each Package's own ledger, so the flag works as documented in a monorepo; the single-package case (root ledger) is unchanged.

Reproduced: a VCS-less monorepo with `crates/a`, then `momo init --version 5.0.0`, writes `./.momo/version`, but `cargo:a` reads `crates/a/.momo/version`, so `status` reports `current: —`.
