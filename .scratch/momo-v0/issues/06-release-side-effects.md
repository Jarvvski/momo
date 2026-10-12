# Release side-effects & tag format

Type: grilling
Status: open
Blocked by: 12, 16

## Question

When `momo release` runs, what does it actually change? Settle:

- Does it **write the version back** to a version file / `Cargo.toml`?
- Does it **edit the changelog** (promote `Unreleased` → a versioned section)?
- Does it **commit** those changes, or leave the tree dirty?
- Does it create the **VCS tag itself**, or leave that to `momo tag`?
- **Tag name format:** `v1.2.3` / `1.2.3` / `v1.2.3-rc-1`? Annotated vs
  lightweight vs signed?

Note: jj has no staging area and thin tag support — coordinate with ticket 12
(research) before deciding how `release` and `tag` behave under jj vs git.
