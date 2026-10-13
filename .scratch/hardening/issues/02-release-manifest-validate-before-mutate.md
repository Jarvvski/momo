# 02: `--dry-run` never validates the manifest, so it green-lights a doomed release

Type: bug
Status: wontfix
Severity: medium
Area: cli/release
Verified: yes — read at the cited lines

## Symptom

`momo release --dry-run` reports a clean plan without ever parsing or
write-testing the manifest it is about to rewrite. A malformed or unwritable
`Cargo.toml` passes the dry run and only fails in the real run — after the
changelog has been written and the fragments consumed (ticket 01).

## Evidence

`src/cli/release.rs:213-229`:

```rust
let Some(manifest) = sync.locate(package_dir, package.manifest_override()) else {
    return Ok(None);
};
if dry_run {
    return Ok(None);          // <-- skips sync.write, which is what parses the TOML
}
Ok(sync.write(&manifest, version)?)
```

The dry-run `Outcome.manifest` is `None`, so `report_release` cannot even say that
a manifest *would* be touched. `warn_stale_lockfile` is gated on
`outcome.manifest` (`src/cli/commands.rs:184`), so the dry run is silent about the
lockfile too.

## Trigger

`momo release --dry-run` with a `Cargo.toml` that has a syntax error. Dry run
succeeds. `momo release` then fails at the manifest step.

## Suggested direction

Give the manifest sync a read-only validation step (locate + parse + report the
target) that runs during `--dry-run`, and carry a "would write `<path>`" marker
into the outcome so the report and the lockfile warning both fire.

## Open questions for triage

- Should `--dry-run` fail on an unparseable manifest, or warn and continue?

## Triage

**Verdict:** Real, but subsumed by 37's validate-first phase; folded in and closed
as superseded. Its dry-run *reporting* requirement must be carried into 37.

**Corrections:** Accurate. Confirmed at `src/cli/release.rs:222-228`:
`sync_manifest` runs `locate` (an `is_file` check) and then returns `Ok(None)`
*before* `sync.write`, so the TOML is never parsed under `--dry-run`. Because
`Outcome.manifest` is then `None`, `warn_stale_lockfile`
(`src/cli/commands.rs:184, 290`) is silent and the report cannot mention the
manifest.

**Scope:** Duplicate/merged into 37. The validate phase 37 introduces is exactly
the read-only manifest parse this needs; 37 must additionally populate
`Outcome.manifest` under `--dry-run` (without writing) so the lockfile warning
and the report both fire.

**Implementation sketch:** See 37 (prepare phase + dry-run outcome).
