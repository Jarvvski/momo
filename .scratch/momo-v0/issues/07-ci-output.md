# CI / non-interactive output & dry-run shape

Type: grilling
Status: claimed
Blocked by: 01

## Question

How does momo behave in CI, and what does `--dry-run` emit?

- **Output shape** for the computed next version: human text, `--format json`,
  a `--version-only` mode, or written to a file?
- **How CI "bakes in" the version:** env var, file, or stdout parse?
- **Exit codes:** non-zero when there is nothing unreleased? distinct codes for
  "would change" vs "changed"?
- **Safety:** which commands are read-only (`status`, `--dry-run`) vs mutating
  (`release`)?
- Does momo read CI context (branch, tag, PR) or stay VCS-agnostic?
