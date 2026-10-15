# 07: Cascade — rewrite dependent ranges in the same release

Type: task
Status: resolved
Points: 13
Bucket: Monorepo release engine
Severity: high
Area: cli/release + adapters/manifest
Blocked by: 03, 14

## Problem

A monorepo releases package A with a breaking change → `A 2.0.0`. Dependent B still
declares `a = { path = "../a", version = "1.2" }` (Cargo) or
`"@acme/ui": "^2.3.0"` (npm). momo writes A's changelog and manifest version and
**never touches B**. B's published manifest now advertises a range A's new version
does not satisfy. The workaround today is a hand-edit of every dependent.

This is the changesets `updateInternalDependencies` gap. It is the wedge-blocker:
every competitor cascades, and without it the mise-polyglot monorepo story is
blocked.

## Evidence

- `src/adapters/taskenv/graph.rs` (dependencies fetched and discarded — see ticket
  03).
- `src/cli/release/prepare.rs:159` (`render_manifest`), `:178-186` (`Prepared.writes`),
  `:223` (`Staged::commit`).
- `src/cli/release/manifest.rs:42-69` (`reject_shared_manifest_conflicts`).
- `src/domain/manifest.rs:22-34, 63-82` (the port).
- Verified: `dependencies` is a bare edge list with **no range** — momo must read
  the dependent's manifest. `dependency_provenance[dep].source` names the declaring
  file, but is the **member**, not the root, for Cargo `workspace = true`.

## Proposal

**Scope: (b) rewrite ranges for dependents already in the release set.** Not (a)
report-only (that is ticket 12), and not (c) force-release.

- **(c) force-release is rejected for this ticket.** Manufacturing a dependent bump
  with no recorded Change contradicts the documented v0 stance
  (`docs/usage.md:87`, `spec.md:134`, issue 16). It is an **owner ruling**, not a
  plan — see the map.
- **(b) is defensible** because it only ever edits a manifest momo was already
  going to write: B is in `planned` because B has its own unreleased Changes
  (`commands.rs:180-186`). The range edit rides along with B's version write.
- **One file, one render.** B's version-scalar write and B's range edit target the
  *same* manifest. Two entries with the same target both stage and both rename, so
  the second **silently overwrites** the first. Compose them into one rendered text
  — this forces the port change in ticket 14 (`ManifestEdits`).
- **Range resolution.** Read the dependent's manifest. For Cargo, follow
  `workspace = true` inheritance (reuse `cargo.rs:163 workspace_root`) and match
  renamed deps by `package =`; scan `[dependencies]`, `[dev-dependencies]`,
  `[build-dependencies]`, `[target.*.dependencies]`. **Widen only, refuse anything
  unrecognised** (`None`), never narrow.
- **Never run on `candidate`.** `^5.0.0` is not satisfied by `5.1.0-next.0`, so a
  prerelease would rewrite ranges spuriously. Gate on the existing `Mode`
  (`prepare.rs:25`) — `candidate` and `release` share `prepare` but must diverge
  here.
- **Ecosystem scope: Cargo-only for the first cut.** Node/uv/Go edges are labelled
  "unsupported ecosystem, not checked" — never "satisfied".

## Surface

- `release` report (human + JSON) gains a `ranges` array per package:
  `{ dependent, dependency, requirement, target }`.
- Opt-in rewrite: `release --sync-ranges`, or a config key
  `sync_dependency_ranges = true` resolved like every other key. Default **off** so
  existing output is byte-stable.

## Implementation sketch

- `src/domain/manifest.rs`: `DependencyRequirement { name, requirement }`,
  `DependencyRangeEdit { name, requirement, target }`, and a generalised port:
  `render(&self, manifest, edits: &ManifestEdits)` /
  `write(...)` where
  `ManifestEdits { version: Option<Version>, ranges: Vec<DependencyRangeEdit> }`.
  `ManifestUpdate` gains `ranges: Vec<(String, String)>`.
- `src/adapters/manifest/cargo.rs`: `dependencies(manifest) -> Vec<DependencyRequirement>`
  and `satisfied(requirement, version) -> bool` / `widen(requirement, target) ->
  Option<String>` over Cargo's caret/`~`/`=`/`>=`/`*` forms.
- `src/cli/release/prepare.rs`: thread `ManifestEdits` through `render_manifest`.
- `src/cli/release/manifest.rs`: extend `reject_shared_manifest_conflicts` to cover
  range edits, not just versions (two packages writing different text to one shared
  Cargo root).
- Tests: graph fixture with `dependency_provenance`; Cargo range round-trips incl.
  `workspace = true` and renamed deps; a release rewriting a dependent's range in
  one transaction; a candidate leaving ranges alone.

## Scoring

Points **13** = Value 5×2 + Ease 2 + Novelty 1. Highest value after ticket 02, but
large effort (port change + single-render composition + inheritance) and not novel
— everyone cascades.

## Risks

- **`source` ≠ range location** (Cargo `workspace = true`). Miss it and every
  inherited dependency is skipped or rewritten in the wrong file.
- **Wrong rewrite is a semver lie.** Restrict to widening; refuse unknown syntax.
- **Same-target double write** silently drops the version bump if not composed.
- **Incomplete edges** (uv/Go) must be labelled unsupported.

## Related

- 03 (graph, prerequisite), 14 (port plan, prerequisite), 12 (report-only first
  cut), `docs/usage.md:87`.
