# 10: The `/vN` warning's `+incompatible` claim is wrong, and `gopkg.in` warns falsely

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/tag
Verified: yes — code path confirmed against the Go Modules Reference

## Symptom

Two problems in `go_major_warning`:

1. Its message claims `go get` "serves this one as `+incompatible`", but
   `+incompatible` is only used for a module with **no** go.mod. A `go:` module
   discovered by mise always has a go.mod, so tagging it v2+ without a `/vN`
   path makes the version unresolvable — not `+incompatible`.
2. A `gopkg.in/….vN` module path encodes its major as `.vN`, not `/vN`, so a
   valid v2+ module (`gopkg.in/yaml.v2`) still gets a warning.

## Evidence

`src/domain/tag.rs:85-99`: the suffix check is `module_path.ends_with(&suffix)`
with `suffix = "/vN"` (`:90-91`), and the message (`:94-98`) asserts
`+incompatible`. Go Modules Reference: *"A tag with major version v2 or higher
may belong to a module without a major version suffix if no go.mod file is
present"* — with a go.mod the tag does not resolve. The existing test
(`tag.rs:157-163`) only asserts the message *contains* `/v2` and `+incompatible`,
so it pins the wrong text.

## Fix direction

Reword the warning so it says the version will not resolve (drop the
`+incompatible` claim), and treat a `gopkg.in/` path's `.vN` suffix as satisfying
the major. Add tests for a plain v2 path (warns, without `+incompatible`) and for
`gopkg.in/yaml.v2` (silent).
