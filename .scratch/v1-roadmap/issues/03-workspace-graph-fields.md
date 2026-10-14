# 03: Consume the workspace-graph fields

Type: task
Status: ready-for-agent
Points: 16
Bucket: Monorepo release engine
Severity: medium
Area: adapters/taskenv + domain

## Problem

`graph.rs` documents the **old config-root model** ("lists each project's ecosystem
ID and its directory… requires `monorepo_root = true`") and `parse_projects` reads
only `projects[].id` and `projects[].root`. The graph mise actually emits today
(verified against mise 2026.10.9) is **manifest-inferred** and carries much more,
all discarded:

```json
{ "id": "cargo:b", "root": "crates/b",
  "metadata": { "workspace_source": "Cargo.toml" },
  "dependencies": ["cargo:a"],
  "provenance": { "provider": "cargo", "source": "crates/b/Cargo.toml" },
  "dependency_provenance": { "cargo:a": { "provider": "cargo", "source": "crates/b/Cargo.toml" } },
  "tasks": {} }
```

Without parsing this, there is no cascade (tickets 07/12), no way to name the file
that declared an edge, and a module comment that lies.

## Evidence

- `src/adapters/taskenv/graph.rs:1-7` (stale comment), `:52-95` (`parse_projects`
  reads two fields), `:147-161` (fixture already contains `dependencies` and
  `metadata.workspace_source`).
- Verified against mise 2026.10.9: `dependencies` is a **bare edge list with no
  range**; `dependency_provenance[dep].source` names the declaring file;
  `provenance.source` is `"./package.json"` for a pnpm root package; the Cargo and
  Node providers **dedup** all dependency kinds to one edge; the uv provider emits
  edges only for `[tool.uv.sources]`.

## Proposal

Parse `dependencies`, `dependency_provenance`, `provenance.source` and
`metadata.workspace_source`; **ignore `tasks`** (task-inference output, no
versioning use).

- **Parse defensively; do not pin a schema version.** mise owns this format and it
  is moving; missing optional fields must degrade (older mise emits only
  `id`/`root`/`dependencies`), unknown fields ignored. Keep the existing strictness
  where it protects the repo: `id`, `root`, and now `source` go through
  `validate_root` (`:98`).
- **Identity stays the graph `id`.** `Package::from_identity` already takes the
  ecosystem-prefixed id and derives the slug, so IDs already align with mise.
- **Do NOT adopt `mise run --affected`.** It compares **git revisions** and needs a
  task name — git-only, whereas momo is VCS-optional and jj-first. Worse, it
  answers "which files changed", not "which packages should release": momo's model
  is that a **recorded Change** justifies a release. Borrow only its **reason
  vocabulary** (`changed-path`, `dependent`, `workspace-global`, `lockfile`) as the
  shape of the cascade report.
- **Keep the `monorepo_root` gate.** `mise tasks graph --json` still errors without
  it, so `monorepo.rs::is_monorepo` and the `Discovery::degraded` path stay.
- Fix the stale module comment and `docs/usage.md:53`.

## Surface

None. Internal parser widening.

## Implementation sketch

- `src/adapters/taskenv/graph.rs`: rename `parse_projects` → `parse_graph`,
  returning packages + edges; read `dependencies`, `dependency_provenance`,
  `provenance.source`, `metadata.workspace_source`; validate every `source` with
  `validate_root`; keep the dedup-by-identity and conflicting-root error.
- New pure `src/domain/graph.rs` (or extend `domain/workspace.rs`):
  `Project { id, root, manifest: Option<PathBuf>, workspace_source: Option<PathBuf>,
  provider: Option<String> }` and `Edge { dependent, dependency, manifest }`, plus
  `DependencyGraph` and `dependents_of`. No I/O.
- `src/cli/workspace.rs::load`: thread edges into `Workspace::new`; `discover`
  returns `(Vec<Package>, Vec<Edge>)` (or a graph struct); `Discovery` carries it.
- Update the fixture at `graph.rs:147` to include `dependency_provenance`; add
  tests for missing optional fields, an absolute `source`, and a deduped
  multi-kind edge.

## Scoring

Points **16** = Value 4×2 + Ease 4 + Novelty 4. The most novel *machinery* (no
competitor discovers packages via a task runner), cheap, and it unlocks the wedge
(tickets 07/12).

## Risks

- **Schema churn.** Defensive parsing + a test tolerating absent optional fields.
- **`source` path handling.** Leading `./` and `.` roots; `validate_root` must
  accept these while rejecting absolute/`..`.
- **Incomplete edges** (uv/Go) will be surfaced by ticket 07 and must be labelled
  unsupported, never "satisfied".

## Related

- 07, 12 (cascade). `docs/usage.md:53`.
