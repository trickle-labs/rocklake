# SlateDuck Overall Assessment - v0.18 Post-Implementation Review

## Executive Summary

SlateDuck has a strong foundation: the workspace is well organized, CI is broad, the core catalog writer stages most structural mutations before committing snapshots, and the project has unusually rich documentation for a pre-1.0 Rust system. The v0.18 surface is also visibly present across the right crates: catalog tags, SQL classification, PG-wire handlers, extension schema rows, snapshot leases, rowid counters, and mixed frontier types exist in code.

The most serious issue is that several v0.18 acceptance items are implemented as surface-level plumbing rather than complete behavior. `table_changes()` currently returns one synthetic row per added data file with rowid `0`, no user columns, no deletes, no update pre/post-image pairs, and no Parquet row scan. `SnapshotDiff` ignores the `from_snapshot` input for data files and only reports files added exactly at `to_snapshot`. That means the advertised O(delta) CDC contract is not yet usable for pg-trickle or any other row-level consumer.

The second major risk is concurrency correctness. Writer fencing is epoch-based, but `CatalogStore::open()` unconditionally overwrites `SYSTEM_WRITER_EPOCH`, and `CatalogWriter::check_epoch()` treats a missing epoch as success. GC lease checks and retain-from updates are not in one serializable transaction, extension schema row-id allocation is read/put/read/put without a transaction, and several catalog metadata writes bypass `create_snapshot()` entirely. These issues are fixable, but they need to be handled before v1.0 because they cut directly against the single-writer-many-readers guarantee.

The third major risk is safety and operability around the edges. The FFI layer exposes raw pointers and returns a `&'static mut` from `validate_catalog()`, unsafe blocks are mostly undocumented, TLS setup has panicking unwraps, CI does not run sanitizers or MSRV tests, and the license audit is not enabled in CI. These are classic pre-GA hardening issues: none invalidates the architecture, but all become expensive if delayed.

Commands run during this review: `cargo deny check advisories bans sources` passed with two stale ignored advisories, while `cargo deny check licenses` failed because license policy is not configured/enforced. I also inspected the current file contents directly rather than relying on prior v0.18 context.

## Critical Findings (must fix before v1.0)

1. **`table_changes()` does not return real row-level CDC**

   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L1358)
   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L1407)
   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L1415)
   - File: [crates/slateduck-sql/src/table_changes.rs](../crates/slateduck-sql/src/table_changes.rs#L129)
   - File: [crates/slateduck-sql/src/table_changes.rs](../crates/slateduck-sql/src/table_changes.rs#L133)
   - Severity: Critical
   - Description: `execute_table_changes()` builds a schema of only `rowid`, `change_type`, and `table_ref`, then emits one row per `diff.added_data_files` with hardcoded rowid `0`. The standalone `compute_table_changes()` helper caps output at `added_row_count.min(100)` / `removed_row_count.min(100)` and uses `columns_json: "{}"`. Neither path reads Parquet rows, preserves real rowids, returns user columns, emits deletes, or detects updates.
   - Impact: The CDC contract is not usable. pg-trickle cannot reconstruct target state, incremental refreshes receive synthetic rows, and updates/deletes are silently absent.
   - Recommended Fix: Move `table_changes()` execution to a real table function/operator that resolves added and removed files, reads affected Parquet files, emits actual row payloads including `__sd_rowid`, and correlates removed/added rows into `update_preimage` and `update_postimage` pairs. Add a property test that applies emitted changes to the start snapshot and exactly reconstructs the end snapshot.

2. **`SnapshotDiff` cannot support change windows or deletes for data files**

   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L22)
   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L489)
   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L493)
   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L563)
   - File: [crates/slateduck-core/src/rows.rs](../crates/slateduck-core/src/rows.rs#L161)
   - Severity: Critical
   - Description: `SnapshotDiff` has `added_data_files` but no `retired_data_files`. `snapshot_diff()` converts `from_snapshot` to `_from` and never uses it, then only includes data files where `row.snapshot_id == to`. `DataFileRow` only has `snapshot_id`; it does not have `begin_snapshot` / `end_snapshot` like versioned catalog rows.
   - Impact: A request for `table_changes(start_snapshot := 42, end_snapshot := 45)` misses changes at 43 and 44, cannot represent deletes, and cannot support update pre/post-image detection. Time-window CDC is fundamentally incomplete.
   - Recommended Fix: Either version `DataFileRow` with `begin_snapshot` and `end_snapshot`, or add first-class delete/retire rows that `SnapshotDiff` can return. Update `snapshot_diff()` to scan the full `(from, to]` interval and return both added and retired data files.

3. **Writer epoch fencing can be overwritten by a second opener**

   - File: [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs#L56)
   - File: [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs#L65)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L1326)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L1330)
   - Severity: Critical
   - Description: `CatalogStore::open()` generates a new epoch and unconditionally writes `SYSTEM_WRITER_EPOCH` after initialization. There is no compare-and-set, lease ownership check, or previous epoch validation. `check_epoch()` only errors when the key exists and differs; if the key is missing, the writer proceeds.
   - Impact: Two processes can open the same catalog and overwrite each other's fencing token. A late opener can fence an active writer without a controlled handoff, and a missing epoch does not fail closed.
   - Recommended Fix: Replace unconditional epoch writes with a transactional writer-lease acquisition protocol: read current epoch, validate ownership/expiry, CAS a new epoch, and fail closed when the epoch key is missing or malformed. Add concurrent open/failover tests with two processes or two independent `Db` handles.

4. **Extension schema row-id allocation is non-transactional and collision-prone**

   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L55)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L63)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L80)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L90)
   - Severity: Critical
   - Description: `insert_extension_row()` reads the marker row to infer `next_id`, writes the data row, then writes the marker with `next_id + 1`. None of these operations is in a serializable transaction. Two concurrent inserts can read the same marker and both write the same row ID.
   - Impact: Extension tables can lose rows or overwrite rows under concurrent pg-trickle metadata writes. The provenance/frontier tables become unreliable exactly where they are supposed to provide durable coordination.
   - Recommended Fix: Store extension table counters under `TAG_COUNTERS` or a dedicated counter key and allocate inside a `SerializableSnapshot` transaction. Commit row and counter atomically. Add a concurrent insert test that proves row IDs are unique.

5. **The FFI handle validator returns an unsound `&'static mut`**

   - File: [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs#L142)
   - File: [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs#L146)
   - File: [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs#L317)
   - File: [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs#L318)
   - Severity: Critical
   - Description: `validate_catalog()` converts a raw pointer into `Option<&'static mut SlateduckCatalog>`, even though the referenced allocation only lives until `slateduck_close()`. `slateduck_close()` reads and zeroes `magic` through raw pointers and then calls `Box::from_raw()`; there is no synchronization for concurrent close/use from C.
   - Impact: This creates undefined-behavior risk: use-after-free, aliasing violations, and double-close races are possible across the C boundary.
   - Recommended Fix: Remove the `'static` lifetime, avoid returning mutable references from raw pointer validation, and centralize access through short-lived closures that validate and use the handle immediately. Document every unsafe block with `SAFETY:` invariants and add ASAN/Miri tests for null, double-close, use-after-close, and concurrent misuse.

## High Priority Findings

1. **GC lease checks and retain-from updates are not atomic**

   - File: [crates/slateduck-catalog/src/gc.rs](../crates/slateduck-catalog/src/gc.rs#L93)
   - File: [crates/slateduck-catalog/src/gc.rs](../crates/slateduck-catalog/src/gc.rs#L104)
   - File: [crates/slateduck-catalog/src/gc.rs](../crates/slateduck-catalog/src/gc.rs#L116)
   - File: [crates/slateduck-catalog/src/gc.rs](../crates/slateduck-catalog/src/gc.rs#L127)
   - Severity: High
   - Description: `gc_apply()` reads pins and leases, then later does a plain `db.put()` of `SYSTEM_RETAIN_FROM`. Despite the comment saying "transactionally advance retain-from", the check and write are not one transaction.
   - Impact: A new lease can be acquired after the lease scan but before retain-from advances. GC can then move past a snapshot that a consumer believes is protected.
   - Recommended Fix: Wrap current retain-from read, pin scan, lease scan, and retain-from write in a `SerializableSnapshot` transaction, or introduce a single GC coordination row updated via CAS.

2. **Several catalog writes bypass snapshot commits**

   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L413)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L427)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L432)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L452)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L659)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L780)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L805)
   - Severity: High
   - Description: `update_table_stats()`, `upsert_file_column_stats()`, and other writer methods use `self.db.put()` directly instead of staging rows for `create_snapshot()`.
   - Impact: Metadata can become visible independently of the snapshot that should make it durable. Failed commits can leave partially applied stats or scheduling state.
   - Recommended Fix: Convert these direct writes into staged mutations or split them into explicitly documented non-MVCC system rows with separate consistency tests.

3. **LISTEN/UNLISTEN acknowledge success but never subscribe a session**

   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L505)
   - File: [crates/slateduck-pgwire/src/session.rs](../crates/slateduck-pgwire/src/session.rs#L145)
   - File: [crates/slateduck-pgwire/src/notify.rs](../crates/slateduck-pgwire/src/notify.rs#L21)
   - File: [crates/slateduck-pgwire/src/notify.rs](../crates/slateduck-pgwire/src/notify.rs#L85)
   - Severity: High
   - Description: `NotifyManager` and `ConnectionSubscriptions` exist, but `SessionState` has no subscriptions field and the executor arms simply return `LISTEN` / `UNLISTEN` tags without using the manager.
   - Impact: Clients believe they are subscribed, but no notifications can be delivered. pg-trickle will not get event-driven refresh behavior.
   - Recommended Fix: Add a shared `NotifyManager` to server state and `ConnectionSubscriptions` to session state. On `LISTEN`, register the receiver; on snapshot commit, call `notify`; after query execution, flush pending notifications to the client.

4. **Extension schema registration is hardcoded instead of configured**

   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L18)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L26)
   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L1520)
   - File: [crates/slateduck-pgwire/src/main.rs](../crates/slateduck-pgwire/src/main.rs#L340)
   - Severity: High
   - Description: `resolve_extension_id()` accepts `pgtrickle` unconditionally. `is_registered_extension()` exists but is not used by PG-wire, and the CLI help has no `--extension-schemas` flag.
   - Impact: The v0.18 registration model is not enforced. Operators cannot disable `pgtrickle.*`, and unknown future extension schemas cannot be registered without code changes.
   - Recommended Fix: Add an explicit `--extension-schemas` / env configuration, thread it into executor state, and check `is_registered_extension()` before routing extension DDL/DML.

5. **Extension schema JSON serialization is invalid for ordinary values**

   - File: [crates/slateduck-sql/src/params.rs](../crates/slateduck-sql/src/params.rs#L75)
   - File: [crates/slateduck-sql/src/params.rs](../crates/slateduck-sql/src/params.rs#L79)
   - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L1552)
   - Severity: High
   - Description: `ParamValues::to_json_string()` builds JSON with `format!("\"p{}\":\"{}\"", i, val)` and does no escaping. A value containing `"`, `\`, newline, or control characters produces invalid JSON. It also stores positional keys (`p0`, `p1`) rather than extension table column names.
   - Impact: pg-trickle provenance/frontier rows can become malformed or semantically unusable. Reads return opaque broken JSON instead of proper table rows.
   - Recommended Fix: Use `serde_json::Map` / `serde_json::Value` and preserve column names from the parsed INSERT statement. Reject values that cannot be represented.

6. **Hashed catalog keys can collide and discard identity**

   - File: [crates/slateduck-core/src/keys.rs](../crates/slateduck-core/src/keys.rs#L503)
   - File: [crates/slateduck-core/src/keys.rs](../crates/slateduck-core/src/keys.rs#L519)
   - File: [crates/slateduck-core/src/keys.rs](../crates/slateduck-core/src/keys.rs#L530)
   - File: [crates/slateduck-core/src/keys.rs](../crates/slateduck-core/src/keys.rs#L551)
   - Severity: High
   - Description: Snapshot lease keys and extension table keys store only a `u64` hash of `consumer_id` or `table_name`. The original string is not part of the key.
   - Impact: Two distinct consumers or extension tables with the same hash share a key/prefix. A collision can overwrite a lease, merge extension rows, or delete the wrong rows. The current hash choice is also an implementation detail of `DefaultHasher` rather than a durable key encoding contract.
   - Recommended Fix: Encode length-prefixed UTF-8 strings into keys, or use a cryptographic digest with collision handling that stores and validates the original string in the row.

7. **Rowid allocation does unchecked arithmetic**

   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L1308)
   - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L1394)
   - Severity: High
   - Description: Both `next_rowid_range()` implementations calculate `let end = current + count` without `checked_add` and without rejecting `count == 0`.
   - Impact: In release builds, overflow can wrap and re-allocate previously issued rowid ranges. A zero-sized range can also be accepted as a successful allocation.
   - Recommended Fix: Use `current.checked_add(count)` and reject `count == 0` with a typed catalog error. Add property tests near `u64::MAX`.

8. **TLS setup panics on partial TLS config**

   - File: [crates/slateduck-pgwire/src/server.rs](../crates/slateduck-pgwire/src/server.rs#L84)
   - File: [crates/slateduck-pgwire/src/server.rs](../crates/slateduck-pgwire/src/server.rs#L85)
   - Severity: High
   - Description: `build_tls_acceptor()` unwraps `cert_path` and `key_path` even though `TlsConfig` stores them as `Option<String>`.
   - Impact: A malformed config crashes the server instead of returning a clear configuration error.
   - Recommended Fix: Replace unwraps with `ok_or_else()` returning `std::io::Error`, and test cert-only/key-only configurations.

9. **Custom SQLSTATE variant ignores its stored code**

   - File: [crates/slateduck-pgwire/src/error.rs](../crates/slateduck-pgwire/src/error.rs#L65)
   - File: [crates/slateduck-pgwire/src/error.rs](../crates/slateduck-pgwire/src/error.rs#L91)
   - Severity: High
   - Description: `SlateDuckError::SqlState { code, message }` stores an arbitrary SQLSTATE but `sqlstate()` always returns `"55000"` for the variant.
   - Impact: The first non-55000 use of this variant will silently report the wrong SQLSTATE to clients. This defeats the purpose of a generic SQLSTATE error.
   - Recommended Fix: Make `sqlstate()` return `&str` or `Cow<'_, str>` from the stored code, or remove the generic variant and add typed variants for each supported SQLSTATE.

## Medium Priority Findings

1. **`list_data_files()` is a full prefix scan with no MVCC end-state**

   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L211)
   - File: [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs#L221)
   - Severity: Medium
   - Description: The method scans every file for a table and filters only `snapshot_id <= read_snapshot`. It has no secondary index by snapshot and no end-snapshot filter.
   - Impact: Large tables accumulate read amplification, and deleted/replaced files cannot be represented correctly.
   - Recommended Fix: Add file-version visibility metadata and a secondary index keyed by `(table_id, snapshot_id, file_id)` or maintain per-snapshot file manifests.

2. **Retain-from cache uses relaxed atomics**

   - File: [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs#L95)
   - File: [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs#L120)
   - Severity: Medium
   - Description: `read_at()` and `update_retain_from_cache()` use `Ordering::Relaxed`.
   - Impact: The cache is currently a single integer, so this is not the highest risk, but it makes retention visibility harder to reason about across threads.
   - Recommended Fix: Use `Release` on store and `Acquire` on load, or hide the cache behind a small type that documents the memory-ordering invariant.

3. **Lease expiry uses wall-clock time and hides clock errors**

   - File: [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs#L23)
   - File: [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs#L27)
   - File: [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs#L52)
   - Severity: Medium
   - Description: Lease creation and filtering use `SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()`. A backwards clock jump becomes timestamp `0`, and `ttl_seconds * 1000` can overflow.
   - Impact: Leases can expire too early, last too long, or calculate invalid expiries under clock changes or large TTL values.
   - Recommended Fix: Use checked arithmetic, reject absurd TTL values, and prefer a monotonic/process-authoritative time source for lease decisions.

4. **Decode errors in lease and extension scans are silently ignored**

   - File: [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs#L66)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L113)
   - File: [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs#L142)
   - Severity: Medium
   - Description: Corrupt rows are skipped with `if let Ok(row)` and no warning or error.
   - Impact: A corrupted active lease disappears from GC protection; corrupted extension rows disappear from reads and deletes.
   - Recommended Fix: Return a corruption error for system/lease data and log key-level warnings for extension rows. Add repair tooling to quarantine malformed rows.

5. **IVM output writes serialize every row independently**

   - File: [crates/slateduck-ivm/src/output.rs](../crates/slateduck-ivm/src/output.rs#L24)
   - File: [crates/slateduck-ivm/src/output.rs](../crates/slateduck-ivm/src/output.rs#L27)
   - File: [crates/slateduck-ivm/src/output.rs](../crates/slateduck-ivm/src/output.rs#L29)
   - Severity: Medium
   - Description: `write_output_rows()` serializes each row with `serde_json::to_vec()` and stages one inlined insert per output row.
   - Impact: Large materialized views can create huge staged write buffers and high JSON overhead before a snapshot commit.
   - Recommended Fix: Chunk/batch output writes, use a binary row format where possible, and add memory-limit tests for large outputs.

6. **Aggregate deletion for `StringAgg` / `ArrayAgg` is O(N^2)**

   - File: [crates/slateduck-ivm/src/circuit.rs](../crates/slateduck-ivm/src/circuit.rs#L295)
   - File: [crates/slateduck-ivm/src/circuit.rs](../crates/slateduck-ivm/src/circuit.rs#L298)
   - Severity: Medium
   - Description: Deletions loop over negative weight, use `position()`, and then `remove(pos)` on a vector.
   - Impact: Large groups with many deletions can degrade quadratically.
   - Recommended Fix: Store aggregate rescan inputs as a counted multiset or use tombstoned vectors plus periodic compaction.

7. **Group and shard keys repeatedly stringify JSON values**

   - File: [crates/slateduck-ivm/src/circuit.rs](../crates/slateduck-ivm/src/circuit.rs#L460)
   - File: [crates/slateduck-ivm/src/circuit.rs](../crates/slateduck-ivm/src/circuit.rs#L467)
   - File: [crates/slateduck-ivm/src/worker.rs](../crates/slateduck-ivm/src/worker.rs#L270)
   - Severity: Medium
   - Description: Hot paths use `serde_json::to_string()` or `Value::to_string()` for grouping/sharding keys.
   - Impact: High-throughput streams pay repeated allocation and serialization costs.
   - Recommended Fix: Add a stable binary key encoding for `serde_json::Value` and reuse encoded keys through the pipeline.

8. **Adaptive worker row estimate is an unbounded running sum**

   - File: [crates/slateduck-ivm/src/worker.rs](../crates/slateduck-ivm/src/worker.rs#L339)
   - Severity: Medium
   - Description: `estimated_total_rows` increments by every processed batch rather than tracking current input size or an exponential moving estimate.
   - Impact: Long-lived workers will increasingly overestimate table size and make cost-mode decisions from drifted statistics.
   - Recommended Fix: Track current cardinality by source/frontier, or use an EWMA of observed batch/table size.

9. **SQL classifier has brittle manual string parsing before/around sqlparser**

   - File: [crates/slateduck-sql/src/classifier.rs](../crates/slateduck-sql/src/classifier.rs#L288)
   - File: [crates/slateduck-sql/src/classifier.rs](../crates/slateduck-sql/src/classifier.rs#L314)
   - File: [crates/slateduck-sql/src/classifier.rs](../crates/slateduck-sql/src/classifier.rs#L329)
   - Severity: Medium
   - Description: `LISTEN`/`UNLISTEN`, `find_as_keyword()`, and `split_qualified_name()` are handwritten string parsers. They do not validate channel identifiers and do not correctly handle quoted identifiers, comments, or `AS` without surrounding spaces.
   - Impact: Valid SQL can be misclassified, and invalid SQL can be accepted into unsupported execution paths.
   - Recommended Fix: Use sqlparser AST wherever possible. For non-AST commands, implement a small tokenizer with tests for quoted identifiers, comments, whitespace, and invalid identifiers.

10. **Metrics documentation exposes flags/env that the CLI does not implement**

    - File: [docs/operations/monitoring.md](../docs/operations/monitoring.md#L15)
    - File: [docs/operations/monitoring.md](../docs/operations/monitoring.md#L16)
    - File: [docs/operations/monitoring.md](../docs/operations/monitoring.md#L23)
    - File: [crates/slateduck-pgwire/src/main.rs](../crates/slateduck-pgwire/src/main.rs#L240)
    - File: [crates/slateduck-catalog/src/metrics.rs](../crates/slateduck-catalog/src/metrics.rs#L187)
    - Severity: Medium
    - Description: Docs show `--metrics-path` and `SLATEDUCK_METRICS_PATH`, but CLI parsing only supports `--metrics-port` and `--metrics-bind`. The metrics HTTP server also ignores request path and returns metrics for any request.
    - Impact: Operators following docs will pass unsupported flags, and monitoring path behavior is less precise than documented.
    - Recommended Fix: Either implement configurable metrics path/env support and path validation, or remove those docs and state that the endpoint always serves metrics on any path.

11. **CI coverage only reports core and catalog crates**

    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L87)
    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L95)
    - Severity: Medium
    - Description: The coverage job only runs `cargo llvm-cov -p slateduck-catalog -p slateduck-core`.
    - Impact: PG-wire, IVM, FFI, SQL classification, DataFusion integration, and testkit regressions have no coverage visibility.
    - Recommended Fix: Expand coverage to all production crates, or publish per-crate coverage with separate thresholds.

12. **MSRV is declared but not tested**

    - File: [Cargo.toml](../Cargo.toml#L16)
    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L18)
    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L69)
    - Severity: Medium
    - Description: The workspace declares `rust-version = "1.93"`, but CI uses the stable toolchain and only greps for a rust-version field.
    - Impact: Code can accidentally adopt Rust 1.94+ features while still claiming Rust 1.93 compatibility.
    - Recommended Fix: Add a CI job that installs `dtolnay/rust-toolchain@1.93` and runs at least `cargo check --workspace --all-targets`.

13. **License audit is not configured/enforced**

    - File: [deny.toml](../deny.toml#L35)
    - File: [deny.toml](../deny.toml#L39)
    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L73)
    - Severity: Medium
    - Description: `deny.toml` says license checking is intentionally minimal, and CI runs `cargo deny check advisories bans sources` but not licenses. Running `cargo deny check licenses` during this review failed.
    - Impact: License incompatibilities can enter the dependency graph without blocking CI.
    - Recommended Fix: Define an explicit allow list (Apache-2.0, MIT, BSD variants, Unicode, etc.), audit exceptions, and add `licenses` to the CI deny command.

14. **No sanitizer or Miri coverage for FFI unsafe code**

    - File: [.github/workflows/ci.yml](../.github/workflows/ci.yml#L11)
    - File: [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs#L9)
    - Severity: Medium
    - Description: The FFI crate opts out of `clippy::not_unsafe_ptr_arg_deref`, but CI does not run Miri, ASAN, or UBSAN for FFI tests.
    - Impact: UB in the raw pointer layer can survive normal unit tests.
    - Recommended Fix: Add nightly-only sanitizer/Miri jobs for `slateduck-ffi`, allowed to be slower or scheduled if necessary.

15. **Oversized modules are becoming architectural choke points**

    - File: [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs#L26)
    - File: [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs#L1)
    - File: [crates/slateduck-sql/src/classifier.rs](../crates/slateduck-sql/src/classifier.rs#L1)
    - Severity: Medium
    - Description: Current line counts are approximately 1629 for `executor.rs`, 1402 for `writer.rs`, and 990 for `classifier.rs`. `execute_classified()` starts at line 26 and dispatches a very large `match` beginning at line 33.
    - Impact: Reviewability and targeted testing are getting worse; new features tend to grow the same files.
    - Recommended Fix: Split PG-wire executor handlers by feature area, split catalog writer staged-MVCC operations from system-row operations, and move SQL dialect special cases into focused classifier modules.

## Low Priority / Cosmetic Findings

1. **Dead-code suppressions lack ownership or issue links**

   - File: [crates/slateduck-catalog/src/manifest.rs](../crates/slateduck-catalog/src/manifest.rs#L33)
   - File: [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs#L84)
   - File: [crates/slateduck-catalog/src/wal.rs](../crates/slateduck-catalog/src/wal.rs#L12)
   - File: [crates/slateduck-datafusion/src/catalog_provider.rs](../crates/slateduck-datafusion/src/catalog_provider.rs#L320)
   - Severity: Low
   - Description: Several `allow(dead_code)` markers remain without a linked roadmap item or removal condition.
   - Impact: Stubs can linger indefinitely and confuse contributors.
   - Recommended Fix: Add issue links and removal milestones, or delete code that is not part of the current architecture.

2. **Broad workspace features inflate dependency surface**

   - File: [Cargo.toml](../Cargo.toml#L21)
   - File: [Cargo.toml](../Cargo.toml#L28)
   - Severity: Low
   - Description: `object_store` enables AWS/GCP/Azure globally, and `tokio` uses `full` globally.
   - Impact: Builds and binaries include more transitive surface than every crate needs.
   - Recommended Fix: Move broad features to crate-specific dependencies or workspace feature flags.

3. **`read_latest()` derives latest snapshot from in-memory counters only**

   - File: [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs#L105)
   - Severity: Low
   - Description: `read_latest()` uses `self.counters.peek_snapshot_id() - 1`, not a fresh read of `ducklake_snapshot` or counter state.
   - Impact: This is probably fine inside a single open store, but it is a footgun for long-lived read-only processes if another process commits snapshots and this store does not refresh counters.
   - Recommended Fix: Either document that `read_latest()` is local-process latest, or add a `read_fresh_latest()` that reads the counter/snapshot from SlateDB.

4. **Plaintext password auth is allowed unless TLS is explicitly required**

   - File: [crates/slateduck-pgwire/src/handler.rs](../crates/slateduck-pgwire/src/handler.rs#L121)
   - File: [crates/slateduck-pgwire/src/handler.rs](../crates/slateduck-pgwire/src/handler.rs#L155)
   - Severity: Low
   - Description: The server can use PostgreSQL cleartext password authentication while TLS is optional unless `tls_required` is set.
   - Impact: Misconfigured deployments can transmit credentials in cleartext.
   - Recommended Fix: Warn loudly when password auth is enabled without TLS required, or make auth imply TLS by default for production modes.

## Gap Analysis by Area

### 1. Correctness & Bugs

The largest correctness gap is CDC: `table_changes()` is not row-level, `SnapshotDiff` ignores the `from_snapshot` window for data files, and data file deletes are not modeled. Writer fencing and extension row counters also need transactional strengthening. Integer overflow checks are missing in rowid range allocation and lease TTL arithmetic.

### 2. Concurrency & Safety

Core staged catalog writes are on the right path, but system-level coordination rows are weaker: writer epoch acquisition is not CAS-protected, GC lease checks are not atomic with retain-from writes, extension schema counters are read/modify/write without transactions, and FFI close/use has no concurrency protection.

### 3. Error Handling & Resilience

Several error paths are fail-open or silent: missing writer epoch is accepted, corrupted leases/extensions are ignored, `SqlState` ignores its own code, and TLS config panics on `None`. The repair story should explicitly cover malformed system rows, extension row corruption, and writer-epoch recovery.

### 4. Security

Authentication and TLS exist, but the defaults allow cleartext password auth unless TLS is required. Extension schemas are hardcoded rather than operator-registered. The FFI layer is the largest security concern because of raw pointer lifetimes and undocumented unsafe blocks. Hashed keys should be treated as a security/reliability risk because collisions can cross consumer/table boundaries.

### 5. Performance & Scalability

The catalog reader relies heavily on prefix scans plus in-memory filtering. Data file reads lack snapshot indexes. IVM output writes serialize and stage rows individually. Aggregation and grouping rely on vector removal and JSON serialization in hot paths. These are acceptable for small demos but need redesign before 10M-row scale claims.

### 6. Code Quality & Maintainability

`executor.rs`, `writer.rs`, and `classifier.rs` are too large and increasingly mix unrelated responsibilities. The codebase uses `#[allow(clippy::too_many_arguments)]` in multiple writer APIs and `#[allow(dead_code)]` for deferred modules. Refactoring into smaller modules would reduce the cost of future roadmap work.

### 7. Test Coverage & Quality

CI is solid for fmt, clippy, tests, security audit, fault injection, smoke tests, and compatibility matrix. Missing coverage remains around true row-level `table_changes()`, concurrent writer fencing, extension schema concurrency, GC lease TOCTOU, FFI misuse, MSRV compilation, sanitizer/Miri checks, and full-crate coverage reporting.

### 8. API Completeness & DuckLake Standard Compliance

The DuckLake catalog tables are partially surfaced, but `table_changes()` is not compliant, virtual catalog mutations do not have an explicit read-only rejection path, extension schema registration is not configurable, and NOTIFY is not wired into live sessions. v0.18's API surface exists; several semantics still need implementation depth.

### 9. Documentation & Observability

Documentation is much better than average, and metrics docs exist. The main drift found is metrics path/config documentation: docs mention `--metrics-path` and `SLATEDUCK_METRICS_PATH`, while the CLI supports only `--metrics-port` and `--metrics-bind`, and the server responds on any path.

### 10. Dependency & Supply Chain Health

`cargo deny check advisories bans sources` passes, though two ignored advisories are now stale and should be removed from `deny.toml`. License checking is not enforced and `cargo deny check licenses` fails. Workspace dependencies enable broad feature sets that expand transitive dependency surface.

## Positive Findings

- The workspace is modular and the crate boundaries are understandable: core encoding/types, catalog operations, SQL classification, PG-wire serving, IVM, FFI, and testkit each have a clear purpose.
- `create_snapshot()` stages many core catalog mutations and commits them in one serializable transaction, including counters and the snapshot row.
- CI is already broad: fmt, clippy with `-Dwarnings`, tests, cargo-deny advisories/bans/sources, smoke tests, fault injection tests, security tests, coverage, and compatibility matrix are present.
- Documentation is extensive for a pre-1.0 project. Architecture, operations, concepts, metrics, monitoring, and roadmap documents exist and are useful.
- The v0.18 implementation placed new concepts in the expected crates, which makes the remaining work mostly about semantic correctness rather than discovering where features should live.

## Recommended Sprint Plan

### Week 1: Fix correctness blockers

- Replace synthetic `table_changes()` with a design/implementation plan for real row-level Parquet CDC.
- Extend `SnapshotDiff` and data file metadata to represent retired files and `(from, to]` windows.
- Add failing tests first: insert/delete/update windows, multi-snapshot windows, and GC-too-old SQLSTATE.

### Week 2: Harden coordination and transactions

- Implement CAS/lease-based writer epoch acquisition.
- Put GC lease check plus retain-from update into a serializable transaction.
- Move direct writer `db.put()` metadata writes into staged commits or document them as non-MVCC system rows.
- Make extension schema row-id allocation transactional.

### Week 3: Wire operational APIs end to end

- Thread `NotifyManager` and per-connection subscriptions through PG-wire sessions.
- Emit notifications after snapshot commits and add LISTEN/UNLISTEN integration tests.
- Add configurable extension schema registration and reject unregistered schemas.
- Replace `ParamValues::to_json_string()` with schema-aware serde serialization.

### Week 4: Safety, scale, and CI hardening

- Refactor FFI validation and close/free APIs; add `SAFETY:` documentation and sanitizer/Miri jobs.
- Add MSRV CI and license deny enforcement.
- Refactor oversized executor/writer/classifier modules around feature areas.
- Add performance tests for `list_data_files()`, IVM output size, aggregate deletes, and large extension table scans.

## Appendix: Files Reviewed

- [Cargo.toml](../Cargo.toml)
- [deny.toml](../deny.toml)
- [.github/workflows/ci.yml](../.github/workflows/ci.yml)
- [docs/operations/monitoring.md](../docs/operations/monitoring.md)
- [docs/reference/metrics.md](../docs/reference/metrics.md)
- [crates/slateduck-core/src/tags.rs](../crates/slateduck-core/src/tags.rs)
- [crates/slateduck-core/src/keys.rs](../crates/slateduck-core/src/keys.rs)
- [crates/slateduck-core/src/rows.rs](../crates/slateduck-core/src/rows.rs)
- [crates/slateduck-core/tests/property_tests.rs](../crates/slateduck-core/tests/property_tests.rs)
- [crates/slateduck-catalog/src/store.rs](../crates/slateduck-catalog/src/store.rs)
- [crates/slateduck-catalog/src/reader.rs](../crates/slateduck-catalog/src/reader.rs)
- [crates/slateduck-catalog/src/writer.rs](../crates/slateduck-catalog/src/writer.rs)
- [crates/slateduck-catalog/src/gc.rs](../crates/slateduck-catalog/src/gc.rs)
- [crates/slateduck-catalog/src/lease.rs](../crates/slateduck-catalog/src/lease.rs)
- [crates/slateduck-catalog/src/extension.rs](../crates/slateduck-catalog/src/extension.rs)
- [crates/slateduck-catalog/src/metrics.rs](../crates/slateduck-catalog/src/metrics.rs)
- [crates/slateduck-catalog/src/manifest.rs](../crates/slateduck-catalog/src/manifest.rs)
- [crates/slateduck-catalog/src/wal.rs](../crates/slateduck-catalog/src/wal.rs)
- [crates/slateduck-sql/src/classifier.rs](../crates/slateduck-sql/src/classifier.rs)
- [crates/slateduck-sql/src/params.rs](../crates/slateduck-sql/src/params.rs)
- [crates/slateduck-sql/src/table_changes.rs](../crates/slateduck-sql/src/table_changes.rs)
- [crates/slateduck-pgwire/src/executor.rs](../crates/slateduck-pgwire/src/executor.rs)
- [crates/slateduck-pgwire/src/error.rs](../crates/slateduck-pgwire/src/error.rs)
- [crates/slateduck-pgwire/src/session.rs](../crates/slateduck-pgwire/src/session.rs)
- [crates/slateduck-pgwire/src/notify.rs](../crates/slateduck-pgwire/src/notify.rs)
- [crates/slateduck-pgwire/src/server.rs](../crates/slateduck-pgwire/src/server.rs)
- [crates/slateduck-pgwire/src/handler.rs](../crates/slateduck-pgwire/src/handler.rs)
- [crates/slateduck-pgwire/src/main.rs](../crates/slateduck-pgwire/src/main.rs)
- [crates/slateduck-pgwire/Cargo.toml](../crates/slateduck-pgwire/Cargo.toml)
- [crates/slateduck-ivm/src/output.rs](../crates/slateduck-ivm/src/output.rs)
- [crates/slateduck-ivm/src/circuit.rs](../crates/slateduck-ivm/src/circuit.rs)
- [crates/slateduck-ivm/src/worker.rs](../crates/slateduck-ivm/src/worker.rs)
- [crates/slateduck-ivm/src/observability.rs](../crates/slateduck-ivm/src/observability.rs)
- [crates/slateduck-ffi/src/lib.rs](../crates/slateduck-ffi/src/lib.rs)
- [crates/slateduck-datafusion/src/catalog_provider.rs](../crates/slateduck-datafusion/src/catalog_provider.rs)
