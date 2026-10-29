# momo bug hunt — second sweep, 2026-10-29

Type: map
Status: resolved

A second bug hunt over momo 0.7.1, covering the three corners the 2026-10-27 hunt
left un-swept: the output/JSON contract and diagnostic rendering, the Python
`pyproject.toml` writer, and the Go module-path and tag handling. Three read-only
investigators swept one corner each; every finding below was then re-read at its
cited lines by hand and reproduced where the tooling allowed — against the built
binary, real mise, `toml_edit` 0.25, and the Go Modules Reference. Two
investigator claims did not survive: the Go `--sync-ranges` finding is a docs
contradiction rather than a false report (mise does not emit Go dependency edges,
so the path is unreachable), and the diagnostic finding is a duplication in the
human line rather than a JSON-shape error.

## What is here

Eleven tickets under `issues/`, each carrying its `Status:`, `Severity:`, `Area:`
and a `Verified:` line saying how the finding was confirmed.

## Status roll-up

All eleven are fixed and landed, each on its own branch and PR, run by a solo
sub-agent in its own jj workspace.

| Ticket | Fix | PR |
|--------|-----|----|
| 01 | Honour the `--json` shorthand on the failure path | #138 |
| 02 | Stop repeating a source the error message already names | #146 |
| 03 | Stop corrupting multi-line strings when writing a CRLF manifest | #139 |
| 04 | Skip a `poetry-dynamic-versioning` project instead of refusing it | #140 |
| 05 | Render PEP 440 versions the way `packaging` normalises them | #147 |
| 06 | Read a PEP 508 requirement whose extras follow a space | #143 |
| 07 | Exclude the major-version subdirectory from a Go tag prefix | #141 |
| 08 | Treat `v0` and `v1` as one Go module path | #137 |
| 09 | Compare Go versions by precedence, ignoring build metadata | #142 |
| 10 | Correct the Go major-version warning and its `gopkg.in` false positive | #145 |
| 11 | Stop reporting a Go range rewrite the read-only adapter never writes | #144 |

## Grouping into PRs

One focused change per branch, so each ticket lands on its own.

| Wave | Tickets | Theme |
|------|---------|-------|
| 1 | 01, 03, 04, 07, 08 | The five high-severity: a broken error contract, manifest corruption, a blocked release, and two Go tag/path bugs |
| 2 | 02, 05, 06, 09, 10, 11 | The medium and low: diagnostic prose, PEP 440 normalisation, a PEP 508 parse, and three Go correctness gaps |

## Method

Three investigators, one per corner, each read-only and told to report only what
it could point to at an exact line and construct an input for. Findings were then
verified by hand: the output findings against the built binary (the `--json`
failure path and the duplicated diagnostic were both reproduced), the TOML
finding against `toml_edit` 0.25 in a scratch crate, the Python findings against
the PEP 440 grammar and the upstream `poetry-dynamic-versioning` README, and the
Go findings against the Go Modules Reference and real mise (the major-subdirectory
tag prefix was reproduced end to end: momo would tag `api/v2/v0.1.0` where Go
resolves `api/v0.1.0`).
