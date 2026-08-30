# RockLake roadmap

**Current release:** v0.49.0

RockLake is a DuckLake 1.0 catalog sidecar backed by SlateDB and exposed over
the PostgreSQL wire protocol. The supported product path is the `rocklake`
binary with local or cloud object storage.

## Release track

Detailed plans for the release sequence are in
[rocklake-roadmap-proposal.md](rocklake-roadmap-proposal.md).

- [v0.48.0: Surface reduction and product truthfulness](rocklake-roadmap-proposal.md#5-v0480--surface-reduction--product-truthfulness)
- [v0.49.0: Secure-by-default runtime and release integrity](rocklake-roadmap-proposal.md#6-v0490--secure-by-default-runtime--release-integrity)
- [v0.50.0: Operational UX and deployment simplicity](rocklake-roadmap-proposal.md#7-v0500--operational-ux--deployment-simplicity)
- [v0.51.0: Bounded scale, streaming and observability](rocklake-roadmap-proposal.md#8-v0510--bounded-scale-streaming--observability)
- [v0.52.x: Real-cloud validation and maintenance](rocklake-roadmap-proposal.md#9-v052x--real-cloud-validation--maintenance)

## Now

- Keep the default listener on loopback and authenticated startup on
  SCRAM-SHA-256.
- Keep the complete release-certification workflow required for tagged builds.
- Preserve the v0.47.17 production-failure certification gate.
- Keep secret handling, advisory exceptions, and release provenance documented
  and reviewable.

## Next

- Improve startup/readiness diagnostics without changing the catalog model.
- Add operator output only where it answers a demonstrated automation need.
- Measure large-catalog latency and memory before adding caching or streaming.

## Later

- Snapshot-aware pagination and bounded result streaming, if measurements
  justify them.
- Additional deployment integrations only with a maintained build and test
  path.

## Non-goals

- A general PostgreSQL implementation.
- A native DuckDB extension.
- A published Docker image, configuration-file format, or mTLS feature without
  executable implementation and CI coverage.
- New language bindings or engines without a named user and maintainer.

## Acceptance criteria

- `scripts/quickstart.sh` passes locally and in the release certification gate.
- The default bind is `127.0.0.1:5432`; public exposure is explicit.
- Authenticated release-binary startup uses SCRAM-SHA-256.
- The release workflow certifies and builds the exact tagged SHA.
- Every release keeps the v0.47.17 certification job green.
