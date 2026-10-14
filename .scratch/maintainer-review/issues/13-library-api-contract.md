# 13: Decide the library's public-API contract

Type: task
Status: resolved
Severity: medium
Area: api

## Question

`src/lib.rs` exposes `pub mod domain` and `pub mod adapters`, and `docs/contributing.md` calls the library "the public surface". But there is no stated semver or MSRV guarantee for that surface, no `docs.rs` posture, and the crate name (`momo-cli`) reads as a CLI. The crate is published, so its ~119 public items are effectively an API someone can depend on.

Either commit to it, or shrink it.

## Options

- **(a) Commit.** Document the guarantee (semver for the library; MSRV = the pinned toolchain), enable `missing_docs` (ticket 02), add crate-level docs and a `docs.rs` metadata block. Cost: a maintenance promise.
- **(b) Shrink.** Make `domain`/`adapters` `pub(crate)` and expose only `run()`; the crate becomes bin-shaped and the surface stops being a contract. Cost: loses the "reusable versioning model" the README and contributing guide imply.
- **(c) Keep as-is, explicitly unsupported.** Leave the modules public but state the library API is not covered by semver. Cost: users may still depend on it and be surprised.

## Recommendation

**(a)** if the reusable model is a goal — the domain is genuinely clean and worth exporting; otherwise **(b)**. Whichever, record it in an ADR (the repo's convention) and reflect it in `Cargo.toml` and `lib.rs`.

## Decision (2026-10-14)

**(b) Shrink to bin-shaped.** Make `domain` and `adapters` `pub(crate)` and expose only `run()`; the crate is a binary, not a library. Follow the crate-boundary note in `AGENTS.md`.

## Sequencing

Broad visibility change across `src/**` — **hold until the in-flight `spawn-kill` (11) and `json-contract` (12) branches have merged**, or the conflict is semantic. Also check whether any `tests/` integration test depends on the public library API; if one does, that is a signal the API is load-bearing and should be revisited with the owner.

## Related

- 02, 14.
