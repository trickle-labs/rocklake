# RockLake pre-1.0 roadmap and implementation plan

- **Status:** Proposal
- **Baseline:** RockLake v0.51.4
- **Prepared:** 2026-09-02
- **Intended repository path:** `plans/pre-1.0-roadmap.md`
- **Planning model:** Gate-based, not date-based
- **Stable target:** v1.0.0

> This document defines every planned thematic release after v0.51.4 and before
> v1.0.0. Unscheduled patch releases remain available for security,
> correctness, compatibility, and release-engineering fixes, but they do not
> acquire independent feature scope.

## 1. Purpose

RockLake has reached the point where the route to v1.0 should be driven less by
feature count and more by evidence, isolation, operability, compatibility, and
field observation. The core architecture is already coherent: an
object-store-native DuckLake catalog, immutable snapshot history, one
coordinated writer per catalog, and horizontally scalable readers. The
remaining work is to prove that architecture at scale, simplify the internal
request path, add safe multi-catalog service operation, and freeze a supportable
public product.

This roadmap proposes the full sequence from v0.51.5 through the v1.0 release
candidates. It intentionally places multi-tenancy after the current evidence
and consolidation work. Multi-tenancy is implemented as **routing to independent
catalogs**, not by adding a tenant identifier to every key in one shared
catalog.

The proposal is based on the current repository direction recorded in:

- [`ROADMAP.md`](../ROADMAP.md)
- [`docs/assessments/v0.51.md`](../docs/assessments/v0.51.md)
- [`plans/v0.51.4.md`](v0.51.4.md)
- [`docs/adr/catalog-routing.md`](../docs/adr/catalog-routing.md)
- [`COMPATIBILITY.md`](../COMPATIBILITY.md)
- [Issue #92: Multi-tenancy support](https://github.com/trickle-labs/rocklake/issues/92)

## 2. Executive decision

The recommended pre-1.0 sequence is:

1. Repair the binary distribution contract.
2. Produce reproducible local, MinIO, AWS, multi-node, GCS, and Azure evidence.
3. Consolidate the request lifecycle and bounded administrative operations.
4. Add a multi-catalog router over independent catalog locations.
5. Add a managed catalog registry, lifecycle operations, authorization, quotas,
   and isolation certification.
6. Add safe writer ownership and multi-node routing without introducing
   distributed multi-writer semantics.
7. Complete disaster-recovery, security, migration, performance, public-surface,
   and field-validation gates.
8. Enter release-candidate mode with no new features or format changes.

The target v1.0 product is a **boring, measurable, recoverable DuckLake catalog
appliance** that can run either one catalog or many independently isolated
catalogs behind one service endpoint.

## 3. v1.0 product definition

### 3.1 Supported product path

The v1.0 supported path should be limited to:

- The `rocklake` binary.
- PostgreSQL wire protocol as emitted by the named supported DuckDB/DuckLake
  versions.
- Single-catalog operation.
- Multi-catalog routing where every catalog has an independent object-store
  location, writer epoch, retention floor, backup history, limits, and metrics.
- Local filesystem for development and certification.
- MinIO or an explicitly certified S3-compatible service.
- AWS S3.
- GCS and Azure only if their real-cloud evidence gates pass before the public
  surface freeze.

The Rust client, read-only API, DataFusion integration, and language bindings
must retain explicit support levels. They do not automatically become Supported
because the binary reaches v1.0.

### 3.2 Target validation envelope

These are **validation targets**, not claims. The v1.0 documentation must
publish only the envelope actually demonstrated by the evidence releases.

- Catalogs containing 10,000, 100,000, and 1,000,000 visible data-file records.
- One coordinated writer per catalog.
- Reader tests at 1, 4, and 16 concurrent reader processes.
- Multi-catalog tests with 1,000 registered catalogs, at least 100 concurrently
  active catalogs, and independent writes to at least 20 catalogs.
- Slow consumers, early disconnects, cancellation, restart, throttling, and
  transient object-store faults.
- Backup, restore, verification, retention, checkpoint, and maintenance activity
  during sustained reads and writes.
- A minimum 30-day design-partner observation window before the first stable
  release.

Any target that cannot be demonstrated becomes a documented lower limit or a
Preview capability. The target is never converted into a claim merely because
it appears in this roadmap.

### 3.3 Permanent architectural constraints

The following constraints remain in force through v1.0:

- One coordinated writer per catalog.
- No tenant ID in the DuckLake/RockLake catalog keyspace.
- No shared cross-catalog transaction.
- No general PostgreSQL implementation.
- No query engine, ingestion engine, scheduler, or data-plane execution layer.
- No speculative cache whose correctness depends on wall-clock freshness.
- No new public metric without a production recording path and a semantic test.
- No new limit without enforcement and a black-box rejection test.
- No scale claim without raw, reproducible evidence.
- No backend graduation based only on an emulator.

### 3.4 Explicit non-goals before v1.0

- Distributed multi-writer operation within one catalog.
- Shared-keyspace multi-tenancy.
- Cross-catalog joins or atomic commits.
- A public remote catalog-management REST API.
- A native DuckDB extension.
- Additional language bindings without a named maintainer and certification
  path.
- Automatic physical deletion as part of normal visibility GC.
- A promise that an arbitrary S3-compatible implementation behaves like a
  certified backend.

## 4. Versioning and release policy

### 4.1 Planned versions

The versions in this document are the complete planned thematic sequence.
Patch versions not listed here are fix-only. A fix release may be cut at any
point for:

- A security vulnerability.
- A catalog-correctness or recovery defect.
- A supported DuckDB/DuckLake compatibility regression.
- A release-artifact or installation defect.
- A platform regression in an already supported target.

A fix release must not silently introduce a new public feature, catalog format,
configuration family, or compatibility promise.

### 4.2 Release observation rule

A thematic release may begin implementation while the previous release is
being observed, but it may not be declared complete until:

- Its own exit criteria pass.
- The previous release has no unresolved correctness or security regression
  that invalidates the new baseline.
- Its raw evidence and operator documentation are committed.
- Upgrade and restore tests from the previous supported baseline pass.

### 4.3 Permanent release gates

Every release from v0.51.5 onward must retain these gates:

- Format, Clippy, and workspace tests with warnings denied.
- The v0.47.17 production-failure certification suite.
- DuckLake wire-corpus and live-client conformance for named supported versions.
- Exact tagged-SHA certification.
- Security policy checks and sanitizer coverage.
- Strict documentation build.
- Executable quickstart from the **built release artifact**, not from
  `cargo run` or a locally rebuilt binary.
- Previous-version open/read/write/reopen coverage.
- Previous-version backup restore under the new release.
- Clear rejection of an unsafe downgrade.
- Checksums, build metadata, SBOM, and provenance for every released binary.
- A machine-readable release manifest listing every asset and digest.

Material changes to SlateDB, `object_store`, serialization, encryption, key
encoding, the PG-wire library, or the Rust toolchain rerun the relevant recovery
and real-cloud baselines before release.

## 5. Proposed release sequence

| Version | Theme | Principal exit condition |
|---|---|---|
| **v0.51.5** | Distribution correctness | Published installation instructions execute successfully against the exact release assets on every supported platform. |
| **v0.52.0** | Local and MinIO evidence | Fresh-process evidence covers 10k, 100k, and 1M-file catalogs without hidden materialization or unbounded memory growth. |
| **v0.52.1** | AWS S3 evidence | Real-S3 results cover performance, recovery, reader scale, failures, request volume, bytes, and cost. |
| **v0.52.2** | Multi-node soak | A 24-hour workload preserves catalog invariants and avoids progressive resource or latency degradation. |
| **v0.52.3** | GCS, Azure, and evidence closure | Each cloud has an independent evidence decision, and all scale/support claims are reconciled with published results. |
| **v0.53.0** | Request lifecycle consolidation | Connection, request, admission, execution, cancellation, and response observation have explicit ownership boundaries. |
| **v0.53.1** | Unified metadata streaming | All high-cardinality PG-wire metadata responses use one bounded, cancellation-safe encoding pipeline. |
| **v0.53.2** | Bounded administrative jobs | Long-running catalog operations expose resumable progress and bounded resource contracts. |
| **v0.54.0** | Static multi-catalog router | One binary safely routes PostgreSQL database names to independent configured catalog locations. |
| **v0.55.0** | Managed catalog registry | Catalog identity and lifecycle are managed through a versioned object-store-backed registry. |
| **v0.56.0** | Multi-tenant authorization and isolation | Multiple principals, per-catalog grants, quotas, metrics, and isolation tests make the router supportable. |
| **v0.57.0** | Writer availability and multi-node routing | A catalog has one authoritative writer owner, safe handoff, read-only replicas, and tested failure routing. |
| **v0.58.0** | Disaster recovery and maintenance | Backup, restore, verification, retention, and scheduled maintenance have measured recovery contracts. |
| **v0.59.0** | Security, secrets, audit, and governance | Secret rotation, encryption lifecycle, threat-model closure, auditability, and protected releases pass independent review. |
| **v0.60.0** | Compatibility and migration | Catalog, registry, backup, and upstream protocol migrations are restartable, tested, and frozen for v1.0. |
| **v0.61.0** | Performance and cost | Evidence-driven optimizations meet committed regression budgets and produce a supported capacity model. |
| **v0.62.0** | Public surface freeze | CLI, configuration, metrics, errors, artifacts, logs, and operator workflows are frozen and documented. |
| **v0.63.0** | Production beta | The feature-complete system enters design-partner production with no new feature work. |
| **v0.63.1** | Beta fixes and readiness audit | All release-blocking beta findings are closed and the complete v1.0 audit is published. |
| **v1.0.0-rc.1** | First release candidate | No unresolved P0/P1 finding, no format change, and the complete certification matrix passes. |
| **v1.0.0-rc.2** | Final release candidate | Only blocker fixes differ from RC1, and the full matrix and observation gate pass again. |

## 6. Critical path

```text
v0.51.5 distribution repair
        |
        v
v0.52.x evidence -------------------------------+
        |                                        |
        v                                        |
v0.53.x consolidation                            |
        |                                        |
        v                                        |
v0.54.0 static router -> v0.55.0 registry        |
        |                       |                |
        +-----------------------v----------------+
                                v
                     v0.56.0 auth and isolation
                                |
                                v
                     v0.57.0 availability
                                |
                                v
                     v0.58.0 operations and DR
                                |
                                v
                     v0.59.0 security closure
                                |
                                v
                     v0.60.0 migration freeze
                                |
                                v
                     v0.61.0 performance freeze
                                |
                                v
                     v0.62.0 public surface freeze
                                |
                                v
                     v0.63.x production beta
                                |
                                v
                   v1.0.0-rc.1 -> rc.2 -> v1.0.0
```

The multi-catalog work has an additional entry gate from the accepted routing
ADR: at least two named user workloads and a maintainer who owns routing,
authentication, and operational contracts. The architecture may be prepared
before that gate, but the feature must not be called Supported without those
workloads.

---

## 7. Detailed release plans

### v0.51.5 — Distribution correctness and release repair

#### Scope

Make the release binary path truthful and self-testing. Fold the immediate
post-v0.51.4 platform fixes into a certified patch release without changing the
catalog format or public catalog behavior.

#### User outcome

A user can copy the installation commands from the README or release page,
download the named assets and checksum files, install the binary, run
`rocklake --version`, execute `rocklake doctor`, start a local catalog, and run
the DuckDB quickstart.

#### Implementation plan

- [ ] Choose one canonical asset naming contract and record it in an ADR. The
      recommended contract is the existing raw binary names plus explicit
      `.sha256`, `.build-metadata.json`, one `SHA256SUMS`, one release manifest,
      and one SBOM.
- [ ] Make `README.md`, deployment documentation, release notes, and workflow
      output use those exact names.
- [ ] Ensure checksum examples download the checksum before verification.
- [ ] Add `release-manifest.json` containing release version, certified Git SHA,
      target triple, filename, byte length, SHA-256 digest, build metadata
      filename, SBOM filename, and provenance subject.
- [ ] Add an artifact-stage installation test that runs before publication and
      consumes only files produced by the build jobs.
- [ ] Add platform-specific install tests for Linux x86-64, Linux aarch64,
      macOS arm64, and Windows x86-64. Cross-built targets may use an execution
      emulator only when native execution is unavailable; the limitation must
      be explicit.
- [ ] Make `rocklake --version --output json` report the semantic version,
      certified SHA, target triple, Rust version, catalog read/write format,
      and build provenance availability.
- [ ] Include the post-v0.51.4 Windows stack-size and rejected-connection drain
      fixes in the patch release, with regression tests.
- [ ] Remove stale version text from generated configuration examples.
- [ ] Add a repository ruleset requiring release certification, compatibility,
      security, and artifact-install jobs before merge to `main`.
- [ ] Restrict tag creation and require a second human or delegated release
      approver for stable tags.

#### Test and evidence plan

- Run the artifact installation test in an empty temporary directory with no
  Rust toolchain and no repository checkout in `PATH`.
- Verify every digest in `SHA256SUMS` and `release-manifest.json`.
- Compare the executable's reported SHA to the tagged SHA.
- Run the local quickstart against the installed artifact.
- Assert that all documented download URLs correspond to generated filenames.
- Add a documentation test that extracts shell blocks from the README and
  validates asset references against the release manifest schema.

#### Exit conditions

- The exact README installation sequence succeeds on all supported targets.
- No source build is used by the executable quickstart gate.
- The release page, README, deployment guide, and manifest agree on every asset
  name and digest.
- Branch and tag protection are active or the release records a blocking
  governance exception with an owner and removal milestone.

#### Non-goals

- No catalog feature.
- No catalog-format or backup-format change.
- No new package manager or container distribution.

---

### v0.52.0 — Reproducible LocalFS and MinIO scale evidence

#### Scope

Turn the existing benchmark and bounded-read work into reproducible,
fresh-process evidence on LocalFS and MinIO. This release establishes the
measurement system used by all later scale, cost, and regression claims.

#### User outcome

An operator can see the tested catalog sizes, memory behavior, latency,
request counts, and failure characteristics for the development and
S3-compatible reference environments. The project can state a precise tested
envelope rather than a qualitative scale claim.

#### Implementation plan

##### Evidence runner

- [ ] Add an internal `rocklake-evidence` runner under `tools/` or a private
      workspace crate. It is not part of the supported product API.
- [ ] Run every scenario in a fresh child process.
- [ ] Produce a versioned JSON result schema and JSON Lines event stream.
- [ ] Record exact Git SHA, binary digest, OS, kernel, CPU, memory, filesystem,
      container image, Rust version, SlateDB version, `object_store` version,
      backend version, configuration, and dataset seed.
- [ ] Record baseline, peak, and post-close RSS; CPU time; wall time; time to
      first row; total time; rows; bytes; object-store operations; errors; and
      cancellation latency.
- [ ] Make workload generation deterministic and restartable.
- [ ] Keep raw results under `benchmarks/evidence/v0.52.0/` and generate a
      human-readable summary from those raw files.

##### Dataset matrix

- [ ] Generate 10,000, 100,000, and 1,000,000 visible data-file records.
- [ ] Include one-table/high-file-count, many-table, long snapshot history,
      high-cardinality file statistics, delete-file metadata, partition
      metadata, and mapping metadata variants.
- [ ] Include historical snapshots on both sides of a nonzero retention floor.
- [ ] Generate a stable correctness digest for every expected result set.

##### Operation matrix

- [ ] Measure catalog open, read-only open, refresh, `status`, schema/table
      listing, table description, snapshot listing, data-file page traversal,
      data-file stream traversal, the legacy materializing API, verification,
      export, backup, and restore planning.
- [ ] Exercise 1, 4, and 16 reader processes.
- [ ] Exercise slow consumers, early disconnects, cancellation while waiting
      for admission, and mid-stream object-store failures.
- [ ] Record the distinction between interactive, administrative-online, and
      offline operations.

##### Boundedness analysis

- [ ] Define an incremental-RSS calculation that subtracts the fresh-process
      baseline.
- [ ] Require page and stream paths to stay bounded by configured page/queue
      sizes rather than result cardinality.
- [ ] Treat the legacy materializing API as compatibility-only and publish its
      observed growth separately.
- [ ] Fail the gate when a supposedly bounded path performs a full pre-count or
      materializes the complete result before the first row.
- [ ] Commit numerical budgets before the final certification run; do not tune
      thresholds after seeing the final results.

#### Test and evidence plan

- Run on pinned dedicated hardware in addition to ordinary CI.
- Repeat each cold scenario enough times to publish dispersion, not just one
  number.
- Verify that every returned row set matches its deterministic digest.
- Kill the consumer after the first row and prove that scan permits, catalog
  handles, tasks, and buffers return to baseline.
- Inject MinIO delay, timeout, connection reset, and 503 responses.
- Reopen the catalog after every failure class and run full verification.

#### Exit conditions

- Raw LocalFS and MinIO results are committed and reproducible from a documented
  command.
- Bounded page and stream paths show no result-cardinality-driven memory slope
  outside the committed tolerance.
- A one-million-file catalog completes the required read and administrative
  scenarios without OOM, truncation, invariant failure, or leaked permits.
- Product claims and compatibility tables are updated to the demonstrated
  envelope only.

#### Non-goals

- No performance optimization solely to improve one benchmark before the
  measurement contract is frozen.
- No real-cloud claim.
- No guarantee for arbitrary S3-compatible products.

---

### v0.52.1 — AWS S3 production evidence

#### Scope

Run the evidence contract against real AWS S3. Add recovery, throttling,
request-cost, and writer-replacement scenarios that cannot be certified by an
emulator alone.

#### User outcome

An operator receives an evidence-backed AWS deployment envelope with measured
latency percentiles, object-store request volume, transferred bytes, recovery
behavior, and cost assumptions.

#### Implementation plan

- [ ] Create a dedicated least-privilege AWS test account or isolated account
      boundary with budget alarms and automatic cleanup.
- [ ] Pin region, storage class, bucket settings, encryption mode, lifecycle
      policy, network origin, and instance type in the result metadata.
- [ ] Add an object-store observation wrapper that records operation type,
      attempts, retries, bytes, latency, and final status without logging
      credentials, signed URLs, or secret headers.
- [ ] Run cold and warm open/read scenarios for the v0.52.0 dataset matrix.
- [ ] Measure 1, 4, and 16 readers, including independent processes on separate
      hosts where practical.
- [ ] Measure writer commit, writer replacement, stale-writer rejection, reader
      restart, and read-after-commit visibility.
- [ ] Exercise backup creation, restore to a new prefix, verification, and
      cleanup.
- [ ] Inject throttling, transient DNS/network failures, delayed responses,
      connection resets, and selected 5xx responses through a controlled fault
      proxy while keeping the durable target in S3.
- [ ] Record S3 requests and bytes by operation class and calculate a transparent
      cost estimate from a versioned pricing input file.
- [ ] Publish the pricing date and keep pricing separate from measured request
      counts so future cost summaries can be regenerated.
- [ ] Validate IAM separation between catalog metadata and data prefixes.
- [ ] Confirm that logs, traces, evidence files, and support bundles contain no
      AWS credentials or signed request material.

#### Test and evidence plan

- Run full catalog verification after every writer replacement and injected
  failure.
- Compare result digests with LocalFS and MinIO.
- Validate retry classification: permanent authentication/authorization errors
  must not be retried as transient failures.
- Record p50, p95, p99, and maximum latency, but avoid an availability or latency
  SLO until the results are observed.
- Run at least one test from a second network location to expose dependence on
  a single low-latency environment.

#### Exit conditions

- A complete raw AWS evidence bundle is committed with no secrets.
- Recovery and stale-writer fencing preserve all invariants under the tested
  failure matrix.
- The AWS support table distinguishes functional, recovery, and scale
  certification.
- Cost documentation is reproducible from measured operations and a separate
  pricing file.
- The supported envelope and operational recommendations name the exact tested
  AWS configuration.

#### Non-goals

- No assertion that every region or S3-compatible implementation has identical
  behavior.
- No S3 Express support claim unless separately measured and named.

---

### v0.52.2 — Multi-node 24-hour soak and fault matrix

#### Scope

Prove that a sustained deployment preserves correctness and stable resource
behavior while readers, the writer, and administrative tasks churn.

#### User outcome

Operators receive a time-series evidence report showing whether RockLake stays
healthy over an extended mixed workload rather than only during isolated test
cases.

#### Implementation plan

- [ ] Build a reproducible topology with one active writer process, 1/4/16
      reader processes, load generators, an invariant checker, a fault
      controller, and metrics collection.
- [ ] Use both MinIO and AWS S3 for separate soak profiles when cost permits; at
      minimum, use real S3 for the release-certifying profile.
- [ ] Continuously create snapshots, add and retire files, evolve schemas, read
      current and historical snapshots, and refresh read-only clients.
- [ ] Run slow readers, disconnecting readers, reconnect loops, and mixed simple
      and extended PG-wire queries.
- [ ] Schedule backup, restore verification, catalog verification, retention
      planning/application, checkpoint creation, and safe cleanup checks during
      the workload.
- [ ] Kill and restart readers repeatedly.
- [ ] Kill the writer before, during, and after representative commit phases,
      then replace it and verify stale-writer rejection.
- [ ] Inject object-store latency, throttling, timeout, connection reset, and
      short outages according to a deterministic fault schedule.
- [ ] Persist a workload event ledger so every mutation and expected snapshot
      can be reconciled after the run.
- [ ] Record RSS, file descriptors, task counts, connection counts, queue depth,
      permit usage, request latency, first-row latency, error rate, retries, and
      object-store activity throughout the run.
- [ ] Add automatic detection for monotonic memory growth, unreleased permits,
      stuck maintenance jobs, latency drift, and snapshot-progress stalls.

#### Test and evidence plan

- Run for at least 24 continuous hours after warm-up.
- Perform a final clean restart with no retained process state, then verify the
  catalog and compare it with the workload ledger.
- Require every injected fault to have an expected outcome and a recorded
  recovery point.
- Separate expected client-visible failures from invariant failures.
- Preserve logs, metrics, traces, workload ledger, environment manifest, and
  final verification report.

#### Exit conditions

- Zero lost committed snapshots and zero catalog invariant violations.
- No unexplained monotonic resource growth.
- No admission permit, catalog handle, or maintenance lock remains leaked after
  clients and jobs exit.
- The final fresh-process verification matches the workload ledger.
- Latency and resource behavior do not progressively degrade outside the
  precommitted tolerance after warm-up.

#### Non-goals

- This release does not introduce transparent automatic writer failover.
- It does not claim multi-writer availability.

---

### v0.52.3 — GCS, Azure, and evidence closure

#### Scope

Apply the evidence contract to Google Cloud Storage and Azure Blob Storage,
repair emulator coverage where necessary, and close the v0.52 evidence phase
with an authoritative support matrix.

#### User outcome

Each cloud backend has an independent, evidence-based status. A failed or
incomplete backend does not dilute the certification of another backend and is
not hidden behind a generic “object storage supported” statement.

#### Implementation plan

- [ ] Repair the GCS emulator harness so its runtime tests are deterministic and
      no longer reduced to build-only coverage.
- [ ] Keep emulator suites as fast compatibility checks, not scale evidence.
- [ ] Provision least-privilege real-cloud test environments for GCS and Azure
      with budget limits and automated cleanup.
- [ ] Run the common 10k/100k/1M dataset and operation matrix independently for
      each backend.
- [ ] Add backend-specific failure scenarios: authentication expiry, throttling,
      retry-after behavior, transient service errors, network interruption, and
      conditional-operation behavior where relevant.
- [ ] Validate provider credential-chain handling and ensure no token material
      enters logs, traces, backups, or evidence bundles.
- [ ] Record request/byte counts and pricing inputs separately for each cloud.
- [ ] Add `EVIDENCE.md` as the index of all certified bundles, schemas, machines,
      backends, dependency versions, and supported envelopes.
- [ ] Reconcile `README.md`, `COMPATIBILITY.md`, deployment guides, and product
      claims with the complete v0.52 results.
- [ ] Add a release gate that rejects a support-matrix change unless it links to
      a valid evidence bundle and exact Git SHA.

#### Test and evidence plan

- Use identical correctness digests across all backends.
- Run fresh-process restore and full verification after injected failures.
- Confirm that retry semantics match provider error classes.
- Run the artifact binary, not a source build.
- Validate that all raw result files conform to the same versioned schema.

#### Exit conditions

- LocalFS, MinIO, AWS, GCS, and Azure each have a separate decision for
  functional support, recovery certification, and scale certification.
- No generic cloud claim exceeds the least-supported named backend.
- The v0.52 evidence index can reproduce every published chart and table from
  raw data.
- Any backend that fails remains explicitly Preview or functionally supported
  only, with a named gap and owner.

#### Non-goals

- No obligation to graduate both GCS and Azure in the same release.
- No optimization that is specific to a single cloud unless it preserves the
  common correctness contract and is separately configurable.

---

### v0.53.0 — Request lifecycle ownership consolidation

#### Scope

Refactor the PG-wire request path so every resource and state transition has one
owner. Preserve behavior while removing duplicated lifecycle, timing,
admission, cancellation, and response-observation logic.

#### User outcome

There should be no intentional user-visible feature change. The benefit is a
smaller failure surface and a code structure in which later routing, quotas,
and failover can be added without duplicating correctness logic.

#### Implementation plan

- [ ] Define a `ConnectionContext` that owns stable connection identity,
      principal placeholder, selected catalog route, protocol/session state,
      transaction state, and connection cancellation.
- [ ] Define a `RequestContext` that owns query identity, start time, deadline,
      cancellation token, selected snapshot, operation class, tracing fields,
      and response observer.
- [ ] Define typed `AdmissionPermit` values for connection, interactive scan,
      administrative work, and response-buffer capacity.
- [ ] Define a `ResponseObserver` that records first-row time, terminal status,
      rows, bytes, cancellation, error class, and completion exactly once.
- [ ] Move SQL classification, catalog execution, row streaming, and protocol
      encoding behind explicit interfaces with no hidden metric side effects.
- [ ] Ensure request state cannot outlive its connection except for an explicitly
      detached administrative job.
- [ ] Centralize error-to-SQLSTATE mapping and terminal response handling.
- [ ] Remove duplicate query timing and counter updates from simple-query,
      extended-query, COPY, and error paths.
- [ ] Make healthy disconnect, protocol error, timeout, server shutdown, and
      client cancellation separate terminal states.
- [ ] Add compile-time or test-enforced ownership rules preventing a permit from
      being cloned or completed twice.
- [ ] Preserve correlation IDs through catalog spans, object-store spans, logs,
      metrics exemplars where supported, and error reports.
- [ ] Split large modules only where the ownership boundary is stable; avoid a
      file-moving refactor without a semantic boundary.

#### Test plan

- Golden parity tests for every supported wire-corpus statement before and after
  the refactor.
- State-machine tests for connection open, startup, authentication, transaction,
  query, cancellation, shutdown, and close.
- Fault injection at every transition between admission, classification,
  execution, first row, final row, and response completion.
- Assertions that every request emits exactly one terminal observation.
- Assertions that every acquired permit is released under panic, error,
  cancellation, and disconnect.
- Performance comparison against v0.52 baselines with a precommitted regression
  budget.

#### Exit conditions

- Simple, extended, and COPY paths use the same request lifecycle abstraction.
- Metrics totals reconcile with completed request states.
- No duplicate connection/session lifecycle implementation remains in the
  supported path.
- The v0.52 evidence matrix shows no material behavior regression.

#### Non-goals

- No new SQL shapes.
- No router or multi-catalog behavior.
- No public API freeze.

---

### v0.53.1 — Unified bounded metadata streaming and encoding

#### Scope

Replace table-specific high-cardinality response builders with one schema-driven,
bounded, cancellation-safe metadata pipeline.

#### User outcome

Large catalog responses behave consistently across simple query, extended
query, COPY, Rust page APIs, and Rust stream APIs. Slow clients do not cause
unbounded buffering, and mid-stream failures are never reported as successful
truncation.

#### Implementation plan

- [ ] Introduce a common `CatalogRowStream` or equivalent abstraction carrying
      snapshot, schema descriptor, operation class, ordering contract, optional
      row estimate, and a fallible row stream.
- [ ] Generate PostgreSQL `RowDescription` and field encoding from the existing
      authoritative DuckLake schema registry.
- [ ] Use one row encoder for simple query, extended query, and COPY-compatible
      paths.
- [ ] Keep type conversion fallible and propagate the first encoding failure as
      the terminal request error.
- [ ] Enforce queue depth and in-flight byte limits at the encoder boundary.
- [ ] Check cancellation and connection liveness between storage fetches and
      encoded batches.
- [ ] Preserve one documented ordering for each metadata relation across page,
      stream, legacy, and wire paths.
- [ ] Remove full pre-counts that delay the first row.
- [ ] Make estimates optional and prove that requesting an estimate cannot turn
      an incremental path into a full scan.
- [ ] Route file metadata, file statistics, delete files, snapshot history,
      partitions, mappings, tags, and other high-cardinality relations through
      the common pipeline.
- [ ] Retain materializing APIs only as documented compatibility wrappers around
      bounded primitives.
- [ ] Add a schema-registry test that fails when a supported DuckLake table lacks
      a streaming encoder or has a divergent field definition.

#### Test plan

- Cross-path result and ordering parity for every supported DuckLake metadata
  table.
- Results larger than the old 16 MiB boundary.
- One-row-at-a-time consumers with artificial delays.
- Early disconnect and cancellation before first row, between rows, and during
  object-store I/O.
- Mid-stream decode and storage failures.
- Binary and text parameter/field formats used by supported clients.
- Memory-slope comparison at 10k, 100k, and 1M rows.

#### Exit conditions

- Every supported high-cardinality metadata relation uses the common stream and
  encoder path.
- No successful response can hide a mid-stream storage or encoding error.
- Page, stream, legacy, simple-query, extended-query, and COPY ordering match.
- Incremental memory stays within the v0.52 boundedness contract.

#### Non-goals

- No general SQL executor.
- No arbitrary PostgreSQL result expression support.

---

### v0.53.2 — Bounded and resumable administrative jobs

#### Scope

Give backup, restore, export, import, verification, repair, retention,
excision, orphan sweep, and rebuild operations one long-running job contract.

#### User outcome

Operators can start, inspect, cancel, and where safe resume long-running work.
Progress is machine-readable, memory use is bounded, and conflicting
maintenance operations are rejected before they can interfere.

#### Implementation plan

- [ ] Define `JobId`, `JobKind`, `JobState`, `JobProgress`, `JobCheckpoint`,
      `JobError`, and `JobResourceClass`.
- [ ] Classify jobs as interactive, administrative-online, or offline-exclusive.
- [ ] Add a small versioned job ledger under a separate maintenance namespace,
      with no collision with DuckLake catalog keys.
- [ ] Persist operation parameters, catalog format, starting snapshot, last safe
      checkpoint, progress counters, terminal result, and software version.
- [ ] Make resume explicit; never infer that an abandoned destructive operation
      should continue automatically.
- [ ] Add idempotency keys for operations that may be retried.
- [ ] Add conflict rules: for example, restore/excision/rebuild require exclusive
      access; verification may coexist with reads; backup and retention have
      documented interactions.
- [ ] Add separate admission pools for interactive requests and administrative
      jobs so a backup cannot consume every query permit.
- [ ] Stream human and JSON progress from the CLI without retaining the complete
      report in memory.
- [ ] Add `rocklake catalog jobs list|status|cancel|resume`.
- [ ] Migrate existing plan/apply semantics without making destructive defaults
      less explicit.
- [ ] Preserve an immutable terminal summary for audit and support bundles.
- [ ] Add cleanup rules for completed job metadata with an operator-controlled
      retention period.

#### Test plan

- Kill the process at every persisted checkpoint and verify safe resume or clear
  non-resumable failure.
- Repeat an operation with the same idempotency key.
- Attempt every conflicting job pair.
- Cancel before work, during scanning, during output, and after the durable
  commit point.
- Run jobs against 1M-file catalogs with slow object storage.
- Upgrade the binary with jobs in every nonterminal state and verify the
  compatibility decision.

#### Exit conditions

- Every long-running supported catalog command declares its resource class,
  resumability, conflict set, and durable commit point.
- Administrative jobs remain memory-bounded on the certified large catalog.
- Cancellation and crash leave either a resumable checkpoint or a clearly
  terminal state.
- Interactive query capacity remains available under the documented reservation
  policy.

#### Non-goals

- No general workflow engine.
- No distributed scheduler.
- No automatic execution of destructive jobs.

---

### v0.54.0 — Static multi-catalog router

#### Scope

Implement the first useful form of multi-tenancy: one RockLake service routes a
PostgreSQL database name to one of several independently configured catalogs.
No catalog key format changes are permitted.

#### Entry gate

- At least two named user workloads require multiple logical catalogs.
- A maintainer owns routing, authentication integration, and operator contracts.
- The catalog-routing ADR is reaffirmed after the v0.52 evidence results.

#### User outcome

A user connects to the same host and port with different PostgreSQL database
names and reaches different, independently isolated RockLake catalogs.

#### Architecture

```text
DuckDB / PostgreSQL client
           |
           | startup database = catalog alias
           v
+-------------------------------+
| RockLake front door           |
| startup parsing and routing   |
+---------------+---------------+
                |
                v
+-------------------------------+
| immutable route table         |
| alias -> stable CatalogId     |
+--------+--------------+-------+
         |              |
         v              v
 CatalogLocation A   CatalogLocation B
 independent prefix independent prefix
 independent epoch  independent epoch
 independent GC     independent GC
 independent backup independent backup
```

#### Implementation plan

- [ ] Add an internal `rocklake-router` crate while keeping `rocklake` as the
      only supported binary.
- [ ] Define validated `CatalogId`, `CatalogAlias`, `CatalogLocation`,
      `CatalogMode`, `CatalogLimits`, and `CatalogDescriptor` types.
- [ ] Use a stable opaque `CatalogId`; aliases are mutable display/routing names
      and never become storage keys or object-store prefixes implicitly.
- [ ] Add a strict static configuration schema for multiple catalogs.
- [ ] Parse the PostgreSQL startup `database` parameter and resolve it before a
      catalog is opened.
- [ ] Canonicalize scheme, authority, bucket/container, and prefix for every
      catalog and data location.
- [ ] Reject equal, ancestor, descendant, traversal, encoded-traversal, and
      platform-specific prefix overlaps across catalogs.
- [ ] Reject credentials embedded in registry/config URLs; use named credential
      providers or process environment configuration.
- [ ] Load the route table as one immutable snapshot and swap it atomically on an
      explicit reload.
- [ ] Open catalog handles on demand with single-flight behavior so concurrent
      first connections do not open the same catalog repeatedly.
- [ ] Bound the number of open catalog handles and close idle handles safely.
- [ ] Preserve independent writer epochs and read-only refresh state.
- [ ] Add `rocklake catalogs validate|list|status` for the static configuration.
- [ ] Ensure startup summaries and diagnostics redact locations and credentials
      according to a documented policy.
- [ ] Keep the router support level Preview in this release.

#### Configuration sketch

```toml
[router]
mode = "static"
default_catalog = "analytics"
max_open_catalogs = 64
catalog_idle_timeout = 300

[[catalogs]]
id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9"
aliases = ["analytics", "analytics_prod"]
catalog = "s3://company-rocklake/catalogs/analytics"
data = "s3://company-data/analytics"
mode = "read-write"
credential_provider = "aws-default"

[[catalogs]]
id = "018f4f4d-d520-7d91-b9f0-7018b7b50d13"
aliases = ["research"]
catalog = "s3://company-rocklake/catalogs/research"
data = "s3://company-data/research"
mode = "read-only"
credential_provider = "aws-default"
```

#### Test plan

- Two-catalog end-to-end DuckDB lifecycle with identical schema/table names and
  disjoint results.
- 1,000 configured but idle catalogs and a bounded open-handle cache.
- Concurrent first open of one catalog.
- Corrupt, unavailable, or slow catalog while unrelated catalogs remain usable.
- Every prefix-overlap and path-traversal class.
- Alias reload while existing sessions remain bound to their original
  `CatalogId`.
- No catalog existence disclosure before the authentication policy permits it.

#### Exit conditions

- One binary routes multiple aliases to independent catalog locations.
- No tenant/catalog ID appears in the existing catalog keyspace.
- One catalog's failure, retention, backup, or writer epoch cannot alter another
  catalog.
- Resource use is bounded by configured active/open catalog limits.
- The feature remains Preview until managed lifecycle, authorization, quotas,
  and isolation certification are complete.

#### Non-goals

- No dynamic catalog creation.
- No shared cross-catalog operation.
- No remote management API.
- No transparent multi-node writer routing.

---

### v0.55.0 — Managed catalog registry and lifecycle

#### Scope

Replace static-only routing with a versioned object-store-backed registry while
preserving static configuration as a bootstrap and recovery mechanism. Add safe
catalog lifecycle operations through the local CLI.

#### User outcome

Operators can register, create, disable, rename, inspect, and remove catalog
routes without rebuilding the service configuration or editing tenant catalog
keys.

#### Registry design

The registry is a separate control database, not a DuckLake catalog and not a
shared tenant keyspace. It has its own location, format version, writer fencing,
backup, and recovery contract.

```text
service root
├── registry/                 # RockLake registry state
├── catalogs/catalog-id-a/    # independent SlateDB catalog
├── catalogs/catalog-id-b/    # independent SlateDB catalog
└── registry-backups/
```

#### Implementation plan

- [ ] Extend `rocklake-router` with an internal `CatalogRegistry` abstraction.
- [ ] Store registry state in a dedicated SlateDB database or equivalent
      object-store-native store with transactional generation updates.
- [ ] Define registry rows for catalog descriptor, alias index, lifecycle state,
      policy reference, credential-provider reference, limits, creation time,
      update generation, and tombstone.
- [ ] Keep secrets and raw credential material out of registry rows.
- [ ] Require a stable `CatalogId` before any catalog is created.
- [ ] Implement lifecycle states such as `Creating`, `Active`, `ReadOnly`,
      `Disabled`, `Deleting`, `Deleted`, and `Error`, with permitted transitions.
- [ ] Make create and register idempotent with a caller-supplied request ID.
- [ ] Validate all locations and prefix isolation inside the same registry
      transaction that publishes the descriptor.
- [ ] Use a generation/CAS contract so concurrent management commands cannot
      silently overwrite one another.
- [ ] Publish immutable registry snapshots to serving processes; existing
      connections remain bound to the descriptor generation selected at startup.
- [ ] Add `rocklake registry init|status|backup|restore|verify`.
- [ ] Add `rocklake catalogs create|register|rename|set-mode|disable|enable|remove`.
- [ ] Make `remove` detach routing only. Physical catalog/data deletion requires
      a separately named destructive command, plan, confirmation token, and
      exclusive job.
- [ ] Add migration from the v0.54 static configuration.
- [ ] Add registry backup and restore before dynamic lifecycle is enabled.
- [ ] Append every management mutation to an immutable audit sequence.
- [ ] Add a read-only emergency startup mode using the last verified registry
      snapshot or an explicit static recovery file.

#### Test plan

- Concurrent create, alias assignment, rename, disable, and remove operations.
- Crash at every lifecycle transition.
- Router restart with registry updates in flight.
- Registry corruption or unavailability while already-open catalogs continue
  according to a documented fail-closed policy.
- Restore registry to a new prefix and reconnect existing catalog locations.
- Alias reuse only after tombstone and safety rules pass.
- Registry version upgrade and unsafe downgrade rejection.

#### Exit conditions

- Catalog lifecycle is transactional, idempotent, audited, and recoverable.
- Registry loss cannot delete or mutate catalog contents.
- A registry backup restores the complete alias-to-location mapping and policy
  references.
- Static bootstrap and emergency read-only recovery are documented and tested.
- Registry and tenant catalog prefixes are provably separate.

#### Non-goals

- No public management network API.
- No catalog data copy as part of rename.
- No cross-catalog transaction.

---

### v0.56.0 — Multi-principal authorization, quotas, metrics, and isolation certification

#### Scope

Turn the router and registry into a supportable multi-tenant boundary. Replace
the single configured username/password model with multiple principals and
per-catalog grants. Add enforceable resource quotas and complete adversarial
isolation testing.

#### User outcome

Different users can access different catalogs through the same service without
learning about or consuming unbounded resources from catalogs they do not own.

#### Authorization model

Recommended permissions:

| Permission | Allows |
|---|---|
| `CONNECT` | Resolve and open a session to the catalog. |
| `READ` | Read metadata and snapshots. |
| `WRITE` | Perform DuckLake catalog mutations through supported SQL. |
| `MAINTAIN` | Run backup, verification, retention, and other approved jobs. |
| `ADMIN` | Change catalog routing, policy, grants, and lifecycle state. |

`ADMIN` does not imply object-store credentials are returned or displayed.

#### Implementation plan

##### Identity and authentication

- [ ] Define stable `PrincipalId`, role, group, and catalog-grant records.
- [ ] Store SCRAM verifiers rather than plaintext passwords.
- [ ] Use a fake verifier path for unknown users to reduce username timing
      disclosure.
- [ ] Authenticate the principal before revealing whether a requested catalog
      exists.
- [ ] Authorize the resolved `CatalogId`, not the mutable alias string.
- [ ] Bind principal, grant generation, catalog ID, and selected mode to the
      connection context.
- [ ] Define reload and revocation semantics for new and existing sessions.
- [ ] Keep cleartext-password compatibility library-only and unsupported in the
      release binary.

##### Quotas and admission

- [ ] Add process-global and per-catalog limits for connections, concurrent
      scans, queued requests, writer transactions, administrative jobs, open
      handles, and in-flight response bytes.
- [ ] Add optional per-principal connection and request-rate limits.
- [ ] Reserve capacity for health, status, and recovery operations.
- [ ] Return stable SQLSTATE and structured diagnostic fields for each enforced
      rejection.
- [ ] Keep storage-byte and object-count quotas report-only until inventory is
      measured and enforcement can be atomic.
- [ ] Prove that a blocked or slow catalog cannot consume all process-global
      permits.

##### Metrics and observability

- [ ] Keep the default process metrics endpoint bounded in cardinality.
- [ ] Expose per-catalog detail through an authenticated/filtered endpoint or an
      explicitly bounded allowlist, using stable catalog IDs rather than aliases.
- [ ] Record catalog ID, principal ID, and route generation in traces and logs
      with configurable redaction.
- [ ] Prevent catalog names, locations, credentials, SQL parameter values, and
      data paths from leaking in default metrics labels.
- [ ] Add per-catalog health, capacity, rejection, writer state, job state, and
      cache-use observations.

##### Isolation certification

- [ ] Build a threat model covering alias confusion, path traversal, prefix
      overlap, URI parsing, credential-provider confusion, authorization cache
      staleness, metrics/log leakage, backup confusion, restore-to-wrong-target,
      and noisy-neighbor denial of service.
- [ ] Add adversarial and property tests for every boundary.
- [ ] Run independent catalogs containing identical IDs, names, and snapshot
      numbers to prove route-local interpretation.
- [ ] Run at least 1,000 registered catalogs, 100 active catalogs, and writes to
      20 independent catalogs in the evidence harness.

#### Exit conditions

- Unauthorized and nonexistent catalogs are externally indistinguishable until
  policy permits disclosure.
- Per-catalog and global limits are enforced and black-box tested.
- No result, metric, log, trace, backup, or maintenance job crosses catalog
  identity.
- The multi-catalog service can graduate from Preview to Supported only after
  the isolation report is published and all high-severity findings are closed.
- Issue #92 can be closed with documentation explaining the independent-catalog
  routing model and its limits.

#### Non-goals

- No row-level or table-level authorization inside a catalog.
- No OAuth/OIDC over the PostgreSQL protocol unless a concrete supported client
  path is demonstrated.
- No hard storage quota based on approximate accounting.

---

### v0.57.0 — Writer ownership, failover, and multi-node routing

#### Scope

Allow a multi-node RockLake service to maintain one authoritative writer owner
per catalog, route read-write connections to that owner, serve explicitly
read-only connections from replicas, and perform safe handoff. Writer fencing
remains the final correctness boundary.

#### User outcome

A service can restart or replace the node responsible for a catalog without
manual editing of every client connection string and without permitting two
writers to commit concurrently.

#### Availability model

- One active writer owner per catalog.
- Zero or more read-only replicas.
- A front-door routing layer for the read-write endpoint.
- A distinct read-only endpoint or explicit read-only connection mode.
- Writer assignment is an availability hint; the catalog writer epoch is the
  authoritative fence.
- Automatic failover is optional and off by default until its safety gate
  passes.

#### Implementation plan

- [ ] Define `NodeId`, `NodeLease`, `WriterAssignment`, assignment generation,
      health state, and endpoint records in the registry.
- [ ] Make node registration ephemeral and separate from durable catalog
      identity.
- [ ] Add a writer-ownership state machine: `Unassigned`, `Acquiring`, `Active`,
      `Draining`, `Releasing`, `Failed`, and `Fenced`.
- [ ] Require the assigned node to acquire the catalog writer epoch before
      reporting write readiness.
- [ ] Make promotion increment/fence the catalog epoch and verify the old owner
      cannot commit before the new owner is advertised ready.
- [ ] Implement explicit promotion first: `rocklake catalogs promote --id ...
      --expected-generation ...`.
- [ ] Add optional lease-based automatic promotion only after clock-skew,
      partition, delayed-heartbeat, and stale-registry tests pass.
- [ ] Add a front-door route target abstraction for local and remote owners.
- [ ] For remote ownership, proxy the PostgreSQL connection with bounded
      buffers, cancellation, half-close handling, backpressure, TLS policy, and
      connection correlation.
- [ ] Define TLS termination and re-encryption or mTLS between front door and
      owner. Do not proxy plaintext credentials over an unprotected internal
      network.
- [ ] Keep read-write sessions pinned to one owner for their full connection and
      transaction lifetime.
- [ ] Expose read-only routing separately so a connection cannot be moved between
      snapshots or nodes mid-transaction.
- [ ] Drain existing sessions before planned handoff; forcibly terminate them
      after the documented drain deadline.
- [ ] Reject stale route generations and retry only before a transaction begins.
- [ ] Add readiness states for front door, registry, per-catalog reader, and
      per-catalog writer ownership.

#### Test plan

- Simultaneous promotion attempts from two nodes.
- Old owner partitioned from registry but still able to reach object storage.
- New owner promoted while old connections are active.
- Crash before epoch acquisition, after epoch acquisition, before readiness,
  during commit, and during drain.
- Registry delay or stale route snapshot.
- Front-door crash and reconnect.
- TLS/mTLS failure and credential redaction.
- High-latency proxy with slow clients and bounded buffers.
- Read-only replicas refreshing while the writer advances retention.
- Full catalog verification after every failover sequence.

#### Exit conditions

- At most one writer can commit for a catalog in every tested fault schedule.
- Promotion is generation-checked, auditable, and reversible only through a new
  promotion.
- Read-write connections reach the current owner; read-only connections never
  acquire a writer epoch.
- A stale or partitioned owner fails closed through fencing.
- Multi-node routing passes the mixed-workload soak without leaked connections,
  buffers, permits, or ownership records.

#### Non-goals

- No active-active writer.
- No transaction migration between nodes.
- No guarantee that an in-flight transaction is transparently retried after
  owner loss.

---

### v0.58.0 — Disaster recovery, backup sets, and maintenance automation

#### Scope

Turn the existing backup, restore, verification, retention, checkpoint, cleanup,
and job primitives into complete single- and multi-catalog operational
workflows with measured recovery objectives.

#### User outcome

Operators can produce a recoverable backup set, restore it into a new location,
verify both metadata and referenced data expectations, schedule safe
maintenance, and rehearse disaster recovery without internal code knowledge.

#### Implementation plan

##### Backup and restore

- [ ] Define a versioned backup-set manifest covering registry generation,
      catalog IDs, catalog format, latest snapshot, retention floor, checkpoint
      pins, job state policy, object references, checksums, software version,
      and required encryption key IDs.
- [ ] Preserve the distinction between catalog metadata backup and data-file
      copy. A metadata backup must never imply that referenced data files were
      copied.
- [ ] Add optional referenced-data inventory and HEAD verification without
      turning backup into an unbounded pre-count before progress begins.
- [ ] Support full-service backup sets and selected-catalog backup sets.
- [ ] Restore only to new registry/catalog prefixes by default.
- [ ] Require a plan and explicit overwrite token for any existing destination.
- [ ] Make restore restartable through the v0.53.2 job contract.
- [ ] Verify checksums and format compatibility before publishing restored
      routes.
- [ ] Keep restored catalogs Disabled or ReadOnly until final verification
      succeeds.
- [ ] Add restore-as-new-catalog and disaster-recovery promotion workflows.

##### Maintenance automation

- [ ] Add a bounded scheduler for backup, verification, retention planning,
      checkpoint creation, and orphan scanning.
- [ ] Store schedules as policy, but use the existing job engine for execution.
- [ ] Enforce per-catalog maintenance concurrency and global maintenance budgets.
- [ ] Add maintenance windows and blackout periods without using local process
      memory as the only source of schedule truth.
- [ ] Require separate explicit policy for visibility GC and physical excision.
- [ ] Never schedule excision by default.
- [ ] Add stale-job, missed-window, repeated-failure, and backup-age alerts.

##### Recovery contracts

- [ ] Define measured RPO and RTO terminology separately for durable catalog
      commits, periodic backups, registry recovery, and data-file availability.
- [ ] Add recovery drills for lost process, lost registry prefix, accidental
      route deletion, damaged catalog prefix, lost credentials, and region-level
      restore to a new location.
- [ ] Generate a machine-readable recovery report from every drill.

#### Test and evidence plan

- Restore v0.51.4, latest v0.52.x, and every format-changing fixture to the
  current release.
- Interrupt backup and restore at every job checkpoint.
- Restore one catalog from a multi-catalog set without changing the others.
- Run maintenance during sustained reads/writes and writer failover.
- Simulate missing referenced data, stale encryption keys, wrong registry
  generation, and conflicting destination prefixes.
- Measure recovery on LocalFS, MinIO, and AWS S3; add other clouds only if they
  are certified.

#### Exit conditions

- A clean operator can execute backup, restore, verification, and promotion
  using public documentation only.
- Recovery reports publish measured—not aspirational—RPO/RTO results.
- A failed restore cannot publish a partially restored catalog as Active.
- Maintenance remains bounded and cannot starve interactive capacity.
- Every destructive operation remains separately named, planned, confirmed,
  and audited.

#### Non-goals

- No automatic cross-region data-file replication.
- No promise that catalog backup alone protects the referenced Parquet data.
- No hidden automatic excision.

---

### v0.59.0 — Security, secret lifecycle, audit, and release governance

#### Scope

Close the security work required before public-surface freeze: verifier and
secret rotation, encryption-key lifecycle, tamper-evident audit, abuse
resistance, supply-chain hardening, and independent review.

#### User outcome

Operators can rotate credentials, TLS material, and encryption keys without
unsafe downtime or plaintext configuration. Security-relevant actions are
traceable, and releases cannot bypass mandatory review and certification.

#### Implementation plan

##### Authentication and secret lifecycle

- [ ] Replace plaintext password configuration in the supported binary with
      SCRAM verifier files or a named secret provider.
- [ ] Keep secret values out of the registry, logs, traces, diagnostics,
      effective configuration, crash reports, and support bundles.
- [ ] Add atomic credential-set reload with generation numbers and validation
      before activation.
- [ ] Define revocation behavior for new and existing sessions.
- [ ] Add TLS certificate/key reload with rollback to the last valid generation.
- [ ] Add optional mTLS principal mapping only if supported client deployments
      are demonstrated.
- [ ] Add authentication backoff and bounded failure-rate controls that cannot
      become a global denial-of-service vector.

##### Encryption lifecycle

- [ ] Version the encrypted block envelope and include a nonsecret key ID.
- [ ] Support a key ring: one active write key and multiple read keys.
- [ ] Define `prepare`, `activate`, `verify`, `rewrite/compact`, and `retire`
      phases for key rotation.
- [ ] Prevent key retirement until evidence shows no live catalog block or
      backup requires it.
- [ ] Record required key IDs in backup manifests without recording key
      material.
- [ ] Add wrong-key, missing-key, partial-rotation, restore, and rollback tests.
- [ ] Keep KMS/HSM integration Preview unless it has a named provider,
      maintainer, and end-to-end certification.

##### Audit and incident response

- [ ] Unify existing catalog audit records with registry, authentication,
      authorization, failover, backup, restore, retention, excision, and key
      rotation events.
- [ ] Use stable event schemas, sequence numbers, timestamps, actor IDs, target
      catalog IDs, request IDs, outcome, and redacted details.
- [ ] Add hash chaining or equivalent tamper-evidence across audit segments.
- [ ] Support export to an operator-controlled immutable sink.
- [ ] Add incident runbooks for suspected credential leak, unauthorized catalog
      access, accidental deletion, stale writer, corrupt backup, and compromised
      release artifact.

##### Threat model and supply chain

- [ ] Update the multi-catalog threat model and close all high-severity findings.
- [ ] Commission an independent review of authentication, routing, prefix
      isolation, writer handoff, encryption, and backup restore.
- [ ] Pin third-party GitHub Actions to reviewed commit SHAs.
- [ ] Enforce CODEOWNERS for catalog correctness, security, release workflows,
      key encoding, backup, and registry code.
- [ ] Require protected `main`, protected release tags, mandatory status checks,
      and independent approval.
- [ ] Publish vulnerability-reporting and supported-release policies.
- [ ] Generate and attest release provenance and SBOMs; verify them in the
      installation smoke test.

#### Test plan

- Credential and TLS rotation during active connections.
- Concurrent grant revocation and new connection attempts.
- Timing and enumeration tests for users and catalogs.
- Key rotation across old/new blocks, backups, restores, and interrupted
  compaction.
- Audit loss, truncation, reorder, duplicate, and tamper detection.
- Dependency and Action compromise simulations where practical.
- Fuzz URI, alias, startup parameter, SCRAM, and configuration parsers.

#### Exit conditions

- No supported path requires a plaintext password or encryption key in the main
  config file or command line.
- Rotation procedures are atomic, reversible before retirement, and tested
  under load.
- Audit output is complete, bounded, redacted, and tamper-evident.
- All high-severity independent-review findings are closed or the release is
  blocked.
- Release governance cannot be bypassed by an ordinary merge or tag operation.

#### Non-goals

- No claim of formal verification.
- No universal KMS provider support.
- No row-level access control.

---

### v0.60.0 — Compatibility, migration, upgrade, and downgrade safety

#### Scope

Create the migration framework and compatibility policy that will be frozen for
v1.0. Separate DuckLake catalog version, RockLake storage format, registry
format, backup manifest format, evidence schema, and public API versions.

#### User outcome

Operators can determine compatibility before startup, plan and resume required
migrations, upgrade a service safely, restore older backups, and receive a
clear failure before any unsafe downgrade writes occur.

#### Implementation plan

##### Version domains

- [ ] Define explicit version types for:
  - DuckLake catalog protocol/schema version.
  - RockLake catalog storage/key/value format.
  - Registry format.
  - Backup-set manifest.
  - Administrative job ledger.
  - Audit event schema.
  - Public JSON output schema.
  - Evidence result schema.
- [ ] Report all relevant versions in `rocklake status --output json`.
- [ ] Reject unknown required versions before acquiring a writer epoch or
      mutating state.

##### Migration engine

- [ ] Define a registry of typed migrations with source range, target version,
      prerequisites, online/offline class, backup requirement, estimated work,
      checkpoint schema, verification function, and downgrade policy.
- [ ] Make migration `plan` read-only and complete enough to show affected
      formats, expected object scans, backup requirement, and rollback boundary.
- [ ] Execute migrations as v0.53.2 jobs with durable checkpoints and
      idempotency.
- [ ] Publish a migration marker only after verification succeeds.
- [ ] Keep failed targets Disabled/ReadOnly until recovery or rollback.
- [ ] Support a no-op open path for releases sharing the same format.
- [ ] Add a migration fixture for every format-changing release, rather than
      every code release.

##### Upstream compatibility

- [ ] Automate capture and normalization of DuckDB/DuckLake startup and SQL wire
      corpora for candidate upstream versions.
- [ ] Diff query shapes, parameter types, row descriptions, and expected
      semantics against the current corpus.
- [ ] Require live create/read/update/delete/schema-evolution/restart/time-travel
      tests before naming an upstream version Supported.
- [ ] Keep unsupported upstream versions fail-closed with an actionable message.
- [ ] Support multiple named upstream patch versions only where the same bounded
      SQL contract is certified.

##### Service upgrade

- [ ] Define rolling upgrade order for front doors, read-only nodes, writer
      owners, registry writer, and administrative workers.
- [ ] Prevent a newer component from publishing state an older active component
      cannot safely read unless the upgrade plan has crossed an explicit point
      of no return.
- [ ] Add mixed-version compatibility tests for the supported rolling window.
- [ ] Define the stable v1.0 support window and minimum direct-upgrade sources.
      At minimum, preserve fixtures for v0.51.4, the latest v0.52.x evidence
      baseline, and every later format-changing release.

#### Test plan

- Upgrade/open/read/write/reopen for each supported source format.
- Backup from each source format restored under current code.
- Process kill at every migration checkpoint.
- Conflicting migration attempts.
- Mixed old/new routers and readers around a writer upgrade.
- Unsafe downgrade after each point of no return.
- Upstream DuckDB/DuckLake corpus diff and live-client matrix.

#### Exit conditions

- Every persistent format has an owner, version, compatibility table, migration
  path, and downgrade behavior.
- Migrations are restartable and verified before activation.
- The supported rolling-upgrade window is documented and black-box tested.
- No format change is permitted after this release except a blocker that resets
  the RC process.
- The v1.0 upstream compatibility list names exact tested versions.

#### Non-goals

- No speculative support for an unreleased DuckLake format.
- No promise of downgrade after a documented irreversible migration.
- No online migration where an offline exclusive migration is safer.

---

### v0.61.0 — Evidence-driven performance, cost, and capacity contract

#### Scope

Optimize only the bottlenecks demonstrated by v0.52–v0.60 evidence. Freeze the
performance regression process and publish the supported capacity model for
v1.0.

#### User outcome

Operators receive measured capacity guidance, reproducible cost inputs, and
configuration recommendations. Performance improvements do not weaken
snapshot, isolation, or recovery guarantees.

#### Implementation plan

##### Reader efficiency

- [ ] Profile object-store requests, bytes, decode time, allocation, cache use,
      and time to first row for every certified scale class.
- [ ] Optimize snapshot-bound point lookups and prefix scans only where evidence
      identifies a bottleneck.
- [ ] Add bounded prefetch and request coalescing with cancellation support.
- [ ] Keep cache keys bound to catalog ID, storage format, snapshot, and query
      semantics.
- [ ] Prevent alias reuse or route reload from reusing another catalog's cache.
- [ ] Make cache memory budgets global and per-catalog, with fair eviction.
- [ ] Retain an uncached correctness path for parity and diagnosis.

##### Writer and maintenance efficiency

- [ ] Measure commit batch size, serialization, transaction latency, retries,
      WAL/object-store operations, and compaction interactions.
- [ ] Optimize batching without weakening the one-transaction/one-snapshot
      boundary.
- [ ] Bound large DROP, import, restore, repair, and excision operations through
      the job engine.
- [ ] Tune SlateDB only through named profiles backed by the evidence matrix.
- [ ] Treat dependency upgrades as new baselines, not free improvements.

##### Cost and capacity

- [ ] Add `rocklake capacity report` using observed catalog counts, file counts,
      snapshot history, request rates, configured limits, cache budget, and
      evidence profiles.
- [ ] Keep cost calculations parameterized by a separate, dated pricing file.
- [ ] Publish request and byte counts independently from currency estimates.
- [ ] Define supported small/medium/large envelopes from measured evidence.
- [ ] Document when to split catalogs or service instances.
- [ ] Add multi-catalog noisy-neighbor and cache-fairness benchmarks.

##### Regression gates

- [ ] Use pinned hardware and cloud environments for release comparisons.
- [ ] Commit thresholds before candidate runs.
- [ ] Gate p95/p99, time to first row, incremental RSS, request count, byte count,
      recovery time, and writer commit latency where stable enough.
- [ ] Require correctness digests and full verification for every performance
      run.
- [ ] Allow a regression only with an explicit reviewed waiver explaining the
      correctness, security, or operability benefit.

#### Exit conditions

- All material optimizations correspond to a published evidence bottleneck.
- The v1.0 capacity guide states tested limits and configuration, not universal
  claims.
- Performance gates are reproducible and cannot be changed after candidate
  results without invalidating the run.
- Multi-catalog caching and admission preserve fairness and isolation.
- No correctness, recovery, or compatibility test is weakened for performance.

#### Non-goals

- No benchmark-only fast path.
- No eventual-consistency cache for catalog facts.
- No latency claim without environment and percentile definition.

---

### v0.62.0 — Public surface, distribution, and documentation freeze

#### Scope

Freeze the product interfaces intended to remain compatible through the v1.x
line. Complete artifact distribution, operator workflows, support bundles, and
clean-room documentation validation.

#### User outcome

Users know exactly which interfaces are stable, which remain Preview or
Experimental, how they are deprecated, and how to install, operate, upgrade,
recover, and obtain support without reading implementation code.

#### Implementation plan

##### Public surface manifest

- [ ] Expand the existing public-surface manifest to include:
  - CLI commands, flags, exit codes, aliases, and help structure.
  - TOML configuration fields and environment variables.
  - PostgreSQL startup behavior, supported SQL shapes, parameter types,
    RowDescription schemas, and SQLSTATE mapping.
  - Metrics names, labels, units, and semantics.
  - Structured log fields and redaction rules.
  - JSON output schemas.
  - Backup, registry, job, audit, and release manifests.
  - Supported crate/module/API boundaries.
  - Binary filenames, checksums, metadata, SBOM, and provenance.
- [ ] Generate tests from the manifest and fail on undocumented additions,
      removals, or semantic changes.
- [ ] Define v1.x deprecation rules and minimum support windows.
- [ ] Keep hidden pre-1.0 aliases only where the documented removal version has
      not arrived.

##### Support-level decision

- [ ] Re-evaluate the Rust client, read-only API, DataFusion, and each language
      binding independently.
- [ ] Graduate an interface only if it has a maintainer, compatibility tests,
      migration policy, docs, and field use.
- [ ] Leave unsupported integrations Preview/Experimental rather than expanding
      the stable promise for marketing completeness.

##### Distribution

- [ ] Retain the v0.51.5 artifact contract and verify it from a clean
      environment.
- [ ] Add detached signatures only if their trust and rotation model is
      documented; provenance attestations remain mandatory.
- [ ] Publish an OCI image only if it embeds the exact certified binary and
      independently passes startup, filesystem, signal, nonroot, health,
      upgrade, SBOM, and provenance tests. Otherwise keep it Preview.
- [ ] Add generated shell completions and man pages from the frozen CLI schema.
- [ ] Ensure every supported target has an install, upgrade, rollback, and
      uninstall procedure.

##### Operator experience

- [ ] Add a redacted `rocklake support bundle` containing versions,
      configuration shape, status, metrics snapshot, recent bounded logs,
      evidence profile, and verification summaries without secrets or data.
- [ ] Run clean-room operator exercises for install, first query, multi-catalog
      setup, grant creation, overload diagnosis, writer handoff, backup, restore,
      retention, upgrade, and incident response.
- [ ] Validate every command block against the certified binary.
- [ ] Publish known limits and unsupported scenarios prominently.

#### Test plan

- Snapshot the public surface and compare every candidate.
- Install and operate from documentation in clean Linux, macOS, and Windows
  environments.
- Run all operator exercises without repository-internal notes.
- Fuzz structured output parsers for forward-compatible unknown fields.
- Verify support bundle redaction with seeded fake secrets in every input source.

#### Exit conditions

- The complete v1.0 public surface is machine-readable and tested.
- No new stable command, field, metric, format, or API can appear accidentally.
- Support levels are evidence-based and have named owners.
- Installation and operations docs execute against release artifacts.
- After this release, only compatibility-preserving fixes may change the public
  surface before v1.0.

#### Non-goals

- No new feature family.
- No graduation of an interface solely because code exists.
- No package ecosystem expansion without certification.

---

### v0.63.0 — Feature-complete production beta

#### Scope

Declare the implementation feature-complete for v1.0 and place it in real
single- and multi-catalog design-partner deployments. All work after this point
is release-blocking defect correction, evidence collection, and documentation
clarification.

#### Entry gate

- v0.52 evidence is complete for every backend intended to be Supported.
- Multi-catalog isolation has passed v0.56.
- Writer ownership, DR, security, migration, performance, and public-surface
  gates are complete.
- At least two design partners have named workloads and operational owners.

#### Implementation plan

- [ ] Cut a feature-complete build from the frozen public surface.
- [ ] Publish a beta support policy, escalation route, and severity definitions.
- [ ] Deploy at least one large single-catalog workload and one multi-catalog
      workload.
- [ ] Run continuous metrics, audit, backup-age, job, capacity, writer-owner,
      and verification monitoring.
- [ ] Rehearse backup/restore, writer handoff, credential rotation, encryption
      key rotation, upgrade, rollback-before-point-of-no-return, and registry
      recovery in each representative deployment.
- [ ] Run a minimum 30-day observation window with no reset caused by ordinary
      maintenance. A correctness, security, format, or isolation fix resets the
      relevant observation gate.
- [ ] Record every operator surprise, undocumented step, false alert, ambiguous
      error, manual workaround, and support-bundle gap.
- [ ] Triage findings as P0, P1, P2, or post-1.0 enhancement.
- [ ] Accept no new feature request into the beta branch.
- [ ] Publish a beta evidence report containing environment, workload envelope,
      incidents, recoveries, upgrades, and unresolved limits without exposing
      customer data.

#### Exit conditions

- No unresolved P0 or P1 finding.
- No unexplained catalog invariant, isolation, or recovery failure.
- Backup restore and writer handoff have been exercised successfully in every
  representative deployment.
- The stable support envelope reflects real deployments and reproducible lab
  evidence.
- Every repeated operator action has a documented command or runbook.

#### Non-goals

- No new backend, binding, query shape, management API, or storage format.
- No feature work disguised as a beta fix.

---

### v0.63.1 — Beta fixes and complete v1.0 readiness audit

#### Scope

Ship only compatibility-preserving corrections discovered during beta and
publish the final readiness assessment used to decide whether RC1 may be cut.

#### Implementation plan

- [ ] Fix all P0/P1 beta findings and the P2 findings explicitly designated as
      release blocking.
- [ ] Add a regression test and operator note for every corrected field issue.
- [ ] Re-run the affected evidence, fault, migration, security, or observation
      gate; do not rely solely on a unit test for a field failure.
- [ ] Reconcile implementation, public-surface manifest, support matrix, known
      limits, and documentation.
- [ ] Run a fresh independent architecture and operations assessment against the
      v0.63.1 tag.
- [ ] Run a final security review delta from v0.59.0.
- [ ] Verify governance bus factor: at least two people can perform release,
      restore, failover, and security-response procedures from documentation.
- [ ] Publish `docs/assessments/v1-readiness.md` with evidence links, open
      findings, accepted risks, and the RC decision.

#### Exit conditions

- The readiness assessment recommends RC with no unresolved release blocker.
- Every accepted risk has an owner, rationale, user-visible documentation, and
  post-1.0 milestone where appropriate.
- The complete release certification passes on the tag.
- No persistent format or stable public-surface change remains planned.

#### Non-goals

- No optimization without a demonstrated beta blocker.
- No broad refactor unless required to close a correctness or security finding.

---

### v1.0.0-rc.1 — First stable-line release candidate

#### Scope

Create the first candidate carrying the intended v1.0 public compatibility
promise. Only release blockers may be fixed after this point.

#### Implementation plan

- [ ] Set all stable interface and manifest versions to their v1.0 values.
- [ ] Freeze catalog, registry, backup, job, audit, evidence, and JSON schema
      versions.
- [ ] Generate final migration fixtures from every supported source.
- [ ] Produce release assets through the exact stable release workflow.
- [ ] Run the full LocalFS, MinIO, AWS, certified-cloud, multi-node,
      multi-catalog, fault, DR, security, migration, performance, and clean-room
      documentation matrix.
- [ ] Run install and provenance verification from public candidate assets.
- [ ] Publish draft v1.0 release notes, compatibility tables, support policy,
      upgrade guide, known limits, and architecture decision index.
- [ ] Begin an RC observation window in design-partner deployments.

#### Exit conditions

- No P0/P1 finding.
- No format or public-surface mismatch.
- Full certification and artifact-install matrix passes.
- Design partners can install or upgrade using public RC documentation.

#### Change policy after RC1

Allowed:

- Security fixes.
- Correctness and recovery fixes.
- Supported-client compatibility fixes.
- Documentation corrections that do not change behavior.
- Release-artifact fixes.

Not allowed:

- New feature.
- New supported backend or binding.
- New public configuration family.
- New persistent format.
- New automatic behavior.

---

### v1.0.0-rc.2 — Final release candidate

#### Scope

Include only blocker fixes from RC1 and prove that those fixes do not invalidate
any frozen contract or evidence result.

#### Implementation plan

- [ ] Publish a complete RC1-to-RC2 change inventory with the gate affected by
      every change.
- [ ] Add regression coverage for every fix.
- [ ] Re-run the full release matrix, not only the directly affected tests.
- [ ] Re-run evidence baselines when a performance, dependency, storage, or
      lifecycle path changed.
- [ ] Re-run migration and restore from RC1.
- [ ] Re-run security review delta and artifact provenance checks.
- [ ] Run the final observation gate with no new blocker.
- [ ] Prepare the exact v1.0.0 tag/release procedure and rollback plan.

#### Exit conditions

- RC2 differs from RC1 only by documented blocker fixes.
- Full certification passes on the exact tagged SHA.
- No blocker is discovered during the final observation gate.
- The v1.0.0 release can be produced by changing version/release metadata only.

#### Additional release candidates

An `rc.3` or later candidate may be cut only for a newly discovered release
blocker. It inherits the RC2 plan and restarts every affected gate. Additional
RCs are not thematic roadmap versions and introduce no new scope.

---

### 8. v1.0.0 release gate

v1.0.0 is a release decision, not another implementation milestone. The tag may
be created only when all of the following are true.

### 8.1 Correctness and durability

- All catalog invariants pass after clean restart, failure injection, restore,
  migration, retention, and writer handoff.
- No known path can publish a partial DuckLake transaction.
- Writer fencing remains authoritative under partition and stale assignment.
- Historical reads obey snapshot and retention bounds.
- Backup and restore preserve all documented metadata and references.

### 8.2 Scale and performance

- Raw evidence exists for every Supported backend and scale claim.
- The supported envelope names catalog size, concurrency, topology,
  configuration, and environment.
- Bounded paths remain bounded at the largest supported cardinality.
- Regression budgets pass on the exact release candidate.
- Cost calculations can be regenerated from measured request/byte counts.

### 8.3 Multi-catalog isolation

- Catalogs use independent locations, epochs, retention, backups, policies, and
  limits.
- Prefix overlap and path confusion are rejected.
- Authentication precedes catalog disclosure.
- Authorization binds to stable catalog ID.
- Metrics, logs, traces, caches, jobs, backups, and support bundles do not cross
  catalog identity.
- No tenant identifier was added to the existing catalog keyspace.

### 8.4 Operations

- Install, first query, overload diagnosis, backup, restore, verification,
  retention, writer handoff, key rotation, upgrade, and incident response work
  from public documentation.
- Recovery objectives are measured and scoped.
- Destructive operations remain plan/apply and auditable.
- Support bundles are useful and redacted.

### 8.5 Security and supply chain

- Independent high-severity findings are closed.
- Secrets are not accepted in supported command-line arguments or ordinary
  config fields.
- Credential, TLS, and encryption-key rotation are tested.
- Release assets have checksums, build metadata, SBOM, and provenance.
- Protected branch/tag rules and independent release approval are active.

### 8.6 Compatibility

- Exact supported DuckDB/DuckLake versions are named and certified.
- Direct upgrade sources and rolling-version window are documented.
- Migrations and restore fixtures pass.
- Unsafe downgrade fails before mutation.
- Stable interfaces match the public-surface manifest.

### 8.7 Field observation and ownership

- The production beta observation gate completed.
- At least two design-partner workloads are represented.
- No unresolved P0/P1 issue exists.
- Each stable subsystem has a named maintainer and recovery owner.
- At least two people can execute release and disaster-recovery procedures.

## 9. Cross-release certification matrix

| Capability | First owning release | Must remain green through v1.0 |
|---|---:|---|
| Artifact-only installation | v0.51.5 | All supported platforms and every release candidate |
| Fresh-process scale schema | v0.52.0 | Every material storage/runtime dependency change |
| Real AWS recovery and cost | v0.52.1 | Every material object-store/SlateDB change |
| Sustained mixed-workload soak | v0.52.2 | Availability, job, cache, and routing changes |
| GCS/Azure evidence | v0.52.3 | Only for backends that remain Supported |
| Request lifecycle state machine | v0.53.0 | Every protocol and server change |
| Unified bounded encoding | v0.53.1 | Every metadata schema or encoder change |
| Resumable job contract | v0.53.2 | Every maintenance operation |
| Static route isolation | v0.54.0 | Every URI, alias, cache, and routing change |
| Registry lifecycle/recovery | v0.55.0 | Every registry format or management change |
| Authz, quotas, tenant isolation | v0.56.0 | Every auth, metrics, limit, and router change |
| Writer ownership/failover | v0.57.0 | Every writer, registry, network, or topology change |
| Backup sets and recovery drills | v0.58.0 | Every persistent format and key-rotation change |
| Security and audit review | v0.59.0 | Every RC and high-risk dependency change |
| Migration/upgrade matrix | v0.60.0 | Every release candidate |
| Performance/capacity budgets | v0.61.0 | Every runtime/storage dependency or hot-path change |
| Public surface manifest | v0.62.0 | Every commit after freeze |
| Design-partner observation | v0.63.0 | RC blocker fixes restart affected gates |

## 10. Recommended repository structure for the roadmap

If adopted, split this proposal into release-owned files while retaining this
file as the index:

```text
plans/
├── pre-1.0-roadmap.md
├── v0.51.5.md
├── v0.52.0.md
├── v0.52.1.md
├── v0.52.2.md
├── v0.52.3.md
├── v0.53.0.md
├── v0.53.1.md
├── v0.53.2.md
├── v0.54.0.md
├── v0.55.0.md
├── v0.56.0.md
├── v0.57.0.md
├── v0.58.0.md
├── v0.59.0.md
├── v0.60.0.md
├── v0.61.0.md
├── v0.62.0.md
├── v0.63.0.md
├── v0.63.1.md
├── v1.0.0-rc.1.md
└── v1.0.0-rc.2.md
```

Each release file should use the same minimum structure:

1. Scope and non-goals.
2. Named user workload.
3. Entry gate.
4. Architecture/ADR decisions.
5. Implementation checklist by crate/component.
6. Data and compatibility changes.
7. Black-box test matrix.
8. Evidence artifacts.
9. Operator documentation.
10. Exit conditions and support-level change.

## 11. Suggested GitHub epic structure

Create one roadmap epic per thematic version and require the following child
issues before implementation is called complete:

- **ADR/contract issue** — decisions and rejected alternatives.
- **Core implementation issue** — production code and migration impact.
- **Black-box certification issue** — network/process/backend behavior.
- **Fault and recovery issue** — deterministic failure coverage.
- **Evidence issue** — raw data, environment, exact SHA, and report generator.
- **Operator documentation issue** — clean-room workflow.
- **Compatibility issue** — previous version, backup restore, downgrade.
- **Security review issue** — threat model delta and secret review.
- **Release issue** — exact-SHA workflow, artifacts, manifest, SBOM, provenance.

A roadmap checkbox should link to one of these issues or to a committed artifact.
“Implemented” without a production path, test, and operator action is not a
completed item.

## 12. Ownership model

Every thematic release needs four explicit owners, even when one person fills
more than one role:

| Role | Responsibility |
|---|---|
| Release owner | Scope, sequencing, compatibility, final go/no-go. |
| Implementation owner | Production code and internal design. |
| Evidence owner | Reproducible tests, raw results, threshold integrity. |
| Operations/security reviewer | Failure modes, runbooks, secrets, and external boundary. |

Additional required ownership:

- `rocklake-core` key/value and format changes require a CODEOWNER.
- Writer fencing, backup/restore, registry, encryption, and release workflows
  require independent approval.
- Every Supported backend and integration needs a named maintainer.
- An unowned integration remains Experimental or is removed from the supported
  documentation path.

## 13. Risk register

| Risk | Consequence | Mitigation and owning release |
|---|---|---|
| Evidence is run on convenient but unrepresentative hardware | Misleading scale claims | Fresh-process schema, pinned environments, and raw data in v0.52.0 |
| Cloud emulator behavior is treated as production evidence | Recovery or retry surprises | Real-cloud gates in v0.52.1 and v0.52.3 |
| Multi-catalog routing leaks identity through aliases, caches, logs, or metrics | Cross-tenant disclosure | Stable CatalogId, prefix proof, auth-before-disclosure, and adversarial certification in v0.54–v0.56 |
| Registry becomes a new single point of failure | Service-wide route outage | Independent registry backup, immutable snapshots, emergency read-only mode, and DR in v0.55/v0.58 |
| Writer ownership weakens fencing | Split-brain writes | Fencing remains authoritative; assignment is only an availability layer in v0.57 |
| Automatic failover relies on unsafe clock assumptions | Dual ownership or outage | Explicit failover first; automatic mode remains off until partition/skew gates pass |
| Administrative jobs starve queries or resume unsafely | Outage or destructive repetition | Separate admission, durable checkpoints, conflict rules, and idempotency in v0.53.2 |
| Encryption rotation strands old blocks or backups | Irrecoverable catalog | Versioned key IDs, key ring, verify-before-retire, and backup key inventory in v0.59 |
| Upstream DuckDB/DuckLake changes silently alter SQL shapes | Compatibility break or wrong results | Automated corpus diff plus live-client gate in v0.60 |
| Performance optimization weakens snapshot correctness | Silent stale/wrong reads | Snapshot-bound caches, uncached parity path, and correctness digests in v0.61 |
| Public surface freezes too broadly | Unsustainable v1.x compatibility burden | Evidence-based support-level decision in v0.62 |
| Release velocity outruns field observation | Hidden operational defects | Feature freeze and minimum design-partner observation in v0.63 |
| Single-maintainer release/recovery knowledge | Slow or unsafe incident response | Protected releases, CODEOWNERS, and two-person drills before RC |

## 14. Decision points that must not be left implicit

The following decisions require explicit ADRs before their owning release is
implemented:

1. Canonical release artifact names and manifest schema — v0.51.5.
2. Evidence result schema and bounded-memory acceptance method — v0.52.0.
3. Request/connection ownership state machine — v0.53.0.
4. Administrative job ledger location and conflict model — v0.53.2.
5. Static catalog descriptor and prefix-overlap algorithm — v0.54.0.
6. Registry storage model and lifecycle state machine — v0.55.0.
7. Principal/grant model, catalog-disclosure policy, and metric-cardinality model
   — v0.56.0.
8. Writer assignment, routing, TLS boundary, and automatic-failover policy —
   v0.57.0.
9. Backup-set semantics and metadata-vs-data boundary — v0.58.0.
10. Encryption envelope and key-retirement proof — v0.59.0.
11. Version domains, migration point-of-no-return, and rolling window — v0.60.0.
12. v1.0 supported capacity envelope and performance budgets — v0.61.0.
13. Stable public surface and support-level graduation decisions — v0.62.0.
14. Beta severity policy and RC entry criteria — v0.63.0.

## 15. Recommended update to the root roadmap

The root `ROADMAP.md` should remain concise and link to this document. A proposed
replacement release table is:

```markdown
## Pre-1.0 release sequence

| Release | Theme | Status |
|---|---|---|
| v0.51.5 | Distribution correctness | Planned |
| v0.52.0–v0.52.3 | Reproducible scale and cloud evidence | Planned |
| v0.53.0–v0.53.2 | Request, streaming, and job consolidation | Planned |
| v0.54.0 | Static multi-catalog router | Planned |
| v0.55.0 | Managed catalog registry | Planned |
| v0.56.0 | Authorization, quotas, and isolation | Planned |
| v0.57.0 | Writer availability and multi-node routing | Planned |
| v0.58.0 | Disaster recovery and maintenance | Planned |
| v0.59.0 | Security, secrets, audit, and governance | Planned |
| v0.60.0 | Compatibility and migration freeze | Planned |
| v0.61.0 | Performance, cost, and capacity contract | Planned |
| v0.62.0 | Public surface freeze | Planned |
| v0.63.0–v0.63.1 | Production beta and readiness audit | Planned |
| v1.0.0-rc.1–rc.2 | Release candidates | Planned |
| v1.0.0 | Stable release | Gate-based |
```

## 16. Final recommendation

Adopt this roadmap with three immediate actions:

1. Cut v0.51.5 as a release-distribution and platform-correctness patch.
2. Open the four v0.52 evidence epics and freeze the evidence schema before any
   performance claim or optimization work.
3. Convert issue #92 into the v0.54–v0.56 multi-catalog epic, explicitly stating
   that the supported design routes to independent catalog locations and will
   not add tenant IDs to the shared RockLake keyspace.

The most important discipline is sequencing. Multi-tenancy is valuable, but it
should be built on measured single-catalog behavior and the consolidated
request/job model. Likewise, v1.0 should follow field observation rather than a
version-number target. If the evidence or beta gates expose a correctness,
isolation, recovery, or governance gap, the roadmap pauses at that gate and
fixes the gap before advancing.
