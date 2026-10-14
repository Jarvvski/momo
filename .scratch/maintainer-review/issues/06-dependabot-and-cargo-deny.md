# 06: Add Dependabot and cargo-deny

Type: task
Status: ready-for-agent
Severity: medium
Area: supply chain
Verified: yes — `.github/` contains only `workflows/`; there is no `dependabot.yml` or `deny.toml`.

## Symptom

Dependencies are never audited for advisories, license compatibility or duplicates, and nothing proposes version bumps. momo shells out to git/jj/mise and publishes a crates.io package, so its dependency set is part of the trust surface.

## Suggested direction

- `.github/dependabot.yml`: ecosystems `cargo` and `github-actions`, weekly, with a sensible open-PR limit.
- `deny.toml` plus a `deny` job: install `cargo-deny` (e.g. `taiki-e/install-action@cargo-deny`) and run `cargo deny check advisories bans licenses sources`. Start `licenses` from the project's own license (ticket 14); keep `advisories` and `bans` strict.
- Do **not** edit `mise.toml` for this — invoke `cargo deny` directly in the workflow so the gates workspace (Group A) keeps ownership of `mise.toml`.

## Related

- 05, 14, Group B.
