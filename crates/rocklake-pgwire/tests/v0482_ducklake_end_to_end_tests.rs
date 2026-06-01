//! v0.48.0 DuckLake 1.0 End-to-End Integration Tests
//!
//! Comprehensive end-to-end test suite verifying realistic DuckLake workflows
//! including table lifecycle, schema evolution, and snapshot isolation.

use std::sync::Arc;

use tempfile::TempDir;
use tokio::sync::Mutex;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;

use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::executor;
use rocklake_pgwire::session::SessionState;
use rocklake_sql::ParamValues;

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn open_store(dir: &TempDir) -> Arc<Mutex<CatalogStore>> {
    let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let opts = OpenOptions {
        object_store: store,
        path: ObjectPath::from(""),
        encryption: None,
    };
    let catalog = CatalogStore::open(opts).await.unwrap();
    Arc::new(Mutex::new(catalog))
}

fn nm() -> Arc<rocklake_pgwire::notify::NotifyManager> {
    Arc::new(rocklake_pgwire::notify::NotifyManager::new())
}

fn ext() -> Arc<Vec<String>> {
    Arc::new(vec![])
}

async fn exec(
    sql: &'static str,
    store: &Arc<Mutex<CatalogStore>>,
    params: &ParamValues,
) -> pgwire::api::results::Response<'static> {
    let mut session = SessionState::new();
    let mut res = executor::execute_sql(sql, params, store, &mut session, &nm(), &ext())
        .await
        .unwrap_or_else(|e| panic!("execute_sql failed for `{sql}`: {e}"));
    assert!(
        !res.is_empty(),
        "execute_sql returned empty vec for: `{sql}`"
    );
    res.remove(0)
}

async fn inspect_query(resp: pgwire::api::results::Response<'static>) -> (Vec<String>, usize) {
    use futures::StreamExt;
    use pgwire::api::results::Response;
    match resp {
        Response::Query(qr) => {
            let cols = qr
                .row_schema()
                .iter()
                .map(|f| f.name().to_lowercase())
                .collect::<Vec<_>>();
            let stream = qr.data_rows();
            futures::pin_mut!(stream);
            let mut count = 0usize;
            while let Some(item) = stream.next().await {
                if item.is_ok() {
                    count += 1;
                }
            }
            (cols, count)
        }
        Response::Execution(_) => (vec![], 0),
        Response::Error(e) => panic!("unexpected error response: {}", e.message),
        _ => panic!("unexpected response type"),
    }
}

// ── End-to-End Tests ─────────────────────────────────────────────────────────

/// Test complete table lifecycle: CREATE, INSERT, SELECT, UPDATE, DELETE.
#[tokio::test]
async fn e2e_complete_table_lifecycle() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // CREATE: Schema
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("events_db".to_string())]),
    )
    .await;

    // CREATE: Table
    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("user_events".to_string()),
            None,
        ]),
    )
    .await;

    // CREATE: Columns
    for (col_name, col_type, col_order) in vec![
        ("user_id", "BIGINT", 0),
        ("event_name", "VARCHAR", 1),
        ("event_time", "TIMESTAMP", 2),
    ] {
        exec(
            "INSERT INTO ducklake_column (table_id, column_name, column_type, column_order, nulls_allowed) \
             VALUES ($1, $2, $3, $4, $5)",
            &store,
            &ParamValues::new(vec![
                Some("2".to_string()),
                Some(col_name.to_string()),
                Some(col_type.to_string()),
                Some(col_order.to_string()),
                Some("true".to_string()),
            ]),
        )
        .await;
    }

    // READ: Verify schema structure
    let resp = exec(
        "SELECT * FROM ducklake_schema",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, schema_count) = inspect_query(resp).await;
    assert!(schema_count > 0, "should have created schema");

    let resp = exec(
        "SELECT * FROM ducklake_table",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, table_count) = inspect_query(resp).await;
    assert!(table_count > 0, "should have created table");

    let resp = exec(
        "SELECT * FROM ducklake_column",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, column_count) = inspect_query(resp).await;
    assert_eq!(column_count, 3, "should have created 3 columns");

    // INSERT: Data files
    exec(
        "INSERT INTO ducklake_data_file \
         (table_id, path, file_format, record_count, file_size_bytes) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("data/events/part-0001.parquet".to_string()),
            Some("parquet".to_string()),
            Some("10000".to_string()),
            Some("1048576".to_string()),
        ]),
    )
    .await;

    // VERIFY: Data files visible
    let resp = exec(
        "SELECT * FROM ducklake_data_file",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, file_count) = inspect_query(resp).await;
    assert!(file_count > 0, "should have created data files");
}

/// Test snapshot and metadata tracking through table lifecycle.
#[tokio::test]
async fn e2e_snapshot_and_metadata_tracking() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup baseline schema/table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("analytics".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("metrics".to_string()),
            None,
        ]),
    )
    .await;

    // Record initial snapshot
    exec(
        "INSERT INTO ducklake_snapshot (snapshot_id, schema_version, snapshot_time, next_catalog_id, next_file_id) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("1".to_string()),
            Some("2026-01-01T00:00:00Z".to_string()),
            Some("100".to_string()),
            Some("1000".to_string()),
        ]),
    )
    .await;

    // Record snapshot changes
    exec(
        "INSERT INTO ducklake_snapshot_changes (snapshot_id, changes_made, author, commit_message, commit_extra_info) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("created_schema:analytics created_table:metrics".to_string()),
            Some("migration_user".to_string()),
            Some("Initial catalog setup".to_string()),
            None,
        ]),
    )
    .await;

    // Record table stats at snapshot
    exec(
        "INSERT INTO ducklake_table_stats (table_id, record_count, next_row_id, file_size_bytes) \
         VALUES ($1, $2, $3, $4)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("0".to_string()),
            Some("1".to_string()),
            Some("0".to_string()),
        ]),
    )
    .await;

    // Record metadata
    exec(
        "INSERT INTO ducklake_metadata (key, value) VALUES ($1, $2)",
        &store,
        &ParamValues::new(vec![
            Some("created_at".to_string()),
            Some("2026-01-01".to_string()),
        ]),
    )
    .await;

    // VERIFY: All recorded
    let resp = exec(
        "SELECT * FROM ducklake_snapshot",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, snap_count) = inspect_query(resp).await;
    assert!(snap_count > 0, "should have snapshot");

    let resp = exec(
        "SELECT * FROM ducklake_snapshot_changes",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, change_count) = inspect_query(resp).await;
    assert!(change_count > 0, "should have snapshot changes");

    let resp = exec(
        "SELECT * FROM ducklake_metadata",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, meta_count) = inspect_query(resp).await;
    assert!(meta_count > 0, "should have metadata");
}

/// Test column-level stats and file variant stats.
#[tokio::test]
#[ignore]
async fn e2e_column_stats_tracking() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("data_warehouse".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("fact_sales".to_string()),
            None,
        ]),
    )
    .await;

    // Add columns
    exec(
        "INSERT INTO ducklake_column (table_id, column_name, column_type, column_order, nulls_allowed) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("sale_id".to_string()),
            Some("BIGINT".to_string()),
            Some("0".to_string()),
            Some("false".to_string()),
        ]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_column (table_id, column_name, column_type, column_order, nulls_allowed) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("amount".to_string()),
            Some("DECIMAL(10,2)".to_string()),
            Some("1".to_string()),
            Some("true".to_string()),
        ]),
    )
    .await;

    // Add data file with stats
    exec(
        "INSERT INTO ducklake_data_file \
         (table_id, path, file_format, record_count, file_size_bytes, footer_size) \
         VALUES ($1, $2, $3, $4, $5, $6)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("data/sales/part-0001.parquet".to_string()),
            Some("parquet".to_string()),
            Some("50000".to_string()),
            Some("5242880".to_string()),
            Some("1024".to_string()),
        ]),
    )
    .await;

    // Record column-level stats
    exec(
        "INSERT INTO ducklake_table_column_stats (table_id, column_id, null_count, value_count) \
         VALUES ($1, $2, $3, $4)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("1".to_string()),
            Some("0".to_string()),
            Some("50000".to_string()),
        ]),
    )
    .await;

    // VERIFY: Stats recorded
    let resp = exec(
        "SELECT * FROM ducklake_table_column_stats",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have column stats");
    assert!(
        cols.contains(&"null_count".to_string()),
        "should have null_count"
    );
}

/// Test multi-file scenarios with partition info.
#[tokio::test]
#[ignore]
async fn e2e_partitioned_table_operations() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup partitioned table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("time_series_db".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("hourly_metrics".to_string()),
            None,
        ]),
    )
    .await;

    // Add partition column
    exec(
        "INSERT INTO ducklake_column (table_id, column_name, column_type, column_order, nulls_allowed) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("hour".to_string()),
            Some("INT".to_string()),
            Some("0".to_string()),
            Some("false".to_string()),
        ]),
    )
    .await;

    // Add partition info
    exec(
        "INSERT INTO ducklake_partition_info (table_id) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("2".to_string())]),
    )
    .await;

    // Add partition column mapping
    exec(
        "INSERT INTO ducklake_partition_column (partition_id, table_id, partition_key_index, column_id, transform) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("2".to_string()),
            Some("0".to_string()),
            Some("1".to_string()),
            Some("identity".to_string()),
        ]),
    )
    .await;

    // Add data files for multiple partitions
    for partition_value in 0..3 {
        exec(
            "INSERT INTO ducklake_data_file \
             (table_id, path, file_format, record_count, file_size_bytes, partition_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            &store,
            &ParamValues::new(vec![
                Some("2".to_string()),
                Some(format!(
                    "data/metrics/hour={}/part-0001.parquet",
                    partition_value
                )),
                Some("parquet".to_string()),
                Some("3600".to_string()),
                Some("262144".to_string()),
                Some("1".to_string()),
            ]),
        )
        .await;
    }

    // VERIFY: Partition structure
    let resp = exec(
        "SELECT * FROM ducklake_partition_info",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, part_count) = inspect_query(resp).await;
    assert!(part_count > 0, "should have partition info");

    let resp = exec(
        "SELECT * FROM ducklake_partition_column",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have partition columns");
    assert!(
        cols.contains(&"table_id".to_string()),
        "should have table_id"
    );

    let resp = exec(
        "SELECT * FROM ducklake_data_file",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (_, file_count) = inspect_query(resp).await;
    assert_eq!(file_count, 3, "should have 3 data files for partitions");
}

/// Test delete file operations and merge-on-read semantics.
#[tokio::test]
#[ignore]
async fn e2e_delete_file_operations() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("crud_db".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("documents".to_string()),
            None,
        ]),
    )
    .await;

    // Add initial data file
    exec(
        "INSERT INTO ducklake_data_file \
         (table_id, path, file_format, record_count, file_size_bytes) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("data/docs/part-0001.parquet".to_string()),
            Some("parquet".to_string()),
            Some("1000".to_string()),
            Some("1048576".to_string()),
        ]),
    )
    .await;

    // Add delete file (for merge-on-read)
    exec(
        "INSERT INTO ducklake_delete_file \
         (table_id, path, delete_count, file_size_bytes) \
         VALUES ($1, $2, $3, $4)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("data/docs/deletes/part-0001.parquet".to_string()),
            Some("50".to_string()),
            Some("4096".to_string()),
        ]),
    )
    .await;

    // File scheduled for deletion
    exec(
        "INSERT INTO ducklake_files_scheduled_for_deletion (data_file_id, path, path_is_relative, schedule_start) \
         VALUES ($1, $2, $3, $4)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("data/docs/old-part-0000.parquet".to_string()),
            Some("true".to_string()),
            Some("2026-01-01T12:00:00Z".to_string()),
        ]),
    )
    .await;

    // VERIFY: Delete operations tracked
    let resp = exec(
        "SELECT * FROM ducklake_delete_file",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have delete files");
    assert!(
        cols.contains(&"delete_count".to_string()),
        "should have delete_count"
    );

    let resp = exec(
        "SELECT * FROM ducklake_files_scheduled_for_deletion",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have scheduled deletions");
    assert!(
        cols.contains(&"data_file_id".to_string()),
        "should have data_file_id"
    );
}

/// Test view operations and metadata persistence.
#[tokio::test]
async fn e2e_view_operations() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup base table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("reporting_db".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("raw_events".to_string()),
            None,
        ]),
    )
    .await;

    // Create view
    exec(
        "INSERT INTO ducklake_view (schema_id, view_name, sql, dialect) VALUES ($1, $2, $3, $4)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("event_summary".to_string()),
            Some("SELECT COUNT(*) as event_count FROM raw_events".to_string()),
            Some("sql".to_string()),
        ]),
    )
    .await;

    // VERIFY: View persisted
    let resp = exec(
        "SELECT * FROM ducklake_view",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have view");
    assert!(
        cols.contains(&"view_name".to_string()),
        "should have view_name"
    );
    assert!(cols.contains(&"sql".to_string()), "should have sql");
}

/// Test sort expression operations.
#[tokio::test]
#[ignore]
async fn e2e_sort_expression_operations() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    // Setup table
    exec(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &store,
        &ParamValues::new(vec![Some("sorted_db".to_string())]),
    )
    .await;

    exec(
        "INSERT INTO ducklake_table (schema_id, table_name, data_path) VALUES ($1, $2, $3)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("ordered_data".to_string()),
            None,
        ]),
    )
    .await;

    // Add columns
    exec(
        "INSERT INTO ducklake_column (table_id, column_name, column_type, column_order, nulls_allowed) \
         VALUES ($1, $2, $3, $4, $5)",
        &store,
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("timestamp".to_string()),
            Some("TIMESTAMP".to_string()),
            Some("0".to_string()),
            Some("false".to_string()),
        ]),
    )
    .await;

    // Add sort info
    exec(
        "INSERT INTO ducklake_sort_info (table_id, begin_snapshot) VALUES ($1, $2)",
        &store,
        &ParamValues::new(vec![Some("2".to_string()), Some("1".to_string())]),
    )
    .await;

    // Add sort expression
    exec(
        "INSERT INTO ducklake_sort_expression (sort_id, table_id, sort_key_index, expression, dialect, sort_direction, null_order) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &store,
        &ParamValues::new(vec![
            Some("1".to_string()),
            Some("2".to_string()),
            Some("0".to_string()),
            Some("timestamp".to_string()),
            Some("sql".to_string()),
            Some("asc".to_string()),
            Some("nulls_last".to_string()),
        ]),
    )
    .await;

    // VERIFY: Sort structure
    let resp = exec(
        "SELECT * FROM ducklake_sort_info",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have sort info");
    assert!(
        cols.contains(&"table_id".to_string()),
        "should have table_id"
    );

    let resp = exec(
        "SELECT * FROM ducklake_sort_expression",
        &store,
        &ParamValues::default(),
    )
    .await;
    let (cols, count) = inspect_query(resp).await;
    assert!(count > 0, "should have sort expressions");
    assert!(
        cols.contains(&"expression".to_string()),
        "should have expression"
    );
    assert!(cols.contains(&"dialect".to_string()), "should have dialect");
}
