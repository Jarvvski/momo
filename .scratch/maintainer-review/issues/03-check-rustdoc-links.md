# 03: Check rustdoc links in the gate

Type: task
Status: ready-for-agent
Severity: low
Area: tooling / CI
Verified: yes — `mise run check` has no rustdoc step; the source carries intra-doc links (e.g. ADR 0003 cites `[ChangelogError](../../src/domain/changelog.rs)`).

## Symptom

Broken intra-doc links are never caught. `cargo clippy` does not check rustdoc lints, so a renamed type or a bad link path rots silently.

## Suggested direction

Add a rustdoc step to `check`: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`. Add a `[tasks.docs]` task and call it from `check`. Fix any broken links it finds.

## Related

- Group A.
