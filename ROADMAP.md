# RockLake roadmap

- **Status:** Active
- **Current release:** v0.51.0
- **Planning horizon:** v0.51.1 through v0.53.x
- **v1.0:** Deferred intentionally

RockLake is an object-store-native DuckLake catalog. It removes the need for a
separate stateful catalog database. The supported product path is the
`rocklake` binary, the PostgreSQL wire protocol, and DuckDB DuckLake. RockLake
uses immutable snapshot history, one coordinated writer, and horizontally
scalable readers.

The current objective is:

> **Make RockLake a boring, measurable, operationally truthful DuckLake catalog
> appliance.**

## Current assessment

v0.51.0 is the start of an evidence and consolidation phase. It delivered
bounded-read mechanisms, but it did not prove bounded scale across the catalog.

The release preserved the right design choices:

- Data-file continuation tokens bind to a snapshot, table, and page size.
- Data-file streaming is pull-based and holds at most one decoded row.
- The legacy materializing API remains available beside bounded alternatives.
- `ReadOnlyCatalog` opens without a writer epoch and creates snapshot-bound
  readers without writer coordination.
- Tagged releases build from the exact tagged SHA and include binaries,
  checksums, build metadata, and an SBOM.
- The v0.51.0 release workflow completed its 23-job certification suite.

The current maturity profile is:

| Area | Assessment |
|---|---|
| Core architecture | Strong. Preserve the current model. |
| Catalog correctness and recovery | Strong, including snapshot, failure, restore, and conformance coverage. |
| Release engineering | Very strong and the most mature part of the project. |
| Large-catalog behavior | Promising, but not demonstrated. |
| Runtime observability | Broad, but not yet trustworthy enough for operations. |
| Operator ergonomics | A good foundation with too many top-level choices. |
| Rust client ergonomics | Useful but inconsistent, especially for read-only access. |
| Product positioning | Broader than the supported binary and PG-wire path. |

This assessment comes from the v0.51.0 tagged source, release artifacts, tests,
benchmark code, documentation, and repository history. It does not include an
independent real-cloud load or soak test.

## Roadmap rules

- Preserve the object-store-native, single-writer, many-reader architecture.
- Correct existing metrics and limits before adding catalog features.
- Treat a metric as supported only when production code records it and a test
  validates its meaning.
- Treat a limit as supported only when the server enforces it and a black-box
  test exercises it.
- Apply bounded operations according to expected cardinality, not one SQL
  statement type.
- Publish raw, reproducible evidence before making scale claims.
- Keep compatibility promises for specific interfaces even while RockLake
  remains at `0.x`.
- Add features only for a named workload and maintainer.

## Release plan

| Release | Theme | Exit condition |
|---|---|---|
| v0.51.1 | Operational truth | Existing metrics, limits, admission, and drain behavior match their names. |
| v0.51.2 | Boundedness truth | Every high-cardinality catalog path has a bounded contract or an explicit limit. |
| v0.51.3 | Client and operator ergonomics | Read clients share one model, and the binary has one clear install and operation path. |
| v0.52.0 | Reproducible local and MinIO baseline | Fresh-process results cover catalogs with up to one million files. |
| v0.52.1 | AWS S3 validation | Published AWS results cover reader scale, recovery, failures, and cost. |
| v0.52.2 | Multi-node soak | A 24-hour workload preserves performance and catalog invariants. |
| v0.52.3 and later | Other clouds and maintenance | Backends graduate according to demand and reproducible evidence. |
| v0.53.x | Consolidation | PG-wire and metadata streaming have fewer, explicit responsibilities. |

## v0.51.1: Operational truth

This is a focused correction release. Do not bundle new catalog features into
it.

### Metrics and tracing

- Fix PostgreSQL query histogram exposition. Either store non-cumulative
  buckets and accumulate once while rendering, or store cumulative buckets and
  render them directly.
- Parse the Prometheus exposition in tests and verify these invariants:

  ```text
  bucket[n] <= bucket[n+1]
  bucket[+Inf] == count
  sum >= 0
  ```

- Start the request clock at the beginning of `do_query`, before
  classification and execution.
- Record admission, classification, catalog or executor work, response
  encoding, time to first socket write, and total request duration as distinct
  phases.
- Use `connection_id` for the socket lifetime and a new `query_id` for every
  simple or extended query execution.
- Carry `query_id` through classification, execution, catalog operations,
  errors, slow logs, and object-store spans.
- Report connection lifetime as a debug event. Do not classify a healthy
  long-lived connection as a slow operation when it closes.

### Limits and backpressure

- Implement `idle_connection_timeout` as an inactivity timer in the server, or
  remove the option until the server can enforce it.
- Remove `stream_queue_depth` while the stream remains pull-based. Add it only
  if measurements justify a queue.
- Remove `max_buffered_rows`, or measure and limit the rows that are actually
  queued.
- Replace the overloaded response-byte limit with two policies:

  ```text
  max_in_flight_response_bytes
      Limits process memory and transport queues.

  max_result_bytes
      Optionally limits the total result as a workload or abuse policy.
  ```

- Disable `max_result_bytes` or set it much higher for trusted local use.
- Increment `stream_backpressure_total` only for measured wait or saturation
  events. Remove it if the runtime has no such event.

### Admission, sessions, and shutdown

- Reserve session capacity before creating a long-lived connection task.
- If capacity is unavailable, return PostgreSQL error `53300` immediately.
  A bounded admission queue is also acceptable if it has a depth metric and a
  wait timeout.
- Replace overlapping session gauges with:

  ```text
  connections_open
  connections_idle
  queries_in_flight
  ```

- During shutdown, stop accepting connections, reject new queries, drain
  `queries_in_flight`, and close idle sockets.

### Release criteria

- Black-box socket tests cover the idle timeout, overload response, query
  drain, and idle-connection shutdown.
- Prometheus tests parse the exposition instead of matching one bucket string.
- Whole-request timing includes materialized catalog and executor work.
- Each configured limit has a production enforcement path and an operator
  action. Remove settings that do not meet both conditions.
- Add a root `COMPATIBILITY.md` that defines the contracts listed in
  [Compatibility before v1.0](#compatibility-before-v10).

## v0.51.2: Boundedness truth

Data files were the first bounded metadata family. The next pass must cover the
catalog as a whole.

### Ordering contract

Choose and document one data-file ordering contract. The bounded APIs can use
`file_order`, an explicit `DataFileOrder`, or documented snapshot-index order.
Callers must not rely on an unspecified order.

Add a fixture with deliberately non-monotonic `file_order` values and verify:

```text
collect(stream)
== concatenate(all pages)
== legacy list under the documented ordering
```

Run the parity check through the Rust APIs, simple query, extended query, and
`COPY`.

### High-cardinality operations

Inventory every API whose result can grow with files, columns, snapshots, or
tables. Prioritize these paths:

1. File-column statistics, which can grow with files multiplied by columns.
2. Delete files, with a table-scoped index that avoids a global prefix scan and
   per-row data-file lookups.
3. Snapshot changes and history.
4. Partition and mapping metadata.
5. Export, verification, and diagnostic operations.

Provide one common scan contract that carries the operation kind, snapshot ID,
and estimated or known row count with its stream. Apply admission control to
all high-cardinality operations, not only `SelectDataFiles` statements.

### Release criteria

- Stream or page file-column statistics and delete files.
- Document every remaining `list_all_*` or materializing API with its explicit
  bound or bounded alternative.
- Test a slow consumer and a client that disconnects after one row.
- Test cancellation while a query waits for an admission permit.
- Inject an object-store failure mid-stream and return an error instead of a
  truncated success.
- Verify that cancellation releases admission permits and catalog handles.
- Stream a valid result larger than 16 MiB without exceeding the in-flight
  memory budget.

## v0.51.3: Client and operator ergonomics

### One read model

- Define a common read interface for schemas, tables, snapshots, data-file
  pages, and data-file streams.
- Adapt writer-backed, read-only, asynchronous, and synchronous clients to one
  implementation.
- In `ReadOnlyClient`, hold the mutable catalog lock only long enough to create
  a snapshot-bound `CatalogReader`. Do not hold it across a scan.
- Replace the `try_lock()` snapshot accessor with an asynchronous accessor that
  returns `Result<Option<SnapshotId>>`, or an equivalent atomically stored
  state. Do not treat lock contention as a missing snapshot.
- Expose the data-file fields required by supported integrations. If the model
  remains narrow, label the high-level Rust client as Preview.
- Introduce typed catalog-location, credential, and encryption configuration.

### Support levels

Publish and maintain this initial support matrix:

| Level | Interfaces |
|---|---|
| Supported | `rocklake` binary, PG-wire, and DuckDB DuckLake. |
| Preview | Rust client, read-only API, and DataFusion integration. |
| Experimental | Language bindings and engine integrations without a maintained certification path. |
| Internal | Corpus, repair internals, and implementation-level exports. |

### CLI structure

Group commands without breaking existing scripts:

```text
rocklake serve
rocklake doctor
rocklake status

rocklake catalog backup
rocklake catalog restore
rocklake catalog verify
rocklake catalog repair
rocklake catalog gc
rocklake catalog excise
rocklake catalog checkpoint
rocklake catalog migrate

rocklake debug corpus
rocklake debug inspect
rocklake debug export
```

Keep old spellings as hidden or deprecated aliases for at least two minor
releases. Separate command responsibilities:

- `doctor` checks configuration and connectivity before startup.
- `status` reports cheap live operational state.
- `verify` checks durable catalog invariants.
- `diagnose` collects status and verification evidence for an incident.
- `inspect` provides low-level human debugging and is not a routine operation.

### Installation and documentation

- Make release binaries the primary README installation path.
- Show checksum verification before startup.
- Lead the first run through `rocklake doctor` and `rocklake serve`.
- Generate one canonical DuckDB `ATTACH` statement from the executable and test
  that exact syntax in the quickstart.
- Print one concise, redacted startup block:

  ```text
  Catalog:       s3://bucket/catalogs/warehouse-a
  Mode:          writer
  Listener:      127.0.0.1:5432
  TLS:           disabled, loopback only
  Metrics:       http://127.0.0.1:9090/metrics
  DuckDB attach: ATTACH 'ducklake:postgres:...' AS lake (...);
  ```

- Move historical assessments, blueprints, and specification-gap reports under
  a clearly named historical directory. Keep this file as the only live
  roadmap and maintain one compatibility document.

## v0.52.x: Reproducible evidence

The v0.51 Criterion benchmark is instrumentation code, not scale evidence. Its
process peak RSS value spans repeated in-process iterations, has no baseline,
and does not enforce a bound. Do not use it to support scale claims.

Every v0.52 benchmark must run each scenario in a fresh child process. Record
the baseline RSS before catalog open, peak RSS, and RSS after close. Separate
cold and warm runs. Record object-store operation counts and bytes. Commit raw
machine-readable output with the CPU, memory, operating system, object-store
backend, dependency versions, and exact Git SHA.

### v0.52.0: Local and MinIO baseline

Test:

- 10,000, 100,000, and 1,000,000 files.
- Cold and warm catalog opens.
- Legacy, page, and stream traversal.
- 1, 4, and 16 readers.
- Slow consumption and early disconnect.
- Snapshot history and high-cardinality statistics.
- Peak RSS in fresh processes.
- Object-store requests and bytes.

Commit raw JSON results and an environment manifest.

### v0.52.1: AWS S3 validation

Test one writer with 1, 4, and 16 readers. Include writer replacement, reader
restarts, backup and restore, throttling, transient network errors, and a
representative catalog with historical snapshots.

Publish the region, instance type, bucket class, exact Git SHA, SlateDB
version, latency percentiles, RSS, request counts, bytes, and estimated cost.

### v0.52.2: Multi-node soak

Run a 24-hour workload with:

- ongoing commits;
- latest and historical reads;
- a writer restart;
- reader churn;
- slow clients;
- backup and verification;
- object-store fault injection;
- retention and checkpoint operations.

The run passes only if performance remains stable and catalog invariants hold.

### v0.52.3 and later: Other clouds and maintenance

Graduate GCS and Azure according to demand and reproducible evidence. Label
them Preview until they pass the same evidence standard as AWS.

Take dependency and SlateDB upgrades in this series only when each upgrade
reruns the recovery and real-cloud baselines.

## v0.53.x: Consolidation

Split the PG-wire handler into explicit responsibilities:

```text
ConnectionContext
RequestContext
AdmissionController
QueryExecutor
ObservedResponse
ServerLimits
```

Each configured limit must reach the component that enforces it. Build one
generic metadata streaming and encoding path instead of adding a response
stream for each catalog table.

## Multi-tenancy

Do not add tenant IDs to the shared RockLake keyspace. That design would change
key encoding, writer fencing, retention, backup, verification, metrics, and
every catalog API.

After at least two users need multiple logical catalogs, add a catalog router:

```text
PG database or catalog alias
        ↓
independent CatalogLocation
        ↓
independent object-store prefix
        ↓
independent writer epoch, retention, backup, and quotas
```

Require per-catalog authentication, connection limits, and metrics. Keep shared
cross-catalog transactions out of scope until a concrete workload requires
them.

## Compatibility before v1.0

Deferring v1.0 does not defer compatibility promises. `COMPATIBILITY.md` must
define these contracts:

1. On-object catalog format versions and the releases that can read or write
   them.
2. Tested DuckDB and DuckLake versions, plus the policy for new upstream
   releases.
3. CLI and configuration deprecations, with a window of at least two minor
   releases.
4. Backup and export restoration guarantees and forward compatibility.
5. Metrics names that are stable enough for dashboards and alerts.
6. Supported and Preview Rust modules, with their breaking-change policies.
7. Release lines that receive correctness or security fixes.

RockLake can remain at `0.x` and provide production support. Stability attaches
to these contracts, not the leading version digit.

## Product claims

Use these claims until published evidence supports stronger wording:

- "An object-store-native DuckLake catalog. No separate stateful catalog
  database."
- "Horizontal readers without writer coordination."
- "Bounded-read mechanisms are available for data-file metadata."

Do not claim an unbounded number of reader replicas or imply that RockLake
replaces query, ingestion, stream processing, orchestration, or data-plane
components.

## Non-goals

Do not prioritize:

- distributed multi-writer operation;
- speculative caching;
- a native DuckDB extension;
- more language bindings;
- new deployment platforms;
- a shared-keyspace multi-tenant redesign;
- a general PostgreSQL implementation.

Do not add another limit, metric, or CLI command unless it has a production
producer or enforcement path, a black-box test, and a documented operator
action.

## Permanent release gates

- `scripts/quickstart.sh` passes locally and in release certification.
- The default bind is `127.0.0.1:5432`; public exposure is explicit.
- Authenticated release-binary startup uses SCRAM-SHA-256.
- The release workflow certifies and builds the exact tagged SHA.
- Tagged artifacts include binaries, checksums, build metadata, and an SBOM.
- Every release keeps the v0.47.17 production-failure certification job green.
- Patch trains include a field-observation window and explicit operational exit
  criteria before the next thematic minor release.
