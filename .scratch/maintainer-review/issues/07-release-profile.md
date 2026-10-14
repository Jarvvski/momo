# 07: Tune the release profile

Type: task
Status: ready-for-agent
Severity: low
Area: packaging
Verified: yes — `Cargo.toml` has `[profile.dev]` and `[profile.test]` but no `[profile.release]`.

## Symptom

Release binaries are shipped unstripped and without link-time optimisation, so the published tarballs are larger than they need to be.

## Suggested direction

Add to `Cargo.toml`:

```toml
[profile.release]
strip = true
lto = "thin"
codegen-units = 1
```

`strip = true` is free; `lto = "thin"` and `codegen-units = 1` trade a little build time for size/speed. Confirm `mise run build` still succeeds and note the size change in the PR.

## Related

- Group A.
