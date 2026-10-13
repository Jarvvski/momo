# 11: Unreleased heading matching is exact and duplicates are mishandled

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/changelog
Verified: yes — all three behaviours read in the source

## Symptom

`is_unreleased` matches only the exact spellings `[Unreleased]` and `Unreleased`,
case-sensitively. A `## [unreleased]` section is not recognised, so `record`
prepends a *second* Unreleased section; the lowercase one is then carried in
`released` and its entries are never counted or promoted. Two `## [Unreleased]`
headings behave the same way, and an Unreleased section placed *below* a released
section makes `released_versions()` miss the releases above it.

## Evidence

`src/domain/changelog.rs:204-209`:

```rust
fn is_unreleased(line: &str) -> bool {
    matches!(
        line.strip_prefix("## ").map(str::trim),
        Some("[Unreleased]") | Some("Unreleased")
    )
}
```

`src/domain/changelog.rs:69-73` stops the Unreleased body at the *next* `## `
heading, so a second Unreleased heading truncates the first body.

## Trigger

A changelog containing `## [unreleased]` and a bullet. Run `momo add fix "x"`.
The file ends up with two Unreleased sections and the original bullet is orphaned
forever.

## Suggested direction

Match case-insensitively, and when more than one Unreleased heading exists either
merge the bodies or reject the file with a clear error rather than silently
splitting it.

## Open questions for triage

- Merge or reject on duplicates? Rejecting is safer for a hand-edited file but
  worse for a user who did it deliberately.

## Triage

**Verdict:** `ready-for-agent` (narrow scope) — real, verified. Case-insensitive
matching is a one-line, self-contained fix worth landing now. The
duplicate/out-of-order handling is a parse-model problem and is **subsumed by
38**.

**Corrections:**
- Out-of-order is worse than stated: with `## [1.0.0]` above `## [Unreleased]`,
  `parse` puts the released section into `preamble` (`changelog.rs:76`), so
  `released_versions()` misses it **and** a later `release` is inserted *below*
  the `1.0.0` section (wrong order), because `write_section` prepends to
  `self.released`, which starts after the Unreleased body.
- Duplicates: a second `## [Unreleased]` truncates the first body at the next
  `## ` line (`68-73`) and is then carried in `released` verbatim, so `record`
  produces two Unreleased sections — confirmed.
- `is_section` matches only `## ` (level 2), so `#`/`###` headings are not
  boundaries. Fine as-is; worth noting.

**Scope:** Split — the case-insensitivity fix ships here; duplicate and
out-of-order handling fold into 38. Ruled policy for 38: match case-insensitively
and, on more than one Unreleased heading, **fail loudly** rather than silently
split (safer for a hand-edited file).

**Implementation sketch:**
- `src/domain/changelog.rs`: change `is_unreleased` to compare
  case-insensitively, e.g.
  `matches!(line.strip_prefix("## ").map(str::trim), Some(h) if h.eq_ignore_ascii_case("[Unreleased]") || h.eq_ignore_ascii_case("Unreleased"))`.
- Test: `## [unreleased]` with a bullet parses as Unreleased, and a `record`
  lands in that section without spawning a second heading.
- Leave duplicate/out-of-order to 38.
