# 01: A release is not atomic, and its destructive steps run first

Type: bug
Status: wontfix
Severity: high
Area: cli/release
Verified: yes — read at the cited lines

## Symptom

`momo release` mutates the repository in several steps, in an order that puts the
least-recoverable step in the middle. A failure after that point leaves the repo
half-released, and the natural retry refuses to finish the job.

## Evidence

`src/cli/release.rs:176-197`:

```rust
if !dry_run { fs::write(&changelog_path, changelog.render())?; }   // 1. changelog promoted
let consumed = match mode { Mode::Release => recording::consume(...)?, ... };  // 2. fragments DELETED
if !dry_run { FileVersionSource::new(&package_dir).write(&version)?; }         // 3. .momo/version bumped
let manifest = sync_manifest(&package_dir, package, &version, dry_run)?;       // 4. Cargo.toml last
```

Every write is a bare `fs::write` (truncate + write). There is no temp-file +
rename, and no rollback.

## Trigger

In fragments mode, make step 4 fail — a read-only `Cargo.toml`, or invalid TOML
that `set_version` cannot parse. The changelog is already promoted, the fragments
are already deleted, and `.momo/version` already holds the new version. Re-running
`momo release` now finds no unreleased changes and exits
`nothing unreleased to release`. The manifest is permanently out of sync with the
changelog and version file.

This is the unattended GitHub Actions path, so nobody is watching to intervene.

## Suggested direction

Stage every write and commit them together: render each artefact to a temp file
in its target directory, then rename over the target (atomic on POSIX), and do the
fragment deletion last. At minimum, reorder so nothing destructive happens before
every failure-prone step has been validated.

## Open questions for triage

- Is a full staged transaction in scope now, or is reordering + temp-file writes
  enough for a first fix? (See ticket 37.)
- Should a partially-released repo be *detectable* so the retry can finish rather
  than refuse?

## Triage

**Verdict:** Real and correctly diagnosed, but not independently actionable — the
fix is the staged transaction in 37, so this is folded in and closed as
superseded.

**Corrections:** The body is accurate. It under-states one thing: a *reorder
alone* is not a safe fix. Moving the manifest write ahead of
`recording::consume` (the obvious "put the destructive step last" fix) makes the
retry *worse*: `.momo/version` would already hold the new version, so a retry
computes `release_version(changes, new)` and mints the *next* bump (e.g. `0.2.0`)
rather than finishing `0.1.0`. Only a validate-everything-then-commit design
avoids both the half-release and the version skip — which is why 37's
transaction, not a reorder, is the fix.

**Scope:** Duplicate/merged. Implement as part of 37 (same code region, same
change); do not action separately. 37's acceptance criteria include this
ticket's trigger.

**Implementation sketch:** See 37.
