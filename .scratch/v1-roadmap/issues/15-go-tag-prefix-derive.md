# 15: Derive the Go tag prefix; per-package override

Type: task
Status: resolved
Points: 11
Bucket: Ecosystem manifest adapters
Severity: medium
Area: domain/tag + domain/config
Breaking: yes — changes tag naming for Go packages

## Problem

Ticket 06 warns that momo's Go tag prefix is unresolvable by `go get`. This ticket
**fixes** it: derive the Go-resolvable prefix, and add the general per-package
`tag_prefix` escape hatch.

## Evidence

- `src/domain/package.rs:147-153` (`tag_prefix` composes `<slug>-<base>`).
- `src/domain/config/paths.rs:12-22` + `KNOWN_PACKAGE_KEYS` (the per-package keys).
- `src/cli/version.rs:100` (`TagVersionSource`), `src/cli/release/tag.rs:37`.

## Proposal

- **New pure `src/domain/tag.rs`:** `prefix(package, base) -> String` becomes the
  single seam; `Package::tag_prefix` reduces to the slug-only helper it really is.
  `status`, `release` and `tag` all call `tag::prefix`, so the tag source and the
  tag writer stay consistent for free.
- **The rule:** for a `go:` package rooted at `R`, the prefix is `<R>/<base>` (`base`
  alone at the repo root). The `/vN` suffix lives in the module path, never the tag.
- **`/vN` check:** for major ≥ 2, if the module path does not end in `/vN`,
  **warn** (not error) naming the `+incompatible` consequence. Tagging `v2.0.0` is
  legitimate; only the path decides proper-v2 resolution.
- **Per-package `tag_prefix` override:** add `PackageConfig.tag_prefix:
  Option<String>` (`src/domain/config/paths.rs`, `KNOWN_PACKAGE_KEYS`), stored on
  `Package` like `manifest`. `tag::prefix` honours it before the Go derivation.
  This is the general escape hatch.
- **Single-package Go needs nothing** (no `go:` provider; `tag_prefix` is already
  `v`). The `/vN` check cannot run there — acceptable, documented.
- **Do not compare arbitrary path tails.** Derive from the directory (authoritative
  for tags) and check only `/vN` consistency.

## Surface

- New per-package key:
  `[packages."go:example.com/acme/api"] tag_prefix = "api/v"`.
- Warnings at `release`/`candidate`/`tag` on stderr.

## Implementation sketch

- `src/domain/tag.rs`: `pub fn prefix(package, base) -> String`;
  `pub fn go_major_warning(module_path, version) -> Option<String>`.
- `src/domain/package.rs`: keep `slug_prefix`; add the `tag_prefix` override field.
- `src/domain/config/paths.rs` + `keys.rs`: parse/validate/overlay the new key;
  `Config::fmt` renders it. **This adds a key — ticket 01's freeze test must be
  updated deliberately in the same PR.**
- Callers: `src/cli/commands.rs:99,139,265`, `src/cli/release/tag.rs:37`.
- Tests: `go:example.com/acme/api` @ `api/` → `api/v`; root module → `v`; override
  wins; major 2 without `/v2` warns; major 2 with `/v2` silent; non-Go unchanged.

## Scoring

Points **11** = Value 3×2 + Ease 3 + Novelty 2. The correct fix for a real bug, but
it is **breaking** for anyone already releasing Go with momo (though those tags were
unusable) and adds a config key against the freeze.

## Risks

- **Breaking tag naming.** Nothing to preserve (the old tags were unusable), but
  ship with a `docs/migrating.md` note.
- **Config-key addition vs the freeze (01).** Update the freeze test deliberately.
- The 60-char slug truncation no longer affects the Go tag path (good) but still
  affects the fragment store.

## Related

- 06 (warn-only first half), 03 (graph identities), 01 (freeze).
