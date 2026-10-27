# 02: A workspace-inherited dependency rename is reported under the wrong package

Type: bug
Status: ready-for-agent
Severity: high
Area: adapters/manifest/cargo
Verified: yes — read at the cited lines; confirmed against Cargo's inheritance rules

## Symptom

A dependency renamed with `package = "…"` in `[workspace.dependencies]` and inherited by a member with `{ workspace = true }` is reported under the member's table key, not the real crate name. The cascade then filters it out and silently reports nothing for an edge that is genuinely unsatisfied.

## Evidence

`entry_package` (`src/adapters/manifest/cargo.rs:187-193`) reads `package = "…"` off the member's item. Cargo forbids any key other than `workspace`, `features` and `optional` on an inherited entry, so the member item is `{ workspace = true }` and the rename is not there. The real name lives on the workspace-root entry, which `inherited` (`cargo.rs:203-222`) reads for the requirement but never for the name.

The module doc at `cargo.rs:108-112` promises "A dependency renamed with `package = "…"` is reported under its real name".

## Trigger

```toml
# Cargo.toml
[workspace]
members = ["crates/b"]

[workspace.dependencies]
foo = { path = "crates/a", version = "^0.1.0", package = "acme" }
```

```toml
# crates/b/Cargo.toml
[dependencies]
foo = { workspace = true }
```

`cargo metadata` resolves the dependency as `acme`; momo reports `foo`. `cascade::read` (`src/cli/cascade.rs:190-199`) filters on `requirement.package == name`, where `name` is the released package's real name (`acme`), so the edge is dropped and `for_package` reports nothing.

## Suggested direction

Resolve the package name alongside the requirement in `inherited`, returning the root entry's `package` (falling back to the key). Thread it through `entry_requirement`/`read_dependencies` so both the direct and inherited paths agree. `inherited_sites` (`cargo.rs:236-255`) compares `entry_package` too, so it must use the same resolution or the rewrite will not fire. Add a test combining an inherited entry with a rename.
