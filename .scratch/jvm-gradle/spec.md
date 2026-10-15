# momo × JVM (Gradle & Maven) — spec

Status: investigation (findings + recommendation; not yet a buildable spec)

This is a read-only investigation into how momo could support JVM projects. It was
written against `main` at `63c847`. Every non-obvious claim is traced to the
source that owns it: official Gradle and Maven documentation, the Gradle/Maven
plugin portals, Maven Central's publishing requirements, the `jdx/mise` docs, and
momo's own code (`file:line`). Where a behaviour is version-gated or the sources
disagree, it says so and names the versions.

## 1. The honest answer up front

Momo's manifest model is a *format-preserving editor of a declarative version
scalar*: it locates a file, reads a scalar out of it, and writes a new scalar
back without touching anything else (`src/domain/manifest.rs:168-229`). It never
executes the ecosystem's tooling, and it reads dependency ranges statically.

Maven is a **plausible fit** for that model: `pom.xml` is declarative XML, its
`<version>` is a real field Maven reads, and multi-module version propagation is a
solved problem (the MojoHaus `versions-maven-plugin` exists precisely to do it —
<https://www.mojohaus.org/versions/versions-maven-plugin/index.html>).

Gradle is a **poor fit**, for two structural reasons:

1. **A Gradle project's version is a value computed by executing a build script,
   not a declarative field.** The version is `Project.version`, which defaults to
   `unspecified` and is whatever `build.gradle(.kts)` assigns — often from
   `git describe`, a property, or a plugin
   (<https://docs.gradle.org/current/dsl/org.gradle.api.Project.html>). momo
   cannot know statically which assignment wins without running the build, and
   momo deliberately does not run ecosystem tooling (`docs/usage.md:239`).
2. **Gradle's idiomatic intra-build dependency carries no version at all.** A
   module depends on a sibling with `implementation(project(":shared"))`, not a
   `group:artifact:version` coordinate
   (<https://docs.gradle.org/current/userguide/multi_project_builds.html>). There
   is no range to widen, so momo's cascade has almost nothing to act on.

On top of that, **mise has no JVM project provider** (section 4), so a JVM
multi-module build produces no projects and no edges in the graph momo reads —
momo degrades it to a single package.

The recommendation (section 7) is therefore staged and modest: a Maven
`pom.xml` adapter is worth building; a Gradle adapter is only worth building for
the narrow `gradle.properties` case, and should refuse whenever a build script
assigns the version; JVM monorepo discovery and JVM range widening should not be
attempted now.

## 2. Scope and method

In scope: where a JVM version lives; how a format-preserving editor would write
it; JVM version syntax and ordering; the mise identity and graph; the cascade;
prior art; a recommendation.

Out of scope: publishing to Maven Central or any repository (that is JReleaser's
lane, section 6), running Gradle or Maven, and Gradle/Maven task-environment
integration beyond what already exists (momo already lists "task environments
beyond mise (gradle, bun)" as unsupported — `docs/usage.md:238`).

## 3. Q1 — Where a JVM project's version actually lives

The distinction that matters to momo is **declarative data** (a scalar a
format-preserving editor can safely rewrite) versus **executable build script**
(procedural code momo must never parse or rewrite).

| Source | Kind | Holds the project version? |
|---|---|---|
| `gradle.properties` (`version=…`) | properties (declarative data) | Yes, as a Gradle property Gradle applies to the `Project` — but a build script can override it |
| `build.gradle` / `build.gradle.kts` (`version = …`) | Groovy/Kotlin DSL (code) | Yes, and this is the common case |
| `allprojects { version = … }` / `subprojects { … }` | code | Yes — sets it across projects |
| `settings.gradle(.kts)` | code | Can set it; it is still code |
| `gradle/libs.versions.toml` | TOML (declarative data) | **No** — it declares *dependency* versions, not the project's own |
| `version.txt` / `VERSION` | plain text | Only if a build script or plugin reads it — there is no Gradle convention |
| Maven `pom.xml` `<version>` | XML (declarative data) | Yes, or inherited from `<parent>` |
| Maven `${revision}` property | XML `<properties>` (declarative data) | Yes, when `<version>${revision}</version>` is used |

### 3.1 `gradle.properties`

`gradle.properties` is a Java-properties file holding Gradle properties, system
properties (`systemProp.…`) and project properties
(<https://docs.gradle.org/current/userguide/build_environment.html>). Gradle
applies the Gradle properties `version`, `group`, `status`, `buildDir` and
`description` "directly to the `Project` object", which is how `version=1.2.3`
in `gradle.properties` becomes the project version. The committed file is the one
in the project root; `GRADLE_USER_HOME/gradle.properties` and
`GRADLE_HOME/gradle.properties` also feed the same property and take precedence
in that order, so a version there is *not* in the repo. Two caveats for momo:

- **A build script can override it.** `version = "9.9.9"` in `build.gradle.kts`
  wins over the property. momo cannot know statically which assignment runs.
- **Subproject propagation is not crisply documented.** The docs say Gradle
  applies a `version` property to *the* `Project`; they do not state that a
  root `gradle.properties` `version` propagates to subprojects. I could not
  establish this from primary sources, so it is an open question (section 9), not
  a claim.

### 3.2 `build.gradle` / `build.gradle.kts` / `settings.gradle(.kts)`

These are Groovy or Kotlin programs. `version = "1.2.3"` is a statement, and
`allprojects { version = "1.2.3" }` sets it for every project. Parsing or
rewriting them is exactly the "executable build script" case momo must refuse:
there is no format-preserving edit that is guaranteed to preserve behaviour, and
a Groovy/Kotlin DSL has no equivalent of `toml_edit`'s structure-aware rewrite.

### 3.3 Version catalogs (`gradle/libs.versions.toml`)

A version catalog is "a selected list of dependencies"
(<https://docs.gradle.org/current/userguide/version_catalogs.html>). Its
`[versions]`, `[libraries]` and `[plugins]` tables declare **dependency and plugin
versions**, not the project's own version. A project *could* set
`version = libs.versions.something` in its build script, but that is code. So a
catalog is a TOML file a format-preserving editor can edit — but it is **not a
project-version source**, and momo must not treat it as one.

### 3.4 `version.txt` / `VERSION`

There is no Gradle-official version-file convention. A `version.txt` exists only
where a build script or a third-party plugin reads it, which makes it
code-driven. momo should not invent a convention here.

### 3.5 Maven `pom.xml`

`<version>` is one of the three Maven coordinates, and it is **inherited**: the
POM reference lists `version` among the elements inherited from a parent POM, and
`artifactId` and `name` among those that are *not*
(<https://maven.apache.org/pom.html>, "Inheritance"). So a child module may carry
no `<version>` of its own and take it from `<parent>`. A parent can be a released
artifact outside the repo, in which case there is no in-repo scalar to write —
the same situation momo already models as `ManifestError::InheritedVersion`
(`src/domain/manifest.rs:281-288`).

Multi-module Maven has two distinct relationships, and they are not the same
(<https://maven.apache.org/pom.html>, "A final note on (Inheritance v.
Aggregation)"):

- **Inheritance** — a child inherits `version` (and `dependencyManagement`, etc.)
  from its `<parent>`. A parent POM that is also an aggregator can be inherited
  from; an aggregator need not be a parent.
- **Aggregation** — `<modules>` lists child modules; Maven topologically sorts
  them. Listing a module does **not** make it inherit the parent's version.

So "what inherits from the root" in Maven is: `version` is inherited only by a
child that names the root as `<parent>` (directly or transitively). A sibling
module that is merely aggregated does not inherit anything.

### 3.6 Maven `${revision}`

Since Maven 3.5.0-beta-1, `<version>${revision}</version>` with
`<properties><revision>1.0.0-SNAPSHOT</revision></properties>` is the
"CI-friendly" form (<https://maven.apache.org/guides/mini/guide-maven-ci-friendly.html>).
Only `${revision}`, `${sha1}` and `${changelist}` are permitted as the version
placeholder; `${project.version}` (not `${revision}`) is still the way a module
declares a dependency on a sibling, and Maven 3 requires the
`flatten-maven-plugin` to install/deploy such a project at all
(`resolveCiFriendliesOnly` mode). Maven 4 (model `4.1.0`) removes both the
`flatten` requirement and the need to state subproject versions, via automatic
version resolution — a version-gated difference to keep in mind.

## 4. Q4 — The identity and the graph (answered before Q2, because it constrains it)

**Which mise provider prefix does a Gradle/Maven project carry? None.** mise's
workspace project graph has exactly four providers — Cargo, uv, Go and Node.js:

> "mise can read Cargo, uv, Go, and Node.js workspace manifests to learn which
> projects a monorepo contains and how they depend on each other."
> — <https://mise.jdx.dev/tasks/workspace-graph.html>

The provider table on that page lists no `java:`, `gradle:` or `maven:` provider.
mise does manage `java`, `gradle` and `maven` as *tools* (it installs JDKs and
sets `JAVA_HOME`, and can run Gradle through mise —
<https://mise.jdx.dev/lang/java.html>), but a *tool* is not a *project provider*:
tools do not create graph projects or edges.

**Does mise infer edges between JVM modules? No — it does not even create the
projects.** A JVM multi-module build is invisible to the graph.

momo reads that graph and, when it is empty, degrades to a single package:

```rust
// src/cli/workspace.rs:81-93
match crate::adapters::taskenv::discover(root, config.command_timeout) {
    Ok(graph) if !graph.is_empty() => { /* per-project Packages */ }
    Ok(_) => Ok(Discovery::degraded(root, single(), empty_graph())),
    Err(err) => Ok(Discovery::degraded(root, single(), err)),
}
```

What that costs, concretely:

- **No per-module versioning.** A JVM monorepo is treated as one package named
  after the repo directory; there is one tag prefix and one changelog.
- **`-p`/`--all` fail.** `select` returns `CliError::DiscoveryFailed` when
  packages were explicitly requested and the graph was empty
  (`src/cli/workspace.rs:106-116`).
- **No cascade.** The cascade reads `Workspace::graph()`
  (`src/cli/cascade.rs:41`); with no edges it reports nothing — not even an
  "unchecked" entry, because there is no edge to report.

**There is a manual escape hatch.** mise `[monorepo.projects]` overrides can add a
project with a namespaced id and explicit edges even when no provider infers it:

```toml
[monorepo.projects."gradle:lib"]
root = "lib"
depends_add = ["gradle:app"]
```

— <https://mise.jdx.dev/tasks/workspace-graph.html>, "Project overrides". Go is
the precedent for a provider that yields projects but no edges, and mise's answer
there is the same `depends_add` override. So a JVM graph is *possible today* but
entirely hand-declared; momo would then see `gradle:lib` as a Package.

momo's adapter dispatch keys off that prefix, and an unknown prefix currently
means *no adapter*:

```rust
// src/adapters/manifest/mod.rs:32-40
match provider(package.identity()) {
    Some("cargo") => Some(Box::new(CargoManifest)),
    Some("node") => Some(Box::new(NodeManifest)),
    Some("uv") => Some(Box::new(PythonManifest)),
    None => Some(Box::new(Probe::new())),
    Some(_) => None, // unknown ecosystem: no adapter
}
```

So if momo adds JVM adapters it must also define the prefixes they answer to
(`maven:`, `gradle:` are the natural choices) — but discovery will not emit them
until mise grows a provider or the user declares an override. A single-package
JVM repo (no prefix) would reach `Probe`, which today tries Cargo, then Node, then
Python (`src/adapters/manifest/mod.rs:60-70`) and would need the JVM adapters
added.

## 5. Q2 — How momo should write it

The port is `ManifestSync` (`src/domain/manifest.rs:168-229`): `locate`, `render`,
`write`, `dependencies`, `satisfied`, `widen`, plus `ManifestPlan`/`SkipReason`/
`ManifestEdits`. The write is deliberately conservative: "only version scalars and
dependency ranges change" (`src/domain/manifest.rs:16-18`).

### 5.1 What a format-preserving editor can target safely

- **XML (`pom.xml`)** — safe in principle, but there is a tooling gap: momo's
  TOML adapter uses `toml_edit` and its JSON adapter splices a byte span
  (`src/adapters/manifest/node/mod.rs:1-13`). There is no mature
  format-preserving XML editor in the Rust ecosystem comparable to `toml_edit`.
  The pragmatic route is the Node adapter's: locate the `<version>…</version>`
  text span and splice the scalar in place, leaving everything else
  byte-for-byte. That must be tested for CRLF, attributes, namespace
  declarations and self-closing tags.
- **Properties (`gradle.properties`)** — safe as a file format: it is line-based
  `key=value` with `#`/`!` comments. A scalar edit preserves comments and order.
  The *semantic* safety is the problem, not the format (section 3.1).
- **TOML (`gradle/libs.versions.toml`)** — safe as a format (`toml_edit`), but it
  is not a project-version source (section 3.3).

### 5.2 What it must refuse

- **`build.gradle`, `build.gradle.kts`, `settings.gradle(.kts)`** — code. Never
  parse, never rewrite.
- **A version computed at build time** — `version = gitVersion()`,
  `version = scmVersion.version` (axion), `version = "${revision}"` in a Groovy
  script, `git describe` output. The existing `SkipReason::Dynamic(String)` is
  exactly the right shape: "the version is computed elsewhere … The string names
  where the version comes from" (`src/domain/manifest.rs:58-61`). A JVM adapter
  would use it, e.g. `Dynamic("build.gradle.kts computes it from git describe")`.
- **A Maven version inherited from an out-of-repo parent** — reuse
  `ManifestError::InheritedVersion`.

### 5.3 Proposed behaviour

**Maven adapter (`maven:` prefix; `pom.xml`).**

- `locate`: the configured override, else `pom.xml` in the package dir.
- `render`: parse the XML; resolve the authoritative version scalar:
  - `<version>` present as a literal → that element;
  - `<version>${revision}</version>` → the `<properties><revision>` element;
  - `<version>` absent → the `<parent>`'s version. If the parent is in-repo and
    resolvable, write the parent's scalar; if not, `InheritedVersion`.
  - Write one scalar; preserve everything else by byte span.
- Skips: `NoVersion` when no `<version>`/`<revision>` and no resolvable parent;
  `AlreadyCurrent` when the scalar already holds the target.
- `dependencies`/`satisfied`/`widen`: initially return empty/`None` (see Q5).

**Gradle adapter (`gradle:` prefix; `gradle.properties`).**

- `locate`: the configured override, else `gradle.properties`.
- `render`: only when a `version=` line is present **and** no `build.gradle`/
  `build.gradle.kts`/`settings.gradle(.kts)` in the package dir assigns
  `version`. A cheap textual scan for a `version` assignment is a heuristic, so
  the conservative default is to require an explicit `manifest` override before
  touching `gradle.properties` at all.
- Skip `Dynamic("…/build.gradle.kts assigns version")` when a build script owns
  it; `NoVersion` when `gradle.properties` has no `version=`; `AlreadyCurrent`
  otherwise.
- `dependencies`/`satisfied`/`widen`: return empty/`None`.

**Both.** `write` composes the version scalar and any range edit into one write
per path, exactly as Cargo does (`src/domain/manifest.rs:20-24`).

## 6. Q3 — Version syntax and ordering

### 6.1 Maven's ordering is not semver

Maven's version order is documented in the POM reference
(<https://maven.apache.org/pom.html>, "Version Order Specification"). The key
facts for momo:

- It follows Semantic Versioning *1.0.0* precedence only when both strings are
  valid semver 1.0.0 and lower-case. **It is not compatible with SemVer 2.0.0**:
  "Maven does not special case the plus sign or consider build identifiers."
- It is **case-insensitive** (`3.2-ALPHA1` equals `3.2-alpha1`).
- Versions are tokenised on `.`, `-`, `_` and digit/letter transitions; trailing
  nulls (`0`, `""`, `final`, `ga`) are trimmed; qualifiers are ordered with
  `alpha < beta < milestone < rc = cr < snapshot < final = ga = release < sp`.
- Consequently `2.0-rc1 < 2.0`, so the range `[1.0,2.0)` **includes `2.0-rc1`** —
  "contrary to what most people expect". And "Gradle interprets it differently,
  resulting in different dependency trees for the same POM."

### 6.2 Gradle's ordering is different again

Gradle splits on `[. - _ +]`, splits mixed parts (`1a1` → `1.a.1`), and orders
numeric above non-numeric; for non-numeric parts it has its own special-casing:
`dev` is *lower* than any other part, and `rc`, `snapshot`, `final`, `ga`,
`release`, `sp` are *higher*, in that order
(<https://docs.gradle.org/current/userguide/dependency_versions.html>,
"Understanding version ordering"). Reckon's README notes it deliberately tunes
its output "to ensure it sorts correctly both with SemVer rules and Gradle's
built in version sorting (which are subtly different)"
(<https://github.com/ajoberstar/reckon>). So there are **three** orderings in
play — semver, Maven and Gradle — and they disagree.

### 6.3 Rendering a momo semver Version as a Maven version

momo reads the version from tags and renders it per ecosystem; Python maps
semver → PEP 440 and refuses a pre-release with no PEP 440 spelling
(`src/adapters/manifest/python/pep440.rs:9-12,92-101`). The Maven analogue:

| momo semver (tag) | Maven version | Note |
|---|---|---|
| `1.2.3` | `1.2.3` | straight through |
| `1.2.0-rc.1` | `1.2.0-rc.1` | Maven accepts arbitrary qualifiers; `rc` sorts below the release |
| `1.2.0-alpha.1` | `1.2.0-alpha.1` | `alpha` is a known qualifier |
| `1.2.3+abc` | — | **no Maven spelling**: Maven does not consider build metadata → refuse, as PEP 440 refuses an unrepresentable pre-release |

CalVer (`2026.10.15`) and Maven's `.Final`/`-RC1` styles are free-form strings
with no semver semantics. momo's bump policy operates on
`major.minor.patch` (`src/domain/version.rs:37-54`); a CalVer project simply
cannot use momo's bump as-is, and that should be stated rather than papered over.

### 6.4 `-SNAPSHOT` versus momo's candidate model

This is the sharpest mismatch. In Maven the *pre-release* convention is inverted
relative to semver: development happens on `1.2.0-SNAPSHOT`, and the release
*drops* the suffix to become `1.2.0`. A `-SNAPSHOT` sorts *below* its release
(`snapshot < final`), which is the opposite of semver, where a pre-release is
below the release too — but here the "release" is the same `major.minor.patch`
without a suffix, not a separate bump.

momo's candidate is `target` + a pre-release suffix rendered from `rc_format`,
whose default is `-rc.{n}` and which **must contain exactly one `{n}`**
(`src/domain/version.rs:163-188`). So `rc_format = "-SNAPSHOT"` is *rejected* by
`validate_rc_format`, because it has no `{n}`. A Maven SNAPSHOT therefore cannot
be expressed through the existing candidate mechanism without a change: either
`rc_format` grows a no-number form, or a per-ecosystem candidate rendering drops
the number. The candidate counter is also meaningless for SNAPSHOT (a SNAPSHOT
has no `-rc.N`).

Two further consequences:

- `--sync-ranges` is deliberately never applied on `candidate`, because
  "`^5.0.0` is not satisfied by `5.1.0-next.0`" (`docs/usage.md:164`). A Maven
  `-SNAPSHOT` candidate has the same problem against a sibling on the release
  version.
- `satisfied()` must decide whether `1.2.0-SNAPSHOT` satisfies a requirement on
  `1.2.0`. Under Maven ordering it does **not** (snapshot < release), which will
  surprise users.

### 6.5 What Maven Central requires and forbids

- The version "can be an arbitrary string but cannot end in `-SNAPSHOT`, since
  this is the reserved string used to identify versions that are currently in
  development", and semver is *suggested* but not required
  (<https://central.sonatype.org/publish/requirements/>, "Correct Coordinates").
- The Portal *accepts* `-SNAPSHOT` deployments but they do not become Central
  releases: "Deploying `-SNAPSHOT` releases will result in a `version cannot be a
  SNAPSHOT` error in the validation results"
  (<https://central.sonatype.org/faq/snapshot-releases/>).
- Published components are **immutable** — Central does not remove or modify a
  published version; a bug fix means a new release
  (<https://central.sonatype.org/publish/requirements/immutability/>).
- Central also requires POM metadata (name, description, url, licences,
  developers, SCM), GPG signatures, `.md5`/`.sha1` checksums, and Javadoc/sources
  jars (<https://central.sonatype.org/publish/requirements/>).

None of that is momo's job — but it fixes the constraints a Maven adapter's
output must respect: never write a version ending in `-SNAPSHOT` into a
publishable scalar, and never rely on rewriting a published version.

## 7. Q5 — The cascade

The cascade reads `Workspace::graph()`, and for each dependent edge checks whether
the new version satisfies the dependent's requirement, reporting (and optionally
widening) the unsatisfied ones (`src/cli/cascade.rs:35-143`). Only Cargo
requirements are evaluated: `checks_requirements` accepts `cargo` or a bare
identity and reports everything else unchecked (`src/cli/cascade.rs:294-302`).
The widen itself is Cargo's own grammar and refuses a conjunction or an upper
bound rather than guess (`src/adapters/manifest/cargo.rs:278-299`).

**Gradle.** A dependency inside `dependencies { … }` is Groovy/Kotlin code —
`implementation("group:artifact:version")`, `implementation(project(":shared"))`,
rich versions, `constraints { }`. None of it is statically readable without
evaluating the build, and the idiomatic sibling dependency
`implementation(project(":shared"))` has **no version requirement at all** — so
there is nothing for the cascade to read or widen. The one declarative surface is
the version catalog (`gradle/libs.versions.toml`), whose `[libraries]` entries are
TOML `group:artifact:version` coordinates. But (a) the catalog does not say which
subproject depends on which sibling, and (b) the catalog's `[versions]` entries
can be rich versions (`strictly`, `prefer`, `reject`) that are not a single
scalar (`https://docs.gradle.org/current/userguide/version_catalogs.html`).
Verdict: **Gradle dependency requirements should be refused, not guessed** — the
same posture Cargo takes for a conjunction. A catalog entry that is a plain
scalar is readable in principle, but without graph edges it has no counterpart
project to match against.

**Maven.** `<dependency>` entries are declarative XML: `groupId`, `artifactId`,
`<version>`. Versions can be a soft requirement (`1.0`), a hard range
(`[1.0]`, `(,1.0]`, `[1.2,1.3]`, `[1.0,2.0)`, `[1.5,)`, unions), a property
(`${project.version}`, `${revision}`, a `<properties>` value), or supplied by the
parent's `<dependencyManagement>`
(<https://maven.apache.org/pom.html>, "Dependency Version Requirement
Specification" and "Dependency Management"). So Maven is statically readable —
*with inheritance*.

What is safe to read and what must be refused:

- **Readable:** an exact `<version>1.2.3</version>`, and a Maven range as a
  string. `dependencyManagement`-supplied versions resolve to the parent's value.
- **Safely rewritable:** an exact version, by re-anchoring it at the target —
  analogous to Cargo's `=`/bare form. Only when the dependency targets a sibling
  in the release set.
- **Must be refused:** a Maven *range* (`[1.0,2.0)`). Re-anchoring one bound
  could narrow the range rather than widen it, and Maven's own note that
  `[1.0,2.0)` includes `2.0-rc1` shows how counter-intuitive the semantics are.
  Report it, never rewrite it. The Cargo adapter already refuses conjunctions and
  upper bounds for exactly this reason.
- **Must be refused:** Gradle rich versions and constraints, and any requirement
  that lives in a build script.

## 8. Q6 — Prior art

The JVM ecosystem's established tools split into two camps: those that **compute
the version at build time** from SCM, and those that **edit versions in the POM**.

**Compute at build time (SCM as source of truth):**

- **`axion-release-plugin`** derives the version from the nearest tag — a tagged
  commit is a release, commits after it are `-SNAPSHOT` — and injects it with
  `version = scmVersion.version`. The build file holds *no* version scalar
  (<https://github.com/allegro/axion-release-plugin>). momo's SCM-as-authority
  model agrees; but there is nothing for momo to write, so such a project is a
  `Dynamic` skip.
- **`palantir/gradle-git-version`** sets `version = gitVersion()`, where
  `gitVersion()` mimics `git describe --tags --always --first-parent`
  (<https://github.com/palantir/gradle-git-version>). Again code-computed; no
  scalar. Note momo reads tag *names* and would happily tag a `v1.2.3` that
  `git describe` can consume, so the two are compatible at the tag level.
- **`jgitver`** computes a semver version from tags/branches/commits, with a
  "maven like" mode that adds `-SNAPSHOT` and increments the patch; it is applied
  as a **Maven core extension** (`.mvn/extensions.xml`) or a Gradle plugin
  (<https://github.com/jgitver/jgitver>). This is the strongest evidence that the
  natural JVM integration point is a *build extension*, not a file rewrite.
- **`reckon`** infers the next version from git, offers `stages` (beta/rc/final)
  or `snapshots`, overrides `project.version`, and is explicitly tuned to sort
  under both semver and Gradle's ordering
  (<https://github.com/ajoberstar/reckon>). It is the prior art that already
  solved the ordering reconciliation momo would otherwise have to solve.
- **`nebula-release-plugin`** provides `final`/`candidate`/`devSnapshot`/
  `snapshot` tasks over `gradle-git`, where `candidate` produces `-rc.#` and
  `snapshot` produces `-SNAPSHOT`
  (<https://github.com/nebula-plugins/nebula-release-plugin>). It confirms the
  `final` vs `rc` vs `snapshot` distinction that momo's candidate model straddles.

**Edit the POM:**

- **`versions-maven-plugin`** manages versions *in* the POM: `versions:set`,
  `set-property`, `update-child-modules` (fixing parent/child version drift),
  `resolve-ranges`, `lock-snapshots`, `use-releases`
  (<https://www.mojohaus.org/versions/versions-maven-plugin/index.html>). This is
  the tool momo's Maven adapter would overlap with; it is also the proof that
  multi-module version propagation is a known, solvable problem.
- **`flatten-maven-plugin`** resolves `${revision}`/`${sha1}`/`${changelist}` for
  install/deploy (`resolveCiFriendliesOnly`), and is *required* for a CI-friendly
  Maven 3 project to publish at all
  (<https://www.mojohaus.org/flatten-maven-plugin/flatten-mojo.html> and the
  CI-friendly guide). If momo writes `${revision}`, it should *report* that the
  project needs flatten — not manage it.

**Publish (a different job):**

- **JReleaser** "can create and tag a release, upload all files and generate the
  changelog based on the new commits since the previous tag", and deploys to
  Maven Central (<https://jreleaser.org/guide/latest/reference/release/index.html>).
  It does not derive the version; it reads it from the build and tags. Publishing
  is out of momo's lane.

**What to copy:** the tag-as-authority model (axion, jgitver, reckon) — momo
already has it; the `final`/`candidate` distinction (nebula); and the
multi-module parent/child consistency concern (versions-maven-plugin).

**What not to copy:** build-time version *injection* (momo writes a scalar; it
does not become a build plugin); POM-wide dependency bumping (versions-maven-
plugin's broader remit); distribution assembly and repository deployment
(JReleaser).

## 9. Q7 — The recommendation

### 9.1 What to build first

A **Maven `pom.xml` adapter**, version scalar only, behind the existing
`ManifestSync` port. It is the highest-value, lowest-risk slice because
`pom.xml` is declarative, the version is a real field, and the failure modes
(inheritance, `${revision}`) are already modelled by the port
(`InheritedVersion`, `SkipReason`). It needs no graph support and no range
widening to be useful.

Then, and only if the owner wants it, a **Gradle `gradle.properties` adapter**,
gated behind an explicit `manifest` override because momo cannot statically prove
the property is the effective version.

### 9.2 What the port looks like

See section 5.3. The Maven adapter is `locate`/`render`/`write` with
inheritance-aware scalar resolution and a byte-span XML splice; `dependencies`,
`satisfied` and `widen` start as empty/`None`. The Gradle adapter is the same
shape over a properties file, with a build-script-assignment refusal.

### 9.3 The risks

- **No format-preserving XML editor in Rust.** A byte-span splice (like the Node
  adapter's) is the pragmatic route, but it must be proven against CRLF,
  attributes, namespaces and self-closing tags. This is the single largest
  implementation risk.
- **Gradle's `version` semantics are under-documented.** Whether a root
  `gradle.properties` `version` reaches subprojects, and whether a build script
  overrides it, cannot be established statically. A Gradle adapter may be
  correct only in the narrowest case.
- **Maven inheritance can leave the repo.** A `<parent>` that is a released
  artifact means there is no in-repo scalar; the adapter must refuse, not guess.
- **Maven 4 changes the rules** (automatic version resolution, no `flatten`
  requirement) — version-gated.
- **The SNAPSHOT/candidate mismatch** (section 6.4) is a model change, not just
  an adapter.
- **The graph gap** (section 4): without a mise JVM provider, per-module
  versioning and cascading do not work, so the Maven adapter's value is limited
  to single-module or single-package repos unless the user hand-declares
  `[monorepo.projects]` overrides.

### 9.4 What we would not do

- **Not parse or execute Groovy/Kotlin DSL.** No `build.gradle(.kts)` or
  `settings.gradle(.kts)` editing, ever.
- **Not widen Maven ranges or Gradle rich versions/constraints.** Report only.
- **Not run Gradle or Maven.** momo shells out to the VCS, never the ecosystem.
- **Not become a build plugin or Maven extension.** That is jgitver/reckon's
  shape, not momo's.
- **Not treat `gradle/libs.versions.toml` as a project-version source.**
- **Not support CalVer in the bump policy.** momo derives semver; CalVer is a
  different product decision.
- **Not publish to Maven Central.** JReleaser's lane.
- **Not guess a JVM module graph.** If mise emits no edges, momo reports nothing
  rather than inventing edges.

## 10. Open questions

1. **Is JVM in scope at all?** A documented "not supported" is a legitimate v1
   answer and costs nothing. This should be an owner ruling before any adapter
   work (see Proposed work ticket 01).
2. **What prefix should a JVM package carry?** Discovery emits none. momo would
   have to recognise `maven:`/`gradle:` (or similar) while relying on a config
   `identity` override or a mise `[monorepo.projects]` override to ever see one.
3. **Does a root `gradle.properties` `version` propagate to subprojects?** Not
   established from primary sources.
4. **`${revision}` or the literal `<version>`?** Depends on the project's
   convention; the adapter would need a rule for which to prefer when both exist.
5. **How should `-SNAPSHOT` map onto the candidate model?** A no-number
   `rc_format`, or a per-ecosystem candidate rendering? And should `candidate`
   even be offered for Maven, given SNAPSHOT is unpublishable to Central?
6. **Should build metadata (`+abc`) be refused or dropped** when rendering a
   Maven version? PEP 440 refuses; Maven has no spelling, so refusing is the
   consistent choice.
7. **Is a `SkipReason` for "declared in a build script" warranted**, or is
   `Dynamic` (which already names an origin string) sufficient?

## 11. Proposed work

Tickets I would cut, in dependency order. Each is one focused change; none is
created here.

1. **`01-jvm-support-posture`** — Decide and document whether JVM (Gradle/Maven)
   is in scope, and state the boundaries (no DSL parsing, no build execution, no
   publishing) in `docs/opinions.md`/`docs/usage.md`. Blocks everything.
2. **`02-jvm-graph-gap`** — Document, with evidence, that mise has no JVM project
   provider, that a JVM monorepo degrades to a single package, and the
   `[monorepo.projects]` override workaround. Depends on 01.
3. **`03-maven-pom-adapter`** — A `ManifestSync` adapter for `pom.xml`: version
   scalar only, inheritance-aware, byte-span XML splice, `satisfied`/`widen` as
   `None`. Depends on 01.
4. **`04-maven-revision-property`** — Handle `<version>${revision}</version>` by
   writing the `<properties>` scalar, and report the `flatten-maven-plugin`
   requirement rather than manage it. Depends on 03.
5. **`05-maven-cascade-exact`** — Read Maven `<dependency>`/`dependencyManagement`
   requirements; report unsatisfied ones; widen exact versions only, refuse
   ranges. Depends on 03.
6. **`06-gradle-properties-adapter`** — A `gradle.properties` adapter for the
   `version=` scalar, gated behind an explicit `manifest` override, refusing when
   a build script assigns the version. Depends on 01.
7. **`07-maven-version-rendering`** — Render a momo semver Version as a Maven
   version (qualifier mapping, refuse build metadata), and settle the
   `-SNAPSHOT`/candidate question. Depends on 03.
8. **`08-jvm-identity-and-discovery`** — Define the `maven:`/`gradle:` identities,
   extend `for_package` and the single-package `Probe`, and document that
   discovery will not emit them until mise grows a provider or the user declares
   an override. Depends on 02.

Tickets 01 and 02 are cheap and can land immediately. 03 is the first real
build. 06 and 08 are the ones most likely to be argued down to "document as not
supported".
