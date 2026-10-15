# 14: Generalise `ManifestSync` to a plan

Type: refactor
Status: resolved
Points: 11
Bucket: Ecosystem manifest adapters
Severity: medium
Area: domain/manifest + cli/release

## Problem

`ManifestSync` returns `Option<(String, ManifestUpdate)>` with
`ManifestUpdate { manifest, stale_lockfile: Option<PathBuf> }`. Three things break
for any non-Cargo ecosystem:

- **`stale_lockfile` is singular and package-local.** A Node workspace root has
  *one* `pnpm-lock.yaml`/`package-lock.json` outside every package dir; a repo
  mid-migration has two. `CargoManifest::lockfile` assumes `manifest.parent()`.
- **There is no "skip with reason".** `Ok(None)` conflates "no version scalar" with
  "already current". The Python hatch-vcs case *must* be visibly
  "skipped: dynamic version", never silent.
- **`ManifestError::Parse` wraps `toml_edit::TomlError`** — a JSON adapter cannot
  produce it. The port is TOML-shaped by accident.

## Evidence

- `src/domain/manifest.rs:22-34, 63-82` (the port), `:108-116` (`Parse`).
- `src/adapters/manifest/cargo.rs:84-87` (`lockfile` assumes `manifest.parent()`),
  `:139-144` (`Ok(None)` conflation).
- `src/cli/release/prepare.rs:178-186` (`Prepared.writes: Vec<(PathBuf, String)>`).
- `src/cli/release/manifest.rs:42-69` (`reject_shared_manifest_conflicts`).
- `src/cli/commands.rs:312-324` (`warn_stale_lockfile`).

## Proposal

**Return a plan, not a tuple** — the CLI already models writes as a set.

```rust
pub struct ManifestWrite { pub path: PathBuf, pub text: String }
pub enum SkipReason { NoVersion, AlreadyCurrent, Dynamic(String) }
pub struct ManifestPlan {
    pub writes: Vec<ManifestWrite>,
    pub lockfiles: Vec<PathBuf>,            // stale; reported, never rewritten
    pub skipped: Vec<(PathBuf, SkipReason)>,
}
pub trait ManifestSync {
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf>;
    fn render(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError>;
    fn write(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError>;
}
```

- **Lockfile stays reported, never written** — `Vec<PathBuf>`, not an abstraction.
  A lockfile *writer* would need format-preserving JSON/TOML/YAML editors per
  ecosystem; that is the biggest cost sink available and buys nothing the package
  manager cannot do. Reject.
- **Make `ManifestError::Parse` format-neutral**, mirroring `VcsError::Operation`:
  `Parse { path, #[source] source: Box<dyn Error + Send + Sync> }`. Add
  `Conflict { path, message }` for semantic refusals (ticket 08's dynamic+static
  case).
- **Behaviour-neutral for Cargo.** Port `CargoManifest` onto the plan shape and keep
  the existing round-trip suite as the oracle — zero behaviour change.
- Ticket 07 later widens `render`/`write` to `ManifestEdits { version, ranges }`;
  this ticket lands the `ManifestPlan` shape it composes into.

## Surface

- `ReleasePackage` (`src/cli/output.rs:220-240`) gains `manifest: Vec<PathBuf>` and
  `skipped: Vec<{path, reason}>`. This is a pinned JSON contract change — document
  it.
- `warn_stale_lockfile` → `report_manifest(&outcome)`, printing skips and stale
  lockfiles together, deduping lockfiles by path (N packages in one workspace
  report the same root lockfile → one warning).

## Implementation sketch

- `src/domain/manifest.rs`: the new types + trait; format-neutral `Parse`;
  `Conflict`.
- `src/adapters/manifest/cargo.rs`: implement the plan shape; keep `roundtrip.rs`
  green.
- `src/cli/release.rs:77-83` (`manifest_write`),
  `src/cli/release/manifest.rs:42-69` (`reject_shared_manifest_conflicts`) — compare
  against **all** written paths, not one.
- `src/cli/commands.rs`: `report_manifest`.
- `src/cli/output.rs`: the new report fields.

## Scoring

Points **11** = Value 3×2 + Ease 4 + Novelty 1. A refactor, so low standalone
value, but it gates tickets 04 and 08 and improves Cargo; low risk because the
round-trip suite proves it neutral.

## Risks

- **Contract churn** in the JSON reports — pin new tests.
- The refactor **must be behaviour-neutral for Cargo**; the existing `roundtrip.rs`
  is the oracle.

## Related

- 04, 08 (blocked on this), 07 (extends `render`/`write`), 17 (absorbed by this).
