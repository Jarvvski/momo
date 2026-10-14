# 01: Enforce formatting in CI

Type: task
Status: resolved
Severity: medium
Area: tooling / CI
Verified: yes — `mise run check` calls `mise run fmt`, which runs `cargo fmt --all` (writes), and `.github/workflows/ci.yml` runs `mise run check`.

## Symptom

CI never fails on formatting. `mise run check` → `mise run fmt` runs `cargo fmt --all`, which *rewrites* the working tree rather than checking it, so a mis-formatted pull request is silently reformatted on the runner and the check stays green. `main` can therefore hold code that `cargo fmt --check` rejects.

## Evidence

`mise.toml:22-24` (`fmt` = `cargo fmt --all`), `mise.toml:44-46` (`check` calls `fmt`), `.github/workflows/ci.yml:25` (`mise run check`).

## Suggested direction

Split the concern: keep `[tasks.fmt]` as the write task for local use, add `[tasks."fmt:check"]` running `cargo fmt --all -- --check`, and have `check` call `fmt:check`. Local `mise run check` then fails on drift and `mise run fmt` fixes it. `cargo fmt --all -- --check` is already clean on `main`, so this is a no-op today.

## Related

- Group A (gates).
