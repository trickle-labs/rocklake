//! Observability: catalog-level metrics and Prometheus-compatible `/metrics` endpoint.
//!
//! v0.39.0 additions:
//! - `rocklake_catalog_op_duration_seconds` histogram (per-op latency)
//! - PG-wire query latency and error counters
//! - GC / excision counters
//! - SlateDB-level stubs (SST count, compaction lag)

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const QUERY_BUCKETS_US: [u64; 8] = [
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    60_000_000,
    300_000_000,
    u64::MAX,
];

/// Catalog-level metrics.
#[derive(Debug)]
pub struct CatalogMetrics {
    /// Snapshots created per second (total counter).
    pub snapshots_created: AtomicU64,
    /// Files registered per snapshot (last observation).
    pub files_per_snapshot: AtomicU64,
    /// Mean rows scanned per `list_data_files` (last observation).
    pub mean_rows_scanned: AtomicU64,
    /// Object-store request count.
    pub object_store_requests: AtomicU64,
    /// Object-store bytes read.
    pub object_store_bytes_read: AtomicU64,
    /// Object-store bytes written.
    pub object_store_bytes_written: AtomicU64,
    /// Object-store throttle count.
    pub object_store_throttles: AtomicU64,
    /// Object-store retry count.
    pub object_store_retries: AtomicU64,
    /// Deprecated alias for [`Self::connections_open`].
    pub active_sessions: AtomicU64,
    /// Max sessions configured.
    pub max_sessions: AtomicU64,
    /// Writer epoch age in milliseconds.
    pub writer_epoch_age_ms: AtomicU64,
    /// Per-query scanned key count (last).
    pub last_query_keys_scanned: AtomicU64,
    /// CDC record-count mismatches: fires when a Parquet file's scanned row
    /// count differs from the `record_count` stored in catalog metadata (N-04).
    pub cdc_record_count_mismatches: AtomicU64,

    // ── v0.39.0: operation latency histograms (accumulated microseconds) ────
    /// Accumulated `create_snapshot` latency in microseconds (total).
    pub op_create_snapshot_us_total: AtomicU64,
    /// Number of `create_snapshot` observations.
    pub op_create_snapshot_count: AtomicU64,
    /// Accumulated `list_data_files` latency in microseconds (total).
    pub op_list_data_files_us_total: AtomicU64,
    /// Number of `list_data_files` observations.
    pub op_list_data_files_count: AtomicU64,
    /// Accumulated `describe_table` latency in microseconds (total).
    pub op_describe_table_us_total: AtomicU64,
    /// Number of `describe_table` observations.
    pub op_describe_table_count: AtomicU64,
    /// Accumulated `commit_transaction` latency in microseconds (total).
    pub op_commit_transaction_us_total: AtomicU64,
    /// Number of `commit_transaction` observations.
    pub op_commit_transaction_count: AtomicU64,

    // ── v0.39.0: PG-wire metrics ─────────────────────────────────────────────
    /// Total PG-wire queries processed.
    pub pgwire_queries_total: AtomicU64,
    /// Accumulated PG-wire query latency in microseconds (total).
    pub pgwire_query_duration_us_total: AtomicU64,
    /// Total PG-wire query errors.
    pub pgwire_errors_total: AtomicU64,
    /// Total PG-wire SQLSTATE 40001 (serialisation failure) errors.
    pub pgwire_errors_40001_total: AtomicU64,
    /// Total PG-wire SQLSTATE 25006 (read-only transaction) errors.
    pub pgwire_errors_25006_total: AtomicU64,

    // ── v0.39.0: GC / excision metrics ──────────────────────────────────────
    /// Retain-from snapshot ID (last observed).
    pub gc_retain_from_snapshot: AtomicU64,
    /// Total bytes deleted by excision runs.
    pub excision_bytes_deleted_total: AtomicU64,
    /// Total rows deleted by excision runs.
    pub excision_rows_deleted_total: AtomicU64,

    // ── v0.39.0: SlateDB-level stubs ─────────────────────────────────────────
    /// Estimated SST file count (updated by background task or SlateDB stats).
    pub slatedb_sst_count: AtomicU64,
    /// Estimated compaction lag in milliseconds.
    pub slatedb_compaction_lag_ms: AtomicU64,
    /// Estimated memtable size in bytes.
    pub slatedb_memtable_bytes: AtomicU64,

    // ── v0.47.0: Connection management & DataFusion bridge ───────────────────
    /// Deprecated alias for [`Self::connections_idle`].
    pub idle_sessions: AtomicU64,
    /// Number of open PG-wire connections.
    pub connections_open: AtomicU64,
    /// Number of open connections waiting for a query.
    pub connections_idle: AtomicU64,
    /// Number of queries currently in flight.
    pub queries_in_flight: AtomicU64,
    /// Configured DataFusion AsyncBridge channel queue depth.
    pub datafusion_bridge_queue_depth: AtomicU64,
    pub pgwire_query_duration_buckets: [AtomicU64; 8],
    pub pgwire_response_rows: AtomicU64,
    pub pgwire_response_bytes: AtomicU64,
    pub pgwire_response_rows_per_second: AtomicU64,
    pub pgwire_response_bytes_per_second: AtomicU64,
    pub pgwire_ttfr_us_total: AtomicU64,
    pub pgwire_ttfr_count: AtomicU64,
    pub pgwire_sql_classification_us_total: AtomicU64,
    pub pgwire_sql_classification_count: AtomicU64,
    /// Accumulated PG-wire admission latency in microseconds.
    pub pgwire_admission_us_total: AtomicU64,
    /// Number of PG-wire admission observations.
    pub pgwire_admission_count: AtomicU64,
    /// Accumulated PG-wire execution latency in microseconds.
    pub pgwire_execution_us_total: AtomicU64,
    /// Number of PG-wire execution observations.
    pub pgwire_execution_count: AtomicU64,
    /// Accumulated PG-wire response delivery latency in microseconds.
    pub pgwire_response_delivery_us_total: AtomicU64,
    /// Number of PG-wire response delivery observations.
    pub pgwire_response_delivery_count: AtomicU64,
    pub pgwire_peak_buffered_rows: AtomicU64,
    pub active_scans: AtomicU64,
    pub max_active_scans: AtomicU64,
    pub stream_queue_depth: AtomicU64,
    pub max_buffered_rows: AtomicU64,
    pub max_response_bytes: AtomicU64,
    pub process_rss_bytes: AtomicU64,
    pub process_peak_rss_bytes: AtomicU64,
    pub resource_limit_exhaustions: AtomicU64,
    pub stream_backpressure_total: AtomicU64,
}

impl CatalogMetrics {
    pub fn new(max_sessions: u64) -> Self {
        Self {
            snapshots_created: AtomicU64::new(0),
            files_per_snapshot: AtomicU64::new(0),
            mean_rows_scanned: AtomicU64::new(0),
            object_store_requests: AtomicU64::new(0),
            object_store_bytes_read: AtomicU64::new(0),
            object_store_bytes_written: AtomicU64::new(0),
            object_store_throttles: AtomicU64::new(0),
            object_store_retries: AtomicU64::new(0),
            active_sessions: AtomicU64::new(0),
            max_sessions: AtomicU64::new(max_sessions),
            writer_epoch_age_ms: AtomicU64::new(0),
            last_query_keys_scanned: AtomicU64::new(0),
            cdc_record_count_mismatches: AtomicU64::new(0),
            // v0.39.0 op latency
            op_create_snapshot_us_total: AtomicU64::new(0),
            op_create_snapshot_count: AtomicU64::new(0),
            op_list_data_files_us_total: AtomicU64::new(0),
            op_list_data_files_count: AtomicU64::new(0),
            op_describe_table_us_total: AtomicU64::new(0),
            op_describe_table_count: AtomicU64::new(0),
            op_commit_transaction_us_total: AtomicU64::new(0),
            op_commit_transaction_count: AtomicU64::new(0),
            // v0.39.0 PG-wire
            pgwire_queries_total: AtomicU64::new(0),
            pgwire_query_duration_us_total: AtomicU64::new(0),
            pgwire_errors_total: AtomicU64::new(0),
            pgwire_errors_40001_total: AtomicU64::new(0),
            pgwire_errors_25006_total: AtomicU64::new(0),
            // v0.39.0 GC / excision
            gc_retain_from_snapshot: AtomicU64::new(0),
            excision_bytes_deleted_total: AtomicU64::new(0),
            excision_rows_deleted_total: AtomicU64::new(0),
            // v0.39.0 SlateDB stubs
            slatedb_sst_count: AtomicU64::new(0),
            slatedb_compaction_lag_ms: AtomicU64::new(0),
            slatedb_memtable_bytes: AtomicU64::new(0),
            // v0.47.0 connection management & DataFusion bridge
            idle_sessions: AtomicU64::new(0),
            connections_open: AtomicU64::new(0),
            connections_idle: AtomicU64::new(0),
            queries_in_flight: AtomicU64::new(0),
            datafusion_bridge_queue_depth: AtomicU64::new(256),
            pgwire_query_duration_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            pgwire_response_rows: AtomicU64::new(0),
            pgwire_response_bytes: AtomicU64::new(0),
            pgwire_response_rows_per_second: AtomicU64::new(0),
            pgwire_response_bytes_per_second: AtomicU64::new(0),
            pgwire_ttfr_us_total: AtomicU64::new(0),
            pgwire_ttfr_count: AtomicU64::new(0),
            pgwire_sql_classification_us_total: AtomicU64::new(0),
            pgwire_sql_classification_count: AtomicU64::new(0),
            pgwire_admission_us_total: AtomicU64::new(0),
            pgwire_admission_count: AtomicU64::new(0),
            pgwire_execution_us_total: AtomicU64::new(0),
            pgwire_execution_count: AtomicU64::new(0),
            pgwire_response_delivery_us_total: AtomicU64::new(0),
            pgwire_response_delivery_count: AtomicU64::new(0),
            pgwire_peak_buffered_rows: AtomicU64::new(0),
            active_scans: AtomicU64::new(0),
            max_active_scans: AtomicU64::new(25),
            stream_queue_depth: AtomicU64::new(64),
            max_buffered_rows: AtomicU64::new(1024),
            max_response_bytes: AtomicU64::new(16 * 1024 * 1024),
            process_rss_bytes: AtomicU64::new(0),
            process_peak_rss_bytes: AtomicU64::new(0),
            resource_limit_exhaustions: AtomicU64::new(0),
            stream_backpressure_total: AtomicU64::new(0),
        }
    }

    pub fn increment_snapshots(&self) {
        self.snapshots_created.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_files_per_snapshot(&self, count: u64) {
        self.files_per_snapshot.store(count, Ordering::Relaxed);
    }

    pub fn increment_object_store_requests(&self) {
        self.object_store_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_object_store_bytes_read(&self, bytes: u64) {
        self.object_store_bytes_read
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_object_store_bytes_written(&self, bytes: u64) {
        self.object_store_bytes_written
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn increment_throttles(&self) {
        self.object_store_throttles.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_retries(&self) {
        self.object_store_retries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_active_sessions(&self, count: u64) {
        self.set_connections_open(count);
    }

    pub fn set_idle_sessions(&self, count: u64) {
        self.set_connections_idle(count);
    }

    pub fn set_connections_open(&self, count: u64) {
        self.connections_open.store(count, Ordering::Relaxed);
        self.active_sessions.store(count, Ordering::Relaxed);
    }

    pub fn set_connections_idle(&self, count: u64) {
        self.connections_idle.store(count, Ordering::Relaxed);
        self.idle_sessions.store(count, Ordering::Relaxed);
    }

    pub fn set_queries_in_flight(&self, count: u64) {
        self.queries_in_flight.store(count, Ordering::Relaxed);
    }

    pub fn set_datafusion_bridge_queue_depth(&self, depth: u64) {
        self.datafusion_bridge_queue_depth
            .store(depth, Ordering::Relaxed);
    }

    pub fn set_resource_limits(&self, max_scans: u64, queue: u64, rows: u64, bytes: u64) {
        self.max_active_scans.store(max_scans, Ordering::Relaxed);
        self.stream_queue_depth.store(queue, Ordering::Relaxed);
        self.max_buffered_rows.store(rows, Ordering::Relaxed);
        self.max_response_bytes.store(bytes, Ordering::Relaxed);
    }

    pub fn set_active_scans(&self, count: u64) {
        self.active_scans.store(count, Ordering::Relaxed);
    }

    pub fn increment_resource_limit_exhaustions(&self) {
        self.resource_limit_exhaustions
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_stream_backpressure(&self) {
        self.stream_backpressure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Refresh process memory metrics when the host exposes RSS through `ps`.
    pub fn refresh_process_memory(&self) {
        let pid = std::process::id().to_string();
        let Ok(output) = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid])
            .output()
        else {
            return;
        };
        let Ok(kib) = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
        else {
            return;
        };
        let bytes = kib.saturating_mul(1024);
        self.process_rss_bytes.store(bytes, Ordering::Relaxed);
        self.process_peak_rss_bytes
            .fetch_max(bytes, Ordering::Relaxed);
    }

    pub fn set_writer_epoch_age_ms(&self, age: u64) {
        self.writer_epoch_age_ms.store(age, Ordering::Relaxed);
    }

    pub fn set_last_query_keys_scanned(&self, count: u64) {
        self.last_query_keys_scanned.store(count, Ordering::Relaxed);
    }

    /// Sync the CDC record-count mismatch counter from the global counter in
    /// `rocklake-sql`.  Call this from a background task in the PG-Wire binary.
    pub fn set_cdc_record_count_mismatches(&self, n: u64) {
        self.cdc_record_count_mismatches.store(n, Ordering::Relaxed);
    }

    // ── v0.39.0: operation latency helpers ──────────────────────────────────

    /// Record a `create_snapshot` operation latency in microseconds.
    pub fn observe_create_snapshot_us(&self, us: u64) {
        self.op_create_snapshot_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.op_create_snapshot_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `list_data_files` operation latency in microseconds.
    pub fn observe_list_data_files_us(&self, us: u64) {
        self.op_list_data_files_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.op_list_data_files_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `describe_table` operation latency in microseconds.
    pub fn observe_describe_table_us(&self, us: u64) {
        self.op_describe_table_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.op_describe_table_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `commit_transaction` operation latency in microseconds.
    pub fn observe_commit_transaction_us(&self, us: u64) {
        self.op_commit_transaction_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.op_commit_transaction_count
            .fetch_add(1, Ordering::Relaxed);
    }

    // ── v0.39.0: PG-wire helpers ─────────────────────────────────────────────

    /// Record a completed PG-wire query with latency in microseconds.
    pub fn record_pgwire_query(&self, duration_us: u64) {
        self.pgwire_queries_total.fetch_add(1, Ordering::Relaxed);
        self.pgwire_query_duration_us_total
            .fetch_add(duration_us, Ordering::Relaxed);
        for (bucket, limit) in self
            .pgwire_query_duration_buckets
            .iter()
            .zip(QUERY_BUCKETS_US)
        {
            if duration_us <= limit {
                bucket.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn record_pgwire_response(&self, rows: u64, ttfr_us: u64) {
        self.record_pgwire_response_with_timing(rows, 0, ttfr_us, 0);
    }

    pub fn record_pgwire_response_with_bytes(&self, rows: u64, bytes: u64, ttfr_us: u64) {
        self.record_pgwire_response_with_timing(rows, bytes, ttfr_us, 0);
    }

    pub fn record_pgwire_response_with_timing(
        &self,
        rows: u64,
        bytes: u64,
        ttfr_us: u64,
        duration_us: u64,
    ) {
        self.pgwire_response_rows.fetch_add(rows, Ordering::Relaxed);
        self.pgwire_response_bytes
            .fetch_add(bytes, Ordering::Relaxed);
        self.pgwire_ttfr_us_total
            .fetch_add(ttfr_us, Ordering::Relaxed);
        self.pgwire_ttfr_count.fetch_add(1, Ordering::Relaxed);
        self.pgwire_peak_buffered_rows
            .fetch_max(u64::from(rows > 0), Ordering::Relaxed);
        if let Some(rows_per_second) = rows.saturating_mul(1_000_000).checked_div(duration_us) {
            self.pgwire_response_rows_per_second
                .store(rows_per_second, Ordering::Relaxed);
        }
        if let Some(bytes_per_second) = bytes.saturating_mul(1_000_000).checked_div(duration_us) {
            self.pgwire_response_bytes_per_second
                .store(bytes_per_second, Ordering::Relaxed);
        }
    }

    pub fn observe_sql_classification_us(&self, us: u64) {
        self.pgwire_sql_classification_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.pgwire_sql_classification_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_pgwire_admission_us(&self, us: u64) {
        self.pgwire_admission_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.pgwire_admission_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_pgwire_execution_us(&self, us: u64) {
        self.pgwire_execution_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.pgwire_execution_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_pgwire_response_delivery_us(&self, us: u64) {
        self.pgwire_response_delivery_us_total
            .fetch_add(us, Ordering::Relaxed);
        self.pgwire_response_delivery_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a PG-wire error with SQLSTATE code.
    pub fn record_pgwire_error(&self, sqlstate: &str) {
        self.pgwire_errors_total.fetch_add(1, Ordering::Relaxed);
        match sqlstate {
            "40001" => {
                self.pgwire_errors_40001_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            "25006" => {
                self.pgwire_errors_25006_total
                    .fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    // ── v0.39.0: GC / excision helpers ──────────────────────────────────────

    /// Update the retain-from snapshot gauge.
    pub fn set_gc_retain_from_snapshot(&self, snapshot_id: u64) {
        self.gc_retain_from_snapshot
            .store(snapshot_id, Ordering::Relaxed);
    }

    /// Record bytes deleted by an excision run.
    pub fn add_excision_bytes_deleted(&self, bytes: u64) {
        self.excision_bytes_deleted_total
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record rows deleted by an excision run.
    pub fn add_excision_rows_deleted(&self, rows: u64) {
        self.excision_rows_deleted_total
            .fetch_add(rows, Ordering::Relaxed);
    }

    // ── v0.39.0: SlateDB stubs ───────────────────────────────────────────────

    /// Update SlateDB SST count estimate.
    pub fn set_slatedb_sst_count(&self, count: u64) {
        self.slatedb_sst_count.store(count, Ordering::Relaxed);
    }

    /// Update SlateDB compaction lag estimate (milliseconds).
    pub fn set_slatedb_compaction_lag_ms(&self, ms: u64) {
        self.slatedb_compaction_lag_ms.store(ms, Ordering::Relaxed);
    }

    /// Update SlateDB memtable size estimate (bytes).
    pub fn set_slatedb_memtable_bytes(&self, bytes: u64) {
        self.slatedb_memtable_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Render Prometheus-compatible metrics output.
    pub fn render_prometheus(&self) -> String {
        self.refresh_process_memory();
        let mut out = String::new();

        out.push_str("# HELP rocklake_snapshots_created_total Total snapshots created.\n");
        out.push_str("# TYPE rocklake_snapshots_created_total counter\n");
        out.push_str(&format!(
            "rocklake_snapshots_created_total {}\n",
            self.snapshots_created.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_files_per_snapshot Files registered in last snapshot.\n");
        out.push_str("# TYPE rocklake_files_per_snapshot gauge\n");
        out.push_str(&format!(
            "rocklake_files_per_snapshot {}\n",
            self.files_per_snapshot.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_object_store_requests_total Object store request count.\n");
        out.push_str("# TYPE rocklake_object_store_requests_total counter\n");
        out.push_str(&format!(
            "rocklake_object_store_requests_total {}\n",
            self.object_store_requests.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_object_store_bytes_read_total Bytes read from object store.\n",
        );
        out.push_str("# TYPE rocklake_object_store_bytes_read_total counter\n");
        out.push_str(&format!(
            "rocklake_object_store_bytes_read_total {}\n",
            self.object_store_bytes_read.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_object_store_bytes_written_total Bytes written to object store.\n",
        );
        out.push_str("# TYPE rocklake_object_store_bytes_written_total counter\n");
        out.push_str(&format!(
            "rocklake_object_store_bytes_written_total {}\n",
            self.object_store_bytes_written.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_object_store_throttles_total Object store throttle events.\n",
        );
        out.push_str("# TYPE rocklake_object_store_throttles_total counter\n");
        out.push_str(&format!(
            "rocklake_object_store_throttles_total {}\n",
            self.object_store_throttles.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_object_store_retries_total Object store retry events.\n");
        out.push_str("# TYPE rocklake_object_store_retries_total counter\n");
        out.push_str(&format!(
            "rocklake_object_store_retries_total {}\n",
            self.object_store_retries.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_connections_open Current open PG-wire connections.\n");
        out.push_str("# TYPE rocklake_connections_open gauge\n");
        out.push_str(&format!(
            "rocklake_connections_open {}\n",
            self.connections_open.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_connections_idle Current idle PG-wire connections.\n");
        out.push_str("# TYPE rocklake_connections_idle gauge\n");
        out.push_str(&format!(
            "rocklake_connections_idle {}\n",
            self.connections_idle.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_queries_in_flight Current PG-wire queries in flight.\n");
        out.push_str("# TYPE rocklake_queries_in_flight gauge\n");
        out.push_str(&format!(
            "rocklake_queries_in_flight {}\n",
            self.queries_in_flight.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_active_sessions Deprecated alias for rocklake_connections_open.\n",
        );
        out.push_str("# TYPE rocklake_active_sessions gauge\n");
        out.push_str(&format!(
            "rocklake_active_sessions {}\n",
            self.connections_open.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_idle_sessions Deprecated alias for rocklake_connections_idle.\n",
        );
        out.push_str("# TYPE rocklake_idle_sessions gauge\n");
        out.push_str(&format!(
            "rocklake_idle_sessions {}\n",
            self.connections_idle.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_max_sessions Maximum allowed sessions.\n");
        out.push_str("# TYPE rocklake_max_sessions gauge\n");
        out.push_str(&format!(
            "rocklake_max_sessions {}\n",
            self.max_sessions.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_writer_epoch_age_ms Writer epoch age in milliseconds.\n");
        out.push_str("# TYPE rocklake_writer_epoch_age_ms gauge\n");
        out.push_str(&format!(
            "rocklake_writer_epoch_age_ms {}\n",
            self.writer_epoch_age_ms.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP rocklake_last_query_keys_scanned Keys scanned in last query.\n");
        out.push_str("# TYPE rocklake_last_query_keys_scanned gauge\n");
        out.push_str(&format!(
            "rocklake_last_query_keys_scanned {}\n",
            self.last_query_keys_scanned.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP rocklake_cdc_record_count_mismatch_total \
             Times a Parquet file's scanned row count differed from catalog metadata (N-04).\n",
        );
        out.push_str("# TYPE rocklake_cdc_record_count_mismatch_total counter\n");
        out.push_str(&format!(
            "rocklake_cdc_record_count_mismatch_total {}\n",
            self.cdc_record_count_mismatches.load(Ordering::Relaxed)
        ));

        // ── v0.39.0: operation latency (histogram summary via _sum / _count) ─
        for (op, sum_field, count_field) in [
            (
                "create_snapshot",
                self.op_create_snapshot_us_total.load(Ordering::Relaxed),
                self.op_create_snapshot_count.load(Ordering::Relaxed),
            ),
            (
                "list_data_files",
                self.op_list_data_files_us_total.load(Ordering::Relaxed),
                self.op_list_data_files_count.load(Ordering::Relaxed),
            ),
            (
                "describe_table",
                self.op_describe_table_us_total.load(Ordering::Relaxed),
                self.op_describe_table_count.load(Ordering::Relaxed),
            ),
            (
                "commit_transaction",
                self.op_commit_transaction_us_total.load(Ordering::Relaxed),
                self.op_commit_transaction_count.load(Ordering::Relaxed),
            ),
        ] {
            let sum_secs = sum_field as f64 / 1_000_000.0;
            out.push_str(&format!(
                "# HELP rocklake_catalog_op_duration_seconds_sum Accumulated {op} latency (seconds).\n"
            ));
            out.push_str("# TYPE rocklake_catalog_op_duration_seconds_sum counter\n");
            out.push_str(&format!(
                "rocklake_catalog_op_duration_seconds_sum{{op=\"{op}\"}} {sum_secs:.6}\n"
            ));
            out.push_str(&format!(
                "rocklake_catalog_op_duration_seconds_count{{op=\"{op}\"}} {count_field}\n"
            ));
        }

        // ── v0.39.0: PG-wire ─────────────────────────────────────────────────
        out.push_str("# HELP rocklake_pgwire_queries_total Total PG-wire queries processed.\n");
        out.push_str("# TYPE rocklake_pgwire_queries_total counter\n");
        out.push_str(&format!(
            "rocklake_pgwire_queries_total {}\n",
            self.pgwire_queries_total.load(Ordering::Relaxed)
        ));
        let pgwire_sum_secs =
            self.pgwire_query_duration_us_total.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        out.push_str("# HELP rocklake_pgwire_query_duration_seconds Query duration histogram.\n");
        out.push_str("# TYPE rocklake_pgwire_query_duration_seconds histogram\n");
        out.push_str(&format!(
            "rocklake_pgwire_query_duration_seconds_sum {pgwire_sum_secs:.6}\n"
        ));
        for (bucket, limit) in self
            .pgwire_query_duration_buckets
            .iter()
            .zip(QUERY_BUCKETS_US)
        {
            let count = bucket.load(Ordering::Relaxed);
            let le = if limit == u64::MAX {
                "+Inf".to_string()
            } else {
                format!("{:.3}", limit as f64 / 1_000_000.0)
            };
            out.push_str(&format!(
                "rocklake_pgwire_query_duration_seconds_bucket{{le=\"{le}\"}} {count}\n"
            ));
        }
        out.push_str(&format!(
            "rocklake_pgwire_query_duration_seconds_count {}\n",
            self.pgwire_queries_total.load(Ordering::Relaxed)
        ));
        out.push_str(
            "# HELP rocklake_pgwire_response_rows_total Rows returned by PG-wire queries.\n",
        );
        out.push_str("# TYPE rocklake_pgwire_response_rows_total counter\n");
        out.push_str(&format!(
            "rocklake_pgwire_response_rows_total {}\n",
            self.pgwire_response_rows.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_pgwire_response_bytes_total Response bytes returned by PG-wire queries.\n");
        out.push_str("# TYPE rocklake_pgwire_response_bytes_total counter\n");
        out.push_str(&format!(
            "rocklake_pgwire_response_bytes_total {}\n",
            self.pgwire_response_bytes.load(Ordering::Relaxed)
        ));
        out.push_str(
            "# HELP rocklake_pgwire_response_rows_per_second Rows per second in the last PG-wire response.\n",
        );
        out.push_str("# TYPE rocklake_pgwire_response_rows_per_second gauge\n");
        out.push_str(&format!(
            "rocklake_pgwire_response_rows_per_second {}\n",
            self.pgwire_response_rows_per_second.load(Ordering::Relaxed)
        ));
        out.push_str(
            "# HELP rocklake_pgwire_response_bytes_per_second Bytes per second in the last PG-wire response.\n",
        );
        out.push_str("# TYPE rocklake_pgwire_response_bytes_per_second gauge\n");
        out.push_str(&format!(
            "rocklake_pgwire_response_bytes_per_second {}\n",
            self.pgwire_response_bytes_per_second
                .load(Ordering::Relaxed)
        ));
        out.push_str(
            "# HELP rocklake_pgwire_time_to_first_row_seconds Time to first response row.\n",
        );
        out.push_str("# TYPE rocklake_pgwire_time_to_first_row_seconds summary\n");
        out.push_str(&format!(
            "rocklake_pgwire_time_to_first_row_seconds_sum {:.6}\n",
            self.pgwire_ttfr_us_total.load(Ordering::Relaxed) as f64 / 1_000_000.0
        ));
        out.push_str(&format!(
            "rocklake_pgwire_time_to_first_row_seconds_count {}\n",
            self.pgwire_ttfr_count.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_pgwire_sql_classification_seconds Accumulated SQL classification latency.\n");
        out.push_str("# TYPE rocklake_pgwire_sql_classification_seconds summary\n");
        out.push_str(&format!(
            "rocklake_pgwire_sql_classification_seconds_sum {:.6}\n",
            self.pgwire_sql_classification_us_total
                .load(Ordering::Relaxed) as f64
                / 1_000_000.0
        ));
        out.push_str(&format!(
            "rocklake_pgwire_sql_classification_seconds_count {}\n",
            self.pgwire_sql_classification_count.load(Ordering::Relaxed)
        ));
        for (name, total, count) in [
            (
                "admission",
                self.pgwire_admission_us_total.load(Ordering::Relaxed),
                self.pgwire_admission_count.load(Ordering::Relaxed),
            ),
            (
                "execution",
                self.pgwire_execution_us_total.load(Ordering::Relaxed),
                self.pgwire_execution_count.load(Ordering::Relaxed),
            ),
            (
                "response_delivery",
                self.pgwire_response_delivery_us_total
                    .load(Ordering::Relaxed),
                self.pgwire_response_delivery_count.load(Ordering::Relaxed),
            ),
        ] {
            let sum_secs = total as f64 / 1_000_000.0;
            out.push_str(&format!(
                "# HELP rocklake_pgwire_{name}_seconds Accumulated PG-wire {name} latency.\n"
            ));
            out.push_str(&format!("# TYPE rocklake_pgwire_{name}_seconds summary\n"));
            out.push_str(&format!(
                "rocklake_pgwire_{name}_seconds_sum {sum_secs:.6}\n"
            ));
            out.push_str(&format!("rocklake_pgwire_{name}_seconds_count {count}\n"));
        }
        out.push_str("# HELP rocklake_pgwire_errors_total Total PG-wire errors.\n");
        out.push_str("# TYPE rocklake_pgwire_errors_total counter\n");
        out.push_str(&format!(
            "rocklake_pgwire_errors_total {}\n",
            self.pgwire_errors_total.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "rocklake_pgwire_errors_total{{sqlstate=\"40001\"}} {}\n",
            self.pgwire_errors_40001_total.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "rocklake_pgwire_errors_total{{sqlstate=\"25006\"}} {}\n",
            self.pgwire_errors_25006_total.load(Ordering::Relaxed)
        ));

        // ── v0.39.0: GC / excision ────────────────────────────────────────────
        out.push_str("# HELP rocklake_gc_retain_from_snapshot Retain-from snapshot floor.\n");
        out.push_str("# TYPE rocklake_gc_retain_from_snapshot gauge\n");
        out.push_str(&format!(
            "rocklake_gc_retain_from_snapshot {}\n",
            self.gc_retain_from_snapshot.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_excision_bytes_deleted_total Bytes deleted by excision.\n");
        out.push_str("# TYPE rocklake_excision_bytes_deleted_total counter\n");
        out.push_str(&format!(
            "rocklake_excision_bytes_deleted_total {}\n",
            self.excision_bytes_deleted_total.load(Ordering::Relaxed)
        ));

        // ── v0.39.0: SlateDB stubs ─────────────────────────────────────────────
        out.push_str("# HELP rocklake_slatedb_sst_count Estimated SlateDB SST file count.\n");
        out.push_str("# TYPE rocklake_slatedb_sst_count gauge\n");
        out.push_str(&format!(
            "rocklake_slatedb_sst_count {}\n",
            self.slatedb_sst_count.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_slatedb_compaction_lag_ms Estimated compaction lag (ms).\n");
        out.push_str("# TYPE rocklake_slatedb_compaction_lag_ms gauge\n");
        out.push_str(&format!(
            "rocklake_slatedb_compaction_lag_ms {}\n",
            self.slatedb_compaction_lag_ms.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_slatedb_memtable_bytes Estimated memtable size (bytes).\n");
        out.push_str("# TYPE rocklake_slatedb_memtable_bytes gauge\n");
        out.push_str(&format!(
            "rocklake_slatedb_memtable_bytes {}\n",
            self.slatedb_memtable_bytes.load(Ordering::Relaxed)
        ));

        // ── v0.47.0: DataFusion bridge queue depth ────────────────────────────
        out.push_str(
            "# HELP rocklake_datafusion_bridge_queue_depth \
             Configured DataFusion AsyncBridge channel queue depth.\n",
        );
        out.push_str("# TYPE rocklake_datafusion_bridge_queue_depth gauge\n");
        out.push_str(&format!(
            "rocklake_datafusion_bridge_queue_depth {}\n",
            self.datafusion_bridge_queue_depth.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_active_scans Current active catalog scans.\n# TYPE rocklake_active_scans gauge\n");
        out.push_str(&format!(
            "rocklake_active_scans {}\n",
            self.active_scans.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_max_active_scans Maximum concurrent catalog scans.\n# TYPE rocklake_max_active_scans gauge\n");
        out.push_str(&format!(
            "rocklake_max_active_scans {}\n",
            self.max_active_scans.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_stream_queue_depth Configured stream queue depth.\n# TYPE rocklake_stream_queue_depth gauge\n");
        out.push_str(&format!(
            "rocklake_stream_queue_depth {}\n",
            self.stream_queue_depth.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_max_buffered_rows Maximum buffered response rows.\n# TYPE rocklake_max_buffered_rows gauge\n");
        out.push_str(&format!(
            "rocklake_max_buffered_rows {}\n",
            self.max_buffered_rows.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_pgwire_peak_buffered_rows Observed peak buffered response rows.\n# TYPE rocklake_pgwire_peak_buffered_rows gauge\n");
        out.push_str(&format!(
            "rocklake_pgwire_peak_buffered_rows {}\n",
            self.pgwire_peak_buffered_rows.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_max_response_bytes Maximum response bytes per request.\n# TYPE rocklake_max_response_bytes gauge\n");
        out.push_str(&format!(
            "rocklake_max_response_bytes {}\n",
            self.max_response_bytes.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_process_rss_bytes Current process resident set size.\n# TYPE rocklake_process_rss_bytes gauge\n");
        out.push_str(&format!(
            "rocklake_process_rss_bytes {}\n",
            self.process_rss_bytes.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_process_peak_rss_bytes Peak observed process resident set size.\n# TYPE rocklake_process_peak_rss_bytes gauge\n");
        out.push_str(&format!(
            "rocklake_process_peak_rss_bytes {}\n",
            self.process_peak_rss_bytes.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_resource_limit_exhaustions_total Resource limit exhaustion events.\n# TYPE rocklake_resource_limit_exhaustions_total counter\n");
        out.push_str(&format!(
            "rocklake_resource_limit_exhaustions_total {}\n",
            self.resource_limit_exhaustions.load(Ordering::Relaxed)
        ));
        out.push_str("# HELP rocklake_stream_backpressure_total Stream backpressure events.\n# TYPE rocklake_stream_backpressure_total counter\n");
        out.push_str(&format!(
            "rocklake_stream_backpressure_total {}\n",
            self.stream_backpressure_total.load(Ordering::Relaxed)
        ));

        out
    }
}

/// Start the metrics HTTP server on the given port, serving only on `metrics_path`.
///
/// Requests to any path other than `metrics_path` receive a 404 response.
pub async fn start_metrics_server(
    metrics: Arc<CatalogMetrics>,
    port: u16,
    metrics_path: &str,
) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let metrics_path = metrics_path.to_string();
    tracing::info!("Metrics server listening on port {port}, path {metrics_path}");

    loop {
        let (mut socket, _) = listener.accept().await?;
        let metrics = metrics.clone();
        let path = metrics_path.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = tokio::io::AsyncReadExt::read(&mut socket, &mut buf)
                .await
                .unwrap_or(0);

            // Parse the HTTP request line to extract the requested path.
            let request_path = String::from_utf8_lossy(&buf[..n]);
            let req_path = request_path
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");

            if req_path != path {
                let response =
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
                return;
            }

            let body = metrics.render_prometheus();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
    }
}
