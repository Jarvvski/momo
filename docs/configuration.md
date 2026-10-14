# Configuration

momo reads `momo.toml` from the repo root. It is **optional**: write commands create it lazily on first use, and read-only commands (`status`, `--dry-run`) never create it. `momo init` writes it up front and refuses to overwrite an existing file.

`momo init` writes exactly this:

```toml
# momo configuration.
# Precedence: MOMO_* environment variables > this file > built-in defaults.
recording = "changelog"
changelog = "CHANGELOG.md"
changes_dir = ".momo/changes"
tag_prefix = "v"
```

`momo init --version <x.y.z>` additionally records a start version in `.momo/version`, so the first release bumps from there instead of `0.0.0`.

## Keys

**momo is not accepting new keys.** The recognised set is deliberately closed: a key is a permanent compatibility promise, and one is added only when reality genuinely varies or a safety valve is needed. [The option philosophy](option-philosophy.md) states the rule and what a request has to show before it is considered.

| Key | Default | Meaning |
|---|---|---|
| `recording` | `"changelog"` | `"changelog"` or `"fragments"` |
| `changelog` | `"CHANGELOG.md"` | Changelog path, relative to the package root |
| `tag_prefix` | `"v"` | Prefix on the tag version |
| `remote` | *(unset)* | Override the changelog footer's remote; supplies one with no VCS |
| `command_timeout` | `60` | Seconds a spawned `git`/`jj`/`mise` command may run before it is killed |

`remote` and `command_timeout` are optional; the rest have defaults that suit most projects. Three further keys are scoped, managed or deprecated rather than general knobs — see below.

`changelog`, `changes_dir` and a package's `manifest` are **paths**. They must be relative and must not escape upward with `..`: an absolute path would discard the directory they are joined onto, and `..` would write outside the tree, so both are rejected at parse time. A key that momo does not recognise is an error too, so a misspelled key fails loudly instead of silently taking its default.

The changelog's release heading is stamped with the **UTC** date (`## [0.1.0] - 2026-10-13`), so a release is reproducible across machines and CI runners regardless of the local timezone. Every package in one run shares the date: it is read once, before the per-package loop.

`remote` overrides the remote derived from the VCS and supplies one when there is no VCS; with neither, the changelog omits its reference-link footer. A value that is set but is not a recognisable remote URL is an error, not a silent omission:

```console
$ momo release
error: reading /repo/momo.toml: invalid value `not a remote` for `remote`; expected a remote URL
```

`command_timeout` bounds every external command momo spawns (`git`, `jj` and `mise`). A child that outlives it is killed and the command fails with a timeout error, so a hung process — a held jj workspace lock, for instance — cannot stall an unattended release job. It is a whole number of seconds and must be positive; raise it for a very large repository where a read legitimately takes longer:

```toml
command_timeout = 300
```

### Scoped keys

`changes_dir` (`".momo/changes"`) sets the Fragment store, relative to the repo root. It is meaningful only when `recording = "fragments"`: the default `changelog` model writes no fragments, so the key does nothing there. Set it alongside the model that uses it:

```toml
recording = "fragments"
changes_dir = ".changes"
```

Like the other path keys it must be relative and must not contain `..`.

### Managed keys

`task_env` records the task environment `momo install` installed into, and `install --uninstall` clears it. Do not set it by hand: it is a record of what install did, not a knob.

### Deprecated keys

`rc_format` (`"-rc.{n}"`) is **deprecated**. It templates the release-candidate suffix, a fixed concept the `-rc.N` default already covers. momo still parses and honours it, so an existing `momo.toml` keeps working, but it is no longer a supported knob and is scheduled for removal at **1.0**. Leave it unset. A value set here is not warned about at runtime, by design — see [the option philosophy](option-philosophy.md).

## Per-package overrides

In a monorepo, overrides are keyed by ecosystem identity:

```toml
[packages."cargo:momo"]
identity = "cargo:renamed"   # override the identity
slug = "alpha"               # override the ref/path-safe slug
manifest = "Cargo.toml"      # override the discovered manifest, relative to the package directory
```

`manifest` is resolved **relative to the package's directory**, not the repo root. For a package rooted at `crates/momo`, `Cargo.toml` resolves to `crates/momo/Cargo.toml`; a repo-relative value like `crates/momo/Cargo.toml` would resolve to `crates/momo/crates/momo/Cargo.toml` and silently fail to find the manifest. Like the other path keys it must be relative and must not contain `..`.

`slug` is derived from the identity unless set explicitly, and it namespaces the package's tags and Fragment store. An `identity` override with no `slug` recomputes the slug from the new identity, so renaming a package moves its tag namespace and Fragment store to the new name; pin `slug` explicitly to keep the old ones. Two packages that derive the same slug are an error — give one a distinct `slug`.

## Precedence

Configuration is layered, lowest to highest:

**`MOMO_*` environment variables > `momo.toml` > built-in defaults.**

Each layer sets only the keys it actually sets, so a sparse file leaves everything else at its default.

## Environment variables

`MOMO_RECORDING`, `MOMO_CHANGELOG`, `MOMO_CHANGES_DIR`, `MOMO_TAG_PREFIX`, `MOMO_REMOTE`, `MOMO_TASK_ENV`, `MOMO_RC_FORMAT`, `MOMO_COMMAND_TIMEOUT`.

An empty value is treated as unset, so `MOMO_CHANGES_DIR=` does not redirect the Fragment store. Unrelated `MOMO_*` variables are ignored.

```console
$ MOMO_TAG_PREFIX="rel-" momo status
```
