# 01: `--json` is ignored on the failure path

Type: bug
Status: resolved
Severity: high
Area: cli/output, cli/diagnostic
Verified: yes — reproduced with the built binary

## Symptom

`--json` is documented as a shorthand for `--format json`, but a run that
*fails* renders the error as human text on stderr instead of one JSON document
on stdout. `--format json` and `--json` therefore diverge exactly on the path the
error contract exists for.

## Evidence

`src/lib.rs:45` selects the failure rendering with `cli.format.unwrap_or_default()`,
which reads only the long `--format` flag. The success path
(`src/cli/output.rs:51`, `render`) uses `cli.output_format()`, which honours the
`--json` shorthand (`src/cli/mod.rs:117`). Reproduced:

```
$ momo release --json          # exit 1, stdout empty, human error on stderr
$ momo release --format json   # exit 1, one {"error":{…}} document on stdout
```

`docs/ci.md:10` says `--json` is the shorthand for `--format json`, and
`docs/ci.md:183` says `--format json` renders the error on **stdout**.

## Fix direction

Use `cli.output_format()` in `src/lib.rs:45` so the failure path honours the
shorthand exactly as the success path does. `tests/cli.rs:449-451` currently pins
the buggy behaviour: it asserts a `--json` rejection lands on stderr while the
`--format` case is checked on stdout — update it so both land on stdout. Add a
regression test that `momo release --json` (nothing unreleased) prints one
`{"error":…}` document on stdout with empty stderr.
