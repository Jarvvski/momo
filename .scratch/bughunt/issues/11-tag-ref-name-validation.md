# 11: Tag-name validation misses two `git check-ref-format` rules

Type: bug
Status: resolved
Severity: low
Area: adapters/vcs/tags
Verified: yes — compared with `git check-ref-format`

## Symptom

`is_valid_ref_name` accepts names git rejects, so `create_tag` reaches `git tag`/`jj tag set` and surfaces the failure as an opaque operation error rather than `VcsError::InvalidTagName`.

## Evidence

`src/adapters/vcs/tags.rs:38-54` checks a trailing `.lock` and `..`, `//`, `@{`, but not a component that begins with `.` nor a `.lock` that ends a non-final component. `git check-ref-format refs/tags/release.lock/v1.2.3` and `refs/tags/.1.2.3` both reject.

## Trigger

`checked_tag_name("release.lock/", &Version::new(1,2,3))` → `Ok("release.lock/v1.2.3")`; `checked_tag_name(".", &Version::new(1,2,3))` → `Ok(".1.2.3")`. Reachable only via a `tag_prefix` containing a `/.`/leading-dot component or a `.lock`-ending directory.

## Suggested direction

Split on `/` and reject a component that starts with `.` or ends with `.lock`. Extend `valid_ref_names_mirror_git_check_ref_format` with `a/.b/c`, `release.lock/x` and `.1.2.3`.
