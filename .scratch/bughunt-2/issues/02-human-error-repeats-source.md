# 02: The human error line repeats a source already in its parent's message

Type: bug
Status: resolved
Severity: medium
Area: cli/diagnostic
Verified: yes — reproduced with the built binary

## Symptom

When an error's `Display` already interpolates its `#[source]` (a thiserror
`{0}`/`{source}` in the message), the human rendering prints that source text
twice, because `chain` is built with `format!("{err:#}")` — anyhow's alternate
display, which appends every source again.

## Evidence

`src/cli/diagnostic.rs:83` sets `chain: format!("{err:#}")`, and `human()`
(`:90`) renders `error[<code>]: <chain>`. Reproduced:

```
$ printf 'not-a-version\n' > .momo/version && momo tag
error[momo.version.invalid]: `…/.momo/version` does not contain a valid version: unexpected character 'n' while parsing major version number: unexpected character 'n' while parsing major version number
```

`VersionError::Invalid` (`src/domain/version.rs:326`) embeds `{source}` *and*
declares `#[source]`; `VersionError::Vcs` (`:344`, `"reading the latest tag: {0}"`)
and `TaskEnvError::Graph` (`src/domain/taskenv.rs:154`) have the same shape.
`docs/ci.md:200` documents the human line as `error[<code>]: <message>`, matching
the JSON `message` — which does not duplicate.

## Fix direction

Build the human `chain` from `err.chain()` and skip an element whose `Display` is
already a suffix of what has been accumulated, so a source embedded in its
parent's message is not appended again. This keeps the tested context+message
form (`diagnostic.rs:277`: `validating the configuration: nothing unreleased`)
and makes the two reproduced cases render once. Add unit tests for a coded error
with an embedded source (`VersionError::Invalid`) and for the wrapper case
(`VersionError::Vcs`). Consider aligning `docs/ci.md:200`, which says the line is
just `<message>` while the code (and its test) include the context.
