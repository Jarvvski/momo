# The library is not a public API

`momo-cli` ships a library target beside the `momo` binary, and after [ADR 0002](0002-ship-as-a-single-crate.md) collapsed the workspace it exported `pub mod domain` and `pub mod adapters` — roughly 119 public items. Nothing said what that surface promised: no semver guarantee, no MSRV beyond the pinned toolchain, no `docs.rs` posture, and a crate name (`momo-cli`) that reads as a CLI. A published crate's public items are an API whether or not anyone means them to be, so the surface was a contract by accident.

momo therefore **shrinks to a bin-shaped crate**. `domain` and `adapters` are crate-private, `cli` was already private, and the only exported item is `run()`. The library target exists because the binary and its unit tests need the modules to be reachable, not because anything outside the crate should depend on them. The versioning model stays reusable in the sense that matters — it is cohesive, pure and unit-tested — but it is not a supported surface, and it may change in any release.

The decision is recorded in the ticket that raised it: [#13](https://github.com/Jarvvski/momo/blob/main/.scratch/maintainer-review/issues/13-library-api-contract.md). Option (a) — commit to the library, document semver and MSRV, enable `missing_docs` and add a `docs.rs` block — was rejected because nobody consumes momo as a library; the cost of the promise buys nothing.

## Considered Options

- **Commit to the library** — rejected: a semver and MSRV promise over a surface with no known consumer, plus the `docs.rs` metadata to make it look intentional.
- **Leave the modules public but state the API is unsupported** — rejected: users can still depend on it and be surprised, and the public items still cost maintenance and review.
- **Shrink to bin-shaped** — chosen.

## Consequences

- `src/lib.rs` declares `mod domain;` and `mod adapters;`, and exports only `run()`.
- `cargo install momo-cli` and the release archives are unaffected: both drive the binary, not the library.
- The two tests that reached the library from `tests/` move inline, because an integration test compiles against the public API and a bin-shaped crate has none to give. The changelog property tests become `src/domain/changelog/property_tests.rs` under `#[cfg(test)]`; the `momo.toml` round-trip assertion moves into `src/domain/config.rs`'s test module.
- The private modules carry `#[cfg_attr(not(test), allow(dead_code))]`: the internal API is broader than the CLI, so in a non-test build the parts only the tests exercise look dead. The test build, which `mise run lint` also compiles, still runs dead-code analysis, so genuinely unused code is still caught — as `Workspace::root` was, and was deleted.
- The adapters' re-export facades were dressing for the public surface: the re-exports nothing used are gone, and the test modules that used one import the defining module directly.
- The crate boundary noted in `AGENTS.md` is the rule that keeps this honest: only `src/`, `tests/` and the top-level files ship, and `docs/`, `.scratch/` and the rest stay out.
- Amends ADR 0002, which described `domain` and `adapters` as the public library surface.
