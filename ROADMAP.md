# RockLake roadmap

- **Status:** Active
- **Current release:** v0.51.2
- **Planning horizon:** v0.51.2 through v0.53.x
- **v1.0:** Deferred intentionally

RockLake is an object-store-native DuckLake catalog. The supported product path
is the `rocklake` binary, the PostgreSQL wire protocol, and DuckDB DuckLake.
RockLake keeps immutable snapshot history, one coordinated writer, and
scalable readers.

The current objective is to make RockLake a boring, measurable, operationally
truthful DuckLake catalog appliance.

## Current state

v0.51.0 started the evidence and consolidation phase. It added bounded-read
mechanisms for data-file metadata, but it did not prove bounded scale across
the catalog.

The [v0.51.0 assessment](docs/assessments/v0.51.md) records the evidence and
open findings. The release preserved the object-store-native, single-writer,
many-reader model and completed its 23-job certification suite.

## Roadmap rules

- Preserve the object-store-native, single-writer, many-reader architecture.
- Correct existing metrics and limits before adding catalog features.
- Support a metric only when production code records it and a test validates
  its meaning.
- Support a limit only when the server enforces it and a black-box test
  exercises it.
- Define boundedness by operation cardinality, not by SQL statement type.
- Publish raw, reproducible evidence before making scale claims.
- Keep compatibility promises for specific interfaces while RockLake remains at
  `0.x`.
- Add features only for a named workload and maintainer.

## Release sequence

The v0.51.x releases are a patch train. Each patch changes one failure domain.

| Release | Theme | Exit condition |
|---|---|---|
| v0.51.1 | Telemetry correctness | Metrics and request timing describe the work the server performed. |
| **v0.51.2** | Connection lifecycle | Admission, session state, idle timeout, and shutdown have enforced contracts. |
| v0.51.3 | Bounded metadata | High-cardinality operations stream or page results within documented limits. |
| v0.51.4 | Client and operator ergonomics | The binary, client APIs, installation path, and operator tasks have one clear path. |
| v0.52.0 | Local and MinIO evidence | Fresh-process results cover catalogs with up to one million files. |
| v0.52.1 | AWS S3 evidence | Published AWS results cover reader scale, recovery, failures, and cost. |
| v0.52.2 | Multi-node soak | A 24-hour workload preserves performance and catalog invariants. |
| v0.52.3 and later | Other clouds and maintenance | Backends and material dependency changes graduate through the evidence gates. |
| v0.53.x | Consolidation | PG-wire lifecycle and metadata streaming have explicit ownership boundaries. |

Detailed work and black-box coverage live in the [v0.51.1 plan](plans/v0.51.1.md),
[v0.51.2 plan](plans/v0.51.2.md), [v0.51.3 plan](plans/v0.51.3.md), and
[v0.51.4 plan](plans/v0.51.4.md).

## v0.52.x: Reproducible evidence

Run each benchmark scenario in a fresh child process. Record baseline, peak,
and post-close RSS, cold and warm results, object-store requests and bytes,
the environment, dependency versions, and the exact Git SHA. Commit raw,
machine-readable output.

Test catalogs with 10,000, 100,000, and 1,000,000 files. Include legacy,
page, and stream traversal; 1, 4, and 16 readers; slow consumers; early
disconnects; snapshot history; high-cardinality statistics; and fresh-process
memory measurements.

AWS validation adds writer replacement, reader restarts, backup and restore,
throttling, transient network errors, latency percentiles, and estimated cost.
The multi-node soak adds ongoing commits, reader churn, slow clients, backup,
verification, retention, checkpoint operations, and object-store fault
injection.

### Backend evidence

Functional support, recovery certification, and scale certification are separate
claims.

| Backend | Functional support | Recovery certified | Scale certified |
|---|---:|---:|---:|
| Local filesystem | Yes | Yes | Pending v0.52.0 |
| MinIO or S3-compatible | Yes | Yes | Pending v0.52.0 |
| AWS S3 | Yes | Yes | Pending v0.52.1 |
| Google Cloud Storage | Yes | Yes | Not yet |
| Azure Blob Storage | Yes | Yes | Not yet |

GCS and Azure can remain functionally supported while their scale evidence is
pending. Update this table when a backend passes a release gate.

## v0.53.x: Consolidation

Remove duplication from the request lifecycle and build one generic metadata
streaming and encoding path. Introduce explicit ownership boundaries for
connection state, request state, admission, execution, response observation,
and server limits. The implementation details belong in the release plan.

## Bounded operation classes

Boundedness has different meanings for different operations:

| Class | Contract |
|---|---|
| Interactive | Stream or page results, support cancellation, and obey admission limits. |
| Administrative online | Stream progress, remain memory-bounded, and allow longer controlled work. |
| Offline or rebuild | May scan the full catalog, but stays memory-bounded, reports cost, and is restartable where practical. |

Row-count estimates are optional. An estimate must not force a full scan before
the first result.

## Compatibility and support

RockLake makes compatibility promises before v1.0. The root
[`COMPATIBILITY.md`](COMPATIBILITY.md) defines the contracts for catalog
formats, DuckDB and DuckLake versions, CLI and configuration deprecations,
backups, metrics, Rust modules, and supported release lines.

Every thematic release includes this upgrade check:

- A previous supported release creates a catalog.
- The current release opens, reads, writes, and reopens that catalog.
- A backup from the previous release restores under the current release.
- An incompatible downgrade fails clearly before it can corrupt state.

Correctness and security hotfixes do not wait for a field-observation window.
Thematic minor releases require an observation window and explicit operational
exit criteria before the next thematic minor release.

Security and correctness dependency updates are allowed in any release. SlateDB,
object-store, serialization, and other material dependency changes also rerun
the recovery and real-cloud baselines.

## Multi-tenancy

Do not add tenant IDs to the shared RockLake keyspace. The decision and its
rejected alternative are recorded in the [catalog routing ADR](docs/adr/catalog-routing.md).

If at least two users need multiple logical catalogs, add a router that gives
each catalog its own location, object-store prefix, writer epoch, retention,
backup, quotas, authentication, connection limits, and metrics. Keep shared
cross-catalog transactions out of scope until a named workload requires them.

## Product claims

Use these claims until stronger evidence is published:

- RockLake is an object-store-native DuckLake catalog with no separate
  stateful catalog database.
- Readers scale horizontally without writer coordination.
- Bounded-read APIs are available for data-file metadata.

Do not claim an unbounded number of reader replicas. RockLake does not replace
query, ingestion, stream-processing, orchestration, or data-plane components.

## Non-goals

Do not prioritize distributed multi-writer operation, speculative caching, a
native DuckDB extension, more language bindings, new deployment platforms, a
shared-keyspace multi-tenant redesign, or a general PostgreSQL implementation.

Do not add a limit, metric, or CLI command without a production enforcement or
recording path, a black-box test, and a documented operator action.

## Permanent release gates

- `scripts/quickstart.sh` passes locally and in release certification.
- The default bind is `127.0.0.1:5432`; public exposure is explicit.
- Authenticated release-binary startup uses SCRAM-SHA-256.
- The release workflow certifies and builds the exact tagged SHA.
- Tagged artifacts include binaries, checksums, build metadata, and an SBOM.
- Every release keeps the v0.47.17 production-failure certification job green.
