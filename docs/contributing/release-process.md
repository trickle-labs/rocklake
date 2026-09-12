# Release process

Releases are prepared from the tagged source commit. For v0.63.1, the
release-blocking `Release Certification` workflow retains the complete
correctness matrix:
formatting, clippy, workspace tests, DuckLake conformance, public-surface and
backend execution, Windows, security, docs, quickstart, compatibility, Miri,
sanitizers, and the preserved v0.47.17 production-failure certification. The
v0.53.0 lifecycle tests cover shared connection and request ownership,
cancellation, typed admission permits, and exactly-once response observation.
v0.54.0 router tests cover route validation, prefix isolation, and stable alias
resolution. v0.57.0 registry tests cover lifecycle, CAS generations,
idempotency, audit sequence, backup/restore, tombstones, and registry/tenant
prefix separation. v0.58.0 backup-set, restore verification, and durable
maintenance tests cover recovery metadata and bounded scheduling. v0.59.0
encryption envelope, SCRAM verifier-file, and release-governance checks cover
the supported security boundary. v0.60.0 version-domain, migration-plan,
verified no-op, downgrade-rejection, status JSON, and v0.59.0 direct-upgrade
checks cover the compatibility freeze. The v0.63.1 gate validates the public
surface manifest, beta support policy, evidence ledger, redacted support
bundle, and release artifact contract.
The publication stage also tests the built artifacts without rebuilding from
source.

## Before the release PR

Run the checks that apply locally:

```bash
cargo fmt --all -- --check
cargo test --workspace
mkdocs build --strict
python scripts/validate_compatibility_manifest.py
bash scripts/quickstart.sh
cargo test -p rocklake-pgwire --test v046_cli_tests support_bundle_is_redacted_and_non_overwriting
test -s docs/operations/beta-support.md
test -s docs/operations/beta-evidence-v0.63.0.md
test -s docs/assessments/v1-readiness.md
```

Update `CHANGELOG.md` and current version references. Keep claims tied to
tests: v0.63.1 supports the binary, DuckLake 1.0 targets covered by CI, local
and cloud object storage, server-side TLS, password authentication,
SCRAM-SHA-256 authentication, multi-principal grants, bounded quotas, and
typed TOML configuration. Rust client, read-only API, DataFusion, and language
bindings remain Preview or Experimental unless the support-level manifest says
otherwise. It does not publish Docker images or support mTLS or certificate
hot-reload.

After the release PR merges, run `Release Certification` with the merge commit
SHA as `certified-ref`, then wait for every job to pass:

```bash
git fetch origin main
CERTIFY_SHA=$(git rev-parse origin/main)
gh workflow run release-certification.yml --ref main -f certified-ref="$CERTIFY_SHA"
gh run watch
```

Create the version tag only after that run passes. The tag-triggered release
workflow certifies the same SHA again, checks the tag against the Cargo version,
and builds artifacts without pushing a version bump.

## Tagging

After the PR checks pass, merge the release PR. After manual Release
Certification passes on the merge SHA, tag that commit:

```bash
git tag v0.63.1
git push origin v0.63.1
```

Release artifacts must be built from that tag. The release contains raw
target-named binaries, per-binary checksums and build metadata, `SHA256SUMS`,
`release-manifest.json`, one SPDX SBOM, and provenance attestations. The
v0.47.17 certification remains a required regression gate for later releases.
