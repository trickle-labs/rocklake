# Release process

Releases are prepared from the tagged source commit. For v0.61.0, the
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
checks cover the compatibility freeze. The v0.61.0 capacity gate validates
the capacity report, dated pricing input, bounded evidence profiles, and
performance regression thresholds.
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
```

Update `CHANGELOG.md` and current version references. Keep claims tied to
tests: v0.61.0 supports the binary, DuckLake 1.0 targets covered by CI, local
and cloud object storage, server-side TLS, password authentication,
SCRAM-SHA-256 authentication, multi-principal grants, bounded quotas, and
typed TOML configuration. It does not publish Docker images or support mTLS or
certificate hot-reload.

The release workflow must certify the exact commit SHA before a version tag is
created. It verifies the tag, Cargo version, and built artifacts against that
SHA; it does not push a version bump after tagging.

## Tagging

After review and green CI, merge the release PR and tag the merge commit:

```bash
git tag v0.61.0
git push origin v0.61.0
```

Release artifacts must be built from that tag. The release contains raw
target-named binaries, per-binary checksums and build metadata, `SHA256SUMS`,
`release-manifest.json`, one SPDX SBOM, and provenance attestations. The
v0.47.17 certification remains a required regression gate for later releases.
