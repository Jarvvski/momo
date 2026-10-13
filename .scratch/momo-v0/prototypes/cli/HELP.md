# momo CLI — rough take (ticket 11)

Throwaway prototype. React to this; validated decisions move into `spec.md`.

## `momo --help`

```
momo — manage a project's versioning

Usage: momo [OPTIONS] <COMMAND>

Commands:
  add      Record an added change
  fix      Record a fixed change
  break    Record a breaking change
  status   Show unreleased changes and the version they would mint
  release  Mint a version from the unreleased changes
  rc       Cut or increment a release candidate
  tag      Create the VCS tag for the current version
  init     Scaffold momo.toml
  install  Install momo into a task environment
  help     Print this message or the help of the given subcommand(s)

Options:
      --dry-run   Show what would happen, change nothing
  -h, --help      Print help
  -V, --version   Print version
```

## Subcommand shapes

```
momo add <TEXT>
    Record an added change

    Arguments:
      <TEXT>  The change description

    Options:
      -p, --package <PKG>  Target a package (default: the one containing cwd)

momo status
    Show unreleased changes and the version they would mint

    Options:
      --format <FMT>    [default: human] [possible values: human, json]
      --version-only    Print only the computed version
      --check           Exit non-zero when there is nothing to release
      -p, --package <PKG>

momo release
    Mint a version from the unreleased changes (writes changelog + manifests)

    Options:
      -p, --package <PKG>
      --all                Every package with unreleased changes

momo rc
    Cut or increment a release candidate

momo tag
    Create the VCS tag for the current version

momo init
    Scaffold momo.toml

momo install [ENV]
    Install momo into a task environment (auto-detected when omitted)

    Options:
      --update     Refresh the installed tasks to this momo version
      --uninstall  Remove momo's tasks
```

## `momo status` (human, monorepo)

```
Unreleased

  cargo:momo
    Breaking
      - Drop the legacy --v0 flag (#142)
    Added
      - status --format json (#150)
    → next: 1.2.0

  cargo:momo-lsp
    Fixed
      - Handle missing config (#147)
    → next: 0.4.1
```

## Open points

1. `add|fix|break` argument shape: positional `<TEXT>` vs `-m`; one change per call.
2. `--dry-run` placement: global vs per-command.
3. Interactive prompting vs flags-only.
4. `momo rc` vs `momo release --rc` — roles.
5. Whether the `status` presentation above reads well.
