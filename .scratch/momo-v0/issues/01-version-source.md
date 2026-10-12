# Version source of truth & first-release bootstrap

Type: grilling
Status: open

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
