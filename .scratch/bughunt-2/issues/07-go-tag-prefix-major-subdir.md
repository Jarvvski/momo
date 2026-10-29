# 07: A Go module in a major-version subdirectory gets a tag prefix that includes `vN`

Type: bug
Status: resolved
Severity: high
Area: domain/tag
Verified: yes — reproduced end to end with real mise + momo

## Symptom

A Go module whose go.mod lives in a major-version subdirectory (`api/v2/`) is
tagged with the `vN` directory included in the prefix, producing a tag Go cannot
resolve. momo would tag `api/v2/v2.0.0`; Go resolves `api/v2.0.0`.

## Evidence

`src/domain/tag.rs:43-49` (`go_resolvable_prefix`) builds the prefix from
`package.relative_root()`, the physical directory. But Go's tag prefix is the
*module subdirectory*, which does not include the major-version suffix even when
the module is in a major-version subdirectory (Go Modules Reference, *Mapping
versions to commits* and the glossary). Reproduced with real mise + momo:

```
mise tasks graph: id go:example.com/acme/api/v2, root "api/v2"
$ momo release -p go:example.com/acme/api/v2 && momo tag --dry-run
would tag api/v2/v0.1.0        # Go expects api/v0.1.0
```

The existing prefix tests (`tag.rs:107-121`) only use `api` and the repo root, so
none catch it.

## Fix direction

Derive the prefix from the module subdirectory, not the raw directory: when the
module path (`go_module_path`, `tag.rs:72`) ends in `/vN`, drop a matching
trailing `vN` component from `relative_root` before building the prefix — so
`api/v2` → `api/`, and a module at the repo root in a `v2/` subdirectory → the
base alone. Add tests for `go:example.com/acme/api/v2` at `api/v2` and for a
root-level `v2` module.
