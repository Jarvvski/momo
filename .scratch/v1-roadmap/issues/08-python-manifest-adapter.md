# 08: Python `pyproject.toml` manifest adapter

Type: task
Status: resolved
Points: 13
Bucket: Ecosystem manifest adapters
Severity: medium
Area: adapters/manifest/python
Blocked by: 14

## Problem

A `uv:<project>` package gets no sync (`for_package` matches only `cargo`). The
version lives in `[project].version` (PEP 621) or `[tool.poetry].version` and must
match the tag for anything published to PyPI, where the wheel version is
`pyproject.toml.version`. For Python monorepos on uv and published libraries.

## Evidence

- `src/adapters/manifest/mod.rs:20-25` (`for_package` Cargo-only).
- mise provider id (verified): Python's prefix is **`uv:`**, not `python:`/`pip:`.
- `src/adapters/manifest/cargo.rs:180-201` (`preserve_line_endings`, `uses_crlf` —
  reusable).

## Proposal

Reuse the plan-shaped port from ticket 14 and reuse `toml_edit`.

- **Extract the shared TOML machinery** from `cargo.rs` into
  `src/adapters/manifest/toml.rs` (`read`, `preserve_line_endings`, `uses_crlf`, a
  generic decor-preserving `set_string`). Cargo and Python then own only key
  resolution — the refactor pays for itself.
- **Which key:** `[project].version` → `[tool.poetry].version`. If **both** exist
  (Poetry 2 migration), rewrite **both** — one document, two scalar edits, never
  leave one stale.
- **Detect and SKIP dynamic versions — the key correctness point.**
  `[project].dynamic = ["version"]`, `[tool.setuptools_scm]`,
  `[tool.setuptools.dynamic].version`, `[tool.hatch.version]`,
  `[tool.poetry-dynamic-versioning]` all mean **the tag *is* the version**; writing
  a scalar would break the setup:
  - dynamic && no static scalar → `Skip(Dynamic(backend))`, visibly.
  - dynamic && a static scalar present → `ManifestError::Conflict`.
  - neither → `Skip(NoVersion)`.
- **PEP 440 ↔ semver, one direction only.** momo reads versions from tags, never
  from `pyproject.toml`, so the adapter needs only semver→PEP 440 for the written
  value: `1.2.0-rc.1` → `1.2.0rc1`, `-beta.2` → `b2`, `-alpha` → `a0`. An
  unrepresentable pre-release **errors**, never passes through silently. Document
  the consequence: the *tag* keeps semver (`v1.2.0-rc.1`) while the manifest holds
  `1.2.0rc1`.
- **`setup.cfg` (INI) and `setup.py` (code): out of scope.** A `setup.cfg`-only
  project stays a silent skip. The plan-shaped port already accommodates a second
  write if it is demanded later.

## Surface

None new. `uv:` selects the adapter; add Python to the single-package probe chain.
`rc_format` stays semver-shaped (the adapter translates); note it in
`docs/configuration.md`.

## Implementation sketch

- `src/adapters/manifest/toml.rs` — extracted from `cargo.rs`.
- `src/adapters/manifest/python/mod.rs` — `PythonManifest`; `version_sites(doc) ->
  Vec<Site>`, `dynamic_backend(doc) -> Option<DynamicBackend>`; the
  skip/conflict/rewrite decision.
- `src/adapters/manifest/python/pep440.rs` — `render(&Version) -> Result<String,
  Pep440Error>`.
- `src/adapters/manifest/python/roundtrip.rs`.
- Tests: `[project].version` with comments/CRLF; `dynamic = ["version"]` → skip;
  `setuptools-scm`/`hatch-vcs` → skip; `[tool.poetry].version`; both sites rewritten
  together; `rc`→`rcN`, `beta`→`b`; unrepresentable → error; none → skip;
  dynamic+static → conflict.

## Scoring

Points **13** = Value 4×2 + Ease 4 + Novelty 1. Real value for the uv audience, low
risk (TOML machinery exists), not novel.

## Risks

- Dynamic-detection false negatives are **safe** (no scalar found → `Skip`).
- Poetry `[project]`/`[tool.poetry]` divergence → handled by writing both.
- `[tool.setuptools.dynamic].version = {file = "VERSION"}` → momo skips (correct).
- `toml_edit` line-ending normalisation → reuse `preserve_line_endings`.

## Related

- 14 (port plan, prerequisite), 04 (Node, sibling), 15 (Go).
