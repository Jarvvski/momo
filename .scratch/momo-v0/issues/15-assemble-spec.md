# Assemble the v0 spec

Type: task
Status: resolved
Blocked by: 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 16, 17, 18

## Question

Consolidate the resolved decisions into `.scratch/momo-v0/spec.md` — a single,
coherent v0 spec a builder can implement from, with no open questions. Ensure
`GLOSSARY.md` and any ADRs are consistent with it, and that the map's
Decisions-so-far is complete.

## Answer

Done. `.scratch/momo-v0/spec.md` now consolidates every resolved decision into a
single, coherent v0 spec (sections 1–16), with no open questions. `GLOSSARY.md` is
consistent with it (Kind / Fragment / Release / Release Candidate / Changelog /
Task Environment). Prototypes are retained under `prototypes/`; research under
`research/`.

The map's decision frontier is **clear**: every ticket is resolved. The only
remaining builder notes are captured in spec §16 — the exact failure messages, and
the documented `--check` exit-code contract.
