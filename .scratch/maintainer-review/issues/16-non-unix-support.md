# 16: Decide non-Unix support

Type: task
Status: resolved
Severity: low
Area: adapters/spawn

## Question

`docs/ci.md` says there is no Windows asset and `cargo install` is the only Windows path. `src/adapters/spawn.rs` keeps a `#[cfg(not(unix))]` no-op for `lead_its_own_group`, so on non-Unix the timeout stops only the direct child — untested, and the repo has no Windows CI.

## Options

- **(a) Declare Unix-only.** Add a `compile_error!` on non-Unix (or gate the crate), so a Windows build fails clearly instead of running half-supported. Update the README.
- **(b) Keep the best-effort fallback** and document it as unsupported-but-compiles.
- **(c) Actually support Windows** (job objects for group kill) — out of proportion for now.

## Recommendation

**(a)**: honest and cheap. Pair it with a one-line README platform note.

## Decision (2026-10-14)

**(a) Declare Unix-only.** Replace the `#[cfg(not(unix))]` no-op with a `compile_error!` on non-Unix (or gate the crate), so a Windows build fails clearly. Add a one-line platform note to the README.

## Sequencing

Touches `src/adapters/spawn.rs` and the README — **hold until 11 (`spawn-kill`) and 09 (`gates`) have merged**.

## Related

- 04, 11.
