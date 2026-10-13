# Repo & module shape

Type: grilling
Status: resolved

## Question

How is momo structured as a Rust project?

`Cargo.toml` already anticipates a `crates/*` workspace, with the root package
as the sole member "for now". Settle:

- Single crate vs workspace split (e.g. `momo-core` + `momo-cli`?).
- Module boundaries — domain (changes/semver), vcs (git/jj), changelog,
  taskenv, cli.
- Where the `TaskEnvironment` trait lives.
- Where the binary lives.

Weigh against ticket 14's dependency findings.

## Answer

Settled:

1. **Workspace split (option b).** A virtual-workspace root; code moves out of the
   current root package into `crates/*`. Rationale: faster mid-term incremental
   compilation, and cleaner navigation for future agents.
2. **Crate layout:**
   - `momo-core` — domain (Kind/Change, bump policy, version, changelog, config)
     **and the ports** (`Vcs`, `TaskEnvironment` traits). Pure: no I/O, no `clap`.
   - `momo-vcs` — adapters: git + jj (impl `Vcs`).
   - `momo-taskenv` — adapters: mise (impl `TaskEnvironment`); gradle/bun later as
     new crates/impls with **no core change**.
   - `momo-cli` — the `momo` binary: clap tree + command handlers; wires the
     adapters.
3. **Ports in core, adapters in their own crates**, wired by `momo-cli`
   (hexagonal). The root becomes a **virtual workspace manifest** (no root package).
4. **`Vcs` is a trait** (mirroring `TaskEnvironment`), with git/jj impls and a fake
   for tests.
