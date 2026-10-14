# 05: Add CI concurrency and pin actions to commit SHAs

Type: task
Status: resolved
Severity: low
Area: CI
Verified: yes — `.github/workflows/ci.yml` has no `concurrency`; both workflows reference third-party actions by tag (`actions/checkout@v4`, `jdx/mise-action@v2`, `Swatinem/rust-cache@v2`, `actions/upload-artifact@v4`, `actions/download-artifact@v4`).

## Symptom

Two things:

1. Every push to a pull request starts a fresh `check` run, so rapid pushes stack redundant, expensive builds.
2. Third-party actions are referenced by mutable tags, so a compromised or retagged action version runs unreviewed code in CI — and in the release job, which holds the registry token.

## Suggested direction

- Add to `ci.yml`: `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }`.
- Pin every third-party action to a full commit SHA with the version as a trailing comment (e.g. `actions/checkout@<sha> # v4`). Keep `actions/*` pinned too; Dependabot (ticket 06) can bump the pins.

## Related

- 06, Group B.
