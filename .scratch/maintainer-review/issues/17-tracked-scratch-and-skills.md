# 17: Decide whether `.scratch/` and the agent skill dirs stay tracked

Type: task
Status: ready-for-agent
Severity: low
Area: repo hygiene

## Question

The repo tracks `.scratch/` (96 files of internal tickets, triage and owner rulings) and ~60 files under `.claude/skills/` and `.agents/skills/`. All are excluded from the published crate, so they do not affect users, but they bloat a fresh clone and make the history noisy for outside contributors.

## Options

- **(a) Keep tracking** `.scratch/` (it is the documented issue tracker) and the skills; accept the noise.
- **(b) Stop tracking `.scratch/`** — add it to `.gitignore` and keep it local. Issues become unshared.
- **(c) Keep `.scratch/` but stop tracking the vendored skills** — they are reproducible from `skills-lock.json`.
- **(d) Keep the issue files but untrack the large `.scratch/*/research` and `prototypes` payloads.**

## Recommendation

Owner's call. **(c)** is the least-loss option if the goal is a leaner clone: the skills are a lockfile artefact, the tickets are project memory.

## Decision (2026-10-14)

**(d) Keep the issue files, untrack the research/prototypes payloads.** Keep `.scratch/*/issues/**` and the map tracked. Stop tracking `.scratch/*/research/**` and `.scratch/*/prototypes/**` and the vendored skills stay tracked for now.

Implement non-destructively: add the patterns to `.gitignore` and use `jj file untrack <path>` (jj, not git) so the files remain on disk. Do not delete them.

## Related

- —
