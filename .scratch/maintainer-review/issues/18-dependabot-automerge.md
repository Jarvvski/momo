# 18: Auto-merge Dependabot pull requests once CI is green

Type: task
Status: resolved
Severity: low
Area: supply chain
Verified: yes — Dependabot (ticket 06) is enabled and opening weekly bumps for `cargo` and `github-actions`, but no workflow reacts to them, so every bump waits for a manual merge.

## Symptom

Dependabot is doing its job: `.github/dependabot.yml` opens a weekly pull request for each `cargo` and `github-actions` bump, each advancing a commit-SHA pin and its trailing version comment. Nothing lands them. A green bump sits open until a maintainer notices it and merges by hand, so the pins drift behind and the queue grows.

## Decision (2026-10-14)

Auto-merge Dependabot pull requests. "Merge if things look good" is read as *the repo's required checks pass*: the workflow enables GitHub auto-merge and lets GitHub perform the merge once `check` is green, rather than merging directly. Every update type auto-merges — majors included — because the `check` job (fmt/lint/docs/build/test) is the gate. A commented, one-line `if:` in the workflow lets the owner exclude `version-update:semver-major` later if majors should be reviewed by hand.

## Resolution

`.github/workflows/dependabot-automerge.yml`, on `dependabot-automerge`.

- Triggered on `pull_request`, with the job guarded by `if: github.actor == 'dependabot[bot]'`.
- `dependabot/fetch-metadata` (SHA-pinned, like every action here) identifies the update and its type.
- `gh pr merge --auto --squash "$PR_URL"` with `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` enables auto-merge; GitHub then merges once the required checks pass, so the workflow never merges past a red check.
- `GITHUB_TOKEN`, not a PAT: it is minted per run and expires with it, so there is no long-lived credential to rotate or leak. Dependabot-triggered runs get a restricted, read-only token, so the workflow's `permissions: { contents: write, pull-requests: write }` is what enables auto-merge.

## Caveat

The owner's `gh` token lacks the `workflow` OAuth scope, so the pull request that adds this file cannot be merged — nor auto-merged — from the CLI. The owner merges it, or refreshes the token scope.

## Related

- 06 (Dependabot and cargo-deny), 05 (action SHA pins).
