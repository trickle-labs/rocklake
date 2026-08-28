# v0.47.17 Production Failure Certification

This report records the v0.47.17 production-boundary certification gate. The
gate checks that injected failures leave no partially committed catalog state,
that reopening the real SlateDB catalog preserves committed values, and that
unsupported or failed operations return errors instead of empty success.

## Version matrix

| Component | Version |
|---|---|
| RockLake | v0.47.17 |
| Rust toolchain | rustc 1.98.0 / cargo 1.98.0 |
| SlateDB | 0.13.1 |
| DuckDB | 1.5.3 |
| DuckLake | 1.0 |
| Catalog format | 1 / DuckLake catalog version 7 |
| Local backend | `object_store::local::LocalFileSystem` |
| MinIO | Testcontainers-backed suite |
| GCS | `fsouza/fake-gcs-server:latest` |
| Azure | `mcr.microsoft.com/azure-storage/azurite:latest` |

## Required checks

The production failure suite runs with one test thread because failpoints are
deliberately process-global and must not leak between scenarios:

```bash
cargo test -p rocklake-catalog --test v04717_production_failure_tests -- --test-threads=1 --nocapture
cargo test -p rocklake-catalog --test backend_compat -- --test-threads=1
cargo test -p rocklake-catalog --test backend_compat --features gcs-emulator -- --test-threads=1 --nocapture
cargo test -p rocklake-catalog --test backend_compat --features azure-emulator -- --test-threads=1 --nocapture
cargo test -p rocklake-ffi --target x86_64-unknown-linux-gnu
cargo miri test -p rocklake-core
```

The GCS and Azure commands are executed by `.github/workflows/emulator-tests.yml`.
Sanitizer and Miri commands are executed by `.github/workflows/sanitizers.yml`.

## Scenarios and injected failures

| Scenario | Failure boundary or assertion |
|---|---|
| Snapshot rollback | before SlateDB commit; reopen has no staged schema |
| Data-file registration | after object creation and between primary/index staging; no partial file row |
| Overlapping writers | stale epoch commit rejected; current writer’s exact row survives |
| Checkpoint lifecycle | checkpoint create and restore commit failures leave state unchanged |
| Export/import | import commit failure leaves target empty; retry preserves exact table/column rows |
| Historical reads | earlier snapshot returns only earlier values after a later commit |
| Close lifecycle | close errors propagate and the catalog can be reopened |

The exact value-level wire assertion is in
`v04717_ducklake_value_conformance_tests.rs`; it checks column order, wire type,
null positions, and the DuckLake 1.0 snapshot-change values.

## Release gates

- Every production failpoint is followed by a forced reopen, catalog invariant
  verification, and value-level assertions.
- Rollback, overlapping writers, checkpoint restore, migration/import, cleanup,
  reader startup, and historical snapshot scenarios are non-vacuous.
- LocalFS, MinIO, GCS, and Azure use the same nested-prefix and lifecycle
  assertions when their external services are available.
- A required job fails when an emulator or sanitizer cannot execute; build-only
  emulator coverage is not certification evidence.
- Certification is rejected for any invariant violation or silent wrong result.

## Residual limitations

The emulator and sanitizer rows require Docker and the corresponding CI
toolchains. They are required pre-release jobs and are not represented as
passing until the jobs execute successfully on the release candidate.
