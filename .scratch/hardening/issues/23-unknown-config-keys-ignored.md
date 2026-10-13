# 23: Unknown `momo.toml` keys are silently ignored

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/config
Verified: yes — read at the cited lines

## Symptom

`ConfigOverrides::from_str` reads only the keys it knows and never inspects the
rest. A misspelled key parses fine, does nothing, and the default silently wins —
which is exactly the failure mode the config layer is supposed to avoid.

## Evidence

`src/domain/config.rs:329-342`:

```rust
let doc: DocumentMut = input.parse()?;
Ok(Self {
    recording: parse_scalar::<Recording>(&doc, "recording")?,
    changelog: parse_path(&doc, "changelog")?,
    ...
    packages: parse_packages(&doc)?,
})
```

Nothing iterates the document to find unknown keys, at the top level or inside
`[packages.*]`. Unknown *environment* names are ignored by design and documented;
the file has no such statement.

## Trigger

`recordingg = "fragments"` in `momo.toml`. momo runs with `recording = "changelog"`
and says nothing.

## Suggested direction

Detect unknown top-level and per-package keys and error (or warn) naming them.

## Open questions for triage

- Error or warn? Erroring is stricter and catches typos; warning is friendlier for
  forward-compatibility if a newer momo adds keys.

## Triage

**Verdict:** Confirmed. `ConfigOverrides::from_str` (`config.rs:329-342`) reads
only the keys it knows and never inspects the rest; `parse_packages`
(`:455-488`) likewise reads `identity`/`slug`/`manifest` and ignores everything
else in a package table. A misspelled key parses, does nothing, and the default
silently wins.

**Ruling: error, not warn.** The config layer exists to avoid exactly this
silent-default-wins failure mode, and a typo (`recordingg = "fragments"`) should
fail loudly. Forward-compatibility is a weaker concern for a pre-1.0 tool, and
an older momo meeting a newer file should say so rather than silently ignore the
new key. (Detection is identical either way — only the emit differs — so the
owner can downgrade to a warning cheaply if they prefer.)

**Corrections:** The docs only say *environment* names are ignored
(`docs/configuration.md:72`); the ticket is right that the file has no such
statement, so the current behaviour is undocumented. The check must cover both
the top level and every `[packages.*]` table, and must not flag the `packages`
key itself.

**Scope:** Standalone; grouped with 19 and 20 as config integrity (PR D).

**Implementation sketch:**

- Add `ConfigError::UnknownKey { key: String }` with
  `#[error("unknown key `{key}`")]`.
- Define the known top-level keys as a const slice (`recording`, `changelog`,
  `changes_dir`, `tag_prefix`, `remote`, `task_env`, `rc_format`, `packages`;
  drop `tag_kind` per ticket 21) and, in `from_str`, iterate
  `doc.as_table().keys()` and error on anything not in the set.
- In `parse_packages`, iterate each package table's keys and error on anything
  not in `{identity, slug, manifest}`, formatting the key as
  `packages.<identity>.<name>` (reuse the same helper).
- Tests: `recordingg = "fragments"` errors naming the key;
  `[packages."cargo:momo"] slgu = "x"` errors; a fully-valid file still parses;
  `unknown_env_vars_are_ignored` (`:590`) stays green (env is unchanged).
