# 13: Teaching errors

Type: task
Status: ready-for-agent
Points: 12
Bucket: Trust & correctness
Severity: medium
Area: cli/diagnostic + domain

## Problem

momo's error surface is a single line: `error: {err:#}`, exit 1
(`src/lib.rs:38`). The prose is often good, but universally missing:

1. **No code** — nothing stable to grep, document, or allow-list in CI.
2. **No structured error under `--format json`** — `--format` only reaches
   `output::emit`; a JSON consumer gets prose on the one path it most needs to
   handle.
3. **Ad-hoc `bail!` strings** at the CLI boundary — untyped, unmatchable,
   inconsistent: `commands.rs:91, 156, 204, 252, 267, 280, 291, 370`,
   `recording.rs:104`, `prepare.rs:121-127`, `tag.rs:45-49`.
4. **Typed domain errors exist** (`ChangelogError`, `VersionError`, `VcsError`,
   `TaskEnvError`, `GraphError`, `ConfigError`, `SelectionError`, `WorkspaceError`)
   but carry no code, no JSON shape, no URL.

Weakest by user impact: `nothing unreleased` (no why/fix, the most-hit CI path);
`no VCS detected — nothing to tag`; `nothing to tag: run \`momo release\` first`;
`VcsError::TagAlreadyExists`; `ConfigError::UnknownKey` (no URL for the
migration-critical `tag_kind` case).

## Evidence

- `src/lib.rs:33-41` (`eprintln!("error: {err:#}")`), `:37-40` (exit codes).
- `src/cli/commands.rs:78-95` (`reject_output_flags`), `:312-324`
  (`warn_stale_lockfile`).
- `src/domain/version.rs:339-348` (`StaleCandidate` teaches well, no code),
  `src/domain/workspace.rs:51-60` (`Ambiguous`, no code).
- `src/cli/output.rs:33-53` (`--format` never reaches an error path).

## Proposal

**Codes live on the error types, rendered through one CLI `Diagnostic`.** Not a new
monolithic enum.

- Add a pure trait `Coded { fn code(&self) -> &'static str; fn fix(&self) ->
  Option<String> { None } fn url(&self) -> Option<&'static str> { None } }`,
  implemented on each domain enum (one line per variant via a local macro) and on a
  new CLI-local `CliError` that absorbs the `bail!` sites. Returning `&'static str`
  is pure, so the domain-purity guard stays satisfied.
- Codes are per variant, namespaced: `momo.changelog.unversioned-section`,
  `momo.version.stale-candidate`, `momo.vcs.tag-exists`, `momo.selection.ambiguous`,
  `momo.config.unknown-key`, `momo.release.nothing-unreleased`, `momo.tag.no-vcs`,
  `momo.usage.unsupported-flag`, …
- **Coexist with `anyhow`.** Keep `anyhow::Result` and the `.context(...)` chains
  (they are the *why*). At the single boundary (`lib.rs`), walk `err.chain()` and
  take the **first** `Coded` element; the full chain becomes the explanation.
- **Do not code every variant for symmetry** — only errors a user can act on; let
  internal errors fall through to `momo.error.unclassified`.
- **`install` with no task environment stays a warning** (exit 0, by design); make
  it structured/visible, not fatal.

## Surface

Human:
```
error[momo.version.stale-candidate]: the cut candidate `1.1.0-rc.1` is stale: the
changes now imply `2.0.0`
  fix: cut a new candidate for `2.0.0` (`momo candidate`)
```

JSON, on `--format json`, **to stdout**, exit still non-zero:
```json
{ "error": { "code": "momo.version.stale-candidate", "message": "…",
  "fix": "…", "url": "https://…/docs/errors.md#momo-version-stale-candidate",
  "context": ["validating the configuration"] } }
```

- New `docs/errors.md` (one section per code) + an integration test that
  enumerates `code()` values and asserts each has a section (the same
  no-drift discipline as `every_command_has_an_installed_verb`).
- `docs/migrating.md:337-349`: each row gains its code.
- Keep the rejection of `--format` on record verbs; a JSON error from
  `momo add --format json` is itself the rejection, rendered consistently.

## Implementation sketch

- `src/domain/diagnostic.rs`: the `Coded` trait + an `impl_coded!` macro.
- Implement `Coded` on each domain enum, beside its definition.
- `src/cli/diagnostic.rs`: `struct Diagnostic { code, message, fix, url, context }`;
  `diagnose(err: &anyhow::Error) -> Diagnostic` (chain scan, first `Coded` wins,
  fallback `momo.error.unclassified`); `human()`/`json()`.
- `src/cli/commands.rs`: replace each `bail!` with `Err(CliError::….into())`.
- `src/lib.rs::run`: on `Err`, build the `Diagnostic`, render per `--format`.
- `docs/errors.md` + the drift test.

## Scoring

Points **12** = Value 4×2 + Ease 2 + Novelty 2. High leverage for a report-only tool
(errors are its main UX), but broad mechanical work across ~11 enums → L effort.

## Risks

- **Message backwards-compat.** CI and docs grep `error:`; keep the leaf message
  text identical so substring matches survive. Ship as a minor.
- **Over-engineering** — the trait + enum + catalog + drift test is real surface.
  Guard: only code actionable errors.
- **JSON errors on stdout** is a convention choice; if reviewers prefer stderr, flip
  it, but document it.
- **Domain/CLI split of `Coded`** — putting `code()` in the domain couples it to a
  user-facing naming scheme; the domain already owns `Display`, so it is consistent,
  but note it in the PR.

## Related

- 02 (the changelog refusal should arrive already coded), `docs/ci.md`,
  `docs/migrating.md`.
