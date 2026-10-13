# 17: Overriding a package's identity does not recompute its slug

Type: bug
Status: resolved
Severity: medium
Area: domain/package
Verified: yes — read at the cited lines

## Symptom

`Package::from_identity` derives the slug from the identity, but `configured`
lets an override replace the identity without recomputing the slug. The two then
disagree: tags, the fragment store and `matches` all use the stale slug.

## Evidence

`src/domain/package.rs:62-75`:

```rust
pub fn configured(mut self, config: &Config) -> Self {
    if let Some(overrides) = config.packages.get(&self.identity) {
        if let Some(identity) = &overrides.identity {
            self.identity = identity.clone();      // slug not recomputed
        }
        if let Some(slug) = &overrides.slug {
            self.slug = slug.clone();
        }
        ...
```

`from_identity` derives the slug at `package.rs:57`. The two override fields are
semantically coupled but treated as independent.

## Trigger

`[packages."cargo:momo"] identity = "cargo:renamed"` (with no `slug`). The package
ends up with identity `cargo:renamed` but slug `cargo-momo`, so tags become
`cargo-momo-v…`, the fragment store stays `.momo/changes/cargo-momo`, and
`matches("renamed")` fails.

## Suggested direction

When `slug` is not explicitly overridden, recompute it from the new identity.

## Open questions for triage

- Is an `identity` override a supported use case at all, or should the config
  surface only `slug` and `manifest`? If it is supported, should changing the
  identity also be warned about (it changes the tag namespace)?

## Triage

**Verdict:** Confirmed that the two override fields are decoupled. `configured`
(`package.rs:62-75`) sets `self.identity` without recomputing the slug, so with
`identity = "cargo:renamed"` and no `slug`, the package keeps slug `cargo-momo`.
Tag prefix (`package.rs:141`) and fragment store (`package.rs:117-123`) are both
slug-derived, so they stay under the old name.

**Corrections:** One claim is wrong. The ticket says `matches("renamed")` fails;
it does not — `matches` compares the query against the identity's bare name
(`package.rs:153-160`), so `"renamed"` matches `cargo:renamed`. What actually
breaks is the tag namespace and fragment store, which follow the *stale* slug
(`cargo-momo-v…`, `.momo/changes/cargo-momo`). `identity` overrides are a
documented, supported key (`docs/configuration.md:53`), so the decoupling is a
real bug.

**Scope:** Standalone; relates to 16 (same slug derivation). Not a duplicate.

**Implementation sketch:** In `Package::configured`, recompute the slug from the
new identity only when `slug` is not explicitly set:

```rust
if let Some(identity) = &overrides.identity {
    self.identity = identity.clone();
}
match &overrides.slug {
    Some(slug) => self.slug = slug.clone(),
    None if overrides.identity.is_some() => self.slug = slugify(&self.identity),
    None => {}
}
```

`slugify` is already imported (`package.rs:12`). Tests: extend
`configured_overrides_the_identity_and_slug` with an identity-only case asserting
`slug() == "cargo-renamed"`; keep the both-set case asserting the explicit slug
wins. Note in `docs/configuration.md` that renaming the identity moves the tag
namespace and fragment store, and that a user wanting to keep the old tags must
pin `slug` explicitly.

## Resolution

`Package::configured` now recomputes the slug from the new identity when an
`identity` override is set and no `slug` override accompanies it; an explicit
`slug` still wins. So `identity = "cargo:renamed"` alone yields slug
`cargo-renamed`, and the tag prefix and Fragment store follow it.

The triage correction held: `matches` already resolved the new bare name, so the
bug was confined to the slug-derived tag namespace and Fragment store.
`docs/configuration.md` now records that renaming the identity moves both and that
pinning `slug` keeps the old ones.

Tests: `configured_recomputes_the_slug_from_a_new_identity` asserts identity
`cargo:renamed`, slug `cargo-renamed` and tag prefix `cargo-renamed-v`; the
existing both-set test still asserts the explicit slug wins.
