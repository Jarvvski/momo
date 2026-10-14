# 04: Run the test suite on macOS in CI

Type: task
Status: resolved
Severity: medium
Area: CI
Verified: yes — `.github/workflows/ci.yml` runs `check` on `ubuntu-latest` only; the release workflow *builds* on macOS but never runs the tests.

## Symptom

The crate ships macOS binaries and its process code is Unix-specific (`src/adapters/spawn.rs` uses `process_group(0)` and shells out to `kill`), yet the test suite only runs on Linux. A macOS-only regression in the spawn/process path would not be caught before release.

## Suggested direction

Add a macOS runner to the `check` job — either a matrix (`os: [ubuntu-latest, macos-latest]`) or a second job. Keep `MOMO_REQUIRE_TOOLS=1` so the jj/mise tests run. Watch total CI time; if it is too slow, run the full suite on Linux and `cargo test` (without the release build) on macOS.

## Related

- 11 (the process-group kill), Group B.
