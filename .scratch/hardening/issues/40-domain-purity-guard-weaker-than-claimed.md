# 40: The domain purity guard is weaker than it claims

Type: bug
Status: ready-for-agent
Severity: low
Area: mise.toml (tooling)
Verified: yes — read the task definition

## Symptom

`mise run lint:domain` greps for the literal strings `std::fs`, `std::process`,
`std::env`, `std::net`, `clap`, `anyhow`. A `use std::{env, fs};` does not contain
any of those literals, so a future domain change could reach the environment
without tripping the guard. The invariant ("`src/domain` is pure") is enforced
only as strongly as the regex.

## Evidence

`mise.toml:35`:

```bash
if grep -rnE 'std::(fs|process|env|net)|clap|anyhow' src/domain; then
```

`src/domain/config.rs` is currently clean — `from_env` takes an iterator
(`config.rs:294`) rather than calling `std::env` — which is the correct workaround
and exactly the pattern the guard should protect.

## Suggested direction

Enforce purity structurally (the crate split that ADR 0002 collapsed, or a
`cargo`-level lint) or tighten the check (e.g. also match `use std::{...}` forms
and bare `env::`/`fs::` calls).

## Open questions for triage

- Is a stricter grep worth it, given ADR 0002 deliberately collapsed the workspace
  for publishing simplicity? A middle ground: a small integration test that walks
  `src/domain/**` and asserts no import of the forbidden modules.

## Related

- 39 (port vocabulary), ADR 0002.

## Triage

**Verdict.** Confirmed: `mise.toml:35` greps only for the literals
`std::fs|process|env|net`, `clap`, `anyhow`, so `use std::{env, fs};` and a bare
`env::var(..)`/`fs::read(..)` slip through. ADR 0002 deliberately collapsed the
crate split and says the invariant "is now enforced by `mise run check`", so the
structural fix is off the table and enforcement must be textual.
**`ready-for-agent`.**

**Corrections.**
- The guard also omits `std::io`, which the domain uses *today*
  (`vcs.rs:84`, `taskenv.rs:104/114/124/134`, `version.rs:184/202`,
  `manifest.rs:66/76`). Adding `io` to the ban must wait on ticket 39.
- `[lints.*]` in `Cargo.toml` cannot express "no `std::fs` in this module", so a
  cargo-level lint is not an option either; the real choices are a stricter
  pattern and where the check lives.

**Scope.** Keep separate from 39 (39 removes the vocabulary; 40 guards it). Not a
duplicate.

**Ruling — integration test vs stricter grep.** Make a **`tests/domain_purity.rs`
integration test the source of truth**, so the invariant is enforced by
`cargo test` (which `mise run check` already runs) and not only by the
`lint:domain` task a contributor can skip; keep the mise grep as a fast
pre-flight. The *pattern* matters more than the home — both must catch the grouped
and bare forms.

**Implementation sketch.**
- `tests/domain_purity.rs`: walk `src/domain/**/*.rs` with `std::fs`; for each file
  fail on (a) any `std::<mod>` occurrence for `mod ∈ {fs, process, env, net}`,
  (b) any `use std::` line whose import list names one of those modules (catches
  `use std::{env, fs};`), (c) `clap`/`anyhow`. Dependency-free (no `regex` crate).
- Inline `#[cfg(test)]` cases over synthetic snippets: positive
  (`use std::{env, fs};`, `std::process::Command`, `env::var`) and negative
  (`std::collections::BTreeMap`).
- Broaden the `mise.toml` regex to the same effect; leave `io` out until 39 lands.
