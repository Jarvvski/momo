# 04: Node `package.json` manifest adapter

Type: task
Status: ready-for-agent
Points: 14
Bucket: Ecosystem manifest adapters
Severity: high
Area: adapters/manifest/node
Blocked by: 14

## Problem

A `node:@acme/ui` package gets **no manifest sync at all**: `for_package` returns
`None` for any non-`cargo` provider, and `render_manifest` treats that as a silent
skip. After `momo release`, the tag says `node-acme-ui-v1.3.0` but
`packages/ui/package.json` still says `1.2.0`. For anyone who publishes to npm
(`package.json.version` *is* the published version), reads its own version at
runtime, or diffs the tree, that is a wrong release.

## Evidence

- `src/adapters/manifest/mod.rs:20-25` (`for_package` matches only `cargo`).
- `src/cli/release/manifest.rs:25-27` (a missing adapter is a silent skip).
- mise provider IDs (verified): Node is `node:<package name>`, detected from
  `pnpm-workspace.yaml` or `workspaces` in the root `package.json`.

## Proposal

Reuse the plan-shaped port from ticket 14. Selection is by `node:` identity; the
per-package `manifest` override already works.

- **Format-preserving edit by byte span, not a CST.** momo only replaces one string
  literal with another, so locate the byte span of the top-level `"version"` value
  and splice — ~120 lines, **no new dependency**. `serde_json` round-trips lose
  indentation and key spacing; `preserve_order` fixes only ordering. A splice is
  *stronger* than `toml_edit`: nothing is re-rendered, so CRLF, indentation, key
  order and trailing newline survive automatically.
- **`packageManager` is a toolchain pin, not a version.** Keying on the exact
  top-level key `version` leaves `packageManager`, `engines`, `publishConfig` and
  any `version` inside `dependencies`/`devDependencies` untouched by construction.
- **`workspace:*` vs real ranges: reject as out of scope.** momo rewrites only the
  package's own `version`; it never rewrites dependency ranges (that is ticket 07,
  and mise treats version strings as opaque). The distinction only matters for a
  cascade feature.
- **Lockfile: report, never rewrite.** `package-lock.json` embeds the version at
  top level *and* in `packages[""].version`. Locate it by **walking up** to the
  workspace root (in a workspace it is not beside the package) and emit the refresh
  command in the warning (`npm install --package-lock-only`, `pnpm install
  --lockfile-only`, `yarn install --mode update-lockfile`, `bun install
  --lockfile-only`).
- **Single-package Node repo:** add Node to the single-package probe chain, probed
  in a fixed order (Cargo, Node, Python), first `locate` wins; a repo with both
  `Cargo.toml` and `package.json` uses the per-package `manifest` override.
- `private: true` packages still carry and bump a version — no special case.

## Surface

None new. The visible change is the release report's `manifest`/`skipped` fields
(from ticket 14).

## Implementation sketch

- `src/adapters/manifest/node/mod.rs` — `NodeManifest`; `locate` =
  `package_dir/package.json`; `render` = read → `json::set_version` →
  `ManifestPlan` with `lockfiles` found by walking up.
- `src/adapters/manifest/node/json.rs` — the span locator:
  - `version_span(text) -> Result<Option<Range<usize>>, JsonError>` (the byte range
    of the top-level `"version"` string value, quotes included);
  - `set_version(text, version) -> Result<Option<String>, JsonError>` (splice;
    `None` when already equal; last duplicate key wins, matching `JSON.parse`);
  - internals: a `Scanner` with `skip_ws`, `string() -> Range` (respects `\`
    escapes), `skip_value()` (recursive depth walk).
- `src/adapters/manifest/mod.rs` — `Some("node") => NodeManifest`; add to the probe
  chain.
- `src/adapters/manifest/node/roundtrip.rs` — fs round-trips mirroring `cargo`.
- Tests: 2/4-space and tab indentation; key order; trailing newline; CRLF;
  `packageManager`/`engines`/`publishConfig`/`dependencies.version` untouched;
  nested `"version"` untouched; duplicate key last-wins; missing → `Skip(NoVersion)`;
  current → `Skip(AlreadyCurrent)`; invalid JSON → `Parse`; a `proptest` that only
  the version span changes.

## Scoring

Points **14** = Value 5×2 + Ease 3 + Novelty 1. Highest-value adapter (largest
polyglot ecosystem); moderate effort (the locator is the risk); not novel
(changesets et al. do it).

## Risks

- Format preservation is *safer* here than for TOML (splice, no re-render).
- A workspace-root lockfile reported N times → dedupe in the CLI.
- The `npm ci`-style stale-lockfile sharp edge → document the refresh command.
- JSONC comments make the scanner fail loudly — acceptable for `package.json`;
  revisit only if VS Code-extension manifests hit it.

## Related

- 14 (port plan, prerequisite), 08 (Python, sibling), 07 (cascade, later).
