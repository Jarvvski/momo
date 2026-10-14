# 39: Domain port errors carry process vocabulary

Type: refactor
Status: ready-for-agent
Severity: medium
Area: domain/vcs + domain/taskenv
Verified: yes — read at the cited lines

## Symptom

The `Vcs` and `TaskEnvironment` ports are declared as process-free seams, yet
their error enums embed a binary name, a space-joined argument string and trimmed
stderr. Any non-process adapter (an in-process VCS library, a fake, a future
non-CLI task runner) cannot produce these honestly and must fabricate them. This
is the adapter's reporting vocabulary living in the domain.

## Evidence

`src/domain/vcs.rs:77-96`:

```rust
Spawn { program: String, source: io::Error },
CommandFailed { program: String, args: String, stderr: String },
```

`src/domain/taskenv.rs:127-150` has the same shapes, plus a stringly-typed
`Graph(String)` catch-all that collapses "mise absent", "not a monorepo", "invalid
JSON" and "no projects array" into one un-matchable variant.

## Suggested direction

Keep a domain-level "operation failed" variant (optionally with an opaque source),
and let adapters wrap and format the process detail at the CLI boundary. Give
`TaskEnvError::Graph` structured variants so callers can react (degrade vs warn)
without string-matching.

## Open questions for triage

- How much of the process detail does the CLI actually need to show? The current
  messages ("`git tag` failed: …") are good; the question is where they are
  formatted, not whether they exist.

## Related

- 40 (the purity guard), 30 (process behaviour).

## Triage

**Verdict.** Confirmed at the cited lines (`src/domain/vcs.rs:77-96`,
`src/domain/taskenv.rs:127-150`): the port error enums are the domain's, yet they
carry process vocabulary (`program`, space-joined `args`, trimmed `stderr`). It is
a real seam leak, but it is a **refactor with no observable misbehaviour**, and the
fix reshapes the crate's public library surface (ADR 0002 makes `domain`/`adapters`
public API). The ticket's own open question — how much process detail the CLI must
show and where it is formatted — is unresolved, so this is not "fully specified".
**`ready-for-human`.**

**Corrections.**
- `TaskEnvError::Graph(String)` does *not* collapse "mise absent": absence is
  `Spawn` (`adapters/taskenv/graph.rs:28`). What it collapses is "graph not UTF-8",
  "invalid JSON", "no `projects` array", "a project has no string `id`/`root`", and
  the CLI's synthetic "listed no projects" (`cli/workspace.rs:103`) — all into one
  un-matchable string.
- Broader than reported: the domain also embeds `std::io::Error`
  (`vcs.rs:84`, `taskenv.rs:104/114/124/134`, `version.rs:184/202`,
  `manifest.rs:66/76`). That is I/O vocabulary in the domain and is *not* caught by
  the purity guard (ticket 40), because the guard does not list `std::io`.
- The in-tree fake (`src/cli/release.rs`) never produces
  `Spawn`/`CommandFailed`/`Command` — it returns `Ok` — so the "a fake must
  fabricate them" argument only bites a fake that needs to *report* a failure.

**Scope.** Keep separate; not a duplicate. Couples to **40** (once process/I/O
vocabulary leaves the domain, the guard can forbid `std::io`) and **30** (process
behaviour). Do not fold into a test PR.

**Implementation sketch** (needs the design decision first, hence human).
- Domain: keep the domain-meaningful variants (`NoRepository`, `TagAlreadyExists`,
  `Unknown`); collapse the process variants into one opaque
  `Operation { context: &'static str, source: Option<Box<dyn std::error::Error + Send + Sync>> }`;
  replace `TaskEnvError::Graph(String)` with structured variants (`GraphAbsent`,
  `NotMonorepo`, `Malformed { detail }`, `Empty`).
- Adapters: keep `program`/`args`/`stderr` in an adapter-owned error and format
  "`git tag` failed: …" at the CLI boundary.
- Blast radius: `domain/{vcs,taskenv,version}.rs`; `adapters/vcs/{process,git,jj,detect}.rs`;
  `adapters/taskenv/{graph,mise,detect}.rs`; tests in `adapters/vcs/roundtrip.rs`,
  `test_support.rs`, and the `TaskEnvError::Graph` assertions in
  `adapters/taskenv/graph.rs`.

## Decision (owner, 2026-10-14)

**Reshape the ports.** Keep a domain-level "operation failed" variant carrying an
opaque source, and let the adapters format the process detail (program, args,
stderr) at the CLI boundary. Give `TaskEnvError::Graph` structured variants too.
This changes public error types — update the docs and every test that matches on
the old shapes.
