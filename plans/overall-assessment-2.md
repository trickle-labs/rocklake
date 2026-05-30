# RockLake 360° Architecture Audit — Report 2

**Date:** 2026-05-30  
**Codebase version:** 0.45.0 (GA Readiness Gate)  
**Total Rust source:** ~66,800 lines across 8 workspace crates  
**Test files:** 61 dedicated test modules + 2 benchmarks  
**Scope:** Full 5-pillar evaluation targeting DuckLake 1.0 readiness

---

## 1. Executive Summary & Architectural Health Score

RockLake at v0.45.0 represents a remarkably well-executed implementation of a serverless lakehouse catalog. The architecture delivers on its core promise: a fully stateless PG-wire sidecar backed by SlateDB on object storage, with zero external dependencies beyond an S3-compatible bucket. The codebase demonstrates exceptional discipline — zero `TODO`/`FIXME` markers remain, `cargo clippy -D warnings` passes, and the workspace compiles with `#![deny(missing_docs)]` on all core crates.

The system has matured through 45+ releases with a methodical tier-by-tier hardening approach covering writer fencing (CAS-protected monotonic epochs since v0.28.0), fault injection, security (SCRAM-SHA-256, TLS 1.2/1.3), observability (Prometheus + OTLP), and multi-language bindings (Python, Go, Node.js, Java/Kotlin). The K8s deployment story is clean and well-documented.

However, several structural gaps remain for a world-class DuckLake 1.0 production engine at massive scale:

| Pillar | Score | Justification |
|--------|-------|---------------|
| **1. Architecture & Scalability** | **7/10** | Excellent storage-compute separation via SlateDB/object-store; single-writer bottleneck is architecturally sound but limits write throughput to one pod; no distributed reader cache coordination. |
| **2. Performance** | **6/10** | Hot-key optimization and secondary indexes are well-designed; however, `list_data_files` at 100K files takes ~78ms (p50) due to full prefix scan; no local NVMe cache tier; no connection pooling. |
| **3. Ergonomics & Usability** | **8/10** | "First 5 minutes" experience is excellent; CLI is comprehensive; multi-language bindings are idiomatic; `CatalogClientBuilder` pattern is ergonomic; minor gap: no `docker run` one-liner in README. |
| **4. Observability & Day-2 Ops** | **7/10** | Prometheus metrics, OTLP tracing, `rocklake diagnose`, and orphan sweep are all present; gap: no per-query trace correlation, no memory pressure alerts, histogram buckets are hand-rolled atomics rather than proper histograms. |
| **5. Code Quality & Safety** | **8/10** | Zero TODOs, strict clippy, `deny.toml` advisory auditing, comprehensive MVCC tests; remaining issues: 2 production `unwrap()` calls in `stats.rs` decimal comparison, 130 total `unwrap()` calls (most in doc-tests but some in production paths). |

**Overall: 7.2/10** — Production-ready for small-to-medium deployments; needs targeted hardening for massive multi-node scale.

---

## 2. Specification vs. Reality Alignment Audit

### 2.1 README Claims vs. Implementation

| README Claim | Status | Evidence |
|---|---|---|
| "Both Parquet data and catalog in same S3 bucket" | ✅ Verified | `OpenOptions` accepts any `ObjectStore`; K8s manifests show single-bucket config |
| "Stateless binaries — crash and restart" | ✅ Verified | CAS epoch acquisition on open; no WAL state on local disk |
| "Unbounded reader replicas with no coordination" | ⚠️ Partially | Reader code uses `Db::open` (which acquires writer epoch); true read-only SlateDB `DbReader` is not exposed — every reader instance currently opens as a potential writer |
| "Time travel at any snapshot ID" | ✅ Verified | `CatalogReader::new(db, SnapshotId)` + MVCC `is_visible()` filter |
| "28 DuckLake spec tables" | ✅ Verified | Schema registry covers all 28 + 4 extension tables |
| "No changes to DuckDB" | ✅ Verified | Standard `ducklake` extension via PG wire protocol |

### 2.2 ROADMAP vs. Implementation

| Roadmap Item | Claimed Status | Assessment |
|---|---|---|
| v0.28.0 — Writer Fencing (monotonic counter) | Done | ✅ Confirmed: `store.rs` uses CAS loop on `SYSTEM_WRITER_EPOCH` |
| v0.29.0 — Recovery (secondary index in import) | Done | ✅ Fixed per Assessment 1 findings |
| v0.39.0 — Observability (Prometheus, OTLP) | Done | ✅ `metrics.rs` + `telemetry.rs` present |
| v0.42.0 — Performance Benchmarks | Done | ✅ `benchmarks/v0.42-catalog-bench.json` with reproducible measurements |
| v0.43.0 — Scale Testing (16-pod reader) | Done | ⚠️ Test exists but uses `InMemory` object store; no real multi-node validation artifacts |
| v0.70.0 — Native DuckDB Extension | Exploration | ⚠️ `extension/` directory contains CMakeLists.txt stubs; no functional code |
| v1.0 — GA | Planning | Consistent with v0.45.0 readiness gate being final pre-1.0 milestone |

### 2.3 Architecture Docs vs. Code

| Documentation Claim | Reality |
|---|---|
| `docs/architecture/` claims 7 crates | Workspace has 8 (`rocklake-testkit` added, `rocklake-sqlite-vfs` removed) |
| `CONTRIBUTING.md` lists `rocklake-sqlite-vfs` | Crate no longer exists in workspace members |
| Blueprint §1.4 "immutable facts" | ✅ Correctly enforced: `gc_apply` only advances `retain_from`; `excise` is separate |
| K8s docs reference image `ghcr.io/rocklake/rocklake:0.8.0` | Stale version tag; should reference latest or be parameterized |

---

## 3. Deep-Dive Critiques & Inconsistencies (By Pillar)

### Pillar 1: Architecture, Scalability & "Laptop-to-K8s"

**Strengths:**
- Clean separation via `object_store` trait abstraction (S3, GCS, Azure, LocalFS all work transparently)
- Single-writer model with CAS-protected epoch is correct and avoids distributed consensus complexity
- K8s deployment is a simple `Deployment` (not StatefulSet) — operationally excellent
- `Recreate` strategy ensures single-writer invariant during pod replacement

**Vulnerabilities:**

1. **Reader-as-Writer Anti-Pattern** — Every `CatalogStore::open()` acquires a writer epoch, even if the caller only needs read access. This means spinning up 16 reader pods each compete for the writer epoch via CAS retry loops, causing O(N²) transaction conflicts during cold-start of a reader fleet. The `CatalogReader` is only obtainable *after* opening a full `CatalogStore`.

   - File: [crates/rocklake-catalog/src/store.rs](crates/rocklake-catalog/src/store.rs#L46-L98)
   - Impact: Reader scale-out startup latency grows linearly with replica count
   - Fix: Expose a `CatalogStore::open_readonly()` path that skips epoch acquisition

2. **No Distributed Cache Coordination** — The `CacheCounters` in [crates/rocklake-catalog/src/cache.rs](crates/rocklake-catalog/src/cache.rs) are process-local atomics. In a 16-pod fleet, each pod maintains its own independent block cache with no awareness of what other pods have cached. No shared Redis/Memcached tier is integrated or documented.

3. **`CatalogClient` Uses `tokio::sync::Mutex`** — The client wraps `CatalogStore` in `Mutex<Option<CatalogStore>>` ([crates/rocklake-client/src/lib.rs](crates/rocklake-client/src/lib.rs)), meaning all operations are serialized per client instance. Under concurrent DataFusion queries this becomes a bottleneck.

4. **No Connection Pooling** — Each DuckDB session gets a fresh PG-wire connection. The sidecar has `max_sessions` (default 50) but no idle connection reuse, keepalive, or connection draining on shutdown.

### Pillar 2: High-Throughput & Low-Latency Performance

**Strengths:**
- Hot-key optimization: single GET retrieves snapshot + table file counts for cold-start ([crates/rocklake-catalog/src/performance.rs](crates/rocklake-catalog/src/performance.rs#L28))
- Secondary index for O(1) snapshot-scoped file lookups avoids full MVCC scans
- CostMode presets (`conservative`/`balanced`/`latency`) tune SlateDB without exposing internals
- Benchmark suite shows 42μs p50 for warm snapshot reads — excellent

**Vulnerabilities:**

1. **`list_data_files` at Scale** — 100K files takes 78ms p50 / 410ms p99 per the benchmark. This is a full `scan_prefix` over all data-file-by-snapshot keys. For TPC-H SF100 with millions of files, this becomes a multi-second operation that holds the reader lock.

   - File: benchmark `list_data_files_100k` in [benchmarks/v0.42-catalog-bench.json](benchmarks/v0.42-catalog-bench.json)
   - Mitigation: Implement pagination/streaming for `list_data_files`; consider Bloom-filter-based partition pruning at the catalog level

2. **No Local NVMe/SSD Cache Tier** — SlateDB's block cache is in-memory only. For cloud deployments on instances with local NVMe (e.g., `i3en`, `r5d`), there's no mechanism to use local SSD as a persistent read cache between pod restarts. This forces full cold-start re-reads from S3 on every pod reschedule.

3. **`compare_decimal_abs` Panics on Malformed Input** — The stats comparison function in [crates/rocklake-catalog/src/writer/stats.rs](crates/rocklake-catalog/src/writer/stats.rs#L164-L165) uses `unwrap()` on `split_once('.')`. If column stats contain integer-formatted decimals (no dot), the writer panics mid-transaction.

4. **DataFusion AsyncBridge Bounded Channel (64)** — The sync bridge between DataFusion and async catalog operations uses a `sync_channel(64)` ([crates/rocklake-datafusion/src/catalog_provider.rs](crates/rocklake-datafusion/src/catalog_provider.rs)). Under high concurrency (>64 concurrent DataFusion queries), senders block, creating backpressure that can stall the entire query engine.

5. **No Streaming/Chunked Prefix Scan** — `list_data_files`, `list_schemas`, etc. collect all results into a `Vec` before returning. For large catalogs, this forces O(N) memory allocation for queries that might only need the first page of results.

### Pillar 3: Ergonomics, Usability & Operational Simplicity

**Strengths:**
- Quickstart achieves working lakehouse in 3 steps (serve → connect → query)
- CLI arg parsing uses simple positional/flag style without requiring YAML config files for basic operation
- `CatalogClientBuilder` pattern is idiomatic Rust
- Language bindings (Python, Go, Node.js, Java) all follow language-idiomatic patterns
- `rocklake diagnose` provides comprehensive self-service health reporting

**Friction Points:**

1. **No Docker One-Liner** — The README requires building from source. A `docker run ghcr.io/rocklake/rocklake serve ...` command would dramatically improve first-impression velocity. The K8s docs reference a container image but the README doesn't.

2. **`CatalogClientBuilder` Only Supports Local Filesystem** — The builder strips `file://` prefix and creates a `LocalFileSystem`. S3/GCS/Azure URIs require manual `ObjectStore` construction. This is a leaky abstraction for the "high-level" client.

   - File: [crates/rocklake-client/src/lib.rs](crates/rocklake-client/src/lib.rs) — `build()` method
   - Fix: Parse URI scheme and construct appropriate `ObjectStore` (aws, gcs, azure, file)

3. **Node.js Binding Uses `u32` for IDs** — Snapshot IDs, table IDs, and file counts are `u64` in Rust but exposed as `u32` in the napi-rs binding ([bindings/nodejs/src/lib.rs](bindings/nodejs/src/lib.rs)). JavaScript's number type safely handles integers up to 2^53, and napi-rs supports `BigInt`/`i64n`. This will silently truncate catalogs with >4B snapshots.

4. **CLI Arg Parsing is Hand-Rolled** — The main binary in [crates/rocklake-pgwire/src/main.rs](crates/rocklake-pgwire/src/main.rs) parses args via raw `&args[n]` indexing. No `--help` per subcommand, no shell completions, no typo suggestions. `clap` would add these for free.

5. **`CONTRIBUTING.md` References Non-Existent Crate** — Lists `rocklake-sqlite-vfs` which was removed; confusing for new contributors.

### Pillar 4: Observability & Day-2 Operations

**Strengths:**
- `CatalogMetrics` covers snapshots, object-store I/O, PG-wire queries, GC, and SlateDB internals
- OTLP exporter with configurable endpoint and `service_name`
- `rocklake diagnose` produces structured JSON with P0/P1/P2 findings
- Orphan file sweep with configurable grace period prevents data loss from interrupted writes
- Prometheus annotations in K8s manifests

**Gaps:**

1. **Histogram Implementation is Approximate** — Metrics use `AtomicU64` for `_us_total` and `_count`, computing averages. Real histograms (p50/p95/p99) require bucket-based collection. The benchmark JSON shows percentiles but the runtime `/metrics` endpoint cannot produce them.

   - File: [crates/rocklake-catalog/src/metrics.rs](crates/rocklake-catalog/src/metrics.rs#L30-L40)
   - Fix: Integrate `prometheus` crate or `metrics` crate with histogram support

2. **No Per-Query Trace Correlation** — While OTLP spans exist, there's no mechanism to correlate a slow DuckDB query (arriving via PG wire) with the specific catalog operations it triggered. No `trace_id` propagation from the PG-wire session context.

3. **No Memory Pressure Alerting** — No metric for current process RSS, SlateDB memtable size relative to limit, or block-cache pressure that would trigger autoscaler actions in K8s.

4. **`slatedb_sst_count`, `slatedb_compaction_lag_ms` Are Stubs** — These metrics are declared as `AtomicU64` but never updated from actual SlateDB internals (SlateDB doesn't expose these stats in its public API yet).

5. **No Slow-Query Log** — Queries exceeding a configurable threshold are not logged separately, making it hard to identify problematic workloads without full trace analysis.

### Pillar 5: Code Quality, Inconsistencies & Technical Debt

**Strengths:**
- Zero `TODO`/`FIXME`/`HACK` markers — exceptional discipline
- `deny.toml` enforces license compliance and advisory auditing
- `#![deny(missing_docs)]` on core crates ensures API documentation coverage
- 61 dedicated test files covering every major version's features
- Comprehensive MVCC test coverage with `proptest` for property-based testing
- Writer fencing has been correctly fixed (v0.28.0) from the wall-clock issue in Assessment 1

**Issues:**

1. **Production `unwrap()` in Decimal Stats** — [crates/rocklake-catalog/src/writer/stats.rs](crates/rocklake-catalog/src/writer/stats.rs#L164-L165): `split_once('.').unwrap()` will panic if DuckDB sends integer-formatted decimal stats (e.g., `"42"` instead of `"42.0"`).

2. **`SystemTime` Unwrap in Writer** — [crates/rocklake-catalog/src/writer/mod.rs](crates/rocklake-catalog/src/writer/mod.rs#L1297): `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` — technically infallible on modern systems but violates the principle of panic-free production code.

3. **130 Total `unwrap()` Calls** — While most are in doc-tests and test helpers, the sheer count warrants a systematic audit. In production catalog/pgwire code (excluding tests and doc-tests), approximately 5-8 remain in non-trivially-infallible positions.

4. **`CatalogClient` Wraps Store in `Mutex<Option<_>>`** — The `Option` is only `None` after `close()`, but every method must unwrap the option. A `close()` that consumes `self` (as currently designed) makes the `Option` unnecessary — it exists only because `Arc<Mutex<...>>` prevents consuming the inner value.

5. **SlateDB Error Mapping** — Throughout the codebase, SlateDB errors are mapped via `.to_string()` into `CatalogError::SlateDb(String)`. This loses the original error type and makes retry logic (e.g., distinguishing transient from permanent failures) impossible at the caller.

6. **`FaultInjector` Uses Global Static** — [crates/rocklake-catalog/src/fault_injection.rs](crates/rocklake-catalog/src/fault_injection.rs) uses `OnceLock<Arc<Mutex<HashMap>>>` as a global. While acceptable for testing, this prevents concurrent test isolation and could leak between test cases.

---

## 4. Concrete "DuckLake 1.0" Proposals (RFC-Style)

### RFC-01: Read-Only Catalog Access Path

**Problem:** Every `CatalogStore::open()` acquires a writer epoch via CAS, preventing true horizontal reader scale-out and causing startup contention.

**Proposal:** Add `CatalogStore::open_readonly(opts: OpenOptions) -> CatalogResult<CatalogReader>` that:
1. Opens SlateDB in read-only mode (if SlateDB supports it) or opens normally but skips epoch acquisition
2. Returns a `CatalogReader` directly without writer capabilities
3. Reads the current snapshot from the hot-key or latest snapshot key
4. Cannot create snapshots, modify metadata, or advance GC

**API:**
```rust
impl CatalogStore {
    /// Open a catalog for read-only access at the given (or latest) snapshot.
    /// Does NOT acquire a writer epoch — safe for unbounded reader replicas.
    pub async fn open_readonly(opts: OpenOptions) -> CatalogResult<ReadOnlyCatalog> { ... }
}

pub struct ReadOnlyCatalog {
    db: Db,
    snapshot_id: SnapshotId,
}

impl ReadOnlyCatalog {
    pub fn reader(&self) -> CatalogReader { ... }
    pub async fn refresh(&mut self) -> CatalogResult<SnapshotId> { ... }
}
```

**Impact:** Enables true N-replica reader fleet with zero startup coordination. Critical for the K8s HPA reader scale-out pattern.

---

### RFC-02: Tiered Storage Cache with Local SSD Spill

**Problem:** SlateDB's block cache is memory-only. Cloud instances with local NVMe (i3en, r5d, n2d-local) cannot use local SSD for persistent read caching. Every pod reschedule triggers full cold-start from S3.

**Proposal:** Implement a two-tier cache architecture:

```
┌─────────────────────────┐
│    L1: In-Memory LRU    │  (existing SlateDB block cache)
├─────────────────────────┤
│  L2: Local SSD (NVMe)   │  (new: file-backed LRU with mmap)
├─────────────────────────┤
│   L3: Object Storage    │  (S3/GCS/Azure — source of truth)
└─────────────────────────┘
```

**Implementation:**
1. New `--cache-dir /mnt/nvme/rocklake-cache` CLI flag
2. Wrap `ObjectStore` with a caching layer that writes fetched SST blocks to local SSD
3. On startup, scan cache directory and pre-populate L1 from L2
4. LRU eviction when cache directory exceeds `--cache-max-gb` (default: 80% of volume)
5. Cache entries keyed by SST file path + block offset (immutable — never invalidated)

**Data Structures:**
```rust
pub struct TieredCache {
    l1: Arc<dyn BlockCache>,           // existing SlateDB memory cache
    l2_dir: PathBuf,                   // local SSD directory
    l2_max_bytes: u64,                 // eviction threshold
    l2_index: DashMap<CacheKey, u64>,  // key → file offset
}
```

**Impact:** Reduces cold-start latency from seconds (S3 round-trips) to milliseconds (local SSD reads). Critical for serverless/Lambda reader patterns where pods are frequently recycled.

---

### RFC-03: Paginated Prefix Scans with Streaming Iterator

**Problem:** `list_data_files()` and similar operations collect all results into a `Vec` before returning. At 100K+ files this consumes significant memory and latency.

**Proposal:** Introduce `StreamingPrefixScan` that yields results in configurable page sizes:

```rust
pub struct ScanPage<T> {
    pub items: Vec<T>,
    pub continuation_token: Option<Vec<u8>>,
    pub has_more: bool,
}

impl CatalogReader {
    /// List data files for a table with pagination.
    pub async fn list_data_files_paged(
        &self,
        table_id: u64,
        page_size: usize,
        continuation: Option<&[u8]>,
    ) -> CatalogResult<ScanPage<DataFileRow>> { ... }
    
    /// Stream all data files as an async iterator (auto-pagination).
    pub fn stream_data_files(
        &self,
        table_id: u64,
    ) -> impl Stream<Item = CatalogResult<DataFileRow>> + '_ { ... }
}
```

**Wire Protocol Integration:** The PG-wire executor can use streaming to send `DataRow` messages incrementally without buffering the full result set, reducing memory pressure for large `SELECT * FROM ducklake_data_file` queries.

**Impact:** Constant-memory catalog scans regardless of catalog size. Enables true SF100+ workloads without OOM risk on reader pods with limited memory.

---

## 5. Actionable Roadmap / Low-Hanging Fruit

### Quick Wins (High Impact, Low Effort)

| # | Change | Files | Effort | Impact |
|---|--------|-------|--------|--------|
| 1 | Fix `compare_decimal_abs` unwrap → return `Ordering::Equal` on malformed input | `writer/stats.rs:164-165` | 5 min | Prevents writer panic on edge-case stats |
| 2 | Add `docker run` one-liner to README | `README.md` | 10 min | Dramatically improves "first 5 minutes" experience |
| 3 | Update `CONTRIBUTING.md` — remove `rocklake-sqlite-vfs` reference | `CONTRIBUTING.md` | 2 min | Eliminates contributor confusion |
| 4 | Update K8s docs image tag from `0.8.0` to `latest` or template variable | `docs/deployment/kubernetes.md` | 5 min | Prevents stale deployment manifests |
| 5 | Add `open_readonly` stub that skips epoch CAS | `store.rs` | 30 min | Unblocks reader scale-out without full RFC-01 |
| 6 | Replace Node.js `u32` IDs with `i64`/`BigInt` via napi-rs | `bindings/nodejs/src/lib.rs` | 20 min | Prevents silent ID truncation |
| 7 | Add slow-query log (>1s default threshold) | `handler.rs` / `executor/` | 30 min | Immediate operational visibility |
| 8 | Guard `SystemTime::now().duration_since(UNIX_EPOCH)` with `.unwrap_or_default()` | `writer/mod.rs:1297` | 2 min | Removes theoretical panic path |

### Strategic Initiatives (Larger Refactors for Multi-Node Scale)

| # | Initiative | Scope | Priority | Dependency |
|---|-----------|-------|----------|------------|
| A | **Read-Only Catalog Path (RFC-01)** | `store.rs`, `client`, K8s docs | P0 | None |
| B | **Tiered Storage Cache (RFC-02)** | New `rocklake-cache` crate, CLI integration | P1 | SlateDB block-cache API |
| C | **Paginated Scans (RFC-03)** | `reader.rs`, PG-wire executor, client bindings | P1 | None |
| D | **Proper Histogram Metrics** | `metrics.rs`, `/metrics` endpoint | P2 | `prometheus` crate integration |
| E | **Connection Pooling & Drain** | `server.rs`, graceful shutdown | P2 | None |
| F | **Multi-URI CatalogClientBuilder** | `rocklake-client`, object-store URL parsing | P2 | `object_store` URL support |
| G | **CLI Migration to `clap`** | `main.rs` | P3 | None — additive |
| H | **SlateDB Error Type Preservation** | `error.rs`, all `.to_string()` mappings | P3 | SlateDB public error types |

### Recommended v1.0 Gating Criteria

Before cutting v1.0, the following should be achieved:

1. ✅ All P0 findings from this assessment resolved
2. RFC-01 (read-only path) implemented and validated with 16-pod reader fleet on real S3
3. Benchmark suite extended to SF100 catalog operations
4. `compare_decimal_abs` and all production `unwrap()` calls eliminated
5. Container image published to GHCR with documented tags
6. Real multi-node soak test (24h) on actual AWS/GCP infrastructure (not just `InMemory` store)
7. `CONTRIBUTING.md` and architecture docs aligned with current workspace structure

---

## Appendix: Codebase Size Breakdown

| Crate | Lines | Role |
|-------|-------|------|
| `rocklake-core` | 4,174 | Keys, MVCC, types, rows, values |
| `rocklake-catalog` | 13,531 | Full catalog operations (writer, reader, GC, CDC, metrics) |
| `rocklake-pgwire` | 5,059 | PG wire protocol server + CLI binary |
| `rocklake-sql` | ~2,500 | SQL parser/classifier |
| `rocklake-datafusion` | ~1,500 | DataFusion catalog provider |
| `rocklake-client` | 519 | High-level Rust API |
| `rocklake-ffi` | ~1,200 | C ABI for language bindings |
| `rocklake-testkit` | ~500 | Test utilities |
| Tests (all crates) | ~38,000 | Integration + unit tests |
| **Total** | **~66,800** | |

---

*End of Assessment 2*
