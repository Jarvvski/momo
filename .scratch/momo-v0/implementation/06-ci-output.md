# 06: Non-interactive output and `status --check`

**What to build:** The CI-facing surface. `--format json` and `--version-only`
on `status` (and on `release`/`candidate` where meaningful), `--output <path>`
to write the same content to a file, and `status --check`, which exits non-zero
when there is nothing Unreleased. momo stays VCS-agnostic: it never sniffs
`CI_*` or `GITHUB_*`.

**Blocked by:** 04

**Spec:** `.scratch/momo-v0/spec.md` §13

**Status:** resolved

- [x] `momo status --format json` emits a stable, documented shape.
- [x] `momo status --version-only` prints just the version.
- [x] `momo status --check` exits non-zero when nothing is Unreleased, and zero when something is.
- [x] `--output <path>` writes the same content to a file.
- [x] No behaviour changes on the basis of CI environment variables.
