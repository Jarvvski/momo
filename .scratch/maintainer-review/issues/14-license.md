# 14: Decide the license

Type: task
Status: resolved
Severity: medium
Area: legal

## Question

The crate is `GPL-3.0-or-later`. For a developer tool this is the single biggest adoption limiter — many companies ban GPL in their toolchain, and the Rust norm for CLIs is dual `MIT OR Apache-2.0`.

## Options

- **(a) Keep GPL-3.0-or-later.** Intentional copyleft; note *why* in the README so it reads as a choice, not an accident.
- **(b) Relicense to `MIT OR Apache-2.0`.** Maximises adoption; only possible if the copyright holder is the sole contributor. The history looks single-author, but confirm before acting.
- **(c) Weaker copyleft (e.g. `MPL-2.0`).** Middle ground.

## Recommendation

Owner's call; if broad tooling adoption is the goal, **(b)**. Whatever is chosen, align `Cargo.toml` `license`, `LICENSE`, the README badge and `deny.toml` (ticket 06) `licenses.allow`.

## Decision (2026-10-14)

**(b) Relicense to `MIT OR Apache-2.0`.** First confirm the copyright holder is the sole contributor over the whole history; if so, update `Cargo.toml` `license`, replace `LICENSE` with both texts (`LICENSE-MIT`, `LICENSE-APACHE`), update the README badge, and set `deny.toml`'s `licenses.allow` to match.

## Sequencing

Touches files owned by two in-flight branches (`README.md`/`Cargo.toml` in `gates`; `deny.toml` in `ci-supply`) — **hold until both have merged**, then land this as one focused PR.

## Related

- 06, 13.
