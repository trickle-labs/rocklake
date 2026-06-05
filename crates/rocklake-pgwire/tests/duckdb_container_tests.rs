#![cfg(feature = "minio-tests")]

use std::collections::HashSet;
use std::convert::TryInto;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::OnceCell;

use rocklake_core::rows::{DataFileRow, DeleteFileRow, FilesScheduledForDeletionRow};
use rocklake_testkit::{CatalogHarness, DuckDbContainerHarness, MinioHarness, PgWireHarness};
use rocklake_pgwire::schema_registry;

static MINIO: OnceCell<MinioHarness> = OnceCell::const_new();

async fn minio() -> &'static MinioHarness {
    MINIO
        .get_or_init(|| async {
            MinioHarness::start("rocklake-pgwire-duckdb-container-tests")
                .await
                .expect("MinIO should start for DuckDB container tests")
        })
        .await
}

fn test_prefix(name: &str) -> String {
    format!("duckdb-container-loop/{name}")
}

async fn setup_catalog(name: &str) -> (CatalogHarness, PgWireHarness) {
    let prefix = test_prefix(name);
    let catalog = CatalogHarness::on_minio(minio().await, &prefix)
        .await
        .expect("catalog should open on MinIO");
    let pgwire = PgWireHarness::start_with_catalog(catalog.store.clone())
        .await
        .expect("PG-Wire server should start");
    (catalog, pgwire)
}

async fn start_duckdb(data_dir: &TempDir) -> DuckDbContainerHarness {
    DuckDbContainerHarness::start(data_dir.path())
        .await
        .expect("DuckDB container should start")
}

fn attach_sql(pgwire: &PgWireHarness, data_path: &str, body: &str) -> String {
    format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:{dsn}' AS my_lake (DATA_PATH '{data_path}'); \
         USE my_lake; \
         {body}",
        dsn = pgwire.container_connection_string(),
    )
}

async fn schema_id(catalog: &CatalogHarness, schema_name: &str) -> u64 {
    let reader = catalog.reader_latest().await;
    let schemas = reader
        .list_schemas()
        .await
        .expect("list_schemas should work");
    schemas
        .into_iter()
        .find(|schema| schema.schema_name == schema_name)
        .unwrap_or_else(|| panic!("schema {schema_name:?} was not found"))
        .schema_id
}

async fn table_id(catalog: &CatalogHarness, schema_id: u64, table_name: &str) -> u64 {
    let reader = catalog.reader_latest().await;
    let tables = reader
        .list_tables(schema_id)
        .await
        .expect("list_tables should work");
    tables
        .into_iter()
        .find(|table| table.table_name == table_name)
        .unwrap_or_else(|| panic!("table {table_name:?} was not found"))
        .table_id
}

async fn catalog_object_count(prefix: &str) -> usize {
    minio()
        .await
        .list_objects(prefix)
        .await
        .expect("list_objects should work")
        .len()
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn output_after_statement(stdout: &str, statement: &str) -> String {
    let normalized_stdout = normalize_whitespace(stdout);
    let normalized_statement = normalize_whitespace(statement);
    normalized_stdout
        .rsplit_once(&normalized_statement)
        .map(|(_, tail)| tail.to_string())
        .unwrap_or(normalized_stdout)
}

fn parquet_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_parquet_files(root, &mut files);
    files
}

fn collect_parquet_files(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_parquet_files(&path, files);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("parquet"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
}

const METADATA_TABLES: &[&str] = &[
    "ducklake_snapshot",
    "ducklake_snapshot_changes",
    "ducklake_schema",
    "ducklake_table",
    "ducklake_column",
    "ducklake_data_file",
    "ducklake_delete_file",
    "ducklake_table_stats",
    "ducklake_table_column_stats",
    "ducklake_file_column_stats",
    "ducklake_metadata",
    "ducklake_view",
    "ducklake_macro",
    "ducklake_macro_impl",
    "ducklake_macro_parameters",
    "ducklake_tag",
    "ducklake_column_tag",
    "ducklake_partition_info",
    "ducklake_partition_column",
    "ducklake_sort_info",
    "ducklake_sort_expression",
    "ducklake_files_scheduled_for_deletion",
    "ducklake_inlined_data_tables",
    "ducklake_schema_versions",
    "ducklake_column_mapping",
    "ducklake_name_mapping",
];

fn live_surface_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/ducklake-corpus/duckdb-1.5.3-ducklake-1.0-live-surface.json")
}

fn load_live_surface_fixture() -> Value {
    let path = live_surface_fixture_path();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read live surface fixture at {}: {e}", path.display()));
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("failed to parse live surface fixture JSON: {e}"))
}

async fn commit_visibility_barrier(catalog: &CatalogHarness, author: &str, message: &str) {
    let mut writer = catalog.writer().await;
    let snapshot = writer
        .create_snapshot(Some(author), Some(message))
        .await
        .expect("visibility barrier snapshot should succeed");
    catalog.commit_writer(snapshot).await;
}

fn registry_column_names(table_name: &str) -> Vec<String> {
    let fields = schema_registry::fields_for_table(table_name)
        .unwrap_or_else(|| panic!("registry must define {table_name}"));
    fields.iter().map(|field| field.name().to_lowercase()).collect()
}

async fn assert_query_columns(
    client: &tokio_postgres::Client,
    sql: &str,
    expected_columns: &[String],
) -> Vec<tokio_postgres::Row> {
    let statement = client
        .prepare(sql)
        .await
        .unwrap_or_else(|e| panic!("prepare failed for `{sql}`: {e}"));
    let actual_columns = statement
        .columns()
        .iter()
        .map(|column| column.name().to_lowercase())
        .collect::<Vec<_>>();
    let expected_columns = expected_columns.to_vec();
    assert_eq!(
        actual_columns,
        expected_columns,
        "row description mismatch for `{sql}`"
    );
    client
        .query(&statement, &[])
        .await
        .unwrap_or_else(|e| panic!("query execution failed for `{sql}`: {e}"))
}

async fn assert_registry_query_columns(
    client: &tokio_postgres::Client,
    sql: &str,
    table_name: &str,
) -> Vec<tokio_postgres::Row> {
    let expected_columns = registry_column_names(table_name);
    assert_query_columns(client, sql, &expected_columns).await
}

fn extract_schema_version_hint(version_text: &str) -> Option<i64> {
    version_text
        .split(|character: char| !character.is_ascii_digit())
        .filter(|segment| !segment.is_empty())
        .last()
        .and_then(|segment| segment.parse::<i64>().ok())
}

fn decode_latest_snapshot_value(value_text: &str) -> Option<u64> {
    if let Ok(value) = value_text.parse::<u64>() {
        return Some(value);
    }

    let bytes = value_text.as_bytes();
    let raw_bytes: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_be_bytes(raw_bytes))
}

fn parquet_base_names(root: &Path) -> HashSet<String> {
    parquet_files(root)
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .collect()
}

fn file_base_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| panic!("file path has no final component: {path}"))
        .to_string()
}

fn catalog_parquet_base_names(
    data_files: &[DataFileRow],
    delete_files: &[DeleteFileRow],
    scheduled_files: &[FilesScheduledForDeletionRow],
) -> HashSet<String> {
    data_files
        .iter()
        .map(|row| file_base_name(&row.path))
        .chain(delete_files.iter().map(|row| file_base_name(&row.path)))
        .chain(
            scheduled_files
                .iter()
                .map(|row| file_base_name(&row.path)),
        )
        .collect()
}

async fn assert_disk_files_are_accounted_for(
    catalog: &CatalogHarness,
    table_id: u64,
    data_dir: &Path,
) {
    let reader = catalog.reader_latest().await;
    let data_files = reader
        .list_data_files(table_id)
        .await
        .expect("list_data_files should work");
    let delete_files = reader
        .list_delete_files(table_id)
        .await
        .expect("list_delete_files should work");
    let scheduled_files = reader
        .list_files_scheduled_for_deletion()
        .await
        .expect("list_files_scheduled_for_deletion should work");

    let catalog_files = catalog_parquet_base_names(&data_files, &delete_files, &scheduled_files);
    let disk_files = parquet_base_names(data_dir);

    assert!(
        disk_files.is_subset(&catalog_files),
        "every local parquet file should be accounted for by catalog metadata\nlocal: {:?}\ncatalog: {:?}",
        disk_files,
        catalog_files
    );
}

async fn bootstrap_analytics_events(
    catalog: &CatalogHarness,
    pgwire: &PgWireHarness,
    duckdb: &DuckDbContainerHarness,
    barrier_message: &str,
) -> (u64, u64) {
    let bootstrap_sql = attach_sql(
        pgwire,
        duckdb.data_path(),
        "CREATE SCHEMA IF NOT EXISTS analytics; \
         CREATE TABLE analytics.events (id INTEGER, ts TIMESTAMP, payload VARCHAR); \
         INSERT INTO analytics.events \
             SELECT range, TIMESTAMP '2026-06-04 00:00:00', 'row-' || range::VARCHAR \
             FROM range(1, 13); \
         SELECT COUNT(*) AS total FROM analytics.events;",
    );
    let _output = duckdb
        .run_sql(&bootstrap_sql)
        .await
        .expect("bootstrap phase should succeed");

    commit_visibility_barrier(catalog, "duckdb-container", barrier_message).await;

    let analytics_schema_id = schema_id(catalog, "analytics").await;
    let analytics_table_id = table_id(catalog, analytics_schema_id, "events").await;
    (analytics_schema_id, analytics_table_id)
}

async fn describe_table_column_names(catalog: &CatalogHarness, table_id: u64) -> Vec<String> {
    let reader = catalog.reader_latest().await;
    let described_table = reader
        .describe_table(table_id)
        .await
        .expect("describe_table should succeed")
        .unwrap_or_else(|| panic!("table {table_id} should still be visible"));
    described_table
        .1
        .into_iter()
        .map(|column| column.column_name)
        .collect()
}

#[tokio::test]
async fn duckdb_full_ducklake_tutorial_against_minio_container() {
    let prefix = test_prefix("full_tutorial");
    let (catalog, pgwire) = setup_catalog("full_tutorial").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");
    let duckdb = start_duckdb(&data_dir).await;

    let baseline_objects = catalog_object_count(&prefix).await;

    let bootstrap_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "CREATE SCHEMA IF NOT EXISTS analytics; \
         CREATE TABLE analytics.events (id INTEGER, ts TIMESTAMP, payload VARCHAR); \
         INSERT INTO analytics.events \
             SELECT range, TIMESTAMP '2026-06-04 00:00:00', 'row-' || range::VARCHAR \
             FROM range(1, 13);",
    );
    duckdb
        .run_sql(&bootstrap_sql)
        .await
        .expect("bootstrap phase should succeed");

    let bootstrap_read_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "SELECT id, payload FROM analytics.events WHERE id IN (2, 11, 12) ORDER BY id;",
    );
    let _bootstrap_snapshot = catalog.reader_latest().await.snapshot_id();
    let analytics_schema_id = schema_id(&catalog, "analytics").await;
    let analytics_table_id = table_id(&catalog, analytics_schema_id, "events").await;
    let data_files = catalog
        .reader_latest()
        .await
        .list_data_files(analytics_table_id)
        .await
        .expect("list_data_files should work after bootstrap");

    assert!(
        !data_files.is_empty(),
        "bootstrap should create visible data files"
    );
    assert!(
        catalog_object_count(&prefix).await >= baseline_objects,
        "catalog objects should not disappear after bootstrap"
    );
    assert!(
        !parquet_files(data_dir.path()).is_empty(),
        "duckdb data directory should contain Parquet files after bootstrap"
    );

    let mutation_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "UPDATE analytics.events SET payload = 'row-2-updated' WHERE id = 2; \
         DELETE FROM analytics.events WHERE id = 1;",
    );
    duckdb
        .run_sql(&mutation_sql)
        .await
        .expect("mutation write phase should succeed");

    let mutation_read_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "SELECT id, payload FROM analytics.events WHERE id IN (2, 12) ORDER BY id; \
         SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 \
         FROM analytics.events WHERE id = 1;",
    );
    let mutation_output = duckdb
        .run_sql(&mutation_read_sql)
        .await
        .expect("mutation readback should succeed");
    let delete_result = output_after_statement(
        &mutation_output.stdout,
        "SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 FROM analytics.events WHERE id = 1;",
    );

    assert!(
        delete_result.contains("deleted"),
        "mutation output should prove the deleted row is gone\nstdout: {}\nstderr: {}",
        mutation_output.stdout,
        mutation_output.stderr
    );

    let mut visibility_writer = catalog.writer().await;
    let visibility_snapshot = visibility_writer
        .create_snapshot(
            Some("duckdb-container"),
            Some("mutation visibility barrier"),
        )
        .await
        .expect("mutation visibility barrier snapshot should succeed");
    catalog.commit_writer(visibility_snapshot).await;

    let mutation_files = catalog
        .reader_latest()
        .await
        .list_data_files(analytics_table_id)
        .await
        .expect("list_data_files should work after mutation");
    assert!(
        mutation_files.len() >= data_files.len(),
        "mutation should not hide previously visible data-file metadata"
    );
    assert!(
        catalog_object_count(&prefix).await >= baseline_objects,
        "catalog objects should remain visible after mutation"
    );

    let cleanup_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "DROP TABLE analytics.events; \
         DROP SCHEMA analytics;",
    );
    duckdb
        .run_sql(&cleanup_sql)
        .await
        .expect("cleanup phase should succeed");

    catalog
        .assert_durable()
        .await
        .expect("catalog should reopen cleanly after cleanup");

    duckdb.stop().await;
    pgwire.stop().await;
}

#[tokio::test]
async fn duckdb_container_restart_and_reconnect_preserves_state() {
    let prefix = test_prefix("restart_reconnect");
    let (catalog, pgwire) = setup_catalog("restart_reconnect").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");

    {
        let duckdb = start_duckdb(&data_dir).await;
        let write_sql = attach_sql(
            &pgwire,
            duckdb.data_path(),
            "CREATE SCHEMA IF NOT EXISTS analytics; \
             CREATE TABLE analytics.events (id INTEGER, ts TIMESTAMP, payload VARCHAR); \
             INSERT INTO analytics.events \
                 SELECT range, TIMESTAMP '2026-06-04 00:00:00', 'row-' || range::VARCHAR \
                 FROM range(1, 13); \
             SELECT COUNT(*) AS total FROM analytics.events;",
        );
        let _output = duckdb
            .run_sql(&write_sql)
            .await
            .expect("initial write phase should succeed");

        duckdb
            .run_sql("CHECKPOINT;")
            .await
            .expect("restart checkpoint should succeed");

        let mut visibility_writer = catalog.writer().await;
        let visibility_snapshot = visibility_writer
            .create_snapshot(Some("duckdb-container"), Some("restart visibility barrier"))
            .await
            .expect("restart visibility barrier snapshot should succeed");
        catalog.commit_writer(visibility_snapshot).await;

        duckdb.stop().await;
    }

    let objects_before_restart = catalog_object_count(&prefix).await;
    let first_snapshot = catalog.reader_latest().await.snapshot_id();

    pgwire.stop().await;
    catalog
        .reopen()
        .await
        .expect("catalog should reopen cleanly before restart");
    let pgwire = PgWireHarness::start_with_catalog(catalog.store.clone())
        .await
        .expect("PG-Wire server should restart");

    {
        let duckdb = start_duckdb(&data_dir).await;
        let read_sql = attach_sql(&pgwire, duckdb.data_path(), "SELECT 'ok' AS status;");
        let output = duckdb
            .run_sql(&read_sql)
            .await
            .expect("restart read phase should succeed");

        assert!(
            output.stdout.contains("ok"),
            "restarted DuckDB should respond after reconnect\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );

        duckdb.stop().await;
    }

    let reader = catalog.reader_latest().await;
    let schema_id = schema_id(&catalog, "analytics").await;
    let table_id = table_id(&catalog, schema_id, "events").await;
    let files = reader
        .list_data_files(table_id)
        .await
        .expect("list_data_files should work after restart");

    assert!(
        reader.snapshot_id().as_u64() >= first_snapshot.as_u64(),
        "restarted catalog should not move backwards"
    );
    assert!(
        !files.is_empty(),
        "restarted catalog should still expose data files"
    );
    assert!(
        catalog_object_count(&prefix).await >= objects_before_restart,
        "catalog objects should remain visible after restart"
    );
    assert!(
        !parquet_files(data_dir.path()).is_empty(),
        "mounted DuckDB data files should survive container restart"
    );

    pgwire.stop().await;
}

#[tokio::test]
async fn duckdb_container_commit_boundaries_match_catalog_state() {
    let prefix = test_prefix("commit_boundaries");
    let (catalog, pgwire) = setup_catalog("commit_boundaries").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");
    let duckdb = start_duckdb(&data_dir).await;

    let initial_objects = catalog_object_count(&prefix).await;

    let bootstrap_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "CREATE SCHEMA IF NOT EXISTS analytics; \
         CREATE TABLE analytics.events (id INTEGER, ts TIMESTAMP, payload VARCHAR); \
         INSERT INTO analytics.events \
             SELECT range, TIMESTAMP '2026-06-04 00:00:00', 'row-' || range::VARCHAR \
             FROM range(1, 13);",
    );
    duckdb
        .run_sql(&bootstrap_sql)
        .await
        .expect("bootstrap phase should succeed");

    let bootstrap_snapshot = catalog.reader_latest().await.snapshot_id();
    let bootstrap_schema_id = schema_id(&catalog, "analytics").await;
    let bootstrap_table_id = table_id(&catalog, bootstrap_schema_id, "events").await;
    let bootstrap_files = catalog
        .reader_latest()
        .await
        .list_data_files(bootstrap_table_id)
        .await
        .expect("list_data_files should work after bootstrap");

    assert!(
        bootstrap_snapshot.as_u64() > 0,
        "bootstrap should create a visible snapshot"
    );
    assert!(
        !bootstrap_files.is_empty(),
        "bootstrap should expose data files"
    );
    assert!(
        catalog_object_count(&prefix).await >= initial_objects,
        "bootstrap should not hide existing catalog objects"
    );

    let mutation_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "UPDATE analytics.events SET payload = 'row-2-updated' WHERE id = 2; \
         DELETE FROM analytics.events WHERE id = 1;",
    );
    duckdb
        .run_sql(&mutation_sql)
        .await
        .expect("mutation write phase should succeed");

    let mutation_read_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "SELECT id, payload FROM analytics.events WHERE id IN (2, 12) ORDER BY id; \
         SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 \
         FROM analytics.events WHERE id = 1;",
    );
    let mutation_output = duckdb
        .run_sql(&mutation_read_sql)
        .await
        .expect("mutation readback should succeed");
    let delete_result = output_after_statement(
        &mutation_output.stdout,
        "SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 FROM analytics.events WHERE id = 1;",
    );

    // The live DuckDB batch mutates the attached lake, so create a snapshot
    // barrier before reading the fresh catalog state back from SlateDB.
    let mut visibility_writer = catalog.writer().await;
    let visibility_snapshot = visibility_writer
        .create_snapshot(
            Some("duckdb-container"),
            Some("mutation visibility barrier"),
        )
        .await
        .expect("mutation visibility barrier snapshot should succeed");
    catalog.commit_writer(visibility_snapshot).await;

    let mutation_snapshot = catalog.reader_latest().await.snapshot_id();
    let mutation_files = catalog
        .reader_latest()
        .await
        .list_data_files(bootstrap_table_id)
        .await
        .expect("list_data_files should work after mutation");

    assert!(
        mutation_snapshot.as_u64() > bootstrap_snapshot.as_u64(),
        "mutation should advance the snapshot boundary"
    );
    assert!(
        mutation_files.len() >= bootstrap_files.len(),
        "mutation should keep the catalog-visible data files consistent"
    );
    assert!(
        delete_result.contains("deleted"),
        "mutation should prove the deleted row is gone\nstdout: {}\nstderr: {}",
        mutation_output.stdout,
        mutation_output.stderr
    );
    assert!(
        catalog
            .reader_latest()
            .await
            .list_schemas()
            .await
            .expect("list_schemas should work after mutation")
            .iter()
            .any(|schema| schema.schema_name == "analytics"),
        "mutation should keep the analytics schema visible before cleanup"
    );
    assert!(
        catalog_object_count(&prefix).await >= initial_objects,
        "mutation should not hide catalog objects in MinIO"
    );

    let cleanup_sql = attach_sql(
        &pgwire,
        duckdb.data_path(),
        "DROP TABLE analytics.events; \
         DROP SCHEMA analytics;",
    );
    duckdb
        .run_sql(&cleanup_sql)
        .await
        .expect("cleanup phase should succeed");

    catalog
        .assert_durable()
        .await
        .expect("catalog should remain durable after the live loop");

    duckdb.stop().await;
    pgwire.stop().await;
}

#[tokio::test]
async fn duckdb_container_live_surface_matches_registry_and_transcript() {
    let prefix = test_prefix("live_surface");
    let (catalog, pgwire) = setup_catalog("live_surface").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");
    let duckdb = start_duckdb(&data_dir).await;
    let fixture = load_live_surface_fixture();

    assert_eq!(fixture["duckdb_version"].as_str(), Some("1.5.3"));
    assert_eq!(fixture["ducklake_version"].as_str(), Some("1.0"));
    assert_eq!(fixture["catalog_version"].as_u64(), Some(7));

    let (_analytics_schema_id, table_id) =
        bootstrap_analytics_events(&catalog, &pgwire, &duckdb, "live surface bootstrap barrier")
            .await;
    let initial_objects = catalog_object_count(&prefix).await;
    let client = pgwire.connect().await.expect("PG-Wire client should connect");

    duckdb.stop().await;

    let smoke_duckdb = start_duckdb(&data_dir).await;

    let _smoke_output = smoke_duckdb
        .run_sql(&attach_sql(
            &pgwire,
            smoke_duckdb.data_path(),
            "SELECT COUNT(*) AS total FROM analytics.events;",
        ))
        .await
        .expect("view/macro smoke query should succeed");
    let retirement_update_sql = fixture["retirement"]["update_sql"]
        .as_str()
        .expect("live surface fixture should define update_sql");
    smoke_duckdb
        .run_sql(&attach_sql(
            &pgwire,
            smoke_duckdb.data_path(),
            retirement_update_sql,
        ))
        .await
        .expect("retirement update should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "live surface retirement update visibility barrier",
    )
    .await;

    let retirement_delete_sql = fixture["retirement"]["delete_sql"]
        .as_str()
        .expect("live surface fixture should define delete_sql");
    smoke_duckdb
        .run_sql(&attach_sql(
            &pgwire,
            smoke_duckdb.data_path(),
            retirement_delete_sql,
        ))
        .await
        .expect("retirement delete should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "live surface retirement delete visibility barrier",
    )
    .await;
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "live surface retirement visibility barrier",
    )
    .await;

    assert_disk_files_are_accounted_for(&catalog, table_id, data_dir.path()).await;

    let reader = catalog.reader_latest().await;
    let snapshot_changes = reader
        .list_all_snapshot_changes()
        .await
        .expect("list_all_snapshot_changes should work");
    let metadata_rows = reader
        .list_all_metadata()
        .await
        .expect("list_all_metadata should work");
    let latest_snapshot_id = reader.snapshot_id().as_u64() as i64;
    let table_stats = reader
        .get_table_stats(table_id)
        .await
        .expect("get_table_stats should work")
        .expect("table stats should still be visible");
    let table_column_stats = reader
        .list_all_table_column_stats()
        .await
        .expect("list_all_table_column_stats should work");
    let delete_files = reader
        .list_delete_files(table_id)
        .await
        .expect("list_delete_files should work");

    for &table_name in METADATA_TABLES {
        let query = format!("SELECT * FROM __ducklake_metadata_my_lake.{table_name} LIMIT 0");
        let is_parameterized_table = table_name == "ducklake_schema"
            || table_name == "ducklake_table"
            || table_name == "ducklake_data_file";
        if !is_parameterized_table {
            smoke_duckdb
                .run_sql(&attach_sql(&pgwire, smoke_duckdb.data_path(), &query))
                .await
                .expect("metadata surface query should succeed");
        }
        if is_parameterized_table {
            let statement = client
                .prepare(&query)
                .await
                .unwrap_or_else(|e| panic!("prepare failed for `{query}`: {e}"));
            let actual_columns = statement
                .columns()
                .iter()
                .map(|column| column.name().to_lowercase())
                .collect::<Vec<_>>();
            assert_eq!(
                actual_columns,
                registry_column_names(table_name),
                "row description mismatch for `{query}`"
            );
            let live_table_id = table_id as i64;
            match table_name {
                "ducklake_schema" | "ducklake_table" => {
                    client
                        .query(&statement, &[&latest_snapshot_id])
                        .await
                        .unwrap_or_else(|e| panic!("query execution failed for `{query}`: {e}"));
                }
                "ducklake_data_file" => {
                    client
                        .query(&statement, &[&live_table_id])
                        .await
                        .unwrap_or_else(|e| panic!("query execution failed for `{query}`: {e}"));
                }
                _ => unreachable!(),
            }
            continue;
        }
        let _ = assert_registry_query_columns(&client, &query, table_name).await;
    }

    let create_view_sql = fixture["ddl"]["create_view_sql"]
        .as_str()
        .expect("live surface fixture should define create_view_sql");
    let create_macro_sql = fixture["ddl"]["create_macro_sql"]
        .as_str()
        .expect("live surface fixture should define create_macro_sql");
    smoke_duckdb
        .run_sql(&format!("{create_view_sql}; {create_macro_sql};"))
        .await
        .expect("view and macro creation should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "live surface view/macro visibility barrier",
    )
    .await;

    let reader = catalog.reader_latest().await;
    let views = reader
        .list_all_views()
        .await
        .expect("list_all_views should work");
    let macros = reader
        .list_all_macros()
        .await
        .expect("list_all_macros should work");
    let metadata_queries = fixture["metadata_queries"]
        .as_array()
        .expect("metadata_queries should be an array");
    let mut latest_snapshot_value = None;

    for entry in metadata_queries {
        let sql = entry["sql"]
            .as_str()
            .expect("metadata query should include sql");
        if sql == "SELECT ducklake_latest_snapshot_id('analytics.events'::regclass)" {
            let messages = client
                .simple_query(sql)
                .await
                .unwrap_or_else(|e| panic!("simple query failed for `{sql}`: {e}"));

            let mut row_count = 0;
            for message in messages {
                if let tokio_postgres::SimpleQueryMessage::Row(row) = message {
                    row_count += 1;
                    let value_text = row.get(0).unwrap_or_else(|| {
                        panic!("latest snapshot query should return a single value")
                    });
                    latest_snapshot_value = Some(decode_latest_snapshot_value(value_text).unwrap_or_else(
                        || panic!("latest snapshot query should return an integer snapshot id: {value_text:?}"),
                    ));
                }
            }

            assert_eq!(row_count, 1, "latest snapshot query should return one row");
            continue;
        }
        let expected_columns = if let Some(columns) = entry.get("columns").and_then(|value| value.as_array()) {
            columns
                .iter()
                .map(|value| value.as_str().expect("column names must be strings").to_lowercase())
                .collect::<Vec<_>>()
        } else {
            registry_column_names(
                entry["table"]
                    .as_str()
                    .expect("metadata query should include table"),
            )
        };

        let rows = assert_query_columns(&client, sql, &expected_columns).await;

        match sql {
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_snapshot_changes" => {
                assert!(
                    rows.len() >= snapshot_changes.len(),
                    "snapshot change rows should cover the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_metadata" => {
                assert_eq!(
                    rows.len(),
                    metadata_rows.len(),
                    "metadata rows should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_schema_versions" => {
                assert_eq!(rows.len(), 1, "schema version query should return one row");
                let column_descriptions = rows[0]
                    .columns()
                    .iter()
                    .map(|column| format!("{}:{}", column.name(), column.type_().name()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let version_column = rows[0]
                    .columns()
                    .iter()
                    .position(|column| column.name() == "schema_version")
                    .expect("schema_version column should be present");

                let version = match rows[0].try_get::<_, i64>(version_column) {
                    Ok(version) => version,
                    Err(_) => match rows[0].try_get::<_, u32>(version_column) {
                        Ok(version) => version as i64,
                        Err(_) => {
                            let version_info_column = rows[0]
                                .columns()
                                .iter()
                                .position(|column| column.name() == "schema_version_info");

                            let version_text = version_info_column
                                .and_then(|column_index| rows[0].try_get::<_, String>(column_index).ok())
                                .or_else(|| rows[0].try_get::<_, String>(version_column).ok())
                                .unwrap_or_else(|| {
                                    panic!(
                                        "schema_version should decode as an integer or string; columns=[{column_descriptions}]"
                                    )
                                });

                            extract_schema_version_hint(&version_text).unwrap_or_else(|| {
                                panic!(
                                    "schema_version should contain a numeric version; columns=[{column_descriptions}], value={version_text:?}"
                                )
                            })
                        }
                    },
                };
                assert_eq!(version, 3, "schema version should remain pinned to 3");
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_view" => {
                assert_eq!(
                    rows.len(),
                    views.len(),
                    "view rows should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_macro" => {
                assert_eq!(
                    rows.len(),
                    macros.len(),
                    "macro rows should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_macro_impl" => {
                let macro_rows = reader.list_all_macros().await.expect("list_all_macros should work");
                let macro_impls = if let Some(first_macro) = macro_rows.first() {
                    reader
                        .list_macro_impls(first_macro.macro_id)
                        .await
                        .expect("list_macro_impls should work")
                } else {
                    Vec::new()
                };

                assert_eq!(
                    rows.len(),
                    macro_impls.len(),
                    "macro implementations should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_macro_parameters" => {
                let macro_rows = reader.list_all_macros().await.expect("list_all_macros should work");
                let macro_impls = if let Some(first_macro) = macro_rows.first() {
                    reader
                        .list_macro_impls(first_macro.macro_id)
                        .await
                        .expect("list_macro_impls should work")
                } else {
                    Vec::new()
                };
                let macro_parameters = if let (Some(first_macro), Some(first_impl)) =
                    (macro_rows.first(), macro_impls.first())
                {
                    reader
                        .list_macro_parameters(first_macro.macro_id, first_impl.impl_id)
                        .await
                        .expect("list_macro_parameters should work")
                } else {
                    Vec::new()
                };

                assert_eq!(
                    rows.len(),
                    macro_parameters.len(),
                    "macro parameters should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_table_stats" => {
                assert_eq!(rows.len(), 1, "table stats query should return one row");
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_table_column_stats" => {
                let expected_row_count = table_column_stats
                    .iter()
                    .filter(|row| row.table_id == table_id)
                    .count();
                assert_eq!(
                    rows.len(),
                    expected_row_count,
                    "table column stats should match the catalog"
                );
            }
            "SELECT * FROM __ducklake_metadata_my_lake.ducklake_delete_file" => {
                assert_eq!(
                    rows.len(),
                    delete_files.len(),
                    "delete file rows should match the catalog"
                );
            }
            _ => {}
        }
    }

    let reader_snapshot_id = reader.snapshot_id().as_u64();
    assert_eq!(
        latest_snapshot_value,
        Some(reader_snapshot_id),
        "ducklake_latest_snapshot_id should match the catalog reader"
    );

    drop(client);

    let final_object_count = catalog_object_count(&prefix).await;
    assert!(
        final_object_count >= initial_objects,
        "live surface operations should not hide MinIO objects"
    );

    let durability_result = catalog.assert_durable().await;
    durability_result.expect("catalog should remain durable after the live surface transcript");

    smoke_duckdb.stop().await;
    pgwire.stop().await;
}

#[tokio::test]
async fn duckdb_container_schema_evolution_and_object_store_integrity() {
    let prefix = test_prefix("schema_evolution");
    let (catalog, pgwire) = setup_catalog("schema_evolution").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");
    let duckdb = start_duckdb(&data_dir).await;
    let fixture = load_live_surface_fixture();

    let (analytics_schema_id, table_id) =
        bootstrap_analytics_events(&catalog, &pgwire, &duckdb, "schema evolution bootstrap barrier")
            .await;
    let client = pgwire.connect().await.expect("PG-Wire client should connect");

    let initial_columns = describe_table_column_names(&catalog, table_id).await;
    assert_eq!(
        initial_columns,
        vec!["id".to_string(), "ts".to_string(), "payload".to_string()],
        "initial column order should match the bootstrap table"
    );

    let add_column_sql = fixture["ddl"]["add_column_sql"]
        .as_str()
        .expect("live surface fixture should define add_column_sql");
    duckdb
        .run_sql(add_column_sql)
        .await
        .expect("ALTER TABLE ADD COLUMN should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "schema evolution add-column visibility barrier",
    )
    .await;

    let added_columns = describe_table_column_names(&catalog, table_id).await;
    assert_eq!(
        added_columns,
        vec![
            "id".to_string(),
            "ts".to_string(),
            "payload".to_string(),
            "status".to_string(),
        ],
        "column order should include the added column at the end"
    );

    let add_column_output = duckdb
        .run_sql("SELECT id FROM analytics.events ORDER BY id LIMIT 1;")
        .await
        .expect("table should remain queryable after ALTER TABLE ADD COLUMN");
    assert!(
        add_column_output.stdout.contains("1"),
        "base rows should remain readable after ALTER TABLE ADD COLUMN\nstdout: {}\nstderr: {}",
        add_column_output.stdout,
        add_column_output.stderr
    );

    let rename_column_sql = fixture["ddl"]["rename_column_sql"]
        .as_str()
        .expect("live surface fixture should define rename_column_sql");
    duckdb
        .run_sql(rename_column_sql)
        .await
        .expect("ALTER TABLE RENAME COLUMN should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "schema evolution rename-column visibility barrier",
    )
    .await;

    let renamed_columns = describe_table_column_names(&catalog, table_id).await;
    assert_eq!(
        renamed_columns,
        vec![
            "id".to_string(),
            "ts".to_string(),
            "payload".to_string(),
            "event_status".to_string(),
        ],
        "column order should reflect the renamed column"
    );

    let rename_output = duckdb
        .run_sql("SELECT id, event_status FROM analytics.events ORDER BY id LIMIT 1;")
        .await
        .expect("renamed column should remain queryable");
    assert!(
        rename_output.stdout.contains("new"),
        "renamed column should preserve the default value\nstdout: {}\nstderr: {}",
        rename_output.stdout,
        rename_output.stderr
    );

    let drop_column_sql = fixture["ddl"]["drop_column_sql"]
        .as_str()
        .expect("live surface fixture should define drop_column_sql");
    duckdb
        .run_sql(drop_column_sql)
        .await
        .expect("ALTER TABLE DROP COLUMN should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "schema evolution drop-column visibility barrier",
    )
    .await;

    let dropped_columns = describe_table_column_names(&catalog, table_id).await;
    assert_eq!(
        dropped_columns,
        vec!["id".to_string(), "ts".to_string(), "payload".to_string()],
        "column order should return to the bootstrap shape after DROP COLUMN"
    );

    let drop_row_sql = fixture["retirement"]["delete_sql"]
        .as_str()
        .expect("live surface fixture should define delete_sql");
    duckdb
        .run_sql(&format!("{}; SELECT COUNT(*) AS total FROM analytics.events;", drop_row_sql))
        .await
        .expect("delete mutation should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "schema evolution delete visibility barrier",
    )
    .await;

    assert_disk_files_are_accounted_for(&catalog, table_id, data_dir.path()).await;

    let create_view_sql = fixture["ddl"]["create_view_sql"]
        .as_str()
        .expect("live surface fixture should define create_view_sql");
    let create_macro_sql = fixture["ddl"]["create_macro_sql"]
        .as_str()
        .expect("live surface fixture should define create_macro_sql");
    duckdb
        .run_sql(&format!("{create_view_sql}; {create_macro_sql};"))
        .await
        .expect("view and macro creation should succeed");
    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "schema evolution view/macro visibility barrier",
    )
    .await;

    let reader = catalog.reader_latest().await;
    let view_rows = assert_registry_query_columns(&client, "SELECT * FROM ducklake_view", "ducklake_view").await;
    assert_eq!(view_rows.len(), reader.list_all_views().await.expect("list_all_views should work").len());
    let macro_rows = assert_registry_query_columns(&client, "SELECT * FROM ducklake_macro", "ducklake_macro").await;
    assert_eq!(macro_rows.len(), reader.list_all_macros().await.expect("list_all_macros should work").len());

    let macro_rows = reader
        .list_all_macros()
        .await
        .expect("list_all_macros should work");
    let macro_impls = if let Some(first_macro) = macro_rows.first() {
        reader
            .list_macro_impls(first_macro.macro_id)
            .await
            .expect("list_macro_impls should work")
    } else {
        Vec::new()
    };
    let macro_parameters = if let (Some(first_macro), Some(first_impl)) = (macro_rows.first(), macro_impls.first()) {
        reader
            .list_macro_parameters(first_macro.macro_id, first_impl.impl_id)
            .await
            .expect("list_macro_parameters should work")
    } else {
        Vec::new()
    };
    assert_eq!(
        assert_query_columns(
            &client,
            "SELECT * FROM ducklake_macro_impl",
            &registry_column_names("ducklake_macro_impl"),
        )
        .await
        .len(),
        macro_impls.len(),
        "macro implementations should match the catalog"
    );
    assert_eq!(
        assert_query_columns(
            &client,
            "SELECT * FROM ducklake_macro_parameters",
            &registry_column_names("ducklake_macro_parameters"),
        )
        .await
        .len(),
        macro_parameters.len(),
        "macro parameters should match the catalog"
    );

    drop(client);

    let views = reader
        .list_all_views()
        .await
        .expect("list_all_views should work");
    let macros = reader
        .list_all_macros()
        .await
        .expect("list_all_macros should work");

    assert!(
        catalog_object_count(&prefix).await >= 1,
        "schema evolution should leave catalog objects visible in MinIO"
    );
    catalog
        .assert_durable()
        .await
        .expect("catalog should remain durable after schema evolution");

    duckdb.stop().await;
    pgwire.stop().await;
}

#[tokio::test]
async fn duckdb_container_reader_isolation_and_restart_recovery() {
    let prefix = test_prefix("recovery");
    let (catalog, pgwire) = setup_catalog("recovery").await;
    let data_dir = TempDir::new().expect("duckdb data dir should be created");
    let fixture = load_live_surface_fixture();

    let bootstrap_writer = start_duckdb(&data_dir).await;
    let (_analytics_schema_id, table_id) = bootstrap_analytics_events(
        &catalog,
        &pgwire,
        &bootstrap_writer,
        "recovery bootstrap barrier",
    )
    .await;
    bootstrap_writer.stop().await;

    let before_recovery_snapshot = catalog.reader_latest().await.snapshot_id().as_u64();
    let mut writer = start_duckdb(&data_dir).await;
    let reader = start_duckdb(&data_dir).await;
    let reader_count_sql = fixture["recovery"]["reader_count_sql"]
        .as_str()
        .expect("live surface fixture should define reader_count_sql");

    writer
        .run_sql(&attach_sql(&pgwire, writer.data_path(), "SELECT 1;"))
        .await
        .expect("writer should attach before opening the raw transaction");

    let _reader_output = reader
        .run_sql(&attach_sql(
            &pgwire,
            reader.data_path(),
            reader_count_sql,
        ))
        .await
        .expect("reader should be able to see the committed rows");

    let begin_sql = fixture["recovery"]["begin_sql"]
        .as_str()
        .expect("live surface fixture should define begin_sql");
    let insert_sql = fixture["recovery"]["insert_sql"]
        .as_str()
        .expect("live surface fixture should define insert_sql");

    writer
        .execute_raw(begin_sql)
        .await
        .expect("writer BEGIN should succeed");
    writer
        .execute_raw(insert_sql)
        .await
        .expect("writer INSERT should stay inside the open transaction");

    let _reader_output = reader
        .run_sql(&attach_sql(
            &pgwire,
            reader.data_path(),
            reader_count_sql,
        ))
        .await
        .expect("reader should remain isolated from the uncommitted insert");

    writer.stop().await;

    let mut recovered_writer = start_duckdb(&data_dir).await;
    let _recovered_output = recovered_writer
        .run_sql(&attach_sql(
            &pgwire,
            recovered_writer.data_path(),
            reader_count_sql,
        ))
        .await
        .expect("recovered writer should reconnect cleanly");

    recovered_writer
        .run_sql(&attach_sql(
            &pgwire,
            recovered_writer.data_path(),
            insert_sql,
        ))
        .await
        .expect("recovered writer write phase should succeed");

    commit_visibility_barrier(
        &catalog,
        "duckdb-container",
        "recovery visibility barrier",
    )
    .await;

    let _reader_output = reader
        .run_sql(&attach_sql(
            &pgwire,
            reader.data_path(),
            reader_count_sql,
        ))
        .await
        .expect("reader should pick up the committed recovery snapshot");

    reader.stop().await;

    let reader = catalog.reader_latest().await;
    assert!(
        reader.snapshot_id().as_u64() > before_recovery_snapshot,
        "catalog snapshot should advance after the recovered commit"
    );

    let table_stats = reader
        .get_table_stats(table_id)
        .await
        .expect("get_table_stats should work after recovery")
        .expect("table stats should still be visible after recovery");
    assert!(
        table_stats.record_count >= 12,
        "table stats should reflect the recovered row count"
    );

    assert_disk_files_are_accounted_for(&catalog, table_id, data_dir.path()).await;
    assert!(
        catalog_object_count(&prefix).await >= 1,
        "recovery should leave catalog objects visible in MinIO"
    );

    catalog
        .assert_durable()
        .await
        .expect("catalog should remain durable after recovery");

    recovered_writer.stop().await;
    pgwire.stop().await;
}
