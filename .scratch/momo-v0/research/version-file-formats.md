# Where a project's version lives, and how to read/write it safely

Research for momo (a Rust CLI that detects and sets a project's version).

**Question.** For each common ecosystem, where does the version live in the
manifest, and how can a tool read and safely write it while preserving
comments/formatting?

**Method.** Primary sources only: official language/tool documentation and
first-party source. Every claim below cites the page that owns it. Where the
"safest write" is a library rather than a spec, the library's own API docs are
cited. No `git` was run; read-only inspection.

---

## Summary table

| Ecosystem | Canonical location | Exact key/path | Safest read/write | Format-preserving? |
|---|---|---|---|---|
| Rust | `Cargo.toml` | `package.version` (or `workspace.package.version`) | `toml_edit` | Yes (comments, spacing, key order) |
| Node | `package.json` | top-level `version` | minimal JSON text edit (`jsonc-parser`) | Yes if edited, no if reserialized |
| Java/Kotlin (Gradle) | `gradle.properties` or `build.gradle(.kts)` | project property `version` / `version = "…"` | line-oriented properties edit | Yes if line-oriented; no if canonicalized |
| Python | `pyproject.toml` | `project.version` (or `tool.poetry.version`) | `toml_edit` | Yes |
| Maven | `pom.xml` | `<project><version>` | XML text edit | Only with care (`versions:set` reformats) |
| .NET | `.csproj` / `Directory.Build.props` | `<Version>` / `<VersionPrefix>`+`<VersionSuffix>` | XML text edit | Only with care |
| Android | `build.gradle(.kts)` | `android.defaultConfig.versionName` | line edit | Yes if line-oriented |

**Cross-cutting rule:** the version is usually one of several *linked* values
(lockfiles, sibling packages, parent POMs). A correct tool reads the whole
relationship, edits the manifest in place, and then either updates or explicitly
flags the derived files. Do not reserialize a whole manifest to change one
scalar — that churns unrelated formatting and makes review diffs noisy.

---

## Rust — `Cargo.toml`

**Where it lives.** `[package] version`, formatted per SemVer ("major.minor.patch",
optional `-pre` and `+metadata`). The field is *optional* and defaults to
`0.0.0`; it was required before Rust 1.75. It is required to publish.
Source: <https://doc.rust-lang.org/cargo/reference/manifest.html#the-version-field>

**Workspace inheritance (the big pitfall).** In a workspace the version can live
at the root instead of in the member:

```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["bar"]

[workspace.package]
version = "1.2.3"
```

```toml
# [PROJECT_DIR]/bar/Cargo.toml
[package]
name = "bar"
version.workspace = true
```

- `version` is one of the keys inheritable via `{key}.workspace = true`; the
  full list is `authors, categories, description, documentation, edition,
  exclude, homepage, include, keywords, license, license-file, publish, readme,
  repository, rust-version, version`.
- Requires Rust 1.64+.
- A workspace may be a *virtual manifest* (`[workspace]` with no `[package]`),
  in which case there is no member version at all at the root.
- A member can override the auto-search with `package.workspace = "path"`.

Sources: <https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table>,
<https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace>

So a detector must resolve: if the member has `version.workspace = true` (or no
`version`), the authoritative value is `[workspace.package].version` in the
workspace root. Otherwise it is the member's own `[package].version`.

**Safe read/write.** Use [`toml_edit`](https://docs.rs/toml_edit). Its own docs
state it "allows you to parse and modify toml documents, while preserving
comments, spaces" and show round-tripping. Read/write through
`DocumentMut`: `doc["package"]["version"]`. Values created programmatically get
default formatting; to preserve a specific representation, parse the value
(`"'bar'".parse::<Item>()`).

- **Limitation:** `toml_edit` does not preserve the *order of dotted keys*
  (see its "Limitations" section). Everything else round-trips.
- Writing back with `doc.to_string()` keeps the file byte-for-byte except for
  the edited scalar.

**Pitfall — `Cargo.lock`.** `Cargo.lock` records exact dependency information and
is "maintained by Cargo and should not be manually edited"; it also contains an
entry for each local/workspace package at its version. Bumping `Cargo.toml`
without refreshing the lockfile leaves it stale until the next Cargo invocation.
Sources: <https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html>,
<https://doc.rust-lang.org/cargo/reference/workspaces.html>

**Prior art (Rust).** `cargo-edit`'s `cargo set-version` (uses `toml_edit`),
`release-plz`, `cargo-release`.

---

## Node — `package.json`

**Where it lives.** Top-level `"version"`. The name and version together form
the package identifier; the version "must be parseable by node-semver". Both are
optional if you do not publish.
Source: <https://docs.npmjs.com/cli/v10/configuring-npm/package-json#version>

**Workspaces / monorepo.** The `workspaces` field is an array of paths/globs;
each workspace is a nested package with its **own** `package.json` and its own
`version`. The root may be `"private": true` with no version at all.
Sources: <https://docs.npmjs.com/cli/v10/configuring-npm/package-json#workspaces>,
<https://docs.npmjs.com/cli/v10/using-npm/workspaces>

`npm version` operates on the current package; to bump workspaces use the
`workspace`/`workspaces` config (`--workspaces`, `--include-workspace-root`).
Source: <https://docs.npmjs.com/cli/v10/commands/npm-version#workspaces>

**Safe read/write.** JSON has no comments, so the only formatting that matters is
indentation, key order, and trailing newline. Two viable strategies:

1. **Minimal text edit (recommended).** Parse to locate the value's byte span,
   then splice the new string in. The Rust crate
   [`jsonc-parser`](https://docs.rs/jsonc-parser) (dprint) parses JSONC to a CST
   (`parse_to_ast`) and exposes edit/format operations; the JS package of the
   same name exposes `modify` + `applyEdits`. This preserves everything.
2. **Reserialize with detected indent.** This is what npm itself does:
   `@npmcli/package-json` writes with
   `JSON.stringify(content, null, format)` where `format` is the detected indent
   (default two spaces), after parsing with `json-parse-even-better-errors`
   (which preserves key order). It preserves indentation width and key order but
   normalizes other whitespace.
   Source: <https://github.com/npm/package-json/blob/main/lib/index.js>

**Pitfalls.**
- **Lockfiles must move together.** `npm version` writes "the new data back to
  `package.json`, `package-lock.json`, and, if present, `npm-shrinkwrap.json`".
  A tool that only edits `package.json` leaves the lock stale.
  Source: <https://docs.npmjs.com/cli/v10/commands/npm-version>
- **Internal dependency ranges.** A workspace that depends on a sibling as
  `"sib": "^1.0.0"` is not rewritten by a version bump; pnpm/yarn use the
  `workspace:*` protocol to avoid this. Treat sibling ranges as a separate,
  explicit concern.
- **Duplicate `version` keys** are invalid JSON semantics (last wins); do not
  try to "support" them — fail loudly.

**Prior art.** `npm version`, `npm pkg set version=…`, Changesets, Lerna.

---

## Java/Kotlin — Gradle (`gradle.properties`, `build.gradle(.kts)`)

**Where it lives.** The Gradle `Project` has a `version` property, and "the
version defaults to `unspecified`".
Source: <https://docs.gradle.org/current/dsl/org.gradle.api.Project.html>

It is conventionally set one of two ways:

1. **`gradle.properties`** — the docs list "Project properties" as one of the
   three kinds of properties that live in this file (alongside Gradle properties
   like `org.gradle.parallel` and system properties prefixed `systemProp.`), and
   the example includes a project property of the form `spring-boot.version =
   2.2.1.RELEASE`. A plain `version=1.2.3` line sets the project version.
   Source: <https://docs.gradle.org/current/userguide/build_environment.html#the-gradle-properties-file>
2. **The build script** — `version = "1.2.3"` (Groovy or Kotlin DSL) in
   `build.gradle(.kts)`, or `project.version = …`.

**Precedence pitfall.** Gradle resolves `gradle.properties` from three
locations, "with user-level properties taking precedence over project-level and
installation-level properties":
`GRADLE_USER_HOME` > project root > `GRADLE_HOME`. So a project-level `version`
can be shadowed by a user-level one — detection should say *where* the effective
value came from. Source:
<https://docs.gradle.org/current/userguide/build_environment.html#the-gradle-properties-file>

**Android.** `versionName` (the user-visible string, e.g. `"1.0"`) and
`versionCode` (a monotonically increasing positive integer) live in
`android.defaultConfig { … }` in the build script, not in
`AndroidManifest.xml` (which is generated). Source:
<https://developer.android.com/studio/publish/versioning>

**Safe read/write.**
- `gradle.properties` is the Java `java.util.Properties` format. The Rust crate
  [`java-properties`](https://docs.rs/java-properties) reads and writes it,
  following the `java.util.Properties` spec.
- **But** a canonical `Properties` rewrite does not preserve comments or
  ordering. Since `gradle.properties` routinely holds comments and unrelated
  Gradle/system keys, prefer a **line-oriented edit**: find the `version` key's
  line and replace only its value, leaving every other line untouched.
- `build.gradle(.kts)` is code, not a data format — there is no safe parser.
  Prefer reading/writing the property in `gradle.properties`; if the version is
  only in the build script, a targeted line edit (regex anchored to a `version`
  assignment) is the pragmatic option, and it should be reported as a heuristic.

**Pitfalls.**
- Don't confuse `gradle.properties` keys with project properties: `org.gradle.*`
  are Gradle settings, `systemProp.*` are JVM system properties.
- Gradle has no lockfile for *its own* project version; dependency
  verification/locking files (`gradle.lockfile`) are unrelated to the version.

**Prior art.** `axion-release-plugin`, `nebula-release`, Gradle's `version`
catalog conventions.

---

## Python — `pyproject.toml`

**Where it lives.** Under PEP 621, `[project] version` is a string, mapping to
the core-metadata `Version` field. `version` is one of the two keys that are
required *either* statically *or* listed as dynamic (the other being `name`,
which must be static). The `[project]` table itself is optional — its absence
means the build backend supplies all metadata.
Sources:
<https://packaging.python.org/en/latest/specifications/pyproject-toml/#version>,
<https://packaging.python.org/en/latest/specifications/pyproject-toml/#declaring-project-metadata-the-project-table>

**Dynamic / single-sourced version (the main pitfall).** A project may declare
`dynamic = ["version"]` and let the backend read it from elsewhere — e.g.
setuptools:
`dynamic = ["version"]` plus `[tool.setuptools.dynamic] version = {attr = "pkg.__version__"}`
(read from a module attribute or file). In that case `pyproject.toml` holds *no*
version and the real source is Python source or a `VERSION` file.
Source: <https://setuptools.pypa.io/en/latest/userguide/pyproject_config.html>

**Other backends.**
- Poetry: `[tool.poetry] version` (plus a `poetry version` command).
  Source: <https://python-poetry.org/docs/pyproject/#version>
- Hatch: version is configured under `[tool.hatch.version]`, which can read a
  file (`path = "…"`) or a regex source, and has a `hatch version` command.
  Source: <https://hatch.pypa.io/latest/version/>

**Safe read/write.** `toml_edit` (same as Cargo). Key path
`["project"]["version"]` or `["tool"]["poetry"]["version"]`.

**Pitfalls.**
- **`dynamic` wins.** If `version` is listed in `project.dynamic`, do not write
  a static `project.version` — that is an error per the spec (a key may not be
  both static and dynamic except for the extensible list/table keys, and
  `version` is not one of them). Detect and follow the indirection instead.
  Source: <https://packaging.python.org/en/latest/specifications/pyproject-toml/#dynamic>
- **Lockfiles** (`poetry.lock`, `uv.lock`, `pdm.lock`) record the project's own
  version and must be refreshed.
- Legacy `setup.cfg`/`setup.py` projects store `version` there instead.

**Prior art.** `poetry version`, `hatch version`, `bump-my-version`,
`python-semantic-release`, `setuptools_scm`.

---

## Maven — `pom.xml`

**Where it lives.** `<project><version>`. `groupId:artifactId:version` are the
coordinates; `version` (and `groupId`) may be inherited from `<parent>`, in
which case the child omits it.
Sources: <https://maven.apache.org/pom.html#Maven_Coordinates>,
<https://maven.apache.org/pom.html#Inheritance>

**CI-friendly versions.** Since Maven 3.5.0 you can use `${revision}`,
`${sha1}`, and `${changelist}` placeholders for the version, typically with a
`flatten-maven-plugin` to substitute the real value at build time — so the
literal version may not be a plain string in the file at all.
Source: <https://maven.apache.org/maven-ci-friendly.html>

**Safe read/write.** `pom.xml` is XML. There is no formatting-preserving XML
editor that is as mature as `toml_edit`. Options:
- Targeted text splice on the `<version>` element that is a direct child of
  `<project>` (distinguish it from `<parent><version>` and every dependency
  `<version>` — the duplicate-declaration pitfall).
- A streaming editor such as [`quick-xml`](https://docs.rs/quick-xml) lets you
  emit modified XML, but you own whitespace preservation.

Maven's own `versions:set` **rewrites the whole POM** and does not preserve
arbitrary formatting/comments, so it is not a formatting-safe model to copy.

**Pitfalls.** Multi-module reactors (root `<modules>`), the `<parent>` version
(which is often the thing being bumped), `${revision}` indirection, and
`<properties>`-based version constants. Maven has no lockfile for the project's
own version.

**Prior art.** `maven-versions-plugin` (`versions:set`), `maven-release-plugin`,
`flatten-maven-plugin`.

---

## .NET — `.csproj` / `Directory.Build.props`

**Where it lives.** In SDK-style projects, version is an MSBuild property. The
NuGet pack target documents the relationship:

| Property | nuspec field | Default | Note |
|---|---|---|---|
| `Version` | `version` | — | SemVer, e.g. `1.0.0`, `1.0.0-beta` |
| `PackageVersion` | `version` | `Version` | overrides `Version` for the package |
| `VersionPrefix` | `version` | empty | setting `PackageVersion` overwrites it |
| `VersionSuffix` | `version` | empty | setting `PackageVersion` overwrites it |

Source: <https://learn.microsoft.com/en-us/nuget/reference/msbuild-targets#pack-target>

**Central version (the pitfall).** Properties can be set in the project file, in
imported files, or in `Directory.Build.props`, and "it matters where you set a
property … in your project file, in *Directory.Build.props*, or in another
imported file". A repo commonly sets `<Version>` once in `Directory.Build.props`
and lets every `.csproj` inherit it. Detection must walk the import chain
(`DirectoryBuildPropsPath` can override the default search).
Sources:
<https://learn.microsoft.com/en-us/visualstudio/msbuild/customize-your-build?view=vs-2022>,
<https://learn.microsoft.com/en-us/visualstudio/msbuild/common-msbuild-project-properties?view=vs-2022>

**Safe read/write.** XML text edit on `<Version>`/`<VersionPrefix>`/
`<VersionSuffix>` in the chosen `<PropertyGroup>`; prefer editing
`Directory.Build.props` when that is the source of truth.

**Pitfalls.** `VersionPrefix`+`VersionSuffix` are concatenated (so "the version"
may be two properties); `GenerateAssemblyInfo` (default `true`) turns these into
assembly attributes; `packages.lock.json` exists when NuGet lock mode is on.

**Prior art.** `Nerdbank.GitVersioning`, `MinVer`, `dotnet-gitversion`.

---

## Other ecosystems worth knowing (lighter coverage)

- **Ruby** — `*.gemspec` (`spec.version`), often single-sourced from
  `lib/**/version.rb`. Code, not data.
- **PHP** — `composer.json` top-level `version` (often omitted for VCS-driven
  packages). JSON, same editing rules as `package.json`.
- **Dart/Flutter** — `pubspec.yaml` `version: 1.2.3+4` (the `+4` is the build
  number). YAML.
- **Elixir** — `mix.exs` `version: "1.2.3"` inside `project/0`. Code.
- **Go** — *no version in any file*; versions are VCS tags plus `go.mod` module
  paths. A version tool cannot "set" it in a manifest.
- **Python (legacy)** — `setup.cfg` `[metadata] version` or `setup.py`.

---

## Recommended Rust building blocks

| Format | Crate | Why |
|---|---|---|
| TOML (`Cargo.toml`, `pyproject.toml`) | [`toml_edit`](https://docs.rs/toml_edit) | Explicitly preserves comments, spaces, key order (except dotted-key order) |
| JSON (`package.json`, `composer.json`) | [`jsonc-parser`](https://docs.rs/jsonc-parser) | CST with comments/tokens; minimal edits |
| Java properties (`gradle.properties`) | [`java-properties`](https://docs.rs/java-properties) for parse; line edit for write | Round-trip parse; avoid canonical rewrite |
| XML (`pom.xml`, `.csproj`) | [`quick-xml`](https://docs.rs/quick-xml) | Streaming; you control the splice |

Fallback for any format: a **span-based text splice** — locate the scalar's byte
range and replace only those bytes. This is the most conservative approach and
preserves all surrounding formatting by construction.

---

## Pitfalls checklist (cross-cutting)

1. **Workspace/parent inheritance** — Rust `version.workspace = true` →
   `[workspace.package]`; Node workspace packages; Maven `<parent>`; .NET
   `Directory.Build.props`. Resolve to the true source before writing.
2. **Dynamic/indirect versions** — Python `dynamic = ["version"]`, Maven
   `${revision}`, .NET `VersionPrefix`+`VersionSuffix`. The literal may not be
   where it looks.
3. **Duplicate declarations** — every `<version>` in a POM is not *the*
   project version; a `version` string also appears in dependency tables.
4. **Lockfiles** — `Cargo.lock`, `package-lock.json`/`npm-shrinkwrap.json`,
   `poetry.lock`/`uv.lock`/`pdm.lock`, `packages.lock.json` may embed the
   project's own version and must be refreshed or explicitly reported.
5. **Formatting round-trip** — reserializing loses comments (TOML/XML/properties)
   and whitespace. Prefer `toml_edit` and span-based edits; never pretty-print a
   whole manifest to change one scalar.
6. **Precedence** — Gradle user-level `gradle.properties` can shadow the project
   one; MSBuild property ordering matters. Report the effective value *and* its
   source.
7. **SemVer vs. local rules** — Cargo/npm are SemVer; Maven's ordering is only
   SemVer-1.0.0-compatible and case-insensitive; Android needs a separate integer
   `versionCode`. Do not assume one bump strategy fits all.
