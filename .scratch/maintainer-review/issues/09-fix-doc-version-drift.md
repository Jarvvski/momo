# 09: Fix stale versions in the docs

Type: task
Status: ready-for-agent
Severity: low
Area: docs
Verified: yes — `README.md:25` pins `mise use github:Jarvvski/momo@0.2.1` and `docs/ci.md:242` pins `v0.4.0`; the crate is at 0.5.0.

## Symptom

Two install snippets pin versions several releases behind, so a reader following them installs an old momo.

## Suggested direction

Prefer version-agnostic snippets (`mise use github:Jarvvski/momo`; `releases/latest/download/...`) so they cannot drift again. If a pinned version is deliberate for reproducibility, add a note that it is an example and update both to the current version.

## Related

- Group A.
