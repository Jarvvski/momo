# Contributing

## Setup

The toolchain and the task runner are owned by [mise](https://mise.jdx.dev).

```bash
mise install        # pin the Rust toolchain (see mise.toml, rust-toolchain.toml)
mise run build      # cargo build --release
mise run test       # cargo test
mise run fmt        # cargo fmt --all
mise run lint       # cargo clippy --all-targets -- -D warnings
mise run lint:domain # the domain must stay pure
mise run check      # fmt + lint + domain purity + build + test — the pre-land gate
mise run install    # install the `momo` binary to ~/.cargo/bin
```

Run it straight from the checkout with `cargo run -- <args>` or `mise run run`.

Rust edition 2024. `Cargo.lock` is committed — this is an application, not a
library.

## Layout

One crate: a library plus a thin binary. The library is the public surface; the
CLI is private and reached through `momo_cli::run`.

```text
src/
├── lib.rs          the library root, and `run` — what the binary calls
├── main.rs         the binary: `momo_cli::run()`
├── domain/         kinds, bump policy, versions, changelog, config, the
│                   Package/Workspace model, and the ports (Vcs,
│                   TaskEnvironment, ManifestSync). Pure — no I/O, no clap.
├── adapters/
│   ├── vcs/        git + jj, behind the Vcs port
│   ├── taskenv/    mise, behind the TaskEnvironment port
│   └── manifest/   Cargo.toml, behind the ManifestSync port
└── cli/            the clap tree, handlers, and the wiring
```

Ports live in `domain`; adapters implement them; the CLI wires them. `domain`
must stay free of I/O and of `clap` — `mise run lint:domain` enforces it, since
the crate boundary that used to enforce it is gone (see
[ADR 0002](adr/0002-ship-as-a-single-crate.md)).

## Conventions

- `unsafe_code` is **forbidden** crate-wide — a hard compile error, not a lint.
- Clippy runs warnings-as-errors. Fix the root cause; don't paper over a lint.
- No `.unwrap()` outside tests. `.expect("why")` only for true invariants, with a
  message saying why it cannot fail. Propagate with `?`.
- Don't reach for `.clone()`, `Rc<RefCell<_>>` or `Arc<Mutex<_>>` to satisfy the
  borrow checker — restructure ownership instead.
- Prefer iterators and combinators over index loops; borrow in signatures
  (`&str`, `&[T]`); implement std traits (`From`/`TryFrom`/`Display`/`Default`)
  over ad-hoc `to_x` methods.
- Unit tests inline in `#[cfg(test)] mod tests { use super::*; }`; integration
  tests in `tests/`. Cover the happy path, the edges, and the error conditions.
- `rustfmt` defaults; no hand-formatting. No wildcard imports. Doc-comment public
  items; omit comments that restate the code.

`AGENTS.md` is the fuller, agent-facing version of all of this — read it before
making a change.

## Landing a change

This repository uses **Jujutsu (jj)**, not git, in a colocated repo. Never invoke
`git` directly — it can corrupt jj's operation log.

`main` is **protected**: it moves only through a pull request whose CI is green.

1. Make one focused change per commit, on a bookmark cut from `main`:
   `jj new main`, then `jj bookmark create <branch> -r @`.
2. `mise run check` — push only when it passes.
3. `jj describe -m "<imperative one-liner>"`, then
   `jj git push --bookmark <branch>`.
4. `gh pr create -R Jarvvski/momo --base main --head <branch>`.
5. Merge only once the `check` job is green, with `gh pr merge <n> --squash`.
6. Resync: `jj git fetch`, `jj bookmark set main -r main@origin`, `jj new`.

## Releases

Releases are cut with momo itself — `momo release`, commit, `momo tag`. Pushing a
`v*` tag triggers `.github/workflows/release.yml`, which builds Linux and macOS
binaries and attaches them to a GitHub release. Those assets are what
`mise use github:Jarvvski/momo@<version>` resolves.

## More

- [`docs/adr/`](adr/) — architecture decisions.
- [`docs/agents/`](agents/) — how issues are tracked in this repo.
- [`docs/mise-task-distribution.md`](mise-task-distribution.md) — the playbook for
  shipping momo's tasks and a pinned CLI into a consumer repo.
