# Version source of truth & first-release bootstrap

Type: grilling
Status: resolved

## Question

Where does momo read a project's *current* version, and what does it do when
there is none?

- **(a)** Derive it from the latest VCS tag (git/jj) — language-agnostic, works
  for any project.
- **(b)** Read the newest released heading in the changelog.
- **(c)** Read a version file (`VERSION`, `Cargo.toml`, `package.json`, …).

Sub-questions to settle:

- The bootstrap rule when there are **zero tags / no history** (start at
  `0.1.0`? `0.0.0`? require `momo init`?).
- What happens when the changelog and the tags **disagree**.
- Whether momo ever **writes the version back** anywhere (and if so, where).
- How this interacts with `--dry-run` (computing a version with no side effects).

## Answer

Settled:

1. **Source of truth: the latest VCS tag** (git/jj). The changelog *records*
   releases (with timestamps) but does not define the version.
   - **VCS-optional:** momo must also work in a project not tied to a VCS; the
     fallback source is ticket 17.
2. **Monorepo-aware** — a v0 addition (reversing the "one version per repo"
   default). The package model is ticket 16; it feeds tickets 03/04/06/09.
3. **Bootstrap:** baseline `0.0.0`; the first release follows the normal bump
   rules (`add`/`break` → `0.1.0`; `fix`-only → `0.0.1`). `momo init` may set an
   explicit starting version.

Stretch (user-flagged): momo detecting where the version is defined
(`Cargo.toml`, `gradle.properties`, `package.json`, …) and **setting** it — see
ticket 18.

Precedent: the zero-aware pre-1.0 camp (release-plz default, svu `--v0`,
git-cliff `breaking_always_bump_major=false`) matches momo's rules (ticket 13).

Left to fog: the changelog-vs-tags **disagreement** rule.

Unblocks tickets 05 and 07; surfaces tickets 16, 17, 18 (and research 19, 20).
