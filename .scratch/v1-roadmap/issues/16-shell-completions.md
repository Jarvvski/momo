# 16: Shell completions

Type: task
Status: ready-for-agent
Points: 10
Bucket: jdx ecosystem fit
Severity: low
Area: cli
Blocked by: 10

## Problem

momo ships no shell completions. Every jdx-shaped CLI is expected to have them, and
they are the concrete capability that `usage` adoption would eventually buy — but
they need not wait for `usage`.

## Evidence

- `src/cli/mod.rs` (the clap tree; `clap` is already a dependency).
- No completions subcommand or generated assets exist.

## Proposal

- Add `Command::Completions { shell: clap_complete::Shell }` (writes to stdout),
  excluded from `INSTALLABLE_VERBS` (the split from ticket 10).
- Add `clap_complete = "4"` to `Cargo.toml` (~10 lines of handler).
- **Defer `usage` (KDL).** Its payoff (typed specs, man pages, reference docs,
  breaking-change diffing) is real but disproportionate for nine verbs. `usage` can
  *export from clap*, so a later spike can emit a KDL spec without touching the
  parser — revisit when the surface grows or generated reference docs are wanted.

## Surface

- `momo completions <shell>` writes completions to stdout.
- `docs/usage.md`: a "Shell completions" section with the per-shell install line.

## Implementation sketch

- `src/cli/mod.rs`: the `Completions` variant + `clap_complete::Shell`.
- `src/cli/commands.rs`: the handler (`clap_complete::generate`), excluded from
  `INSTALLABLE_VERBS`.
- `Cargo.toml`: `clap_complete = "4"`.
- `tests/cli.rs`: the verb/flag lists; a smoke test that `momo completions bash`
  emits non-empty output.

## Scoring

Points **10** = Value 2×2 + Ease 4 + Novelty 2. Lowest points in the effort — a
nice-to-have, gated on ticket 10's `INSTALLABLE_VERBS` split.

## Risks

- Depends on the `INSTALLABLE_VERBS` split (ticket 10) or the install test breaks.

## Related

- 10 (prerequisite), 05 (attestations).
