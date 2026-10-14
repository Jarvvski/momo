# 08: Publish checksums with the release assets

Type: task
Status: ready-for-agent
Severity: low
Area: release
Verified: yes — `.github/workflows/release.yml` packages tarballs and attaches them with `gh release create`; no checksums are produced.

## Symptom

Users downloading a release binary have nothing to verify it against.

## Suggested direction

In the `build` job's packaging step or the `release` job, generate a `SHA256SUMS` file over `dist/*.tar.gz` and attach it alongside the archives. Pick one hashing tool and be explicit (`sha256sum` on the Linux runners; the macOS runners have `shasum -a 256`). Mention verification in `docs/ci.md`'s install section.

## Related

- Group B.
