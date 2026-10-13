# Ship momo as a single crate, not a five-crate workspace

momo needs `cargo install` as a distribution channel, and a published crate
cannot depend on unpublished path dependencies. As a five-crate virtual
workspace, publishing `momo-cli` to crates.io would mean publishing `momo-core`,
`momo-vcs`, `momo-taskenv` and `momo-manifest` too — in dependency order, every
release, with each dependent waiting on crates.io to index its predecessor
before it can be published. Four of those five crates would exist on crates.io
solely to satisfy cargo, with no independent audience.

momo is therefore a **single crate**: package `momo-cli` (the name `momo` is
taken on crates.io), with a library target and a thin binary named `momo`. The
separation the workspace enforced is kept as modules — `domain/` (pure: no I/O,
no `clap`), `adapters/{vcs,taskenv,manifest}/`, and a private `cli/`. The
`domain` and `adapters` modules are the crate's public library surface, so the
versioning model stays a coherent, reusable API rather than a set of
internals nothing may name. One `cargo publish` per release.

## Considered Options

- **Publish all five crates** — rejected: an ordered publish with an index wait
  per crate, on every release, for four crates nobody installs.
- **Keep the workspace, skip crates.io** — rejected: forgoes the idiomatic Rust
  install path. (Note `cargo install --git <url> --tag <v> momo-cli` works today
  and remains a fallback.)
- **Collapse to one crate** — chosen.

## Consequences

- The "domain is pure" invariant is no longer enforced by a crate boundary. It
  is now enforced by `mise run check`, which fails if `src/domain/` reaches for
  `std::fs`, `std::process`, `std::env`, `clap` or `anyhow`.
- `momo-cli` is the published name; the installed binary stays `momo`, so the
  install is `cargo install momo-cli`.
- The domain and adapters become public API, so the crate is usable as a library
  and the domain keeps the vocabulary the spec gave it.
- The four adapter crates' integration round-trips move to `#[cfg(test)]`
  submodules, since the binary target exposes no library to `tests/`.
- Amends spec §14.
