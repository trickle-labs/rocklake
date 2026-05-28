# RockLake Overall Assessment - Report 1

**Date:** 2026-05-28
**Codebase version:** 0.27.14
**Scope:** Full codebase analysis across correctness, concurrency, security, performance, code quality, tests, API ergonomics, DuckLake conformance, operations, and documentation.

---

## Executive Summary

RockLake has a strong architectural foundation and a broad test suite, but the current codebase still contains several release-blocking issues for a world-class GA posture. The most serious issue is writer fencing: `CatalogStore::open()` derives the writer epoch from wall-clock milliseconds, and the stale-writer tests explicitly sleep to avoid same-millisecond opens. Two writers that compute the same epoch can both pass `check_epoch()`, defeating the intended single-writer guarantee.

The operational migration and backup paths are also not GA-ready. `export-catalog` claims to export all 28 DuckLake catalog tables, but the implementation exports only a small subset and omits many fields. `import_catalog()` writes primary data-file keys but not the secondary `TAG_DATA_FILE_BY_SNAPSHOT` index that `list_data_files()` now depends on, so imported catalogs can appear empty to readers. `rebuild_catalog()` writes a new catalog through many independent `db.put()` calls instead of an atomic batch/transaction.

The PG-wire COPY bootstrap parser silently accepts truncated binary COPY streams and returns partial rows as success. That is a data-corruption class bug: a network interruption or malformed stream can mark bootstrap state as complete with only part of the catalog loaded. Separately, DataFusion integration has multiple silent-empty-result paths (`unwrap_or_default()` and `EmptyExec`) that can convert catalog or storage failures into apparently valid zero-row query results.

Security and protocol posture is generally thoughtful (SCRAM/TLS code exists, SQLSTATE mapping is centralized, clippy passes strictly), but some important edge cases remain: GC lease checks are documented as transactional but are performed outside the transaction, virtual-catalog mutations are not classified into the promised `25006` read-only error path, parameter type errors echo raw parameter values, and length-prefixed extension/lease keys silently truncate long identifiers.

Verification commands run during this assessment:

- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace --all-targets` failed: `rocklake-catalog --test integration_tests` was killed by `SIGKILL` during the full workspace run.
- `cargo test -p rocklake-catalog --test integration_tests -- --test-threads=1` passed, indicating the failure is likely resource/concurrency pressure in the test harness rather than a deterministic test failure.

---

## Severity Legend

| Severity | Meaning |
|---|---|
| P0 - Blocker | Causes data loss, corruption, security breach, or silent wrong results |
| P1 - Critical | Will cause production failures under realistic load or configuration |
| P2 - Major | Degrades reliability, performance, or maintainability significantly |
| P3 - Minor | Polish, ergonomics, or documentation gaps |

---

## Area 1: Correctness & Bugs

### Findings

**[P0] Millisecond wall-clock writer epochs can let two writers share the fencing token**

- File: `crates/rocklake-catalog/src/store.rs` (lines 90-130)
- File: `crates/rocklake-catalog/src/writer/snapshot.rs` (lines 198-213)
- File: `crates/rocklake-catalog/tests/concurrent_writer_fencing.rs` (lines 34-83)
- Description: `CatalogStore::open()` computes `writer_epoch` from `SystemTime::now().as_millis()` and writes it to `SYSTEM_WRITER_EPOCH`. If another writer opens in the same millisecond, it can compute the same epoch. On retry after a transaction conflict, the code only rejects `existing > writer_epoch`; equal epochs are accepted. `check_epoch()` later accepts equality (`stored_epoch != self.writer_epoch`), so both writers can believe they are current. The test suite sleeps for 2 ms before opening another writer, which avoids exactly this collision case.
- Impact: The single-writer guarantee can fail under fast restarts, colocated processes, high-resolution scheduling, or tests running on fast machines. That can lead to concurrent staged writes sharing a fencing token and producing silent catalog corruption or non-deterministic transaction conflicts.
- Fix: Allocate writer epochs from a transactionally incremented monotonic counter (`current_epoch + 1`) or use a unique writer UUID/nonce stored with the epoch. Reject equality from a different writer identity. Add a no-sleep test that forces same-millisecond epoch acquisition.

**[P1] Imported catalogs miss the secondary data-file index used by readers**

- File: `crates/rocklake-catalog/src/export.rs` (lines 421-443)
- File: `crates/rocklake-catalog/src/reader.rs` (lines 281-313)
- File: `crates/rocklake-catalog/src/writer/mod.rs` (lines 668-675)
- Description: `import_catalog()` restores `ducklake_data_file` rows only at `keys::key_data_file(table_id, data_file_id)`. Current reads use `reader.list_data_files()`, which scans `prefix_data_files_by_snapshot_for_table()` and therefore depends on the secondary `key_data_file_by_snapshot()` entries that normal writes create.
- Impact: A catalog restored from NDJSON can contain data-file rows but return no files to query paths that use `list_data_files()`. This is silent wrong results after disaster recovery or migration.
- Fix: On import, write both the canonical data-file key and the secondary `key_data_file_by_snapshot(table_id, begin_snapshot, data_file_id)` key. Add an export/import round-trip test that queries `list_data_files()` and scans data after import.

**[P1] Export includes retired data/delete files at historical snapshots**

- File: `crates/rocklake-catalog/src/export.rs` (lines 181-240)
- Description: `export_catalog()` checks only `begin <= snapshot` for data files and delete files. It does not apply `end_snapshot IS NULL OR end_snapshot > snapshot`, unlike `reader.list_data_files()`.
- Impact: NDJSON exports advertised as point-in-time live catalog snapshots can include rows retired before the target snapshot. Restoring from such an export can resurrect old data files or deletes.
- Fix: Use the same MVCC predicate as readers for every versioned row: `begin_snapshot <= snapshot && (end_snapshot IS NULL || end_snapshot > snapshot)`. Add regression tests with files created, retired, exported at a later snapshot, and re-imported.

**[P1] Binary COPY parser silently accepts truncated streams**

- File: `crates/rocklake-pgwire/src/copy_parser.rs` (lines 27-83)
- File: `crates/rocklake-pgwire/src/handler.rs` (lines 94-129)
- Description: `parse_binary_copy_rows()` returns `Vec<_>` and explicitly documents that it "Silently returns whatever rows were decoded before any parse error." Truncated field counts or field payloads return partial rows. `on_copy_done()` treats those rows as complete and updates bootstrap state.
- Impact: A truncated or malformed binary COPY stream can be accepted as successful catalog bootstrap, leaving missing schemas or snapshot rows without surfacing an error to DuckDB.
- Fix: Change the parser to return `Result<Vec<_>, CopyParseError>`, error on every truncation or malformed header, and propagate the failure through `on_copy_done()` with a PostgreSQL protocol error. Add tests for truncation after signature, mid-field-count, mid-length, and mid-field-body.

**[P2] Checkpoint restore advances the snapshot counter even when no post-checkpoint facts exist**

- File: `crates/rocklake-catalog/src/checkpoint.rs` (lines 99-138)
- Description: `restore_checkpoint()` sets `hide_snapshot` to the current `next_snapshot_id` or `meta.snapshot_id + 1`, hides facts only when `hide_snapshot > meta.snapshot_id + 1`, but always writes `next_snapshot_id = hide_snapshot + 1`.
- Impact: Restoring a checkpoint where no later snapshot exists creates an unnecessary snapshot-ID gap. This is not immediate corruption, but it makes operational reasoning and audit trails noisier.
- Fix: If no post-checkpoint facts are hidden, leave `next_snapshot_id` at `meta.snapshot_id + 1`; only advance past `hide_snapshot` when that snapshot is actually used as a tombstone boundary.

---

## Area 2: Concurrency & Correctness Under Load

### Findings

**[P1] GC lease and pin checks are documented as transactional but run outside the transaction**

- File: `crates/rocklake-catalog/src/gc.rs` (lines 91-154)
- File: `crates/rocklake-catalog/src/lease.rs` (lines 60-99)
- Description: The `gc_apply()` documentation says retain-from read, pin scan, lease scan, and retain-from write are wrapped in one `SerializableSnapshot` transaction. In the implementation, only the retain-from read and write use `tx`; `read_pinned_snapshots(db)` and `minimum_leased_snapshot(db)` scan through the database handle outside the transaction.
- Impact: A reader can acquire a snapshot lease between the non-transactional lease scan and the transactional retain-from write. The GC transaction will not conflict and can advance retention past an active reader.
- Fix: Add transaction-aware helpers for pinned snapshots and active leases, and read them through the same `DbTransaction` used to write retain-from. Add a concurrency test that acquires a lease during `gc_apply()` and expects a conflict/block.

**[P2] Rebuild writes catalog state through many independent puts**

- File: `crates/rocklake-catalog/src/export.rs` (lines 530-614)
- Description: `rebuild_catalog()` initializes counters and then writes schema, table, data-file rows, secondary data-file index rows, snapshot row, and counters through sequential `db.put()` calls. There is no `WriteBatch` or transaction around the operation.
- Impact: A crash, process kill, object-store error, or writer conflict midway can leave a partially rebuilt catalog that looks initialized but is missing rows or counters.
- Fix: Stage the entire rebuild into one atomic batch/transaction and commit only after all rows are prepared. For large rebuilds, write an explicit rebuild staging marker and commit in recoverable chunks with a final manifest flip.

**[P2] Test suite relies on wall-clock sleeps for writer-fencing ordering**

- File: `crates/rocklake-catalog/tests/concurrent_writer_fencing.rs` (lines 34-83)
- File: `crates/rocklake-catalog/tests/v028_atomicity_tests.rs` (lines 102-180)
- Description: Multiple writer-fencing and atomicity tests use `tokio::time::sleep(Duration::from_millis(2))` to ensure the clock advances. This encodes a timing assumption into correctness tests.
- Impact: The tests avoid the same-millisecond writer-epoch bug instead of detecting it. They can also become flaky on slow CI or under time virtualization.
- Fix: Inject a deterministic clock/epoch allocator in tests and add an explicit same-tick collision case.

---

## Area 3: Security

### Findings

**[P2] Parameter type errors echo raw parameter values**

- File: `crates/rocklake-sql/src/params.rs` (lines 16-60)
- File: `crates/rocklake-sql/src/error.rs` (lines 3-18)
- Description: `get_u64()`, `get_i64()`, and `get_bool()` include `actual: val.to_string()` in `TypeMismatch`. The error display renders `got {actual}`.
- Impact: If a client sends a secret, token, file path, or credential in the wrong parameter slot, the value can be returned to the client and written into logs.
- Fix: Replace `actual` with a type/category and maybe length, e.g. `got non-numeric string (len=...)`. Do not echo raw parameter values in client-facing errors.

**[P2] Length-prefixed extension and lease keys silently truncate long identifiers**

- File: `crates/rocklake-core/src/keys.rs` (lines 520-566)
- Description: `key_snapshot_lease()`, `key_extension_schema()`, and `prefix_extension_table()` compute `len = bytes.len().min(u16::MAX as usize)` and append only the truncated prefix.
- Impact: Two consumer IDs or extension table names that share the first 65,535 bytes collide in storage. This is unlikely during normal use but reachable from externally supplied strings and violates the stated "collision-safe key encoding" goal.
- Fix: Reject identifiers longer than `u16::MAX` before key construction, or hash the full identifier into a collision-resistant suffix while preserving a bounded prefix.

**[P2] Virtual catalog mutation errors do not follow the promised read-only SQLSTATE path**

- File: `crates/rocklake-sql/src/classifier/mod.rs` (lines 267-269)
- File: `crates/rocklake-sql/src/classifier/ast.rs` (lines 39-92, 358-425)
- File: `crates/rocklake-pgwire/src/error.rs` (lines 30-76)
- Description: `StatementKind::VirtualCatalogScan` documents that mutations against `rocklake_catalog.*` return `SQLSTATE 25006`. The classifier only recognizes `rocklake_catalog.*` for SELECTs. INSERT and DELETE on a schema-qualified table are routed as extension-table operations, while UPDATE becomes unsupported.
- Impact: Clients and monitoring that rely on `25006` for read-only catalog protection receive `42501` or `0A000` instead. This is a protocol and access-control semantics gap, not a direct arbitrary-write path.
- Fix: Add explicit INSERT/UPDATE/DELETE classification for `rocklake_catalog.*` and map it to `RockLakeError::ReadOnlyReplica`.

---

## Area 4: Performance & Scalability

### Findings

**[P1] DataFusion converts catalog errors into empty schemas/tables/files**

- File: `crates/rocklake-datafusion/src/catalog_provider.rs` (lines 198-217, 277-292, 330-340)
- Description: `schema_names()` and `table_names()` use `unwrap_or_default()` on catalog reads, and table construction uses `list_data_files(...).await.unwrap_or_default()`.
- Impact: I/O failures, decode errors, retention errors, or catalog corruption can produce valid-looking empty DataFusion results instead of query errors. This is silent wrong results.
- Fix: Propagate catalog errors as `DataFusionError::External` where the DataFusion trait allows it. Where the trait returns `Vec<String>`, log at error level with full context and expose a health/error state rather than pretending the catalog is empty.

**[P1] DataFusion returns `EmptyExec` when data files exist but the data root is unsupported**

- File: `crates/rocklake-datafusion/src/catalog_provider.rs` (lines 430-449)
- Description: `scan()` returns `EmptyExec` if there are no Parquet files or `data_root` is `None`. Those are different states: no files is a valid empty table, but data files with no readable root means the scan cannot be executed.
- Impact: S3/GCS/Azure or misconfigured local catalogs can return zero rows despite having registered Parquet files.
- Fix: Return an explicit `DataFusionError` when `parquet_files` is non-empty and `data_root` is `None`. Add tests for non-local object stores and missing `data_path` metadata.

**[P2] DataFusion local root extraction parses `Display` output**

- File: `crates/rocklake-datafusion/src/catalog_provider.rs` (lines 145-170)
- Description: `open()` derives `data_root` by formatting the `ObjectStore` and parsing strings like `LocalFileSystem(file:///path/)` or `LocalFileSystem(root=/path)`.
- Impact: A change in `object_store` Display formatting breaks Parquet resolution. The failure can cascade into empty scans.
- Fix: Carry the local root path explicitly in the provider builder, or use a stable accessor/wrapper instead of parsing Display.

**[P2] AsyncBridge panics on runtime, thread, or channel failure**

- File: `crates/rocklake-datafusion/src/catalog_provider.rs` (lines 35-77)
- Description: `AsyncBridge::new()` uses `expect()` for Tokio runtime creation and thread spawn. `run_sync()` uses `expect()` when sending tasks and receiving results.
- Impact: Thread limits, sandbox restrictions, worker panic, or shutdown can crash the embedding process instead of returning a query error.
- Fix: Make bridge construction fallible and store/report bridge errors as `DataFusionError`. Avoid `expect()` in library code that can run inside another host process.

---

## Area 5: Code Quality & Maintainability

### Findings

**[P2] FFI string conversion silently replaces invalid C strings with empty strings**

- File: `crates/rocklake-ffi/src/lib.rs` (lines 69-86, 263-304, 438-444, 491-497, 543-551, 612-619)
- Description: The FFI layer repeatedly uses `CString::new(...).unwrap_or_default()` for error messages and returned schema/table/column/file strings.
- Impact: Embedded NUL bytes in catalog strings turn into empty C strings. C callers cannot distinguish a real empty string from conversion failure, and error messages can disappear.
- Fix: Centralize `CString` conversion in a helper that returns a `RockLakeError` on embedded NUL. For error messages, use a guaranteed static fallback string instead of an empty string.

**[P2] C++ extension wrapper is a non-functional stub despite loadable entry points**

- File: `extension/src/rocklake_extension.cpp` (lines 1-15, 132-165)
- Description: The file advertises `ATTACH 'ducklake:slatedb:///path/to/catalog' AS lake`, but `rocklake_extension_init()` only checks ABI and returns true. Catalog registration is left as a comment.
- Impact: Users can load the extension but cannot use the advertised native attach path. This is especially risky because older roadmap text marked Strategy C as done.
- Fix: Until DuckDB catalog registration is implemented, make docs and extension metadata say this is an ABI smoke wrapper only. In v0.36, implement registration or fail clearly when the feature is unavailable.

**[P3] Public export/import module suppresses missing-docs while exposing public API**

- File: `crates/rocklake-catalog/src/export.rs` (lines 1-36)
- Description: The module has `#![allow(missing_docs)]`, but exposes public structs and functions used by CLI recovery paths.
- Impact: Public recovery APIs are harder to use safely and are not documented at the code boundary where callers need exact guarantees.
- Fix: Remove the module-level allowance and document all public structs, fields, and functions, especially atomicity and completeness limitations.

---

## Area 6: Test Coverage & Test Quality

### Findings

**[P1] Export/import tests do not validate restored read behavior**

- File: `crates/rocklake-catalog/tests/v04_tests.rs` (lines 300-337)
- File: `crates/rocklake-catalog/src/export.rs` (lines 421-443)
- Description: The export/import round-trip test asserts only that `rows_imported == rows_exported`. It does not open a `CatalogStore`, call `list_data_files()`, read tables, verify counters, or run a query against the imported catalog.
- Impact: The missing secondary-index bug in `import_catalog()` is not caught, and restored catalogs can pass tests while being unreadable for data scans.
- Fix: Extend the test to query schemas, tables, columns, data files, and run a DataFusion or reader scan after import.

**[P2] Full workspace test suite can be killed under default execution**

- File: `crates/rocklake-catalog/tests/integration_tests.rs` (entire integration suite)
- File: `crates/rocklake-catalog/tests/concurrent_writer_fencing.rs` (lines 34-83)
- Description: `cargo test --workspace --all-targets` failed with `SIGKILL` while running `rocklake-catalog --test integration_tests`. Running that test binary serially passed (`11 passed`).
- Impact: CI may be nondeterministic or require more resources than expected. A killed test process gives little diagnostic information and can mask real failures.
- Fix: Identify memory/thread-heavy tests, configure test concurrency in CI, and add resource metrics/logging for large integration suites.

**[P2] FFI has no C/C++ ABI integration test**

- File: `crates/rocklake-ffi/Cargo.toml` (lines 1-13)
- File: `crates/rocklake-ffi/src/lib.rs` (lines 754-930)
- File: `extension/src/rocklake_extension.cpp` (lines 41-126)
- Description: Rust-side tests call exported functions from Rust, but there is no external C or C++ test that compiles against `rocklake.h`, links the produced library, and validates struct layout, calling convention, and free functions.
- Impact: ABI regressions can pass Rust tests and fail only for real consumers.
- Fix: Add a small C or C++ smoke test in CI that opens a catalog, lists schemas, handles an error, and closes/free all returned structures.

---

## Area 7: API Ergonomics & Developer Experience

### Findings

**[P2] `rocklake-client` roadmap foundation is not present in the workspace**

- File: `Cargo.toml` (lines 1-10)
- File: `ROADMAP.md` (lines 86-87, 3667-3755)
- Description: The roadmap now correctly identifies `v0.35.0 - Embedded Catalog Client Library` as the foundation for the native extension, but the workspace currently has no `crates/rocklake-client` member.
- Impact: Non-DuckDB embedders must choose between low-level `rocklake-catalog`, synchronous C FFI, or PG-wire. There is no idiomatic high-level Rust client API yet.
- Fix: Implement the v0.35.0 `rocklake-client` crate before expanding native clients. Keep the C ABI and DuckDB extension as consumers of that foundation.

**[P2] CLI rebuild swallows object-store listing errors and can report success with zero files**

- File: `crates/rocklake-pgwire/src/main.rs` (lines 668-686)
- Description: `cmd_rebuild()` calls `object_store.list(...).try_collect().await.unwrap_or_default()`. Any list failure becomes an empty object list, then `rebuild_catalog()` runs and prints success.
- Impact: Bad credentials, missing buckets, network failures, or permission errors can produce an apparently successful empty rebuild.
- Fix: Propagate list errors with context and fail the command. Add a test with a failing object store/list operation.

**[P3] FFI ownership contract is in docs but not visible from the C header/API surface**

- File: `crates/rocklake-ffi/src/lib.rs` (lines 1-10, 339-357, 640-730)
- File: `extension/include/rocklake.h` (entire header)
- Description: The Rust file has safety comments, and `docs/architecture/ffi-safety.md` exists, but C callers reading the header do not get a concise ownership table and thread-safety contract at the API boundary.
- Impact: C/C++ consumers can easily misuse list free functions or close handles concurrently.
- Fix: Generate or hand-maintain a documented `rocklake.h` with ownership, nullability, thread-safety, and error-lifetime comments for every exported function.

---

## Area 8: DuckLake v1.0 Specification Conformance

### Findings

**[P1] `export-catalog` says it exports all 28 DuckLake tables, but implementation exports a subset**

- File: `crates/rocklake-pgwire/src/main.rs` (lines 72-85, 1214-1240)
- File: `crates/rocklake-catalog/src/export.rs` (lines 60-275)
- Description: CLI help and command output say "Export all 28 catalog tables." `export_catalog()` exports snapshots, schemas, tables, columns, data files, delete files, and inlined inserts. It does not export many v1.0 tables such as table stats, column stats, file column stats, views/macros/tags, partition/sort metadata, mappings, encrypted secrets, schema changes, and several spec fields.
- Impact: Migration, backup, and compatibility workflows can silently lose metadata. This directly contradicts the v1.0 conformance story.
- Fix: Either make `export-catalog` truly enumerate the schema registry for all DuckLake catalog tables, or rename/scope the existing export as a partial legacy export. Add a manifest test asserting all expected tables are exported.

**[P1] DataFusion virtual catalog registers 32 tables while roadmap/docs still emphasize 28 spec tables**

- File: `crates/rocklake-datafusion/src/virtual_catalog.rs` (lines 400-431)
- File: `README.md` (architecture section states 28 DuckLake tables)
- Description: `virtual_catalog_registers_all_32_tables()` asserts 32 catalog tables. The public architecture text and much of the roadmap still describe the implementation as "28 DuckLake tables."
- Impact: Users and compatibility tests do not have a single source of truth for the catalog facade. This can hide whether the extra four tables are extension/convenience tables or spec changes.
- Fix: Define the authoritative table set in one schema registry and update docs to distinguish DuckLake spec tables from RockLake extension/virtual tables.

**[P2] DataFusion type mapping falls back unknown DuckLake types to UTF-8**

- File: `crates/rocklake-datafusion/src/catalog_provider.rs` (lines 384-418)
- Description: `map_data_type()` maps a small set of scalar strings and uses `DataType::Utf8` for everything else.
- Impact: DECIMAL, TIMESTAMP WITH TIME ZONE, nested/list/struct, variant, geometry, and other DuckLake v1.0 types can be scanned with wrong Arrow types.
- Fix: Reuse the DuckLake type parser/model from `rocklake-core` rather than a local string match. Return an error for unsupported types instead of silently using UTF-8.

---

## Area 9: Operational Readiness

### Findings

**[P1] NDJSON backup/restore documentation promises a complete backup that code does not provide**

- File: `docs/operations/backup-restore.md` (lines 38-69)
- File: `docs/operations/cli-reference.md` (lines 265-345)
- File: `crates/rocklake-catalog/src/export.rs` (lines 60-275)
- Description: Docs say NDJSON export includes all live rows and lists partition information, views, sequence states, and permission grants. The implementation does not export these categories and does not support the documented `--at-snapshot`, `--at-time`, `--schema`, `--table`, `--merge`, or `--dry-run` options in `cmd_export()`/`cmd_import()`.
- Impact: Operators can rely on backups that cannot restore the full catalog. Pre-excision and pre-upgrade recovery instructions are unsafe if the export is partial.
- Fix: Make docs match current behavior immediately, then implement complete export/import before recommending NDJSON as disaster recovery.

**[P2] Checkpoint metadata IDs use millisecond timestamps and can collide**

- File: `crates/rocklake-catalog/src/checkpoint.rs` (lines 42-72)
- Description: `create_checkpoint()` uses `SystemTime::now().as_millis() as u64` as the checkpoint ID and writes metadata at `checkpoint_key(id)`.
- Impact: Two checkpoint creates in the same millisecond overwrite the same metadata key. This is plausible in automation.
- Fix: Allocate checkpoint IDs from a counter or use a UUID. If timestamp ordering is desired, combine timestamp with a monotonic sequence.

**[P2] Excision audit keys use millisecond timestamps and can collide**

- File: `crates/rocklake-catalog/src/excise.rs` (lines 182-214)
- Description: `record_audit_entry()` builds the audit key from `timestamp_millis` only.
- Impact: Two excision operations recorded in the same millisecond can overwrite the audit entry, weakening the compliance/audit trail.
- Fix: Include a monotonic counter or random suffix in the audit key. Add a test that records two entries with the same timestamp.

---

## Area 10: Documentation Gaps

### Findings

**[P1] CLI reference documents options that the CLI does not parse**

- File: `docs/operations/cli-reference.md` (lines 265-345)
- File: `crates/rocklake-pgwire/src/main.rs` (lines 594-633)
- Description: Docs show `--at-snapshot`, `--at-time`, `--schema`, `--table`, `--merge`, and `--dry-run`. `cmd_export()` parses only `--output` and `--snapshot-id`; `cmd_import()` parses only `--input`.
- Impact: Operators following docs will either get ignored options or unexpected defaults. This is especially dangerous for backup/restore and scoped export workflows.
- Fix: Update docs to the implemented flags and/or implement the documented flags. Add a CLI docs-to-parser test.

**[P2] Migration docs use commands and formats that do not match the current implementation**

- File: `docs/operations/migration-from-ducklake.md` (lines 20-90)
- File: `crates/rocklake-pgwire/src/main.rs` (lines 1178-1240)
- Description: The docs instruct users to export CSVs from DuckDB and run `rocklake pg-migrate --input snapshot.csv --output snapshot.ndjson`, but `cmd_pg_migrate()` only reads `--input` and writes SQL INSERT statements to stdout. `migrate-from-ducklake` calls `import_catalog()` on NDJSON, not CSV.
- Impact: The documented migration path cannot work as written.
- Fix: Split the migration docs into currently supported NDJSON import and future CSV/28-table conversion. Add runnable examples tested in CI.

**[P2] Native extension docs/comments overstate functionality**

- File: `extension/src/rocklake_extension.cpp` (lines 1-15, 132-165)
- File: `ROADMAP.md` (lines 3749-3819)
- Description: The extension source usage block shows native `ATTACH`, but the implementation does not register an attach handler. The updated roadmap now correctly treats this as v0.36 planning work.
- Impact: Users can misunderstand the project state and spend time trying a non-functional path.
- Fix: Update extension comments/docs to say "ABI wrapper only; attach registration pending v0.36."

---

## Prioritised Action List

| Priority | Area | Title | File(s) |
|---|---|---|---|
| P0 | Correctness | Millisecond wall-clock writer epochs can let two writers share the fencing token | `crates/rocklake-catalog/src/store.rs`, `crates/rocklake-catalog/src/writer/snapshot.rs`, `crates/rocklake-catalog/tests/concurrent_writer_fencing.rs` |
| P1 | Correctness | Imported catalogs miss the secondary data-file index used by readers | `crates/rocklake-catalog/src/export.rs`, `crates/rocklake-catalog/src/reader.rs`, `crates/rocklake-catalog/src/writer/mod.rs` |
| P1 | Correctness | Export includes retired data/delete files at historical snapshots | `crates/rocklake-catalog/src/export.rs` |
| P1 | Correctness | Binary COPY parser silently accepts truncated streams | `crates/rocklake-pgwire/src/copy_parser.rs`, `crates/rocklake-pgwire/src/handler.rs` |
| P1 | Concurrency | GC lease and pin checks are documented as transactional but run outside the transaction | `crates/rocklake-catalog/src/gc.rs`, `crates/rocklake-catalog/src/lease.rs` |
| P1 | Performance | DataFusion converts catalog errors into empty schemas/tables/files | `crates/rocklake-datafusion/src/catalog_provider.rs` |
| P1 | Performance | DataFusion returns `EmptyExec` when data files exist but the data root is unsupported | `crates/rocklake-datafusion/src/catalog_provider.rs` |
| P1 | Test Coverage | Export/import tests do not validate restored read behavior | `crates/rocklake-catalog/tests/v04_tests.rs`, `crates/rocklake-catalog/src/export.rs` |
| P1 | DuckLake Conformance | `export-catalog` says it exports all 28 DuckLake tables, but implementation exports a subset | `crates/rocklake-pgwire/src/main.rs`, `crates/rocklake-catalog/src/export.rs` |
| P1 | Operations | NDJSON backup/restore documentation promises a complete backup that code does not provide | `docs/operations/backup-restore.md`, `docs/operations/cli-reference.md`, `crates/rocklake-catalog/src/export.rs` |
| P1 | Documentation | CLI reference documents options that the CLI does not parse | `docs/operations/cli-reference.md`, `crates/rocklake-pgwire/src/main.rs` |

---

## Recommendations for Next Roadmap Milestone

1. Replace wall-clock writer epochs with a transactional monotonic writer identity before any more compatibility work. Add a deterministic same-tick concurrent writer test.
2. Create a dedicated "Recovery Correctness" milestone: fix export/import completeness, secondary index restoration, MVCC export filtering, counters, and full restored-query tests.
3. Make PG-wire binary COPY parsing fail closed. No partial COPY stream should ever be accepted as a successful catalog bootstrap.
4. Change DataFusion integration to fail closed on catalog/storage errors. Empty results must mean an actually empty table, not an unreadable catalog or unsupported object store.
5. Make GC lease/pin enforcement truly transactional or explicitly serialize GC with lease acquisition.
6. Reconcile the DuckLake catalog table count and schema registry across docs, PG-wire, DataFusion, export/import, and compatibility tests.
7. Downgrade or rewrite backup/restore, migration, and native-extension docs so they exactly match what the current binary does.
8. Add C/C++ ABI smoke tests for `rocklake-ffi` and make the v0.35.0 embedded client library the public ergonomic API before building more native consumers.
9. Add CLI parser/docs conformance tests that fail whenever docs mention a flag the binary does not parse.
10. Keep `cargo clippy --workspace --all-targets -- -D warnings` as a hard gate; it currently passes and is a good baseline to preserve.
