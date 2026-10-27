# 01: `momo add` cannot record into a nested changelog path

Type: bug
Status: ready-for-agent
Severity: high
Area: cli/recording
Verified: yes — reproduced with the built binary

## Symptom

With `changelog = "docs/CHANGELOG.md"`, `momo add` fails and records nothing. `momo release` with the same config works, so the recording model is internally inconsistent: the verb a user reaches for first is the one that breaks.

## Evidence

`src/cli/recording.rs:50-64` (the `Recording::Changelog` arm of `record`) writes through `atomic::write(&path, …)`. `atomic::write` stages a temporary file in the target's own directory (`src/cli/atomic.rs:21-30`, `:59-76`) and never creates that directory, so a `changelog` value with a directory component fails when the directory does not already exist.

The release path does create it: `Staged::add` calls `fs::create_dir_all(parent)` (`src/cli/release/prepare.rs:306-312`). The fragments path creates its directory too (`src/cli/recording.rs:70`), and `FileVersionSource::write` creates `.momo/`.

## Trigger

```console
$ mkdir demo && cd demo
$ printf 'changelog = "docs/CHANGELOG.md"\n' > momo.toml
$ momo add "Add a thing"
error[momo.error.unclassified]: writing …/docs/CHANGELOG.md: No such file or directory (os error 2)
$ echo $?
1
```

## Suggested direction

Create the changelog's parent directory before the write, exactly as the release path does — `fs::create_dir_all` on `path.parent()` when it is non-empty. Keep it inside `record`'s changelog arm, after the `dry_run` check, so a dry-run add still writes nothing. Add a regression test with a nested `changelog` path.
