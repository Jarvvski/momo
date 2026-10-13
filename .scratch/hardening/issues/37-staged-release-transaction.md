# 37: A staged release transaction

Type: refactor
Status: ready-for-agent
Severity: high
Area: cli/release
Verified: yes — the current ordering is read at the cited lines

## Symptom

A release mutates the changelog, the fragment store, `.momo/version` and the
manifest across four independent writes with no staging and no rollback (ticket
01). Any failure leaves an inconsistent repo, and the retry refuses to finish.

## Evidence

`src/cli/release.rs:176-197` — the write order, each a bare `fs::write`.

## Suggested direction

Introduce a small "release transaction": validate everything first (including the
manifest — ticket 02), render each artefact to a temp file in its target
directory, then rename them all into place, deleting fragments last. On any
failure before the rename phase, nothing on disk has changed.

This closes tickets 01, 02 and 35 (for the release path), and removes the
"half-released, retry says nothing unreleased" trap.

## Open questions for triage

- How much machinery is warranted? A full transaction type may be over-engineering
  for four writes; "validate all, then write all via temp+rename" may be enough.
- Rename is atomic per file but not across files. Is per-file atomicity sufficient,
  or does the changelog/version/manifest trio need a journal?
- Interacts with ticket 15, which may write a manifest *outside* the package
  directory.

## Related

- 01, 02, 15, 35.

## Triage

**Verdict:** ready-for-agent — the canonical release-integrity ticket. It
subsumes 01 (ordering/atomicity) and 02 (validate-before-mutate, incl. dry-run
reporting); implement it and close those two. It does **not** close 35 (see
Corrections).

**Corrections:** The line "This closes tickets 01, 02 and 35 (for the release
path)" is imprecise. 37 does not touch 35: 35 is about `momo add`'s recording
writes (the changelog read-modify-write and the fragment `exists()`-then-write
TOCTOU in `src/cli/recording.rs`), which the release transaction never exercises
— the release path only *deletes* Fragments (`recording::consume`). The two
share the need for an atomic-write helper, nothing more.

**Scope:** One ticket. 01 and 02 are its symptoms/acceptance criteria; they are
closed as superseded. Shares an atomic-write helper with 35 (build it once).
Interacts with 15: a workspace-inherited manifest can live *outside* the package
directory, so temp files must be created beside each target, never in one staging
directory.

**Open-question rulings:**
- Machinery: no journal type. "Validate everything, then commit via temp-file +
  rename, deleting Fragments last" is the right size.
- Per-file atomicity is sufficient. The residual — a process killed between the
  renames, mid-commit — is a narrow window; with Fragments deleted last a retry
  is safe on the version source but could re-promote the changelog. Treat
  crash-mid-commit as out of scope / a follow-up, not a reason to build a journal.

**Implementation sketch:**
- **Files:** `src/cli/release.rs` (restructure `run()`); a new tiny helper module,
  e.g. `src/cli/atomic.rs`, with `write_atomic(target, contents)` (temp file in
  `target.parent()`, then `fs::rename`; remove the temp on error) — shared with
  35; `src/domain/manifest.rs` + `src/adapters/manifest/cargo.rs` to add a
  read-only render/preview; `src/cli/output.rs` if the dry-run report gains a
  manifest line.
- **Shape — split `run()` into prepare/commit:**
  - *Prepare (touches nothing):* read the changelog, compute the target version,
    render the changelog text; `locate` the manifest and render it (parse +
    `set_version`) to get the new text and a `ManifestUpdate`; collect the
    Fragment paths to consume; render the version-file text. Write each rendered
    artefact to a temp file beside its own target.
  - *Commit:* `fs::rename` each temp over its target, then delete the Fragments
    last.
  - Any prepare error ⇒ remove the temps and return `Err`; the tree is untouched.
  - `--dry-run` ⇒ run prepare, remove the temps, and build `Outcome` from the
    plan — including `manifest: Some(ManifestUpdate { manifest, stale_lockfile })`
    so `warn_stale_lockfile` and the report fire.
- **Manifest read-only step:** cleanest is to split the port — add
  `ManifestSync::render(&self, manifest, version) -> Result<Option<(String,
  ManifestUpdate)>, ManifestError>` (read + parse + render, no write) and
  implement `write` on top of it. A port-free alternative is to copy the manifest
  to a sibling temp and call `write` on the copy; prefer the port split.
- **Tests to add:**
  - `src/cli/release.rs` unit: an invalid-TOML manifest makes `release` return
    `Err` and leaves the changelog absent, the Fragments present and
    `.momo/version` absent (the 01 trigger). `--dry-run` with invalid TOML ⇒
    `Err`. `--dry-run` with a valid manifest ⇒ `outcome.manifest.is_some()` and
    no file written.
  - `tests/release.rs` integration: a read-only `Cargo.toml` makes `release`
    fail with nothing promoted; chmod writable, then the retry succeeds.
    `release --dry-run` on a malformed `Cargo.toml` fails.
  - Update the existing `a_dry_run_release_writes_no_manifest` expectation: the
    manifest is now *reported* under dry-run, still unwritten.
