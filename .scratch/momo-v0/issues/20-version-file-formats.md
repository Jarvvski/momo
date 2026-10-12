# Version-file formats & safe writing

Type: research
Status: resolved

## Question

For each common ecosystem, **where does the version live** in the manifest, and
how can a tool **read and safely write** it (preserving comments / formatting)?

Cover at least:

- Rust `Cargo.toml` (including workspaces and `version.workspace = true`);
- Node `package.json`;
- Java/Kotlin `gradle.properties` + `build.gradle(.kts)`;
- Python `pyproject.toml`;
- anything else worth supporting (e.g. `.csproj`, `pom.xml`).

For each: the exact key/path, and the safest read/write approach (e.g.
`toml_edit` for TOML, a JSON-preserving editor for `package.json`, a properties
editor for gradle). Note pitfalls (workspace inheritance, lockfiles, duplicate
version declarations).

## Answer

Full findings: [`../research/version-file-formats.md`](../research/version-file-formats.md).

Per ecosystem — canonical location + safest write:

- **Rust `Cargo.toml`:** `package.version`, or `workspace.package.version` when a
  member has `version.workspace = true`. Write with `toml_edit`; refresh
  `Cargo.lock`.
- **Node `package.json`:** top-level `version`. Prefer a **minimal JSON text
  edit** (`jsonc-parser`); lockfiles must move together.
- **Gradle:** project property `version` in `gradle.properties` (line-oriented
  edit; user-level can shadow project-level) or `build.gradle(.kts)`; Android uses
  `versionName` + integer `versionCode`.
- **Python `pyproject.toml`:** `[project].version` or `[tool.poetry].version`;
  `dynamic = ["version"]` means the real source is elsewhere. `toml_edit`; refresh
  the lockfile.
- **Maven `pom.xml` / .NET `.csproj`:** XML splice (`.NET` often centralises in
  `Directory.Build.props`).

Rust building blocks: `toml_edit`, `jsonc-parser`, `java-properties`, `quick-xml`;
fallback is a **span-based text splice**. Never reserialize a whole manifest to
change one scalar.

Cross-cutting pitfalls: workspace/parent inheritance, dynamic versions, duplicate
declarations, lockfiles, formatting round-trip, and precedence shadowing.

Unblocks ticket 18.
