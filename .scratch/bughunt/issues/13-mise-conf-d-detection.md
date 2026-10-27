# 13: A mise config that lives only under `conf.d` is not detected

Type: bug
Status: ready-for-agent
Severity: low
Area: adapters/taskenv
Verified: yes — reproduced against real mise

## Symptom

A monorepo whose only mise config is a `mise/conf.d/*.toml` fragment is never detected, so discovery never runs and momo falls back to a single package.

## Evidence

`detect::find_root` (`src/adapters/taskenv/detect.rs:36-44`) looks only for the `CONFIG_FILES` names or a `.mise` directory. `monorepo::is_monorepo` (`src/adapters/taskenv/monorepo.rs:16`, `:35-38`) *does* read `conf.d`, and `src/adapters/taskenv/mod.rs:14-16` claims both "recognise exactly the configurations mise would".

## Trigger

A repo with `mise/conf.d/monorepo.toml` (`monorepo_root = true`), a workspace `Cargo.toml` and `crates/a`; mise emits `cargo:a`, but `momo status` reports one package named after the directory.

## Suggested direction

Make `find_root` recognise a `mise/conf.d` directory (or a `.toml` beneath it) the same way `is_monorepo` does, so detection and the monorepo check agree. Add a test alongside `detects_a_dot_mise_toml_file`.
