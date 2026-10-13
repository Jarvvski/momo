# 19: An empty `MOMO_*` value counts as "set" and can redirect writes

Type: bug
Status: resolved
Severity: medium
Area: domain/config
Verified: yes — read at the cited lines

## Symptom

`from_env` stores any present value, including the empty string, as an override.
For path and prefix keys an empty value is destructive rather than merely wrong.

## Evidence

`src/domain/config.rs:307-309`:

```rust
"MOMO_CHANGELOG" => overrides.changelog = Some(PathBuf::from(value)),
"MOMO_CHANGES_DIR" => overrides.changes_dir = Some(PathBuf::from(value)),
"MOMO_TAG_PREFIX" => overrides.tag_prefix = Some(value.to_owned()),
```

`repo_root.join("")` is the repo root itself (`src/domain/package.rs:118`), and
`latest_tag("")` parses *every* tag as a version (`src/adapters/vcs/tags.rs:15-16`).

## Trigger

- `MOMO_CHANGES_DIR=""` with `recording = "fragments"` makes the fragment store
  the repo root, and `recording::consume` (`src/cli/recording.rs:109-116`) then
  deletes every `<kind>-*.md` in the repo root.
- `MOMO_CHANGELOG=""` makes release try to `fs::write` the package directory.
- `MOMO_TAG_PREFIX=""` makes every tag look like a version.

The file keys (`parse_path`, `src/domain/config.rs:417-419`) behave the same.

## Suggested direction

Treat an empty environment value as unset (skip the override), or reject empty
values for keys where they are meaningless.

## Open questions for triage

- Is an empty value ever meaningful? (`MOMO_TAG_PREFIX=""` could arguably mean
  "no prefix" — but then it should be explicit, not an empty string.)

## Triage

**Verdict:** Confirmed. `from_env` (`config.rs:307-309`, `:316-317`) stores any
present value verbatim, so an empty string becomes an override. For the path and
prefix keys that is destructive, not merely wrong: `repo_root.join("")` is the
repo root (`package.rs:118`), so `MOMO_CHANGES_DIR=""` makes the fragment store
the repo root and `recording::consume` (`recording.rs:109-116`) deletes every
`<kind>-*.md` there; `MOMO_CHANGELOG=""` makes release `fs::write` a directory.

**Corrections:** Two imprecisions. (1) The ticket says `latest_tag("")` parses
*every* tag as a version; it parses every tag that is itself a bare semver
(`tags.rs:15-16` `Version::parse(tag)`), ignoring non-versions. (2) The same
empty-value hole exists in the **file** layer: `parse_path` (`config.rs:417-419`)
also accepts `""`. An empty env var is a shell accident and should be treated as
unset; an explicit `changelog = ""` in the file is a written mistake and should
be rejected. Cover both.

**Scope:** Overlaps 24 (both validate config values; the same `MOMO_CHANGELOG`
path triggers both). Keep separate — 19 is empty handling, 24 is absolute/`..`
handling — but they can land as one "validate path-valued config" change.

**Implementation sketch:**

- In `ConfigOverrides::from_env`, skip empty values before the match:
  `if value.is_empty() { continue; }`, documented as "an empty `MOMO_*` is
  treated as unset".
- In `parse_path`, reject `Some("")` with a clear error (share the
  `validate_path` helper with 24).
- Tests: `from_env([("MOMO_CHANGELOG", "")])` yields
  `ConfigOverrides::default()`; `changelog = ""` in the file errors; a set
  `MOMO_TAG_PREFIX` still wins over the file.

## Resolution

`ConfigOverrides::from_env` now skips empty values before the match, documented
as "an empty `MOMO_*` is treated as unset", so `MOMO_CHANGES_DIR=""` can no
longer point the Fragment store at the repo root. The file layer rejects an empty
path through the shared `validate_path` helper (`ConfigError::EmptyPath`). Tests:
`an_empty_env_value_is_treated_as_unset`, `an_empty_path_in_the_file_is_rejected`.
