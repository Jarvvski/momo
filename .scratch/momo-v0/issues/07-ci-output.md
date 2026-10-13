# CI / non-interactive output & dry-run shape

Type: grilling
Status: resolved
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

## Answer

Settled (all recommendations accepted):

1. **Output:** human by default; `--format json` for the full picture (unreleased
   changes + per-package computed versions); `--version-only` prints just the
   version.
2. **CI bake-in:** stdout by default, `--output <path>` to write to a file. No
   CI-specific env vars.
3. **Exit codes:** exit 0 on success; **`momo status --check` exits non-zero when
   there are no unreleased changes** — the CI skip gate. *Owner note: this exit-code
   contract must be documented.* A "PR changed code but added no fragment" gate is
   deferred (it needs a VCS diff).
4. **CI context:** VCS-agnostic — momo reads the VCS (tags) and never `GITHUB_*` /
   `CI_*`. CI orchestrates; momo just answers.

**Documentation debt:** the `--check` exit-code contract is called out as something
the spec / docs must state explicitly.
