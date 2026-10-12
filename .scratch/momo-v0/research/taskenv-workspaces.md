# Task-runner workspace / monorepo models (ticket 19)

Type: research
Status: resolved

Scope: how **mise**, **Gradle**, and the **Node.js package managers**
(npm / yarn / pnpm / bun) model workspaces / monorepos — the exact files and
fields that declare the module graph, and the exact interface a versioning tool
(momo) should read to enumerate packages. Every claim is traced to the owning
primary source (official docs or the tool itself); claims marked *verified* were
confirmed by running the tool locally.

Local toolchain used for the *verified* claims: `mise 2026.10.8 macos-arm64`
(read-only probes in a scratch directory outside this repo; **no VCS commands
were run against this repo**).

---

## TL;DR

| Ecosystem | Declares membership in | Exact field(s) / syntax | Canonical machine-readable enumeration |
|---|---|---|---|
| **mise** | root `mise.toml` | `monorepo_root = true` + `[monorepo].config_roots = [...]` | **`mise tasks graph --json`** (project graph) and **`mise tasks --all --json`** (tasks, with `//path:task` names) |
| **Gradle** | `settings.gradle(.kts)` | `include("a", "services:api")`; composite: `includeBuild("../lib")` | **Tooling API `GradleBuild.getProjects()`** (Java); CLI `gradle projects` is text-only; `--task-graph` (9.1.0+) prints tasks |
| **npm / yarn / bun** | root `package.json` | `"workspaces": ["packages/*"]` (yarn classic object form: `workspaces.packages`) | read root manifest + globs, then each package's `package.json` `name`/`version` |
| **pnpm** | `pnpm-workspace.yaml` | `packages: ["packages/*"]` | same as above; **`pnpm-workspace.yaml` wins over `package.json`** when both exist |

**The seam momo should use:** if the task environment is mise, read
`mise tasks graph --json` — it is a documented, stable, tool-independent JSON
project graph that already unifies Cargo / uv / Go / Node workspaces and needs
none of the package managers installed.

---

## 1. mise

mise has **first-class monorepo support** (not just task running), in three
layers: (a) config-root addressing, (b) a project graph inferred from ecosystem
manifests, (c) affected-task selection. Sources:
<https://mise.jdx.dev/tasks/monorepo.html>,
<https://mise.jdx.dev/tasks/workspace-graph.html>.

### 1.1 Declaring a monorepo

The repo root's `mise.toml` sets `monorepo_root = true` and lists project
directories under `[monorepo]`:

```toml
monorepo_root = true
[monorepo]
config_roots = ["packages/*", "services/*", "apps/*"]
```

- A **config root** is "the directory that owns a mise config file"
  (`mise.toml`, `.mise/config.toml`, or a `mise-tasks/`/`.mise/tasks/` dir).
- `config_roots` entries are **paths relative to the monorepo root, or a
  single-level glob** (`services/*`). Recursive `**` globs, absolute paths, and
  `..` are ignored with a warning. A glob match with no config file is skipped
  silently; an explicit path with no config prints a warning.
- **Automatic filesystem discovery is deprecated** — without `config_roots`,
  mise walks the tree and warns. `mise install --monorepo` and
  `mise ls --monorepo` **fail** without `config_roots`.
- Other `[monorepo]` keys: `path_aliases`, `lockfile`, `task_defaults`,
  `projects` (graph overrides).

### 1.2 Addressing and running tasks by path

- `//projects/frontend:build` — absolute from the monorepo root.
- `:build` — nearest enclosing config root; bare `build` behaves like `:build`.
- Wildcards: `//...:test`, `//projects/...:build`, `//projects/frontend:**`.
- Tasks receive `MISE_MONOREPO_ROOT` and `MISE_PROJECT_ROOT`.
- Tool/env/vars layering: root → intermediate configs → the config root's own
  config → the task's own `tools`/`env`, later winning.

### 1.3 Reading "what projects exist" — the canonical API

**`mise tasks graph`** is the documented enumeration command
(<https://mise.jdx.dev/tasks/workspace-graph.html>):

> `mise tasks graph` — each project's ID, root, dependencies, and metadata
> `mise tasks graph --explain` — which provider and file contributed each
> project, edge, and task field
> `mise tasks graph --json` — the same information as JSON

The graph is **separate from config roots**: it comes from ecosystem manifests,
and *"a project does not need its own `mise.toml` to appear in it."* It requires
`monorepo_root = true` (verified: without it, `mise tasks graph --json` errors
`no config file in scope sets monorepo_root = true`).

**Verified `--json` schema** (probed with a two-crate Cargo workspace and a
two-package npm workspace):

```json
{
  "projects": [
    {
      "id": "cargo:b",
      "root": "crates/b",
      "metadata": { "workspace_source": "Cargo.toml" },
      "dependencies": ["cargo:a"],
      "provenance": { "provider": "cargo", "source": "crates/b/Cargo.toml" },
      "dependency_provenance": {
        "cargo:a": { "provider": "cargo", "source": "crates/b/Cargo.toml" }
      },
      "tasks": {}
    }
  ]
}
```

So each project exposes: **`id`** (namespaced by provider), **`root`** (relative
dir), **`metadata`**, **`dependencies`** (edges), and **`provenance`** /
**`dependency_provenance`** (provider + source file). The `--json` output is
explicitly framed as a consumer contract: *"Task suggestions carry field-level
provenance, so other tools can tell … a `turbo.json` output declaration from a
root task default."*

**Providers** (docs table) — note the ID scheme and edge sources:

| Provider | Detected from | Project ID | Dependency edges from |
|---|---|---|---|
| Cargo | `[workspace]` in root `Cargo.toml` | `cargo:<package.name>` | local `path` deps, incl. `workspace = true` |
| uv | `[tool.uv.workspace]` in root `pyproject.toml` | `uv:<normalized name>` | `[tool.uv.sources]` with `workspace = true`/`path` |
| Go | `use` directives in root `go.work` | `go:<module path>` | none (use overrides) |
| Node.js | `pnpm-workspace.yaml` **or** `workspaces` in root `package.json` | `node:<package name>` | `dependencies`, `devDependencies`, `optionalDependencies`, `peerDependencies` naming a workspace package |

Project IDs come from **ecosystem metadata, not directories** — moving a project
does not change its ID. *"Every provider parses manifests directly: `cargo`,
`uv`, Python, `go`, and the Node.js package managers do not need to be
installed to build the graph."*

**Overrides / corrections** (`[monorepo.projects]`, quoted IDs): fields
`remove`, `root`, `metadata`, `depends`, `depends_add`, `depends_remove`. A new
project needs a namespaced ID (`custom:docs`) and a `root`. The final graph must
have no cycles.

### 1.4 Other machine-readable mise surfaces (verified)

- **`mise tasks --all --json`** lists every monorepo task; each task carries
  `name` (e.g. `//apps/web:build`), `source`, `config_sources`, and `dir` — i.e.
  you can map a task back to its project root. (`mise tasks` with no `--all`
  shows only the current config root + ancestors.)
- **`mise tasks ls --json`** / **`mise tasks info <name> --json`** — same object;
  `info` adds `cache`, `usage_spec`, etc.
- **`mise run --affected --affected-json <task>`** prints the affected selection
  as JSON: an object with the `base` and `head` revisions, the affected
  `projects` (with roots and reasons), and matched `tasks` (with project IDs).
  `--affected` requires Git and a monorepo root; reasons are one of
  `changed path`, `workspace-global path`, `lockfile change`,
  `depends on affected project`.
- **`mise config ls --json`** lists loaded config files, lowest precedence first;
  `--tracked-configs` lists every config file in any project.
- **`mise ls --monorepo`** / **`mise install --monorepo`** operate on the union
  of tools across config roots.

### 1.5 mise config hierarchy (for per-directory resolution)

From <https://mise.jdx.dev/configuration.html>: mise merges (1) system
`/etc/mise`, (2) global `~/.config/mise`, (3) config files in **every directory
from the filesystem root down to the current directory**, deeper overriding
shallower. Within a directory, precedence (high→low): `mise.local.toml`,
`mise.toml`, `mise/config.toml`, `mise/conf.d/*.toml`, `.mise/config.toml`,
`.mise/conf.d/*.toml`, `.config/mise.toml`, `.config/mise/config.toml`,
`.config/mise/conf.d/*.toml`, `.tool-versions`. `[tasks]` merge **per task
name**; `[tools]`/`[env]`/`[vars]` merge per key, nearest wins.

**Relevant to a versioning tool:** mise's monorepo model gives you project IDs,
roots, and the dependency graph for free; it does **not** carry a `version`
field per project — versions still live in each ecosystem's manifest
(`Cargo.toml` `[package].version`, `package.json` `version`, Gradle `version`).

---

## 2. Gradle multi-project builds

Source: <https://docs.gradle.org/current/userguide/multi_project_builds.html>
(version 9.8.1).

### 2.1 Declaring the module graph

A multi-project build is **one root project + subprojects, all declared in a
single `settings.gradle(.kts)`**:

```kotlin
rootProject.name = "my-project"
include("app", "core", "util")
```

- `include()` takes **project paths**; by default a path maps to a relative
  directory — `services:api` → `./services/api`. **Colon (`:`) is the nesting
  separator**; `:shared` is a top-level subproject.
- **Project descriptors** can override the mapping:
  `project(":project-a").projectDir = file("custom/my-project-a")` and
  `project(":project-a").buildFileName = "project-a.gradle.kts"`.
- Since **Gradle 9.0.0**, a missing/read-only project directory **fails the
  build** (previously silent).
- **Composite builds** are a different axis: `includeBuild("my-utils")` in
  `settings.gradle(.kts)` (or `--include-build`) includes *whole builds*, not
  subprojects. Included builds are **isolated** (no shared repos/plugins/props)
  and are **not** subprojects — you cannot `project(":other-build:module")`.
  Source: <https://docs.gradle.org/current/userguide/composite_builds.html>.

### 2.2 Enumerating subprojects

There is **no built-in JSON project listing**. Options, best→worst:

1. **Tooling API (the real interface).** `GradleConnector.connect()` →
   `ProjectConnection` → the **`GradleBuild`** model exposes
   `getProjects(): DomainObjectSet<BasicGradleProject>`,
   `getRootProject()`, `getIncludedBuilds()`, `getEditableBuilds()`. This is the
   API IDEs/CI use and is version-independent.
   <https://docs.gradle.org/current/userguide/tooling_api.html>,
   <https://docs.gradle.org/current/javadoc/org/gradle/tooling/model/gradle/GradleBuild.html>.
   From a **phased build action**, query it in the `projectsLoaded` phase — it
   "runs after the settings have been evaluated, before any project is
   configured" and is "the place to query models that describe the build
   structure, such as `GradleBuild`." (Java dependency:
   `org.gradle:gradle-tooling-api`.)
2. **CLI text.** `gradle projects` "gives you a list of the subprojects of the
   selected project, displayed in a hierarchy." Text only — brittle to parse.
   `gradle tasks --all`, `gradle help --task <t>`, `gradle :properties`.
   <https://docs.gradle.org/current/userguide/command_line_interface.html#sec:command_line_project_reporting>
3. **`--task-graph`** (since Gradle 9.1.0) prints the **task** dependency graph
   with actions disabled — not a project listing.

**Caveat for a tool:** `settings.gradle(.kts)` is **executable code** (Kotlin or
Groovy DSL), not data. You cannot reliably enumerate subprojects by regex — the
graph only exists after settings evaluation. Either use the Tooling API (needs a
JVM + the Gradle Tooling API jar) or shell out to `gradle projects` and parse.
Gradle's own `version` lives in each `build.gradle(.kts)` (`version = "1.2.3"`)
or `gradle.properties` — per-project, not per-build.

---

## 3. Node.js workspaces (npm / yarn / pnpm / bun)

All four use the root `package.json` `name`/`version` for the root package and
each member's `package.json` `name`/`version` for its version. They differ only
in **where membership is declared**.

### npm

- **`"workspaces": ["packages/a", "packages/*"]`** in the root `package.json` —
  "an array of file patterns that describes locations within the local file
  system that the install client should look up to find each workspace."
  <https://docs.npmjs.com/cli/v10/configuring-npm/package-json#workspaces>
- Run across all: `npm run <script> --workspaces`; one: `--workspace=a`.
  <https://docs.npmjs.com/cli/v10/using-npm/workspaces>

### yarn (classic)

- Same `workspaces` array; **`private: true` is required** on the root.
- Also accepts the **object form** `"workspaces": { "packages": [...] }`
  (confirmed by mise's provider table, which lists "the Yarn Classic object form,
  `workspaces.packages`").
  <https://classic.yarnpkg.com/lang/en/docs/workspaces/>
- Yarn Berry uses `workspaces` in `package.json` too (and `nodeLinker` in
  `.yarnrc.yml` for layout).

### pnpm

- **`pnpm-workspace.yaml`** at the root, with a `packages:` list:
  ```yaml
  packages:
    - packages/*
  ```
  "A workspace must have a `pnpm-workspace.yaml` file in its root."
  <https://pnpm.io/workspaces>
- Since **v12.7.0**, if a repo has only a `package.json` `workspaces` field,
  `pnpm install` **creates** `pnpm-workspace.yaml` with the same patterns.
- **Precedence:** when both exist, **`pnpm-workspace.yaml` defines membership**
  and a divergent `workspaces` field only warns — "only `pnpm-workspace.yaml` is
  read." (pnpm docs; also mise's provider table.)
- Dep edges via `dependencies`/`optionalDependencies`/`devDependencies`;
  the `workspace:` protocol is opaque to membership
  (`workspace:*`, `workspace:^`, `workspace:../foo`, aliases).

### bun

- Same `"workspaces": ["packages/*"]` array in the root `package.json`; **full
  glob syntax including negative patterns** (`!packages/**/test/**`).
  <https://bun.com/docs/pm/workspaces>
- Also an **object form** with extra keys:
  `{ "workspaces": { "packages": [...], "selfContained": ["apps/desktop"] } }`
  (the `selfContained` entries are workspace paths or package names).
- Bun replaces `workspace:*` with the package's `package.json` version at
  publish time — i.e. the manifest version is the source of truth.

### What a versioning tool reads (Node)

1. Root `package.json` → `workspaces` (array, or object `packages`) — **unless**
   `pnpm-workspace.yaml` exists, in which case read its `packages:` list.
2. Expand the globs (positive/negative, `**`, braces like `packages/{web,api}`)
   skipping `.git` and `node_modules`.
3. For each member dir, read `package.json` `name` (required) and `version`.
4. Edges: dependency entries whose name exactly matches another member's `name`
   (all four dependency kinds; `workspace:*`/`catalog:`/ranges are opaque).

This is exactly the algorithm mise's Node provider implements, so reading
`mise tasks graph --json` short-circuits steps 1–4.

---

## 4. What momo should read, per environment

| Task environment | Read this | Gives you |
|---|---|---|
| **mise** (v0's only impl) | `mise tasks graph --json` | project `id`, `root`, `dependencies`, `metadata`, `provenance` — provider-agnostic across Cargo/uv/Go/Node, no package manager installed |
| mise (fallback) | `mise tasks --all --json` | per-task `name` (`//path:task`), `source`, `dir` → project root |
| Gradle | Tooling API `GradleBuild.getProjects()` (or parse `include()`/`gradle projects` as a fallback) | `BasicGradleProject` set; `settings.gradle(.kts)` is code, not data |
| npm/yarn/bun | root `package.json` `workspaces` (or `workspaces.packages`), then each member's `package.json` `name`/`version` | members + versions + edges |
| pnpm | `pnpm-workspace.yaml` `packages:` (authoritative over `package.json`) | members; versions still in each `package.json` |

Notes for the grilling ticket (16):

- mise's graph already answers "what packages exist" and "how do they depend on
  each other" — but **not** "what version is each at." momo must still read each
  ecosystem's manifest for the version, or keep versions in momo's own config.
- Project **IDs are ecosystem-namespaced** (`cargo:momo`, `node:@acme/ui`,
  `uv:my-package`), stable across directory moves — a good basis for per-package
  tags like `pkg-v1.2.3` (cf. release-plz `{{package}}-v{{version}}`).
- mise's graph requires `monorepo_root = true`; a repo without it needs either
  that opt-in or a momo-owned discovery fallback.

---

## Sources

- mise — Monorepo tasks: <https://mise.jdx.dev/tasks/monorepo.html>
- mise — Workspace project graph: <https://mise.jdx.dev/tasks/workspace-graph.html>
- mise — Tasks: <https://mise.jdx.dev/tasks/>
- mise — mise.toml / config hierarchy: <https://mise.jdx.dev/configuration.html>
- mise source (repo): <https://github.com/jdx/mise>
- Gradle — Multi-project builds: <https://docs.gradle.org/current/userguide/multi_project_builds.html>
- Gradle — Composite builds: <https://docs.gradle.org/current/userguide/composite_builds.html>
- Gradle — Command-line interface (project reporting, `--task-graph`): <https://docs.gradle.org/current/userguide/command_line_interface.html>
- Gradle — Tooling API: <https://docs.gradle.org/current/userguide/tooling_api.html>
- Gradle — `GradleBuild` model: <https://docs.gradle.org/current/javadoc/org/gradle/tooling/model/gradle/GradleBuild.html>
- npm — `workspaces` field: <https://docs.npmjs.com/cli/v10/configuring-npm/package-json#workspaces>
- npm — Using workspaces: <https://docs.npmjs.com/cli/v10/using-npm/workspaces>
- Yarn (classic) — Workspaces: <https://classic.yarnpkg.com/lang/en/docs/workspaces/>
- pnpm — Workspaces: <https://pnpm.io/workspaces>
- Bun — Workspaces: <https://bun.com/docs/pm/workspaces>

## Verification log

Probed with `mise 2026.10.8 macos-arm64` in scratch dirs under the system temp
directory (read-only w.r.t. this repo; no VCS commands run):

- `mise tasks graph --json` on a 2-crate Cargo workspace → project IDs
  `cargo:a` / `cargo:b`, `cargo:b` → depends on `cargo:a` (via
  `workspace = true` path dep), `metadata.workspace_source = "Cargo.toml"`,
  provenance sources `crates/{a,b}/Cargo.toml`.
- `mise tasks graph --json` on a 2-package npm workspace → IDs `node:@acme/ui`
  / `node:@acme/web`, edge from `@acme/web` → `@acme/ui`,
  `metadata.workspace_source = "package.json"`.
- `mise tasks graph --json` with no `monorepo_root` → error
  `no config file in scope sets monorepo_root = true`.
- `mise tasks --all --name-only` → `//apps/web:build`, `//packages/ui:build`;
  `mise tasks --all --json` objects carry `name`, `source`, `config_sources`,
  `dir`.
