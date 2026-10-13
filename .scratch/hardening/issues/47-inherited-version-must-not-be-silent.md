# 47: An unresolvable inherited version must not be silent (15a)

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest
Verified: yes — split out of ticket 15 during triage (batch 4)

## Symptom

When a member manifest inherits its version (`version.workspace = true`) and the
`[workspace.package].version` is not in the same document — i.e. every real Cargo
workspace — `set_version` returns `Ok(None)`. The release reports success while
leaving the manifest untouched. The silence is the bug.

This is the agent-ready slice of ticket 15. The *policy* for a shared workspace
version is a separate, human-owned decision (ticket 15b, still tracked in 15).

## Evidence

`src/adapters/manifest/cargo.rs:121-135` collapses the inherited-but-unresolvable
case to `None`; the test
`an_inherited_version_without_a_workspace_root_is_skipped` (`cargo.rs:254`) pins
that silence as intended.

## Trigger

A standard workspace member (`crates/a/Cargo.toml` with
`version.workspace = true`, root holds `[workspace.package] version = "0.1.0"`),
then `momo -p cargo:a release`. Changelog and `.momo/version` advance;
`crates/a/Cargo.toml` and the root do not, and nothing warns.

## Suggested direction

Make `version_key` return a three-way result — `Key::Package`, `Key::Workspace`
(resolvable in-document), or a new `Inherited` marker — instead of collapsing the
last case to `None`. Thread it through so an inherited-but-unresolvable version
returns a distinct `ManifestError::InheritedVersion { manifest }` rather than
`Ok(None)`.

**Implementation sketch:** add the error variant in `src/domain/manifest.rs`; in
`src/cli/release.rs::sync_manifest`, propagate it (or warn-and-skip, per the
owner's preference — ticket 15b). Tests: repoint
`an_inherited_version_without_a_workspace_root_is_skipped` at the new signal, and
add a release-level test asserting the member is not reported as synced.

## Open questions for triage

- Error or warn? Erroring is honest; warning keeps a release moving in a repo momo
  cannot fully sync. This may be settled by ticket 15b's policy answer, so the
  implementer should check 15b first — but the *signal* is agent-doable either
  way.

## Related

- 15 (parent; 15b is the human policy decision), 02 (dry-run should surface this
  too), 37 (the staging transaction).

## Resolution

`version_key` in `src/adapters/manifest/cargo.rs` is now three-way —
`Key::Package`, `Key::Workspace`, or the new `Key::Inherited` — instead of
collapsing the last case to `None`. `set_version` returns the new
`ManifestError::InheritedVersion { manifest }` (added to
`src/domain/manifest.rs`) for an inherited-but-unresolvable version.

The release path propagates it: `render_manifest` (the read-only replacement for
`sync_manifest`, used by 37's prepare phase) errors, so the member is never
reported as synced and nothing is written. `--dry-run` errors too.

**Decision (error vs warn):** implemented the ticket's primary sketch —
propagate the error. This is honest (momo cannot sync such a manifest) and fits
37's validate-before-mutate shape. Ticket 15b may soften it to a warning once the
shared-workspace-version policy is ruled; the *signal* is in place either way.

**Tests:** `cargo.rs` — the former
`an_inherited_version_without_a_workspace_root_is_skipped` is repointed to
`..._is_reported` and asserts `ManifestError::InheritedVersion`.
`src/cli/release.rs` and `tests/release.rs` — an inherited member version fails
the release with the changelog absent, the version file absent, and the Fragment
intact.
