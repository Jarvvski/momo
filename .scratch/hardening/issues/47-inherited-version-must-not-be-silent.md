# 47: An unresolvable inherited version must not be silent (15a)

Type: bug
Status: ready-for-agent
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
