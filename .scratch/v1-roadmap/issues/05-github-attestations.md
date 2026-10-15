# 05: Publish GitHub artifact attestations and verify in the installer

Type: task
Status: resolved
Points: 14
Bucket: jdx ecosystem fit
Severity: medium
Area: .github/workflows + install.sh

## Problem

momo's headline install is `mise use github:Jarvvski/momo`, and mise's `github:`
backend **auto-verifies GitHub artifact attestations when the release publishes
them**. momo publishes a `SHA256SUMS` file but no attestation, so the one
verification the target ecosystem performs has nothing to verify. This is the
cheapest trust signal available to the mise wedge.

## Evidence

- `.github/workflows/release.yml` builds per-platform assets and a `SHA256SUMS`;
  top-level `permissions: contents: read`, the `release` job has `contents: write`.
- `install.sh:187` (`verify_checksum`) already checks the archive against the
  published checksums.
- mise security docs: the `github:` backend verifies GitHub artifact attestations
  when present.

## Proposal

**GitHub/Sigstore build-provenance attestations, not minisign/cosign.** Rationale:
attestations are the one scheme the `github:` backend verifies automatically, with
**no key custody or secret rotation** (OIDC + Sigstore). A custom minisign/cosign
key is a custody burden that no `github:` backend auto-verifies (that is the
`aqua:`/`packslip:` path). Keep the `SHA256SUMS` check and *add* attestations.

- In the `release` job: grant `id-token: write` and `attestations: write`, and run
  `actions/attest-build-provenance@v2` with `subject-path: dist/*.tar.gz` **before**
  `gh release create`.
- In `install.sh`: after `verify_checksum`, if `gh` and `gh attestation verify` are
  available, verify the archive against `--repo Jarvvski/momo`; fall back silently
  to the checksum when `gh` is absent.

**Do not put "signed" in the README until attestations actually publish** and
`gh attestation verify` is documented — otherwise it is an unverifiable claim.

## Surface

- `README.md` Install section: an optional `gh attestation verify` note.
- `docs/ci.md`: the `gh attestation verify` snippet beside the installer snippet.

## Implementation sketch

| Change | Files |
|---|---|
| Attest step + permissions | `.github/workflows/release.yml` (`release` job) |
| Installer verification | `install.sh` (after `verify_checksum`) |
| Docs | `README.md`, `docs/ci.md` |

## Scoring

Points **14** = Value 4×2 + Ease 4 + Novelty 2. High value for the mise wedge (it
is the one thing mise verifies); moderate effort; not novel, but ecosystem-exact.

## Risks

- **Attestation permissions.** Forgetting `id-token: write` / `attestations: write`
  fails the step (or fails late in a release). Test on a dry-run release first.
- **Over-claiming.** Ship the docs only once attestations publish.
- This is the only packaging change in the effort; it moves the needle on trust
  more than any code change in bucket F.

## Related

- 10 (jdx CLI citizenship), 16 (completions).
