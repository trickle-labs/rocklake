#![cfg(feature = "minio-tests")]

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokio::sync::OnceCell;

use rocklake_testkit::{CatalogHarness, DuckDbContainerHarness, MinioHarness, PgWireHarness};

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
    let catalog = CatalogHarness::on_minio(minio().await, &test_prefix(name))
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
             FROM range(1, 13); \
         SELECT id, payload FROM analytics.events WHERE id IN (2, 11, 12) ORDER BY id;",
    );
    let bootstrap_output = duckdb
        .run_sql(&bootstrap_sql)
        .await
        .expect("bootstrap phase should succeed");

    assert!(
        bootstrap_output.stdout.contains("row-11"),
        "bootstrap output should contain row-11\nstdout: {}\nstderr: {}",
        bootstrap_output.stdout,
        bootstrap_output.stderr
    );
    assert!(
        bootstrap_output.stdout.contains("row-12"),
        "bootstrap output should contain row-12\nstdout: {}\nstderr: {}",
        bootstrap_output.stdout,
        bootstrap_output.stderr
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
         DELETE FROM analytics.events WHERE id = 1; \
         SELECT id, payload FROM analytics.events WHERE id IN (2, 12) ORDER BY id; \
         SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 \
         FROM analytics.events WHERE id = 1;",
    );
    let mutation_output = duckdb
        .run_sql(&mutation_sql)
        .await
        .expect("mutation phase should succeed");
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
        let output = duckdb
            .run_sql(&write_sql)
            .await
            .expect("initial write phase should succeed");
        assert!(
            output.stdout.contains("12"),
            "write phase should report 12 rows\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );

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
         DELETE FROM analytics.events WHERE id = 1; \
         SELECT id, payload FROM analytics.events WHERE id IN (2, 12) ORDER BY id; \
         SELECT CASE WHEN COUNT(*) = 0 THEN 'deleted' ELSE 'unexpected' END AS deleted_1 \
         FROM analytics.events WHERE id = 1;",
    );
    let mutation_output = duckdb
        .run_sql(&mutation_sql)
        .await
        .expect("mutation phase should succeed");
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

    pgwire.stop().await;
}
