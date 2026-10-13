# Monorepo / workspace model

Type: grilling
Status: resolved
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

## Answer

Settled (all recommendations accepted):

1. **Unit of versioning: the package.** A monorepo is many packages, each
   versioned independently (its own version, changelog, tag); a single-package
   repo is just a monorepo with one package. **No cascading** to dependents in v0.
2. **Discovery: the task environment's graph** — `mise tasks graph --json`
   (requires `monorepo_root = true`). momo's own manifest discovery is deferred.
3. **Changelog: one `CHANGELOG.md` per package.** Fragments live repo-level at
   `.momo/changes/<package>/<kind>-<slug>.md` (the package as a path segment).
4. **Package identity: the ecosystem ID** (`cargo:momo`, `node:@acme/ui`) with a
   derived ref/path-safe **slug** (`cargo-momo`); **overridable via config**
   (→ ticket 04). Targeting accepts the ID or a short name.
5. **Tags:** `<slug>-v<version>` per package; plain `v<version>` when the repo is
   a single package. RC: `<slug>-v<version>-rc.N`.
6. **Targeting:** cwd-relative default; `-p/--package` to override; `--all` for
   every package. Aggregate commands: `momo release` with no `-p` releases every
   package that has unreleased changes; `momo status` shows them all.

Feeds tickets 03 (per-package changelog), 04 (identity overrides, per-package
config), 06 (per-package tags), 09 (mise integration), 18 (per-package manifests).

Unblocks tickets 03, 06, and 18.
