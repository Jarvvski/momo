# 06: Warn on an unresolvable Go tag prefix

Type: bug
Status: ready-for-agent
Points: 14
Bucket: Ecosystem manifest adapters
Severity: medium
Area: domain/tag + cli

## Problem

A `go:` package has no version scalar anywhere — a Go module's version *is* the git
tag, so `ManifestSync` is correctly a no-op. The bug is in **tag naming**: `Package`
composes `<slug>-<base>`, so `go:example.com/acme/api` slugs to
`go-example-com-acme-api` (lowercased, punctuation collapsed, truncated at 60
chars) and momo tags `go-example-com-acme-api-v1.3.0`. Go resolves a module version
by **directory path**, not module name: a submodule at `api/` must be tagged
`api/v1.3.0`, a root module `v1.3.0`. Every tag momo produces for a Go monorepo
package is **unresolvable by `go get`**.

## Evidence

- `src/domain/package.rs:147-153` (`tag_prefix` composes `<slug>-<base>`).
- `src/domain/slug.rs:4` (slug truncates at 60 chars).
- Go module tags resolve by path; GoReleaser explicitly rejects tag prefixes that
  do not match the module name.
- The graph identity carries the module path (`go:example.com/acme/api`), so no file
  read is needed in the monorepo case.

## Proposal

This ticket is the **warn-only** half (T4a): the free-standing bug fix that lands
immediately without changing any tag. The derive-and-fix half is ticket 15.

- At `release`/`candidate`/`tag`, when a `go:` package's computed prefix differs
  from the Go-resolvable one, **warn** (stderr, non-fatal), naming the `go get`
  consequence and pointing at the fix.
- Compute the Go-resolvable prefix purely: for a `go:` package rooted at `R`
  (relative to the repo root), the prefix is `<R>/<base>` (`base` alone at the
  root). The `/vN` suffix lives in the module path, never in the tag, so the prefix
  is major-agnostic.
- **No `ManifestSync` adapter for Go** — `for_package` keeps returning `None`.
- **Single-package Go needs nothing:** no `go:` provider, identity is the directory
  name, `tag_prefix` is already `v` — correct. The bug is monorepo-only.

## Surface

Warnings at `release`/`candidate`/`tag` on stderr. No new keys, no CLI change.

## Implementation sketch

- `src/domain/tag.rs` (new, pure): `go_resolvable_prefix(package, base) -> String`
  and `go_prefix_warning(package, base) -> Option<String>`.
- Callers: `src/cli/commands.rs` (release/candidate/tag paths) and
  `src/cli/release/tag.rs`.
- Tests: `go:example.com/acme/api` at `api/` warns; a root module does not; a
  non-Go package is silent.

## Scoring

Points **14** = Value 3×2 + Ease 5 + Novelty 3. Free-standing, tiny, and it
surfaces a real `go get` footgun few tools warn about; narrower audience than the
Node adapter, hence the lower value.

## Risks

- A warning, not an error: tagging `v2.0.0` is legitimate; only the *path* decides
  proper-v2 resolution. A hard error would block a legal workflow.
- Keep it in the pure domain (no I/O) — the module path is already in the identity.

## Related

- 15 (derive the prefix + per-package override), 03 (graph identities).
