# v1.0 readiness assessment

Assessment date: 2026-09-12. Candidate release: v0.63.1. RC decision: hold.

## Assessment boundary

This source review covers the v0.63.1 release candidate, its compatibility
manifest, release checks, and checked-in beta ledger. The release changes
metadata and documentation only. It does not change runtime behavior, a
persisted format, or the stable public surface.

This report does not claim an independent deployment audit. The beta ledger
contains no customer data, and its observation window remains in progress.

## Open gates

- **Beta observation.** The ledger starts on 2026-09-11 and requires 30 days.
  At this assessment date the window has not elapsed. No field findings are
  recorded after beta entry, but the production exercises and workload evidence
  have not been published in the ledger. Keep the RC1 gate closed until the
  window and required exercises finish.
- **Independent architecture and operations review.** No second reviewer has
  signed an assessment of the v0.63.1 candidate. The links below support a
  repository review; they do not replace an independent review.
- **Governance bus factor.** `.github/CODEOWNERS` names only `@grove`. The
  repository does not show that a second person can complete release, restore,
  failover, and security response from the published procedures. Record and
  link those exercises before RC1.

These are RC1 blockers. They do not change the v0.63.1 beta support boundary.

## Security delta from v0.59.0

Since v0.59.0, the compatibility work added typed migration and downgrade
rejection in v0.60.0, and the public-surface freeze added redacted support
bundles in v0.62.0. The release gate covers version and migration behavior,
support-bundle redaction, `cargo deny`, and PG-wire security tests. Workspace
tests retain the encryption-envelope and SCRAM verifier coverage from v0.59.0.
This v0.63.1 candidate changes no security-sensitive runtime code. Its exact
merge SHA must pass the security jobs in Release Certification before tagging.

## Accepted risks

| Risk | Owner | Rationale | User-facing documentation | Milestone |
|---|---|---|---|---|
| AWS S3, GCS, and Azure scale are not certified. | Release maintainer (@grove) | No qualifying scale evidence is recorded; support claims remain bounded to the tested envelope. | [Beta support](../operations/beta-support.md) | v0.64.0–v0.64.2 |

## Evidence

- [Beta evidence ledger](../operations/beta-evidence-v0.63.0.md)
- [Beta support boundary](../operations/beta-support.md)
- [Release certification workflow](https://github.com/trickle-labs/rocklake/blob/main/.github/workflows/release-certification.yml)
- [Public-surface compatibility test](https://github.com/trickle-labs/rocklake/blob/main/crates/rocklake-pgwire/tests/v0478_surface_manifest_tests.rs)
- [PG-wire security tests](https://github.com/trickle-labs/rocklake/blob/main/crates/rocklake-pgwire/tests/security_tests.rs)
- [Encryption envelope implementation](https://github.com/trickle-labs/rocklake/blob/main/crates/rocklake-catalog/src/encryption.rs)
- [Support-bundle documentation](../operations/support-bundles.md)
- [Security policy](https://github.com/trickle-labs/rocklake/blob/main/SECURITY.md)
- [Release and restore procedures](../contributing/release-process.md)
- [Backup and restore](../operations/backup-restore.md)
- [Failover](../operations/failover.md)
- [Security operations](../operations/security.md)
