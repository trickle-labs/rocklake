# Overall SlateDuck Assessment 1

Date: 2026-05-24

Scope reviewed: Rust crates under `crates/`, integration/property tests, CI workflows, benchmark artifacts, README, roadmap, and deployment/operations docs. The full workspace test suite was run with `cargo test --workspace` and passed: 233 tests across unit, integration, property, and doc-test targets.

## Executive Summary

SlateDuck has a strong architecture direction and a broad documentation set, but the implementation is not yet world-class production ready. The biggest risks are not formatting or isolated edge cases; they are cross-cutting correctness gaps in write/session state, security features that are configured but not enforced, and operational features whose docs are ahead of the code.

The highest priority is to make write commits and snapshot/counter state authoritative. Today, multiple write sessions on the same `CatalogStore` can reuse stale counters, and catalog mutations are committed before the snapshot row that is supposed to publish them. That undermines MVCC, time travel, failover, and PG-Wire correctness.

## Summary Table - Critical and High Findings

| ID | Severity | Area | Location | Finding | First Recommendation |
|---|---|---|---|---|---|
| F-01 | Critical | Correctness | `crates/slateduck-catalog/src/store.rs:69-90`, `crates/slateduck-pgwire/src/executor.rs:427-495` | `CatalogStore` never updates its in-memory counters after writer commits, so later write sessions and `read_latest()` can reuse or report stale IDs. | Make `CatalogStore` the authoritative writer state or reload/update counters after every committed writer session. |
| F-02 | Critical | Correctness | `crates/slateduck-catalog/src/writer.rs:47-72`, `crates/slateduck-catalog/src/writer.rs:457-490` | Catalog mutations are committed before `create_snapshot()`, so aborted or failed write sessions can leak unpublished facts into later snapshots. | Commit all buffered catalog changes and the snapshot row in one transaction. |
| F-03 | Critical | Security | `crates/slateduck-pgwire/src/handler.rs:11`, `crates/slateduck-pgwire/src/handler.rs:37-61` | Auth configuration is stored but the PG-Wire startup handler is `NoopStartupHandler`; configured usernames/passwords are ignored. | Implement a real PostgreSQL startup auth handler and add denial tests. |
| F-04 | High | Correctness | `crates/slateduck-pgwire/src/executor.rs:500-518` | `UPDATE end_snapshot` uses placeholder IDs, including `schema_id = 0` for tables and `entity_id` as both table and column ID. | Decode enough key fields from SQL/params or look up existing rows before calling drop/update methods. |
| F-05 | High | Operations | `crates/slateduck-catalog/src/gc.rs:91-120`, `crates/slateduck-catalog/src/gc.rs:210` | GC advances `retain-from`, but normal readers never enforce it, so snapshots reported as hidden remain readable. | Make `read_at()`/PG-Wire snapshot reads validate retain-from or rename the feature to metadata-only. |
| F-06 | High | Operations | `crates/slateduck-catalog/src/excise.rs:65-66`, `crates/slateduck-catalog/src/excise.rs:102-108` | Excision treats `retain_from == 0` as safe and allows physical deletion without first advancing retention. | Require `retain_from != 0 && retain_from >= before_snapshot` before plan/apply can be safe. |
| F-07 | High | Recovery | `crates/slateduck-catalog/src/checkpoint.rs:102-118` | Checkpoint restore only resets `next_snapshot_id`, leaving future facts in place and allowing snapshot ID reuse. | Implement real restore semantics or make restore read-only/logical without reusing snapshot IDs. |
| F-08 | High | FFI Safety | `crates/slateduck-ffi/src/lib.rs:203`, `crates/slateduck-ffi/src/lib.rs:274`, `crates/slateduck-ffi/src/lib.rs:310`, `crates/slateduck-ffi/src/lib.rs:350`, `crates/slateduck-ffi/src/lib.rs:388` | FFI functions dereference caller pointers without null/handle validation; invalid C input can cause UB or crashes. | Add null checks to every FFI entrypoint and validate opaque handles before dereference. |
| F-09 | High | Data Integrity | `crates/slateduck-catalog/src/export.rs:291-418`, `crates/slateduck-catalog/src/export.rs:548-584` | Import silently converts missing/malformed fields to `0`, empty strings, or decoded zero bytes. | Replace ad hoc `serde_json::Value` extraction with typed deserialization and validation errors. |
| F-10 | High | Rebuild | `crates/slateduck-catalog/src/export.rs:473-489` | `rebuild_catalog()` registers data files against `table_id = 1` but never creates a `TableRow`. | Create schema/table rows per inferred table and verify the rebuilt catalog before returning success. |
| F-11 | High | Scalability | `crates/slateduck-pgwire/src/executor.rs:74-85`, `crates/slateduck-pgwire/src/executor.rs:88-134` | PG-Wire holds the global `Mutex<CatalogStore>` across async reads, serializing concurrent sessions. | Clone the needed `Db`/reader state, drop the mutex guard before awaits, and load counters separately. |
| F-12 | High | Docs/Security | `crates/slateduck-pgwire/src/main.rs:200-218`, `crates/slateduck-pgwire/src/main.rs:888-921`, `docs/operations/cli-reference.md:44-50`, `docs/deployment/tls.md:49-55`, `docs/deployment/gcs.md:111-112` | Docs advertise auth flags, env vars, `--tls-required`, GCS/Azure, and encryption behavior that the CLI does not implement. | Either implement the documented surface or mark unsupported features clearly. |
| F-13 | High | CI/CD | `.github/workflows/ci.yml:24-34`, `.github/workflows/docs.yml:44` | CI runs fmt, clippy, tests, and strict docs, but has no coverage, security audit, cargo-deny, MSRV, sanitizer, or perf-regression gates. | Add staged quality gates before calling the project production-ready. |

---

## 1. Correctness & Bugs

### F-01: Stale `CatalogStore` counters corrupt later write sessions

- Severity: Critical
- Location: `crates/slateduck-catalog/src/store.rs:69-90`, `crates/slateduck-pgwire/src/executor.rs:427-495`
- Description: `CatalogStore::begin_write()` clones `self.counters` into a new `CatalogWriter`, and writer methods persist updated counters to SlateDB. The original `CatalogStore.counters` is never updated. `CatalogStore::read_latest()` also derives latest from this stale in-memory cache.
- Impact: Two write sessions from the same `CatalogStore` can reuse `snapshot_id`, `catalog_id`, and `file_id`. `SELECT max(snapshot)` can return stale data. PG-Wire `execute_commit()` creates a fresh writer per commit, so common client workflows are exposed.
- Recommendation: Route commits through a `CatalogStore::commit` API that updates in-memory counters only after transaction success, or reload counters from SlateDB before each writer session. Add regression tests for two sequential `begin_write()` calls on one store and for `read_latest()` after a commit.

### F-02: Catalog mutations are not atomic with snapshot publication

- Severity: Critical
- Location: `crates/slateduck-catalog/src/writer.rs:47-72`, `crates/slateduck-catalog/src/writer.rs:103-126`, `crates/slateduck-catalog/src/writer.rs:457-490`
- Description: Mutating operations (`create_schema`, `create_table`, etc.) each open and commit their own transaction using the current `peek_snapshot_id()`. The snapshot row is committed later by `create_snapshot()`.
- Impact: If a client buffers DDL/DML then fails before inserting the snapshot row, the catalog contains unpublished rows with the next snapshot ID. A later snapshot can accidentally publish those stale rows. This violates the claim that write operations produce new snapshots atomically.
- Recommendation: Treat a DuckLake transaction as the atomic unit. Buffer rows in memory and commit all row writes, counter updates, and the snapshot row in a single SlateDB transaction. If per-operation APIs remain, document them as staging APIs and prevent readers from observing staged snapshot IDs.

### F-03: PG-Wire `UPDATE end_snapshot` is implemented with wrong keys

- Severity: High
- Location: `crates/slateduck-pgwire/src/executor.rs:500-518`
- Description: For `ducklake_table`, the executor calls `drop_table(0, entity_id, begin_snapshot)`, using schema ID `0`. For `ducklake_column`, it calls `drop_column(entity_id, entity_id, begin_snapshot)`, using the same value for table ID and column ID.
- Impact: DuckLake deletes/updates can fail to find the intended row or mutate the wrong row if IDs collide. Since these are catalog lifecycle operations, this can leave dropped tables/columns visible or corrupt MVCC end snapshots.
- Recommendation: Parse or query the required key components before applying updates. For tables, resolve `(schema_id, table_id, begin_snapshot)` from existing visible rows. For columns, resolve `(table_id, column_id, begin_snapshot)` explicitly. Add tests covering table and column drops through PG-Wire.

### F-04: Import accepts corrupted catalog rows by defaulting fields

- Severity: High
- Location: `crates/slateduck-catalog/src/export.rs:291-418`, `crates/slateduck-catalog/src/export.rs:548-584`
- Description: `import_catalog()` extracts fields from `serde_json::Value` with `unwrap_or(0)`, `unwrap_or("")`, and `unwrap_or(true)`. Invalid base64 characters decode to zero because the decode table defaults to `0` for non-base64 bytes.
- Impact: A malformed NDJSON export can import rows under ID `0`, snapshot `0`, empty paths, empty names, or corrupted inlined payloads. The import reports success, making later verification difficult.
- Recommendation: Define per-table import structs with required fields and `serde` validation. Return `CatalogError::Value` or a new import error with line number/table name. Use the `base64` crate or an equivalent checked decoder.

### F-05: Rebuild creates data files for a table that does not exist

- Severity: High
- Location: `crates/slateduck-catalog/src/export.rs:473-489`
- Description: `rebuild_catalog()` creates a default schema and registers data files with `table_id = 1`, but it never writes a `TableRow` for table `1`.
- Impact: A rebuilt catalog can contain data files that cannot be reached through normal table discovery. `verify_catalog()` also does not currently catch this missing FK, so the command can appear successful while producing an unusable catalog.
- Recommendation: Create table rows for inferred tables before registering files, set counters from actual max IDs, and run verification before returning. Add a test that rebuild output can list schema, table, and data files through `CatalogReader`.

### F-06: Checkpoint restore reuses snapshot IDs and leaves future facts live

- Severity: High
- Location: `crates/slateduck-catalog/src/checkpoint.rs:102-118`
- Description: `restore_checkpoint()` only sets `next_snapshot_id` to `checkpoint.snapshot_id + 1`. It does not remove, hide, branch, or tombstone facts written after the checkpoint.
- Impact: After restoring snapshot 1, a new write can reuse snapshot 2 while old rows from the original snapshot 2 remain in the catalog and become visible in the new timeline. This is a correctness hazard for disaster recovery.
- Recommendation: Do not reuse snapshot IDs. Implement logical restore as a new snapshot that marks later facts ended, or make checkpoint restore purely read-only/time-travel. Add tests for writing after restore and confirming post-checkpoint facts do not reappear.

### F-07: Float NaN comparison treats incomparable values as equal

- Severity: Medium
- Location: `crates/slateduck-core/src/types.rs:90-97`, `crates/slateduck-core/src/types.rs:217-220`
- Description: `compare_floats()` uses `partial_cmp().unwrap_or(Ordering::Equal)`. Any comparison involving NaN becomes `Equal` unless the predicate itself is exactly `NaN`/`nan` and caught earlier.
- Impact: File pruning can keep or prune inconsistently when min/max stats contain NaN or non-normal float values. It also makes the comparison relation non-total and surprising.
- Recommendation: Handle NaN explicitly. For pruning, fail closed by returning `Keep` or an error instead of `Equal`. Add tests for NaN in predicate, min, and max stats.

### F-08: `pg_migrate()` generates unescaped SQL strings

- Severity: Medium
- Location: `crates/slateduck-catalog/src/export.rs:587-620`
- Description: SQL strings are built with `format!("... '{}' ...", value)` and no SQL literal escaping.
- Impact: Schema/table/column names containing `'` break migration output. If migration output is consumed automatically, malicious input can alter generated SQL.
- Recommendation: Use a SQL literal escaping helper or generate structured copy/import output instead of ad hoc SQL text.

---

## 2. Code Quality & Maintainability

### F-09: The public API separates staging and publishing poorly

- Severity: High
- Location: `crates/slateduck-catalog/src/writer.rs:47-490`
- Description: Public writer methods both persist rows and appear to be part of a higher-level transaction that is only finalized by `create_snapshot()`. This split makes it hard for callers to reason about atomicity.
- Impact: Client code can accidentally create persistent staged rows. The current API shape encourages correctness bugs in PG-Wire and FFI wrappers.
- Recommendation: Introduce an explicit transaction/builder API: stage operations in memory, then `commit_snapshot()`. Keep low-level row writes private or mark them as internal repair/rebuild primitives.

### F-10: Placeholder crates/features are shipped as workspace members

- Severity: Medium
- Location: `crates/slateduck-sqlite-vfs/src/lib.rs:1`, `crates/slateduck-sqlite-vfs/Cargo.toml:1-12`
- Description: `slateduck-sqlite-vfs` is a workspace crate with dependencies and dev-dependencies but only contains a placeholder module comment and has zero tests.
- Impact: It creates the impression of a supported integration and adds maintenance/build surface without behavior.
- Recommendation: Remove it from the workspace until implementation starts, or add a clear `experimental` feature gate plus compile-time docs explaining it is intentionally empty.

### F-11: Error types erase useful lower-level context

- Severity: Medium
- Location: `crates/slateduck-catalog/src/error.rs:9`, repeated `.map_err(|e| CatalogError::SlateDb(e.to_string()))` throughout catalog modules
- Description: SlateDB errors are collapsed into strings. Transaction conflicts, object-store errors, decode problems, and transient failures become hard to classify programmatically.
- Impact: Retrying, SQLSTATE mapping, alerting, and repair tooling cannot distinguish transient from permanent failures reliably.
- Recommendation: Preserve source errors using `#[source]` or structured variants where possible. Add error context for operation/table/key when mapping errors.

---

## 3. Performance & Scalability

### F-12: PG-Wire serializes concurrent reads behind a global mutex

- Severity: High
- Location: `crates/slateduck-pgwire/src/executor.rs:74-85`, `crates/slateduck-pgwire/src/executor.rs:88-134`
- Description: Read paths lock `Arc<Mutex<CatalogStore>>` and keep the guard alive while awaiting SlateDB scans.
- Impact: Concurrent sessions cannot independently list schemas/tables/files even though reads should scale horizontally. A slow prefix scan can block unrelated sessions and writes.
- Recommendation: Extract the reader or cloned `Db` while holding the mutex, then drop the guard before any await. Better, make `CatalogStore` internally cheap-to-clone for readers and reserve locking for write-state mutation.

### F-13: `describe_table()` scans every table in the catalog

- Severity: Medium
- Location: `crates/slateduck-catalog/src/reader.rs:68-92`, `crates/slateduck-core/src/keys.rs:108-116`, `crates/slateduck-core/src/keys.rs:442-448`
- Description: Table keys are shaped by `schema_id | table_id | begin_snapshot`, but `describe_table(table_id)` only has `table_id`, so it scans all `TAG_TABLE` rows and filters after decode.
- Impact: Describing a table is O(number of table versions). Large catalogs will pay avoidable scan latency in common planning paths.
- Recommendation: Require schema ID in the API, add a secondary table-id index, or maintain packed metadata keyed by table ID.

### F-14: DataFusion sync methods bridge async work by spawning threads

- Severity: Medium
- Location: `crates/slateduck-datafusion/src/catalog_provider.rs:60-88`, `crates/slateduck-datafusion/src/catalog_provider.rs:126-159`
- Description: `schema_names()` and `table_names()` call `Handle::try_current()`, spawn a thread, then `block_on()` async reads. If no Tokio runtime exists, they silently return empty lists.
- Impact: Behavior depends on caller context. Query planning outside a Tokio runtime appears as an empty catalog, and repeated planning creates thread overhead.
- Recommendation: Use DataFusion async provider hooks where available, maintain a metadata cache, or own a runtime inside the provider and return explicit errors/log warnings instead of empty lists.

### F-15: DataFusion `scan()` always returns `EmptyExec`

- Severity: Medium
- Location: `crates/slateduck-datafusion/src/catalog_provider.rs:287-298`
- Description: Table discovery returns schemas, but scanning any table returns an empty execution plan.
- Impact: DataFusion queries over SlateDuck tables return zero rows, making the integration metadata-only despite being presented as a DataFusion integration.
- Recommendation: Either implement Parquet reading using catalog data files, or rename/document the crate as a metadata provider and make scan return an explicit unsupported error.

---

## 4. Security

### F-16: Configured PG-Wire authentication is not enforced

- Severity: Critical
- Location: `crates/slateduck-pgwire/src/handler.rs:11`, `crates/slateduck-pgwire/src/handler.rs:37-61`
- Description: `AuthConfig` is stored in the handler, but startup uses `NoopStartupHandler`.
- Impact: Operators can deploy with `--username`/`--password` believing the catalog is protected, while any client can connect.
- Recommendation: Implement cleartext password or SCRAM auth through pgwire's startup interfaces. Deny by default when auth is configured. Add end-to-end tests with valid and invalid credentials.

### F-17: FFI entrypoints can dereference invalid pointers

- Severity: High
- Location: `crates/slateduck-ffi/src/lib.rs:203`, `crates/slateduck-ffi/src/lib.rs:274`, `crates/slateduck-ffi/src/lib.rs:310`, `crates/slateduck-ffi/src/lib.rs:350`, `crates/slateduck-ffi/src/lib.rs:388`
- Description: `slateduck_open()` calls `CStr::from_ptr(uri)` before checking for null. Read functions dereference `catalog` without checking null or ownership.
- Impact: A malformed DuckDB extension call, wrong binding, or double-close can segfault or cause undefined behavior inside Rust.
- Recommendation: Make all extern functions defensive: null-check inputs, return structured errors, validate handle ownership with an opaque magic/version field, and add sanitizer/Miri coverage.

### F-18: TLS/auth CLI and docs diverge from implementation

- Severity: High
- Location: `crates/slateduck-pgwire/src/main.rs:200-218`, `docs/operations/cli-reference.md:44-50`, `docs/deployment/tls.md:49-55`, `docs/deployment/tls.md:376-385`
- Description: Docs advertise `--auth-user`, `--auth-password`, env vars, and `--tls-required`. Code parses `--username`/`--password`, does not read the documented env vars, and has no `--tls-required` option.
- Impact: Production deployments can be misconfigured silently. Some documented hardening steps cannot actually be applied.
- Recommendation: Align CLI parser, docs, and tests. Reject partial TLS/auth config instead of silently disabling features.

### F-19: Encryption is configuration-only, not wired into storage

- Severity: Medium
- Location: `crates/slateduck-catalog/src/encryption.rs:1-31`, `crates/slateduck-pgwire/src/main.rs:200-203`
- Description: `EncryptionConfig` validates a key, and CLI accepts `--encryption-key`, but the parsed key is discarded and `CatalogStore::open()` has no encryption option.
- Impact: Operators can believe at-rest catalog encryption is enabled when no encryption is applied by SlateDuck.
- Recommendation: Either wire encryption into SlateDB open options and test encrypted round trips, or remove/mark the flag and docs as planned.

---

## 5. Test Coverage & Quality

### F-20: Tests miss the highest-risk writer-state bugs

- Severity: High
- Location: `crates/slateduck-catalog/tests/v09_tests.rs:444-454`, `crates/slateduck-catalog/tests/integration_tests.rs:24-41`
- Description: Existing tests mostly keep one `CatalogWriter` alive. The failover test creates fresh writers, but it only checks that schemas are non-empty, not that snapshot IDs advanced uniquely or that `read_latest()` returns the true latest.
- Impact: The stale counter/read-latest bug passes the current suite.
- Recommendation: Add tests for sequential write sessions on one `CatalogStore`, `SELECT max(snapshot)` after every commit, snapshot ID monotonicity after failover, and aborted write sessions.

### F-21: Security tests are config-only, not protocol tests

- Severity: High
- Location: `crates/slateduck-pgwire/tests/integration_tests.rs:876-915`
- Description: TLS/Auth tests only check `is_enabled()` on config structs. They do not connect with invalid credentials or verify TLS handshakes/required plaintext rejection.
- Impact: The no-op authentication handler remains undetected.
- Recommendation: Add end-to-end PG client tests for no-auth, valid auth, invalid auth, TLS success, TLS bad cert, and TLS-required plaintext rejection once implemented.

### F-22: FFI and DataFusion coverage is too shallow for their risk

- Severity: Medium
- Location: `crates/slateduck-ffi/src/lib.rs:585-640`, `crates/slateduck-datafusion/tests/integration_tests.rs:1-127`
- Description: FFI has four basic tests and no invalid pointer/double-free/error-cleanup tests. DataFusion has five happy-path metadata tests and no scan, no-runtime, or concurrency tests.
- Impact: Memory-safety and integration behavior gaps are likely to survive CI.
- Recommendation: Add FFI safety tests using C-compatible calls and sanitizer jobs. Add DataFusion tests for actual query execution, missing runtime behavior, concurrent planning, and unsupported scan semantics.

### F-23: SQLite VFS has zero implementation tests because it has no implementation

- Severity: Medium
- Location: `crates/slateduck-sqlite-vfs/src/lib.rs:1`
- Description: The crate has no code beyond a placeholder comment.
- Impact: Roadmap and README references to SQLite VFS/native extension paths can be misread as available.
- Recommendation: Remove from supported surface or add a tracked implementation plan and failing/ignored acceptance tests.

---

## 6. Error Handling & Observability

### F-24: Silent defaults turn client/protocol errors into wrong answers

- Severity: Medium
- Location: `crates/slateduck-pgwire/src/executor.rs:90-129`, `crates/slateduck-pgwire/src/executor.rs:380-394`
- Description: Missing or invalid parameters become `0`, `u64::MAX`, empty strings, false, or true depending on the path.
- Impact: A protocol bug or unexpected DuckDB query can read the wrong snapshot/table or write corrupt rows instead of returning a clear SQLSTATE.
- Recommendation: Replace defaults with explicit parameter validation for every classified statement. Default only when PostgreSQL/DuckDB semantics require it.

### F-25: Operational features lack tracing spans and metrics on critical paths

- Severity: Medium
- Location: Catalog reader/writer modules broadly; `crates/slateduck-catalog/src/metrics.rs` exists but is not integrated into writer/read paths.
- Description: Metrics support exists, but core operations do not consistently emit spans/counters for snapshot commits, conflicts, scan sizes, GC/excision actions, auth failures, or FFI errors.
- Impact: Production incidents will be hard to diagnose and SLOs cannot be measured from the process itself.
- Recommendation: Add `tracing::instrument` on commit/read/maintenance entrypoints and metrics for latency, row counts scanned, conflicts, auth failures, and repair/excision outcomes.

---

## 7. Documentation & Developer Experience

### F-26: Documentation is ahead of implementation in several production areas

- Severity: High
- Location: `README.md:23`, `docs/deployment/gcs.md:111-112`, `docs/operations/cli-reference.md:44-50`, `docs/operations/cli-reference.md:486-496`, `docs/deployment/tls.md:49-55`
- Description: Docs describe GCS/Azure support, auth env vars, S3 endpoint flags, read-only flags, metrics bind flags, and TLS-required behavior that are absent or differently named in code.
- Impact: Operators following docs can deploy with no auth, no encryption, unsupported storage URLs, or unexpected defaults.
- Recommendation: Add doc-code conformance tests for CLI help and supported URL schemes. Mark unsupported sections as planned, or implement them before release claims.

### F-27: Roadmap status overstates current implementation quality

- Severity: Medium
- Location: `ROADMAP.md:43-57`
- Description: v0.4 through v0.9 are marked Done, including production hardening, security, performance, DataFusion, Kubernetes, and failover. The implementation still has critical gaps in those exact areas.
- Impact: Contributors and users may treat the project as GA-adjacent when several features are scaffolds or docs-only.
- Recommendation: Convert the roadmap from phase-complete to acceptance-criteria-complete. Add release gates tied to tests, security scans, benchmarks, docs conformance, and operational drills.

---

## 8. Dependency & Supply Chain Health

### F-28: No automated dependency security policy is enforced

- Severity: Medium
- Location: `.github/workflows/ci.yml:24-34`, repository root lacks `deny.toml`/audit config
- Description: CI does not run `cargo audit`, `cargo deny`, license checks, advisory checks, or duplicate-version policy.
- Impact: Vulnerable or disallowed transitive dependencies can enter unnoticed.
- Recommendation: Add `cargo deny check advisories bans licenses sources` and `cargo audit` to CI, with documented exceptions.

### F-29: MSRV and dependency feature policy are undefined

- Severity: Medium
- Location: `Cargo.toml:10-45`
- Description: No `rust-version` is declared. Workspace dependencies enable broad features such as `tokio = { features = ["full"] }` and `object_store = { features = ["aws", "gcp", "azure"] }` for all consumers.
- Impact: Builds can change behavior as stable Rust advances, and downstream users pay for unused cloud/runtime features.
- Recommendation: Declare MSRV in workspace package metadata, test it in CI, and split optional cloud/runtime features by crate.

---

## 9. Architecture & Design Gaps

### F-30: Writer fencing is not sufficient without atomic writer state

- Severity: High
- Location: `crates/slateduck-catalog/src/store.rs:43-51`, `crates/slateduck-catalog/src/writer.rs:876-889`
- Description: Writer epoch checks protect against stale writers, but they do not solve the stale `CatalogStore` counters or the multi-transaction snapshot publish model.
- Impact: The design can still produce split timelines or ID reuse inside a single process/session even when epoch checks pass.
- Recommendation: Define a single write protocol: acquire writer epoch, load counters, stage changes, commit rows/snapshot/counters atomically, update in-memory state, publish observability event.

### F-31: Retention, restore, and excision semantics conflict with immutability docs

- Severity: High
- Location: `README.md:27-31`, `crates/slateduck-catalog/src/gc.rs:91-120`, `crates/slateduck-catalog/src/excise.rs:94-132`, `crates/slateduck-catalog/src/checkpoint.rs:102-118`
- Description: Docs promise immutable time travel and safe excision. GC says it hides snapshots but readers ignore it. Excision can run before retention is advanced. Checkpoint restore reuses IDs.
- Impact: Operators cannot reason safely about historical reads, compliance erasure, or recovery timelines.
- Recommendation: Write a formal state-machine spec for snapshot lifecycle: committed, retained, hidden, excised, restored/branched. Then make code and tests enforce it.

### F-32: DataFusion and native extension APIs are not feature-complete integrations

- Severity: Medium
- Location: `crates/slateduck-datafusion/src/catalog_provider.rs:287-298`, `crates/slateduck-ffi/src/lib.rs:196-235`
- Description: DataFusion exposes metadata but cannot scan data. FFI only supports local filesystem paths despite roadmap/native extension positioning.
- Impact: Ecosystem integrations are not yet production-grade and should not be advertised as complete.
- Recommendation: Define integration acceptance tests: open catalog, list metadata, read real data files, handle errors, work under cloud object-store URLs, and pass ABI compatibility checks.

---

## 10. CI/CD & Build System

### F-33: CI lacks world-class quality gates

- Severity: High
- Location: `.github/workflows/ci.yml:24-34`, `.github/workflows/compatibility.yml:1-37`, `.github/workflows/docs.yml:44`
- Description: CI currently covers formatting, clippy, tests, compatibility replay, object-store builder validation, and strict docs. It does not measure coverage, run security audits, run sanitizers, verify MSRV, run benchmark regression checks, or validate documented CLI flags.
- Impact: Important regressions can pass CI, including the authentication bypass and stale-counter correctness issue.
- Recommendation: Add separate jobs for `cargo llvm-cov`, `cargo deny`, `cargo audit`, MSRV, FFI sanitizers, documented CLI smoke tests, and criterion benchmark comparison.

### F-34: Release automation and production acceptance criteria are incomplete

- Severity: Medium
- Location: repository root and `.github/workflows/` lack release workflow/changelog automation
- Description: No release workflow was found for signed artifacts, checksums, crates publishing, binary publishing, or benchmark/security sign-off.
- Impact: Reproducibility and user trust suffer as the project approaches GA.
- Recommendation: Add a release workflow that builds pinned artifacts, publishes checksums/SBOM, signs tags, runs full quality gates, and ties release notes to a changelog.

---

## Gaps vs. Roadmap

- v0.4 Production Hardening: Partially implemented. GC, excision, repair, checkpoint, encryption, and metrics modules exist, but GC is not enforced by readers, excision safety is wrong for `retain_from == 0`, checkpoint restore is unsafe for future writes, encryption is not wired into storage, and metrics are not integrated into key paths.
- v0.5 Native Extension: FFI exists but is not safe enough for hostile or buggy callers. It is local-filesystem only and needs null-pointer, double-free, sanitizer, ABI-version, and large-result tests before being treated as production.
- v0.6 Multi-Client & Security: TLS config exists, but auth is a no-op and documented CLI/env flags do not match code. GCS/Azure docs exist, but CLI resolution supports only S3 and local paths.
- v0.7 Performance & Ecosystem: Benchmarks and DataFusion code exist, but DataFusion cannot scan rows and PG-Wire serializes reads under a mutex. Benchmark regression protection is not in CI.
- v0.8 Documentation: Documentation volume is strong, but doc-code drift is now a risk. Docs need conformance tests and unsupported feature labels.
- v0.9 Production Readiness: Not yet. The core write protocol, security enforcement, recovery semantics, and CI gates need to be corrected before GA planning.

## Prioritized Action Plan

1. Fix the write protocol: make counters, row mutations, snapshot rows, and schema version updates commit atomically and update `CatalogStore` state after success.
2. Add regression tests for sequential write sessions, failover monotonicity, aborted write sessions, `read_latest()`, and PG-Wire `SELECT max(snapshot)`.
3. Implement PG-Wire authentication or remove all auth claims until it is real. Add end-to-end valid/invalid credential tests.
4. Fix destructive lifecycle semantics: enforce `retain-from`, block excision at `retain_from == 0`, and redesign checkpoint restore to avoid snapshot ID reuse.
5. Harden FFI: null checks, handle validation, documented ownership contract, sanitizer CI, and invalid-input tests.
6. Replace silent import/parameter defaults with structured validation errors.
7. Align docs, CLI, and supported backends. Add CLI help/doc conformance tests.
8. Remove or clearly gate placeholder integrations (`sqlite-vfs`, metadata-only DataFusion scan) until their acceptance tests pass.
9. Add CI quality gates: coverage, cargo-deny/audit, MSRV, sanitizer, benchmark regression, and docs conformance.
10. Define v1.0 acceptance criteria in measurable terms: correctness invariants, security checks, recovery drills, benchmark thresholds, and supported deployment matrix.

## Verification Notes

- `cargo test --workspace` passed in this environment.
- No code fixes were made as part of this assessment.
- This report intentionally prioritizes issues that are observable in source/tests/docs over speculative improvements.
