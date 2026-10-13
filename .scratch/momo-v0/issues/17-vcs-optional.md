# VCS-optional operation

Type: grilling
Status: resolved

## Question

momo's primary version source is VCS tags, but it should also work in a project
**not tied to a VCS**. How?

Settle:

- What is the version **source when there is no git/jj**? A momo-managed state
  file (e.g. `.momo/version`)? The changelog's newest released heading? A version
  file (see ticket 18)?
- How does momo **detect** "no VCS"?
- Which commands **degrade** without a VCS (`tag` has nothing to tag; does
  `release` still work, writing the changelog only)?
- If a VCS is added later, does momo reconcile tags against its own record?

## Answer

Settled (all recommendations accepted):

1. **No-VCS version source: `.momo/version`** — a momo-managed state file written
   on every release, the direct analogue of a tag. (Not the changelog heading, and
   not an ecosystem version file.)
2. **Degradation:** `status` / `release` / `candidate` work without a VCS (writing
   the changelog + manifests + `.momo/version`); **`tag` errors clearly** ("no VCS
   detected — nothing to tag").
3. **Reconciliation:** the VCS wins when present — its tags become authoritative
   and `.momo/version` is the fallback; `momo tag` can create the tag for the
   current version to bring the two into sync.

Detection is per ticket 12: `.jj` first, else `.git`.
