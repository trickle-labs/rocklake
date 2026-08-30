# RockLake roadmap

**Current release:** v0.48.0

RockLake is a DuckLake 1.0 catalog sidecar backed by SlateDB and exposed over
the PostgreSQL wire protocol. The supported product path is the `rocklake`
binary with local or cloud object storage.

## Now

- Keep the local binary + DuckDB quickstart passing in CI.
- Keep DuckLake 1.0 compatibility claims tied to executable tests.
- Preserve the v0.47.17 production-failure certification gate.
- Keep CLI, TLS, authentication, storage, and operational docs aligned with
  the typed command surface.

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

- `scripts/quickstart.sh` passes locally and in the release CI gate.
- DuckLake 1.0 support is stated only for versions covered by live tests.
- Unsupported interfaces are removed from product documentation.
- Every release keeps the v0.47.17 certification job green.
