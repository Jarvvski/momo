# 27: The SSH port leaks into the normalised web base URL

Type: bug
Status: resolved
Severity: medium
Area: domain/remote
Verified: yes — read at the cited lines

## Symptom

For an `ssh://` remote with an explicit port, `normalise` keeps the port in the
authority and builds an `https://host:port/...` base. Every generated compare/tag
link is then a web URL on an SSH port, which does not resolve.

## Evidence

`src/domain/remote.rs:135-148`:

```rust
let (authority, path) = match url.split_once("://") {
    Some((_scheme, rest)) => rest.split_once('/')?,
    None => url.split_once(':')?,
};
let host = authority.rsplit('@').next()?;
...
Some(format!("https://{host}/{path}"))
```

For `ssh://git@github.com:2222/o/r.git`: `authority = "git@github.com:2222"`,
`host = "github.com:2222"`, base `https://github.com:2222/o/r`. `Forge::of` still
classifies it as GitHub (substring match), so nothing flags it.

## Trigger

A repo whose `origin` is `ssh://git@github.com:2222/o/r.git`. Changelog footer
links point at `https://github.com:2222/...`.

## Suggested direction

Strip a `:port` suffix from the authority for the `scheme://` form before building
the base.

## Open questions for triage

- Should a non-standard port be rejected instead (the web base is not derivable
  from an SSH port)? Stripping is the pragmatic choice since the forge's web port
  is almost always 443.

## Triage

**Verdict:** Confirmed. `normalise` (`src/domain/remote.rs:135-148`) discards the
scheme and keeps `host:port`, so `ssh://git@github.com:2222/o/r.git` becomes
`https://github.com:2222/o/r`; `Forge::of` still classifies it GitHub, so nothing
flags it.

**Corrections:** The suggested fix ("strip a `:port` suffix for the `scheme://`
form") is imprecise and would break a self-hosted forge on a non-standard **web**
port, e.g. `https://gitlab.example.com:8443/group/repo`. Strip the port only for
non-`http(s)` schemes (ssh, git, git+ssh), where the port belongs to the transport
rather than the web base. Related but out of scope: the code rewrites `http://`
to `https://` unconditionally.

**Scope:** Single, medium (needs an unusual SSH port). Keep.

**Implementation sketch:** In `normalise`, keep the scheme from
`split_once("://")`; in the scheme branch, when the scheme is not `http`/`https`,
strip a trailing `:<digits>` from `authority` before taking the host. Tests:
`ssh://git@github.com:2222/o/r.git` → `https://github.com/o/r`;
`https://gitlab.example.com:8443/group/repo` keeps `:8443`.

## Resolution

`normalise` (`src/domain/remote.rs`) now keeps the scheme from
`split_once("://")`. For a non-`http(s)` scheme it drops a trailing `:<digits>`
from the authority via a new `strip_port` helper before building the web base; an
`http(s)` authority keeps its port, since that may be a real web port.

Tests: `ssh://git@github.com:2222/o/r.git`, `git://github.com:9418/o/r.git`, and
`git+ssh://git@github.com:2222/o/r` all normalise to `https://github.com/o/r`;
`https://gitlab.example.com:8443/group/repo` and its `http://…​.git` form keep
`:8443`.
