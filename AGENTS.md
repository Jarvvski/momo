# AGENTS.md - momo project memory

> `CLAUDE.md` is a symlink to this file. Edit this file; never the symlink.

## What this is

momo is a standalone, monorepo-aware Rust CLI that turns a project's accumulated changes into semver releases, a changelog and VCS tags. The crate is `momo-cli`; the binary is `momo`. It is VCS-optional (git, jj, or no VCS at all) and treats `mise` as the task environment.

## Landing a change (jj, NOT git)

The owner uses **Jujutsu exclusively** in a colocated repo. **Never invoke `git`** - it can corrupt jj's operation log. jj has no staging area; the working copy is always a commit.

`main` is **protected**: it moves only through a pull request whose CI is green. Land a change like this:

1. Make **one focused change** per commit, on its own bookmark cut from `main`: `jj new main`, then `jj bookmark create <branch> -r @`.
2. Run `mise run check` (fmt + lint + domain purity + build + test); push only when it passes. (jj fires no git hooks, so fmt is a task, not a pre-commit hook.)
3. `jj describe -m "<imperative one-liner>"`, then `jj git push --bookmark <branch>`.
4. Open a PR: `gh pr create -R Jarvvski/momo --base main --head <branch>`.
5. Merge **only** once the `check` job is green: `gh pr checks <n>`, then `gh pr merge <n> --squash`.
6. Resync: `jj git fetch`, then `jj bookmark set main -r main@origin`, then `jj new`, so `@` is again an empty commit above `main`.

Agents may push feature branches, open PRs, and merge green PRs. Agents must **never** push or force-push directly to `main`, and never merge past a failing or missing check. The owner handles releases and tags.

## Toolchain

**mise** owns tasks and the Rust pin (see `mise.toml`, `rust-toolchain.toml`). Use the tasks, don't hand-roll cargo:

- `mise run check` - the pre-land gate: fmt + lint + domain purity + build + test.
- `mise run run` / `build` / `test` / `fmt` / `lint` - individual steps.
- `mise run lint:domain` - the domain purity guard: no I/O, processes, the environment, clap or anyhow under `src/domain`.
- `mise run install` - install the binary to `~/.cargo/bin`.

Rust edition 2024. `Cargo.lock` is committed (this is an app, not a library).

## Layout

One crate: a library plus a thin binary. `src/domain/` is pure - kinds, bump policy, versions, changelog, config and the ports; `src/adapters/` implements those ports against git, jj, mise and `Cargo.toml`; `src/cli/` is private and reached through `momo_cli::run`. See `docs/contributing.md` for the tree, and ADR 0002 for why it is one crate.

**Crate boundary.** Only `src/`, `tests/` and the top-level files ship in the published crate; `Cargo.toml`'s `exclude` list is what keeps `.claude/`, `.agents/`, `.scratch/`, `docs/` and `assets/` out of it. A new crate needs the same list repeated - nothing fails if it is forgotten.

## Rust conventions

**Build loop.** After any substantive change, run `mise run check`; land only when green. Use `cargo check` for fast iteration and run the full test suite only when logic changed. Clippy runs **warnings-as-errors** - fix the root cause, never paper over a lint with a scattered `#[allow(...)]` (if a lint must be disabled, do it locally with a comment saying why). Leave no `dbg!`, stray `println!`, or commented-out code behind.

**Correctness.** Code that compiles is not code that is correct - Rust stops memory bugs, not logic bugs. Verify behavior; don't assume "it compiled so it works." Read compiler and clippy spans in full and apply the suggested fix - they are precise.

**Error handling.** No `.unwrap()` outside tests; use `.expect("why")` only for true invariants, with a message explaining why it cannot fail. Propagate with `?`; return `Result<T, E>` for anything fallible. This is a binary - use `anyhow` + `.context(...)` at the top level; define a `thiserror` enum only when callers need to match on specific error variants. Prefer `Option<T>` over sentinel values.

**Ownership (the #1 agent pitfall).** Do NOT reach for `.clone()`, `Rc<RefCell<_>>`, or `Arc<Mutex<_>>` just to silence the borrow checker - restructure ownership/lifetimes instead. When a closure fights the borrow checker, add `move` and adjust ownership. Never return a reference to a local/temporary; return owned data if lifetimes get hard. A `.clone()` on a non-`Copy` type is a conscious decision, not a reflex.

**Idioms.** Prefer iterators/combinators over manual index loops (`enumerate()`, `if let`/`while let`). Borrow in signatures (`&str`, `&[T]`); return owned only when needed. Implement std traits (`From`/`TryFrom`/`Display`/`Default`) over ad-hoc `to_x`/`make_x` methods; use newtypes to distinguish semantically different values. Match exhaustively; avoid a catch-all `_` arm when a new enum variant should force a compile error.

**Testing.** Unit tests live inline in a `#[cfg(test)] mod tests { use super::*; }` block; integration tests go in `tests/`. Cover the happy path, edges (empty/boundary), and error conditions for new logic.

**Unsafe.** Forbidden crate-wide (`unsafe_code = "forbid"` in `Cargo.toml`) - `unsafe` is a hard compile error, not a lint. Do not remove the forbid to work around a problem; find a safe design instead (if `unsafe` ever becomes genuinely unavoidable, that's an owner decision).

**Layout & style.** rustfmt defaults; no hand-formatting. No wildcard imports (except a deliberate prelude); group std / external / local and reference intra-crate items via `crate::`. Doc-comment (`///`) public items; omit comments that merely restate the code. Revise files in place - no `main_v2.rs` / `foo_improved.rs` variants.

## Agent skills

### Issue tracker

Issues live as markdown files under `.scratch/<feature>/` in this repo. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical roles, used verbatim: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Mise task distribution

How momo ships tasks + a pinned CLI to other repos, and why it uses self-contained task files rather than a plugin or remote include. See `docs/mise-task-distribution.md`.
