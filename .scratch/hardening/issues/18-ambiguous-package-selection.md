# 18: `-p <bare-name>` silently targets the first of several matches

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/workspace + domain/package
Verified: yes — read at the cited lines

## Symptom

`Package::matches` accepts a bare name (the identity after the provider
namespace), and selection uses `find`, which returns the first match. Two
packages can share a bare name, so `-p ui` can target the wrong package with no
warning.

## Evidence

`src/domain/workspace.rs:94`:

```rust
let found = self.packages.iter().find(|entry| entry.matches(query));
```

`src/domain/package.rs:153-160`:

```rust
pub fn matches(&self, query: &str) -> bool {
    query == self.identity
        || query == self.slug
        || self.identity.split_once(':').is_some_and(|(_, name)| name == query)
}
```

Zero matches is handled well (`no package matches \`x\``); "more than one" is not.

## Trigger

A workspace with `cargo:ui` and `node:ui`. `momo -p ui release` releases whichever
discovery listed first.

## Suggested direction

Collect all matches and bail with the candidate list (fully-qualified identities)
when there is more than one.

## Open questions for triage

- Should a bare name that is unambiguous still be allowed? (Yes, presumably — only
  the ambiguous case needs to error.)

## Triage

**Verdict:** Confirmed. `select` uses `.find(...)` (`workspace.rs:94`), and
`Package::matches` accepts a bare name (`package.rs:153-160`), so with
`cargo:ui` and `node:ui` in the workspace `-p ui` targets whichever discovery
listed first, silently. Zero matches is handled
(`SelectionError::Unknown`, `workspace.rs:97`); "more than one" is not.

**Corrections:** None material. The unambiguous bare name should keep working
(the ticket's own answer), and a slug match can in principle also be ambiguous —
though ticket 16 removes the realistic slug-collision case.

**Scope:** Standalone. Related to 16 (collisions) and 17 (slugs), but the fix is
in selection, not derivation. PR C groups 15+16+17+18.

**Implementation sketch:** Add a variant to `SelectionError` in
`src/domain/workspace.rs`:

```rust
#[error("`{query}` matches more than one package: {candidates}")]
Ambiguous { query: String, candidates: Vec<String> },
```

Replace the `find` with a collect and match on the slice:

```rust
let matches: Vec<_> = self.packages.iter().filter(|e| e.matches(query)).collect();
match matches.as_slice() {
    [] => Err(SelectionError::Unknown { query: query.to_owned() }),
    [only] => Ok(vec![*only]),
    many => Err(SelectionError::Ambiguous {
        query: query.to_owned(),
        candidates: many.iter().map(|p| p.identity().to_owned()).collect(),
    }),
}
```

Tests: `cargo:ui` + `node:ui` with `select(Some("ui"))` errors naming both
identities; `select(Some("cargo:ui"))` and `select(Some("ui"))` on a
single-match workspace still resolve.
