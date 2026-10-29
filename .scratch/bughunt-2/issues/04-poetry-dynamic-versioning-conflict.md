# 04: `poetry-dynamic-versioning` aborts the release as a static/dynamic conflict

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest/python
Verified: yes — code path confirmed; upstream behaviour confirmed against the plugin's README

## Symptom

A project that uses `poetry-dynamic-versioning` fails the release with a
static/dynamic conflict, because the plugin *requires* a static placeholder
version scalar — so momo sees a scalar and a dynamic backend and refuses, where
it should skip with `SkipReason::Dynamic`.

## Evidence

`src/adapters/manifest/python/mod.rs:257-271`: when `dynamic_backend(doc)` is
`Some` and `version_sites(doc)` is non-empty, `apply_version` returns
`Err(ManifestError::Conflict)`. `version_sites` (`:401`) sees
`[tool.poetry].version = "0.0.0"` as a scalar, and `dynamic_backend` (`:452`)
sees `[tool.poetry-dynamic-versioning]` — both are true for the plugin's
required layout. The plugin's own README: *"Poetry still requires the
`tool.poetry.version` field to be present in pyproject.toml, but you are
encouraged to use `version = "0.0.0"` as a standard placeholder."* The PEP 621
mode has the same shape (`[project].dynamic = ["version"]` plus a placeholder).

The module doc (`mod.rs:243`) and `docs/usage.md:68` say a dynamic version is
skipped, not refused. The existing test
(`each_dynamic_backend_is_skipped_with_its_name`, `mod.rs:630`) feeds each backend
without a version scalar, so it never exercises this.

## Fix direction

When the dynamic backend is `poetry-dynamic-versioning`, the static scalar is the
plugin's placeholder, so treat the manifest as `Skip(Dynamic("poetry-dynamic-versioning"))`
even when a scalar is present — mirroring how the plugin is meant to be
configured. Keep the conflict for a genuinely ambiguous mix (a dynamic backend
whose declaration does *not* require a scalar). Add tests for the classic and
PEP 621 layouts from the plugin's README.
