## What this changes

<!-- One paragraph: the problem and the fix. Link the issue it closes, if any. -->

## Checklist

- [ ] `mise run check` is green — fmt:check, lint, domain purity, docs, build and test.
- [ ] The change is recorded with `momo break|add|fix "<description>"`, so the changelog carries it and a release can derive it.
- [ ] One focused change, on its own bookmark cut from `main`.

`main` is protected: it moves only through a pull request whose `check` job is green. See [the landing flow](https://github.com/Jarvvski/momo/blob/main/docs/contributing.md#landing-a-change).
