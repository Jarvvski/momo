# Release side-effects & tag format

Type: grilling
Status: resolved
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

## Answer

Settled (all recommendations accepted):

1. **The release ceremony is a split.** `momo release` writes the changelog (and
   manifests, when sync is enabled) but does **not** commit or tag. You commit,
   then `momo tag` creates the tag. This matches the verb surface (`release` +
   `tag`), fits jj's no-staging model, and lands the tag on the commit that
   contains the changelog. Default remains "leave the tree dirty".
2. **Tag kind:** git → **annotated** (`git tag -a`, message = release title);
   jj → **lightweight** (its only option). The `git -C "$(jj git root)"` workaround
   (ticket 12) covers signed/annotated in a jj repo if ever needed.
3. **`release` drives manifest sync** (ticket 18) when enabled — the changelog
   promotion and the manifest version bump are both release steps.

With per-package versioning (ticket 16): `momo release` with no `-p` releases
every package with unreleased changes, producing one `<slug>-v<version>` tag per
package.

Unblocks ticket 04 (config).
