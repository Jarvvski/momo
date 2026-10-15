# 12: Cascade — report unsatisfied dependent ranges

Type: task
Status: resolved
Points: 12
Bucket: Monorepo release engine
Severity: medium
Area: cli/status + cli/release
Blocked by: 03

## Problem

When a package is released with a bump that leaves an internal dependent's declared
range unsatisfied, momo says nothing. The user learns it when a publish fails or a
consumer resolves a stale dependency. This ticket delivers the **information**;
ticket 07 delivers the rewrite.

## Evidence

- `src/adapters/taskenv/graph.rs` (edges fetched and discarded — ticket 03).
- `src/cli/output.rs:126` (`StatusPackage`), `:269` (`ReleasePackage`).
- `src/domain/manifest.rs` (the port; read-only capability is new here).

## Proposal

**Report-only (T1a).** For each released package, for each edge into it, if the
dependent is Cargo and its requirement is unsatisfied at the target, emit a
`Cascade` entry. This fits the report-only stance and delivers what users actually
need.

- **Cargo-only for the first cut.** Node/uv/Go edges are labelled "unsupported
  ecosystem, not checked" — never "satisfied".
- **Read-only port addition:** `dependencies(manifest) -> Vec<DependencyRequirement>`
  and `satisfied(requirement, version) -> bool`. No writes, so this half does **not**
  need the `ManifestEdits` composition that ticket 07 forces.
- **`--all` vs `-p` honesty.** With `-p A`, only A is planned, so the report still
  shows B's unsatisfied range. With `--all`, B is planned only if B has its own
  unreleased Changes — so cascade mostly *reports* until 07 lands. State this in
  docs; do not paper over it.

## Surface

- `status` (human + JSON) gains a `cascade` array per package:
  `{ dependent, dependency, requirement, target, satisfied }`.
- `release` (human + JSON) gains the same array, so `--dry-run` shows what would
  change.

## Implementation sketch

- `src/domain/graph.rs` (from ticket 03): `Edge`, `dependents_of`.
- `src/adapters/manifest/cargo.rs`: `dependencies` (scan the four tables, resolve
  `workspace = true`, match renamed deps) and `satisfied` (caret/`~`/`=`/`>=`/`*`;
  refuse unrecognised → `None`).
- `src/cli/commands.rs::status`: after computing each package's target, walk the
  edges and push `Cascade` entries.
- `src/cli/output.rs`: add the array to `StatusPackage`/`ReleasePackage`.
- Tests: a graph fixture with `dependency_provenance`; an unsatisfied caret range
  reported; a satisfied range absent; an unsupported ecosystem labelled.

## Scoring

Points **12** = Value 4×2 + Ease 3 + Novelty 1. Useful information, moderate effort,
not novel. Value is capped because it does not fix the manifest (that is 07).

## Risks

- **Silent false negatives.** The graph omits edges Cargo would honour; momo must
  not imply it checked them.
- **Same read as 07** — land 12 first, then 07 reuses the reader.

## Related

- 03 (graph, prerequisite), 07 (the rewrite), `docs/usage.md:87`.
