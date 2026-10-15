# 06: jj remote parsing swallows the `(push: …)` annotation

Type: bug
Status: ready-for-agent
Severity: medium
Area: adapters/vcs/jj
Verified: yes — reproduced with jj 0.46.0

## Symptom

When a jj remote's push URL differs from its fetch URL, `remote()` returns a string that is not a URL, and the malformed value reaches the changelog footer as a broken link.

## Evidence

`JjVcs::remote` (`src/adapters/vcs/jj.rs:61-77`) splits each line on the first space and takes the rest as the URL, under the comment "prints one `<name> <url>` line per remote". jj appends ` (push: <url>)` when fetch ≠ push.

```console
$ jj git remote add origin https://github.com/foo/bar.git --push-url git@github.com:me/bar.git
$ jj git remote list
origin https://github.com/foo/bar.git (push: git@github.com:me/bar.git)
```

`remote::normalise` (`src/domain/remote.rs:139-160`) is lenient, so the annotated string parses "successfully" and `Remote::footer` emits `https://github.com/foo/bar.git (push: …)/releases/tag/v0.1.0`.

## Trigger

Add a jj remote with a distinct push URL, then `momo status`/`release` — the footer links are malformed. No warning is emitted.

## Suggested direction

Take only the first whitespace-delimited field after the name as the URL, e.g. `line.split_whitespace().nth(1)`, rather than everything after the first space. Add a `FakeRunner` test scripting the annotated form.
