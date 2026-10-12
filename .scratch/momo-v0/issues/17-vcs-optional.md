# VCS-optional operation

Type: grilling
Status: open

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
