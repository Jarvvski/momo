# Monorepo / workspace model

Type: grilling
Status: open
Blocked by: 19

## Question

momo is now **monorepo-aware** (a v0 addition). What is a "package" in momo, and
how does versioning work across a monorepo?

Settle:

- Does momo version each **package independently** (changesets-style), or keep
  **one version for the whole repo** while being aware of subprojects?
- How are packages **discovered**? Reuse the task environment's workspace model —
  `mise` (see ticket 19), gradle modules, npm/bun workspaces — or momo's own
  config?
- **Changelog:** one per package, or one repo-level changelog?
- **Tags:** per-package (`pkg-v1.2.3`, cf. release-plz `{{package}}-v{{version}}`)
  or repo-level `v1.2.3`?
- How do `break|add|fix` **target** a package (flags, cwd, config)?
- Does a `break` in one package ever bump its dependents (like changesets)?
