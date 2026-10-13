# Configuration

momo reads `momo.toml` from the repo root. It is **optional**: write commands
create it lazily on first use, and read-only commands (`status`, `--dry-run`)
never create it. `momo init` writes it up front and refuses to overwrite an
existing file.

`momo init` writes exactly this:

```toml
# momo configuration.
# Precedence: MOMO_* environment variables > this file > built-in defaults.
recording = "changelog"
changelog = "CHANGELOG.md"
changes_dir = ".momo/changes"
tag_prefix = "v"
tag_kind = "auto"
```

`momo init --version <x.y.z>` additionally records a start version in
`.momo/version`, so the first release bumps from there instead of `0.0.0`.

## Keys

| Key | Default | Meaning |
|---|---|---|
| `recording` | `"changelog"` | `"changelog"` or `"fragments"` |
| `changelog` | `"CHANGELOG.md"` | Changelog path, relative to the package root |
| `changes_dir` | `".momo/changes"` | Fragment store, relative to the repo root |
| `tag_prefix` | `"v"` | Prefix on the tag version |
| `tag_kind` | `"auto"` | `"auto"`, `"lightweight"`, or `"annotated"` |
| `remote` | *(unset)* | Override the changelog footer's remote; supplies one with no VCS |
| `task_env` | *(unset)* | Recorded by `momo install` |
| `rc_format` | `"-rc.{n}"` | Release-candidate suffix template; `{n}` is the number |

`task_env` is written by `momo install`. `remote` and `rc_format` are optional.

`remote` overrides the remote derived from the VCS and supplies one when there is
no VCS; with neither, the changelog omits its reference-link footer. A value that
is set but is not a recognisable remote URL is an error, not a silent omission:

```console
$ momo release
error: reading /repo/momo.toml: invalid value `not a remote` for `remote`; expected a remote URL
```

## Per-package overrides

In a monorepo, overrides are keyed by ecosystem identity:

```toml
[packages."cargo:momo"]
identity = "cargo:renamed"            # override the identity
slug = "alpha"                        # override the ref/path-safe slug
manifest = "crates/momo/Cargo.toml"   # override the discovered manifest
```

`slug` is derived from the identity unless set explicitly, and it namespaces
the package's tags and Fragment store. An `identity` override with no `slug`
recomputes the slug from the new identity, so renaming a package moves its tag
namespace and Fragment store to the new name; pin `slug` explicitly to keep the
old ones. Two packages that derive the same slug are an error — give one a
distinct `slug`.

## Precedence

Configuration is layered, lowest to highest:

**`MOMO_*` environment variables > `momo.toml` > built-in defaults.**

Each layer sets only the keys it actually sets, so a sparse file leaves
everything else at its default.

## Environment variables

`MOMO_RECORDING`, `MOMO_CHANGELOG`, `MOMO_CHANGES_DIR`, `MOMO_TAG_PREFIX`,
`MOMO_TAG_KIND`, `MOMO_REMOTE`, `MOMO_TASK_ENV`, `MOMO_RC_FORMAT`.

Unrelated `MOMO_*` variables are ignored.

```console
$ MOMO_TAG_PREFIX="rel-" momo status
```
