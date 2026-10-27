# 10: A `--dry-run` release can leave ancestor directories behind

Type: bug
Status: ready-for-agent
Severity: low
Area: cli/release
Verified: yes — read at the cited lines

## Symptom

Staging creates a nested directory for a target such as `docs/release/CHANGELOG.md` but records only the leaf, so `discard` removes `docs/release` and leaves the empty `docs` behind — even on `--dry-run`, which promises to write nothing.

## Evidence

`Staged::add` calls `fs::create_dir_all(parent)` but pushes only `parent` onto `created` (`src/cli/release/prepare.rs:305-317`); `discard` `remove_dir`s each recorded path (`prepare.rs:336-343`), so the ancestors `create_dir_all` made are not removed.

## Trigger

`recording = "fragments"`, `changelog = "docs/release/CHANGELOG.md"`, one fragment, `momo release --dry-run`; `docs/` remains.

## Suggested direction

Record every directory `create_dir_all` actually created (walk up from `parent` until an existing ancestor), and remove them deepest-first on discard. Or track the first missing ancestor and `remove_dir_all` it on discard when it is still empty. Extend the existing `a_dry_run_release_leaves_no_new_directory_behind` test to two levels.
