# 16: Distinct package identities can collapse to the same slug

Type: bug
Status: ready-for-agent
Severity: high
Area: domain/slug + domain/package
Verified: yes — read at the cited lines

## Symptom

`slugify` maps every run of non-alphanumeric characters to a single `-`, so
structurally different ecosystem identities produce the same slug. Two packages
then share a tag prefix and a fragment store, with nothing detecting the
collision.

## Evidence

`src/domain/slug.rs:14-36` collapses non-alphanumerics to `-`.
`src/domain/package.rs:57`: `let slug = slugify(&identity);`
`src/domain/package.rs:141`: `format!("{}-{base}", self.slug)` (tag prefix)
`src/domain/package.rs:117-123`: `base.join(&self.slug)` (fragment store)

The test at `src/domain/package.rs:189-190` confirms `node:@acme/ui` →
`node-acme-ui`.

## Trigger

A monorepo containing both `node:@acme/ui` and `node:acme-ui` (both valid npm
package names). Both slug to `node-acme-ui`, so:

- they share the tag prefix `node-acme-ui-v`, and the second release fails with
  `TagAlreadyExists`;
- they share `.momo/changes/node-acme-ui`, so fragments recorded for one package
  are read and consumed by the other.

`Workspace::new` / discovery has no duplicate-slug check.

## Suggested direction

Detect slug collisions after discovery and fail with a clear error naming both
identities, or require a `[packages.*].slug` override. Longer term, make slug
derivation injective (e.g. escape the separator rather than collapsing it).

## Open questions for triage

- Error on collision, or auto-disambiguate? Erroring is simpler and keeps tags
  predictable.

## Triage

**Verdict:** Confirmed. `slugify` collapses every run of non-alphanumerics to a
single `-` (`slug.rs:14-36`), `Package::from_identity` derives the slug from the
identity (`package.rs:57`), and nothing checks for duplicates — `Workspace::new`
(`workspace.rs:50`) maps `configured` and returns. Two packages then share a tag
prefix (`package.rs:141`), a fragment store (`package.rs:117-123`) and a
selection name, with no error.

**Corrections:** The ticket frames the collision as cross-provider
(`node:@acme/ui` vs `node:acme-ui`). It can also happen *within* one ecosystem —
`cargo:a_b` and `cargo:a-b` both slug to `a-b` — so the check must be on the
slug alone, not on the identity pair. `Workspace::new` is the right choke point:
it is where `configured` applies the `slug` override, so a user's
`[packages.*].slug` can resolve a collision before it is reported.

**Scope:** Standalone. Related to 17 (both concern slug derivation) and 18 (both
concern ambiguous selection), but a distinct bug; the triage plan groups
15+16+17+18 as PR C.

**Implementation sketch:**

- Add `WorkspaceError::DuplicateSlug { slug: String, identities: Vec<String> }`
  (thiserror) to `src/domain/workspace.rs`.
- Make `Workspace::new` return `Result<Self, WorkspaceError>`; after mapping
  `configured`, group by `slug()` and return `Err` naming every colliding
  identity (sorted, fully-qualified) when any slug has more than one package.
  `Workspace::single` stays infallible (wrap in `Ok`).
- Update `src/cli/workspace.rs::load` to return `Result<Discovery>`; a slug
  collision is fatal, not a degraded-discovery warning, so do not reuse the
  `unavailable` field. Callers in `src/cli/commands.rs` already use `?`.
- Tests: a `workspace.rs` unit test with `node:@acme/ui` + `node:acme-ui`
  asserting the error names both; a test that a `[packages.*].slug` override
  resolves the collision; update the `monorepo()` fixture for the new `Result`.
