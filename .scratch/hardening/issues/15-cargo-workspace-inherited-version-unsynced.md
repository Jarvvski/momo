# 15: Monorepo members with an inherited version are silently never synced

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest
Verified: yes — read at the cited lines; the behaviour is pinned by an existing test

## Symptom

For a real Cargo workspace, a member's `Cargo.toml` has
`version.workspace = true` and `[workspace.package]` lives in the *root* manifest.
`version_key` resolves the inherited form by looking for `[workspace.package]` in
the **member's own** document — which by definition does not have it — so it
returns `None` and the manifest is skipped. `momo release` reports success while
leaving the member (and the workspace version) untouched.

## Evidence

`src/adapters/manifest/cargo.rs:121-140`:

```rust
if item.get("workspace").and_then(Item::as_bool) == Some(true) {
    return workspace_version(doc).map(|_| Key::Workspace);   // doc is the MEMBER
}
...
fn workspace_version(doc: &DocumentMut) -> Option<&Item> {
    doc.get("workspace")?.get("package")?.get("version")
}
```

The test `an_inherited_version_without_a_workspace_root_is_skipped`
(`cargo.rs:255`) asserts this `None` as intended behaviour.

## Trigger

A standard workspace:

```
# Cargo.toml (root)
[workspace]
members = ["crates/a"]
[workspace.package]
version = "0.1.0"

# crates/a/Cargo.toml
[package]
name = "a"
version.workspace = true
```

`momo -p cargo:a release` writes the changelog and `.momo/version` and leaves
`crates/a/Cargo.toml` (and the root) at `0.1.0`. `tests/monorepo.rs:77` only ever
uses an explicit `version = "0.1.0"`, so this is untested.

## Suggested direction

Resolve the workspace root from the member (walk up to the manifest declaring
`[workspace]`) and rewrite `[workspace.package].version` there. At minimum return
a distinct "inherited, skipped" signal so the release can warn rather than
silently under-sync.

## Open questions for triage

- Should momo follow the workspace root outside the package directory? That is a
  write outside the package, which the release ceremony may need to treat
  specially (and it interacts with ticket 01's staging).
- Also consider a root manifest that is both a package and a workspace: today
  `[package].version` unconditionally wins over `[workspace.package].version`.

## Triage

**Verdict:** Confirmed, but the ticket understates the problem: this is a model
conflict, not just a missed sync. `version_key` (`cargo.rs:121-135`) resolves the
inherited form by calling `workspace_version(doc)` on the *member's own*
document, which by construction has no `[workspace.package]`, so it returns
`None` and `set_version` returns `Ok(None)` (`cargo.rs:82-84`). The release then
reports success and silently leaves both the member and the root at the old
version. `an_inherited_version_without_a_workspace_root_is_skipped`
(`cargo.rs:254`) pins that silence. The second open question is also real: when a
root manifest is both `[package]` and `[workspace]`, `version_key` returns
`Key::Package` unconditionally (`cargo.rs:122-128`), so the workspace version is
ignored.

**Corrections:** A `[workspace.package].version` is a *single shared* version for
every member, while momo versions each package independently (ticket 16,
`GLOSSARY`). "Walk up and rewrite the root" is therefore not a bug fix but a
semantic decision: it would make one member's release silently change every
sibling's effective version, and under `momo release --all` two members sharing a
root would each write the same scalar with their own target (last write wins,
values may differ).

**Scope:** Split.

- **15a (ready-for-agent):** stop the silence — detect the inherited form that
  cannot be resolved and surface it (error or warning) instead of `Ok(None)`.
- **15b (ready-for-human):** decide the policy for a shared workspace version
  under per-package versioning: refuse, treat the workspace as one package, or
  allow a root write with an explicit conflict rule.

**Writing outside the package directory is not acceptable as-is.** `ManifestSync`
is contractually rooted at `package_dir` (`domain/manifest.rs:30-42`) and
`CargoManifest::locate` joins `package_dir` (`cargo.rs:29-34`); the release
transaction (01/37) stages each artefact's temp file "in its target directory"
and assumes every artefact belongs to one package. A root write puts a second
package's territory inside one package's transaction and breaks the per-file
atomicity story when `--all` touches the same root twice. If 15b lands on "write
the root", it needs a one-writer-per-workspace-root rule and a constraint that
the shared version equal the member's target before 01/37 can stage it.

**Implementation sketch (15a):** In `src/adapters/manifest/cargo.rs`, make
`version_key` return a three-way result — `Key::Package`, `Key::Workspace`
(resolvable in-doc), or a new `Inherited` marker — instead of collapsing the
last case to `None`. Thread it through `set_version` so an inherited-but-
unresolvable version returns a distinct
`ManifestError::InheritedVersion { manifest }` (new variant in
`domain/manifest.rs`) rather than `Ok(None)`. In
`src/cli/release.rs::sync_manifest`, propagate it (error, or a warning + skip if
the owner prefers non-fatal). Tests: repoint
`an_inherited_version_without_a_workspace_root_is_skipped` at the new signal; add
a release-level test asserting the member is not silently reported as synced.

## Decision (owner, 2026-10-14)

**Support the shared version.** Resolve the workspace root from the member — walk
up to the manifest declaring `[workspace]` — and rewrite
`[workspace.package].version` there.

Because that scalar is shared by every member, define the `--all` rule: a release
may only write it when every affected member resolves to the same target;
otherwise fail rather than let the last writer win. This is the 15b policy
decision, now made.

## Resolution

Implemented the Decision; the ticket's `## Triage` split (15a/15b) is superseded
— 15a's `InheritedVersion` error is now only the last resort when no workspace
root can be resolved at all.

**Resolving the root.** `CargoManifest::render` walks up from the member to the
nearest ancestor `Cargo.toml` declaring `[workspace]` and rewrites its
`[workspace.package].version`. A root that declares `[workspace]` but no shared
version is still unresolvable, so it keeps returning
`ManifestError::InheritedVersion` — the silence is gone either way.

**Writing outside the package directory.** `ManifestSync::render` now returns
the manifest it actually wrote in `ManifestUpdate::manifest` (previously it
echoed the input path), and `CargoManifest::write` writes that path rather than
the one it was handed. `ManifestUpdate::manifest` already documented itself as
"the manifest written", so no new port type was needed. The release path is
unchanged in shape: it already stages at `update.manifest`, so a root write
lands in the same staged transaction (`src/cli/atomic.rs`) as the changelog and
version file, and never bypasses it. The port's `locate` stays rooted at
`package_dir`; only the write target can escape it.

**The `--all` rule.** The release path was split into a read-only `prepare`
phase and a `commit` phase (`release::prepare_release` / `prepare_candidate` /
`commit`), so a run resolves every Package before writing any of them.
`reject_shared_manifest_conflicts` then refuses the run when two Packages would
write one manifest with different results, naming both Packages, the manifest
and the two targets. Two members that agree on the target are allowed and
dedupe to one identical write. A root that is both `[package]` and `[workspace]`
and a member of it targeting the same version but different scalars is also
refused: the two writes are to one file, and staging cannot express both.

**Which key wins for a root `[package]`+`[workspace]`.** `[package].version`
wins for the root crate itself. It is that crate's own version; the shared
`[workspace.package].version` is the default its *members* inherit, so a member
release rewrites the shared scalar while the root crate's own version is left
alone. This matches Cargo's semantics (the two are independent scalars) and is
pinned by `a_root_that_is_both_a_package_and_a_workspace_keeps_its_own_version`
and `a_member_of_a_root_that_is_also_a_package_rewrites_the_shared_version`.

**Tests.** Unit: the on-disk walk-up (including CRLF preservation on the root),
the root-both cases, the `--all` conflict and its allow path, and the
release-level rewrite of the root. Integration (`tests/monorepo.rs`, real
binary + mise): a member release rewrites the shared version; `--all` over two
members with different targets fails before anything is written. Byte-preserving
writes (ticket 34) are intact — the root goes through the same
`preserve_line_endings` path.

**Out of scope.** Cargo's `[package] workspace = "path"` redirection is not
followed; only the nearest ancestor declaring `[workspace]` is resolved.
