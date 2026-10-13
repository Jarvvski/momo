# Repo & module shape

Type: grilling
Status: claimed

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
