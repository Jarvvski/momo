# AGENTS.md - momo project memory

> `CLAUDE.md` is a symlink to this file. Edit this file; never the symlink.

## What this is

momo is a new open-source project. Its scope is still being defined; the
description in `Cargo.toml` and `README.md` is deliberately pending.

## Landing a change (jj, NOT git)

The owner uses **Jujutsu exclusively** in a colocated repo. **Never invoke
`git`** - it can corrupt jj's operation log. jj has no staging area; the working
copy is always a commit.

1. Make **one focused change** per commit.
2. Run `mise run check` (fmt + lint + build + test); land only when it passes.
   (jj fires no git hooks, so fmt is a task, not a pre-commit hook.)
3. `jj describe -m "<imperative one-liner>"` then `jj bookmark set main --to @`
   then `jj new`.
4. Invariant: every time `main` moves, the next command is `jj new`, so `@` is
   always an empty commit one above main.

**Landing is strictly local. Agents must NEVER push from this repository.** Do
not run `jj git push`, create a PR as part of landing, or otherwise publish a
landed change. The owner handles all pushes.

## Toolchain

**mise** owns tasks and the Rust pin (see `mise.toml`, `rust-toolchain.toml`).
Use the tasks, don't hand-roll cargo:

- `mise run check` - the pre-land gate: fmt + lint + build + test.
- `mise run run` / `build` / `test` / `fmt` / `lint` - individual steps.
- `mise run install` - install the binary to `~/.cargo/bin`.

Rust edition 2024. `Cargo.lock` is committed (this is an app, not a library).

## Rust conventions

**Build loop.** After any substantive change, run `mise run check`; land only
when green. Use `cargo check` for fast iteration and run the full test suite
only when logic changed. Clippy runs **warnings-as-errors** - fix the root cause,
never paper over a lint with a scattered `#[allow(...)]` (if a lint must be
disabled, do it locally with a comment saying why). Leave no `dbg!`, stray
`println!`, or commented-out code behind.

**Correctness.** Code that compiles is not code that is correct - Rust stops
memory bugs, not logic bugs. Verify behavior; don't assume "it compiled so it
works." Read compiler and clippy spans in full and apply the suggested fix -
they are precise.

**Error handling.** No `.unwrap()` outside tests; use `.expect("why")` only for
true invariants, with a message explaining why it cannot fail. Propagate with
`?`; return `Result<T, E>` for anything fallible. This is a binary - use
`anyhow` + `.context(...)` at the top level; define a `thiserror` enum only when
callers need to match on specific error variants. Prefer `Option<T>` over
sentinel values.

**Ownership (the #1 agent pitfall).** Do NOT reach for `.clone()`,
`Rc<RefCell<_>>`, or `Arc<Mutex<_>>` just to silence the borrow checker -
restructure ownership/lifetimes instead. When a closure fights the borrow
checker, add `move` and adjust ownership. Never return a reference to a
local/temporary; return owned data if lifetimes get hard. A `.clone()` on a
non-`Copy` type is a conscious decision, not a reflex.

**Idioms.** Prefer iterators/combinators over manual index loops (`enumerate()`,
`if let`/`while let`). Borrow in signatures (`&str`, `&[T]`); return owned only
when needed. Implement std traits (`From`/`TryFrom`/`Display`/`Default`) over
ad-hoc `to_x`/`make_x` methods; use newtypes to distinguish semantically
different values. Match exhaustively; avoid a catch-all `_` arm when a new enum
variant should force a compile error.

**Testing.** Unit tests live inline in a `#[cfg(test)] mod tests { use super::*; }`
block; integration tests go in `tests/`. Cover the happy path, edges
(empty/boundary), and error conditions for new logic.

**Unsafe.** Forbidden crate-wide (`unsafe_code = "forbid"` in `Cargo.toml`) -
`unsafe` is a hard compile error, not a lint. Do not remove the forbid to work
around a problem; find a safe design instead (if `unsafe` ever becomes genuinely
unavoidable, that's an owner decision).

**Layout & style.** rustfmt defaults; no hand-formatting. No wildcard imports
(except a deliberate prelude); group std / external / local and reference
intra-crate items via `crate::`. Doc-comment (`///`) public items; omit comments
that merely restate the code. Revise files in place - no `main_v2.rs` /
`foo_improved.rs` variants.

## Agent skills

### Issue tracker

Issues live as markdown files under `.scratch/<feature>/` in this repo. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical roles, used verbatim: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
