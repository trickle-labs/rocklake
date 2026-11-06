//! Real DuckDB binary integration tests (v0.27.4).
//!
//! These tests spawn the actual `duckdb` binary against a live RockLake server
//! to verify that real DuckDB 1.5.x wire-protocol behavior is handled correctly.
//!
//! # Running
//!
//! All tests in this file require the `duckdb` binary to be in `$PATH` and the
//! `ducklake` extension to be installed. Tests that depend on not-yet-implemented
//! features are marked `#[ignore]` and must be run explicitly:
//!
//! ```sh
//! cargo test -p rocklake-pgwire --test duckdb_binary_tests -- --include-ignored
//! ```
//!
//! # What these tests cover
//!
//! - `duckdb_connects_and_pings` — DuckDB can connect, auth, and get a
//!   response for `SELECT 1` without the ducklake extension.
//! - `duckdb_attach_full_lifecycle` —
//!   Full `ATTACH 'ducklake:postgres:...'` + `USE` + `CREATE TABLE` + `SELECT`.

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::keys::MetadataScope;
use rocklake_pgwire::server::ServerConfig;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_catalog_opts(dir: &TempDir) -> OpenOptions {
    let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

/// Returns `false` if the `duckdb` binary is not on `$PATH`.
fn duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

/// Start a plain-text server on an OS-assigned port.
/// Returns `(port, shutdown_tx, handle, catalog)`.
async fn start_server(
    dir: &TempDir,
    initial_version: Option<&str>,
) -> (
    u16,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
    Arc<Mutex<CatalogStore>>,
) {
    let catalog = Arc::new(Mutex::new(
        CatalogStore::open(make_catalog_opts(dir)).await.unwrap(),
    ));
    if let Some(version) = initial_version {
        let mut store = catalog.lock().await;
        let mut writer = store.begin_write();
        writer
            .set_metadata(MetadataScope::Global, 0, "version", version)
            .unwrap();
        let snapshot = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snapshot);
    }

    // Bind on port 0 to get a free port, then drop and re-bind via ServerConfig.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let config = ServerConfig {
        bind_addr: format!("127.0.0.1:{port}").parse().unwrap(),
        ..Default::default()
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    let server_catalog = catalog.clone();
    let handle = tokio::spawn(async move {
        rocklake_pgwire::server::run_server_with_shutdown(config, server_catalog, rx)
            .await
            .unwrap();
    });

    // Give the server a moment to bind.
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    (port, tx, handle, catalog)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Verify DuckDB can connect to RockLake and execute a simple query using
/// `tokio-postgres` (same PgWire path, without triggering COPY).
///
/// This validates the full TCP + PgWire handshake. If this breaks after a
/// rocklake server change, the basic protocol handling is broken.
#[tokio::test]
async fn duckdb_compatible_client_connects_and_pings() {
    let dir = TempDir::new().unwrap();
    let (port, shutdown_tx, handle, _) = start_server(&dir, None).await;

    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let (client, conn) = tokio_postgres::connect(
            &format!("host=127.0.0.1 port={port} dbname=rocklake user=duckdb"),
            tokio_postgres::NoTls,
        )
        .await?;
        tokio::spawn(conn);
        let rows = client.query("SELECT 1", &[]).await?;
        Ok::<_, tokio_postgres::Error>(rows)
    })
    .await;

    let _ = shutdown_tx.send(());
    let _ = handle.await;

    let rows = result
        .expect("connection timed out after 10s")
        .expect("tokio-postgres query must succeed");
    assert_eq!(rows.len(), 1, "SELECT 1 must return 1 row");
}

/// Verify DuckDB binary can connect to RockLake and that the server handles
/// the postgres-scanner handshake without hanging.
///
/// Uses a 20s timeout. If the process doesn't exit within 20s, the server is
/// blocking DuckDB (e.g., an unimplemented protocol command with no response).
/// A non-zero DuckDB exit code is acceptable here — the key assertion is
/// no-hang, not success.
#[tokio::test]
async fn duckdb_binary_connects_without_hang() {
    if !duckdb_available() {
        eprintln!("SKIP: duckdb binary not available");
        return;
    }

    let dir = TempDir::new().unwrap();
    let (port, shutdown_tx, handle, _) = start_server(&dir, None).await;

    let mut child = tokio::process::Command::new("duckdb")
        .arg("-c")
        .arg(format!(
            "LOAD postgres_scanner; \
             ATTACH 'host=127.0.0.1 port={port} dbname=rocklake' AS sd (TYPE POSTGRES); \
             SELECT 1 AS connected;"
        ))
        .kill_on_drop(true)
        .spawn()
        .expect("duckdb process must start");

    let result = tokio::time::timeout(Duration::from_secs(20), child.wait()).await;

    let _ = shutdown_tx.send(());
    let _ = handle.await;

    match result {
        Err(_) => panic!("duckdb process did not exit within 20s — server may be hanging on an unimplemented protocol command"),
        Ok(Ok(_status)) => {
            // DuckDB exited (with or without error is fine — no hang is the assertion).
        }
        Ok(Err(e)) => panic!("failed to wait on duckdb process: {e}"),
    }
}

/// Full DuckLake ATTACH lifecycle: INSTALL ducklake → LOAD ducklake → ATTACH →
/// USE → CREATE TABLE → INSERT → SELECT.
///
/// This test verifies full DuckDB 1.5.x compatibility including:
/// - Postgres scanner initialization sequence
/// - COPY FROM STDIN (binary format) for catalog tables
/// - COPY TO STDOUT (binary format) for catalog reads
/// - Schema/table creation through DuckLake protocol
#[tokio::test]
#[ignore = "requires external ducklake extension install/load and can be slow/flaky in CI"]
async fn duckdb_attach_full_lifecycle() {
    let dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle, _) = start_server(&dir, None).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS my_lake \
             (DATA_PATH '{data_path}'); \
         USE my_lake; \
         CREATE SCHEMA IF NOT EXISTS analytics; \
         CREATE TABLE analytics.inline_events (id INTEGER, payload VARCHAR); \
         INSERT INTO analytics.inline_events VALUES (1, 'inline-old'), (2, 'inline-deleted'); \
         CREATE TABLE analytics.file_events (id INTEGER, payload VARCHAR); \
         INSERT INTO analytics.file_events SELECT range, 'file-' || range::VARCHAR FROM range(1, 13); \
         UPDATE analytics.inline_events SET payload = 'inline-updated' WHERE id = 1; \
         DELETE FROM analytics.inline_events WHERE id = 2; \
         UPDATE analytics.file_events SET payload = 'file-updated' WHERE id = 2; \
         DELETE FROM analytics.file_events WHERE id = 1; \
         ALTER TABLE analytics.inline_events ADD COLUMN note VARCHAR; \
         UPDATE analytics.inline_events SET note = 'schema-change' WHERE id = 1; \
         SELECT id, payload, note FROM analytics.inline_events; \
         SELECT id, payload FROM analytics.file_events WHERE id = 2;"
    );

    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-c")
            .arg(&sql)
            .output(),
    )
    .await
    .expect("duckdb ATTACH lifecycle timed out after 60s")
    .expect("duckdb process must start");

    let _ = shutdown_tx.send(());
    let _ = handle.await;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "duckdb ATTACH lifecycle failed: {}\nstdout: {stdout}\nstderr: {stderr}",
        output.status
    );
    assert!(
        stdout.contains("inline-updated")
            && stdout.contains("schema-change")
            && stdout.matches("file-updated").count() == 1
            && !stdout.contains("file-2"),
        "fresh DuckLake 1.1 should preserve inline/file writes; got:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[tokio::test]
#[ignore = "requires the DuckLake extension and a loopback listener"]
async fn duckdb_migrates_v10_catalog_to_v11_and_keeps_rows_readable() {
    let dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle, catalog) = start_server(&dir, Some("1.0")).await;
    let create_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DUCKLAKE_VERSION '1.0', DATA_PATH '{data_path}'); \
         CREATE SCHEMA lake.main; \
         CREATE TABLE lake.main.events (id INTEGER, payload VARCHAR, details STRUCT(note VARCHAR, code INTEGER)); \
         INSERT INTO lake.main.events VALUES (1, 'before migration', {{'note': 'nested', 'code': 7}}); \
         CALL ducklake_flush_inlined_data('lake', table_name => 'events'); \
         DETACH lake;"
    );
    let create_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&create_sql)
            .output(),
    )
    .await
    .expect("DuckDB v1.0 catalog creation timed out after 60s")
    .expect("duckdb process must start");
    assert!(
        create_output.status.success(),
        "DuckDB v1.0 catalog creation failed: {}\nstdout: {}\nstderr: {}",
        create_output.status,
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let snapshot_after_file_flush = {
        let catalog = catalog.lock().await;
        let reader = catalog.read_latest();
        reader
            .list_all_snapshots()
            .await
            .unwrap()
            .last()
            .expect("file flush must create a snapshot")
            .snapshot_id
    };
    let ordinary_attach_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         SELECT value FROM lake.options() WHERE option_name = 'version'; \
         INSERT INTO lake.main.events VALUES (2, 'inlined before migration', NULL); \
         DETACH lake;"
    );
    let ordinary_attach_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&ordinary_attach_sql)
            .output(),
    )
    .await
    .expect("DuckDB ordinary attach timed out after 60s")
    .expect("duckdb process must start");
    assert!(
        ordinary_attach_output.status.success(),
        "DuckDB ordinary attach failed: {}\nstdout: {}\nstderr: {}",
        ordinary_attach_output.status,
        String::from_utf8_lossy(&ordinary_attach_output.stdout),
        String::from_utf8_lossy(&ordinary_attach_output.stderr)
    );
    let ordinary_stdout = String::from_utf8_lossy(&ordinary_attach_output.stdout);
    assert!(ordinary_stdout.contains("1.0"), "{ordinary_stdout}");

    let (schemas_before, main_schema, tables_before, events_table, files_before, snapshots_before) = {
        let catalog = catalog.lock().await;
        let reader = catalog.read_latest();
        let schemas = reader.list_schemas().await.unwrap();
        let main_schema = schemas
            .iter()
            .find(|schema| schema.schema_name == "main")
            .expect("main schema must remain present")
            .clone();
        let tables = reader.list_tables(main_schema.schema_id).await.unwrap();
        let events = tables
            .iter()
            .find(|table| table.table_name == "events")
            .expect("events table must remain present")
            .clone();
        let files = reader.list_data_files(events.table_id).await.unwrap();
        assert!(
            !files.is_empty(),
            "flush must create a Parquet data-file row"
        );
        let snapshots = reader.list_all_snapshots().await.unwrap();
        (schemas, main_schema, tables, events, files, snapshots)
    };

    let migration_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (AUTOMATIC_MIGRATION TRUE, DATA_PATH '{data_path}'); \
         SELECT id, payload, details FROM lake.main.events ORDER BY id; \
         SELECT id, payload, details FROM lake.main.events AT (VERSION => {snapshot_after_file_flush}) ORDER BY id; \
         SELECT value FROM lake.options() WHERE option_name = 'version';"
    );
    let output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&migration_sql)
            .output(),
    )
    .await
    .expect("DuckDB migration timed out after 90s")
    .expect("duckdb process must start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "DuckDB v1.0 migration failed: {}\nstdout: {stdout}\nstderr: {stderr}",
        output.status
    );
    assert!(stdout.contains("before migration"), "{stdout}");
    assert!(stdout.contains("nested"), "{stdout}");
    assert!(stdout.contains("inlined before migration"), "{stdout}");
    assert_eq!(
        stdout.matches("inlined before migration").count(),
        1,
        "historical reads must exclude the later inlined insert: {stdout}"
    );
    assert!(stdout.contains("1.1"), "{stdout}");
    let (schemas_after, tables_after, files_after, snapshots_after, version) = {
        let catalog = catalog.lock().await;
        let reader = catalog.read_latest();
        (
            reader.list_schemas().await.unwrap(),
            reader.list_tables(main_schema.schema_id).await.unwrap(),
            reader.list_data_files(events_table.table_id).await.unwrap(),
            reader.list_all_snapshots().await.unwrap(),
            reader
                .get_metadata(MetadataScope::Global, 0, "version")
                .await
                .unwrap()
                .unwrap()
                .value,
        )
    };
    assert_eq!(version, "1.1");
    assert_eq!(schemas_after, schemas_before);
    assert_eq!(tables_after, tables_before);
    assert_eq!(files_after, files_before);
    assert!(snapshots_after.len() >= snapshots_before.len());
    assert_eq!(&snapshots_after[..snapshots_before.len()], snapshots_before);

    let downgrade_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DUCKLAKE_VERSION '1.0', DATA_PATH '{data_path}');"
    );
    let downgrade_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&downgrade_sql)
            .output(),
    )
    .await
    .expect("DuckDB downgrade check timed out")
    .expect("duckdb process must start");
    let downgrade_stderr = String::from_utf8_lossy(&downgrade_output.stderr);
    assert!(!downgrade_output.status.success(), "{downgrade_stderr}");
    assert!(
        downgrade_stderr.to_ascii_lowercase().contains("downgrade"),
        "{downgrade_stderr}"
    );
    let migration_catalog = catalog.lock().await;
    assert_eq!(
        migration_catalog
            .read_latest()
            .get_metadata(MetadataScope::Global, 0, "version")
            .await
            .unwrap()
            .unwrap()
            .value,
        "1.1"
    );
    assert_eq!(
        migration_catalog
            .read_latest()
            .list_all_snapshots()
            .await
            .unwrap(),
        snapshots_after
    );
    drop(migration_catalog);

    let _ = shutdown_tx.send(());
    let _ = handle.await;
    drop(catalog);

    let (restart_port, restart_shutdown_tx, restart_handle, restarted_catalog) =
        start_server(&dir, None).await;
    let restart_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={restart_port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         SELECT id, payload FROM lake.main.events ORDER BY id; \
         SELECT id, payload FROM lake.main.events AT (VERSION => {snapshot_after_file_flush}) ORDER BY id; \
         SELECT value FROM lake.options() WHERE option_name = 'version'; \
         DETACH lake;"
    );
    let restart_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&restart_sql)
            .output(),
    )
    .await
    .expect("DuckDB post-restart attach timed out after 60s")
    .expect("duckdb process must start");
    let _ = restart_shutdown_tx.send(());
    let _ = restart_handle.await;
    let restart_stdout = String::from_utf8_lossy(&restart_output.stdout);
    let restart_stderr = String::from_utf8_lossy(&restart_output.stderr);
    assert!(
        restart_output.status.success(),
        "DuckDB post-restart attach failed: {}\nstdout: {restart_stdout}\nstderr: {restart_stderr}",
        restart_output.status
    );
    assert!(
        restart_stdout.contains("before migration"),
        "{restart_stdout}"
    );
    assert_eq!(
        restart_stdout.matches("inlined before migration").count(),
        1,
        "historical reads must exclude the later inlined insert: {restart_stdout}"
    );
    assert!(restart_stdout.contains("1.1"), "{restart_stdout}");
    let restarted_version = restarted_catalog
        .lock()
        .await
        .read_latest()
        .get_metadata(MetadataScope::Global, 0, "version")
        .await
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(restarted_version, "1.1");
}

#[tokio::test]
#[ignore = "requires the DuckLake extension and a loopback listener"]
async fn duckdb_fresh_catalog_defaults_to_v11_and_reopens_after_restart() {
    let dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle, catalog) = start_server(&dir, None).await;
    let create_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         CREATE SCHEMA lake.analytics; \
         CREATE TABLE lake.analytics.events (id INTEGER, payload VARCHAR); \
         INSERT INTO lake.analytics.events VALUES (1, 'file-backed'); \
         CALL ducklake_flush_inlined_data('lake', schema_name => 'analytics', table_name => 'events'); \
         INSERT INTO lake.analytics.events VALUES (2, 'inlined'); \
         CREATE TABLE lake.analytics.signups AS SELECT 1 AS id, NULL AS referrer; \
         SELECT value FROM lake.options() WHERE option_name = 'version'; \
         SELECT id, payload FROM lake.analytics.events ORDER BY id; \
         DETACH lake;"
    );
    let create_output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&create_sql)
            .output(),
    )
    .await
    .expect("DuckDB fresh catalog scenario timed out")
    .expect("duckdb process must start");
    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let create_stderr = String::from_utf8_lossy(&create_output.stderr);
    assert!(
        create_output.status.success(),
        "DuckDB fresh catalog scenario failed: {}\nstdout: {create_stdout}\nstderr: {create_stderr}",
        create_output.status
    );
    assert!(create_stdout.contains("1.1"), "{create_stdout}");
    assert!(create_stdout.contains("file-backed"), "{create_stdout}");
    assert!(create_stdout.contains("inlined"), "{create_stdout}");

    let (version, table_id, data_files, null_table_id, null_snapshot_id, null_type) = {
        let catalog = catalog.lock().await;
        let reader = catalog.read_latest();
        let version = reader
            .get_metadata(MetadataScope::Global, 0, "version")
            .await
            .unwrap()
            .unwrap()
            .value;
        let schema = reader
            .list_schemas()
            .await
            .unwrap()
            .into_iter()
            .find(|schema| schema.schema_name == "analytics")
            .unwrap();
        let table_id = reader
            .list_tables(schema.schema_id)
            .await
            .unwrap()
            .into_iter()
            .find(|table| table.table_name == "events")
            .unwrap()
            .table_id;
        let data_files = reader.list_data_files(table_id).await.unwrap();
        let null_table_id = reader
            .list_tables(schema.schema_id)
            .await
            .unwrap()
            .into_iter()
            .find(|table| table.table_name == "signups")
            .unwrap()
            .table_id;
        let null_type = reader
            .describe_table(null_table_id)
            .await
            .unwrap()
            .unwrap()
            .1
            .into_iter()
            .find(|column| column.column_name == "referrer")
            .unwrap()
            .data_type;
        let null_snapshot_id = reader.get_snapshot().await.unwrap().unwrap().snapshot_id;
        (
            version,
            table_id,
            data_files,
            null_table_id,
            null_snapshot_id,
            null_type,
        )
    };
    assert_eq!(version, "1.1");
    assert!(!data_files.is_empty(), "flush must register a Parquet file");
    assert_eq!(null_type.to_ascii_lowercase(), "null");

    let promote_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         ALTER TABLE lake.analytics.signups ALTER referrer TYPE VARCHAR; \
         INSERT INTO lake.analytics.signups VALUES (2, 'known'); \
         SELECT concat( \
             (SELECT count(*) FROM lake.analytics.signups WHERE referrer IS NULL)::VARCHAR, ':', \
             (SELECT count(*) FROM lake.analytics.signups WHERE referrer = 'known')::VARCHAR \
         ) AS null_counts; \
         DETACH lake;"
    );
    let promote_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&promote_sql)
            .output(),
    )
    .await
    .expect("DuckDB NULL type promotion timed out")
    .expect("duckdb process must start");
    let promote_stdout = String::from_utf8_lossy(&promote_output.stdout);
    let promote_stderr = String::from_utf8_lossy(&promote_output.stderr);
    assert!(
        promote_output.status.success(),
        "DuckDB NULL type promotion failed: {}\nstdout: {promote_stdout}\nstderr: {promote_stderr}",
        promote_output.status
    );
    assert!(promote_stdout.contains("1:1"), "{promote_stdout}");
    {
        let catalog = catalog.lock().await;
        let latest = catalog.read_latest();
        let event_columns = latest.describe_table(table_id).await.unwrap().unwrap().1;
        assert!(
            event_columns
                .iter()
                .any(|column| column.column_name == "payload"),
            "events columns after NULL promotion: {event_columns:?}"
        );
        let promoted_type = latest
            .describe_table(null_table_id)
            .await
            .unwrap()
            .unwrap()
            .1
            .into_iter()
            .find(|column| column.column_name == "referrer")
            .unwrap()
            .data_type;
        assert_eq!(promoted_type.to_ascii_lowercase(), "varchar");
        let historical = catalog
            .read_at(rocklake_core::mvcc::SnapshotId::new(null_snapshot_id))
            .unwrap()
            .describe_table(null_table_id)
            .await
            .unwrap()
            .unwrap()
            .1
            .into_iter()
            .find(|column| column.column_name == "referrer")
            .unwrap()
            .data_type;
        assert_eq!(historical.to_ascii_lowercase(), "null");
    }

    let _ = shutdown_tx.send(());
    let _ = handle.await;
    drop(catalog);

    let (restart_port, restart_shutdown_tx, restart_handle, restarted_catalog) =
        start_server(&dir, None).await;
    let restarted_events_columns = restarted_catalog
        .lock()
        .await
        .read_latest()
        .describe_table(table_id)
        .await
        .unwrap()
        .unwrap()
        .1;
    assert!(
        restarted_events_columns
            .iter()
            .any(|column| column.column_name == "payload"),
        "events columns after server restart: {restarted_events_columns:?}"
    );
    let reopen_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={restart_port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         SELECT value FROM lake.options() WHERE option_name = 'version'; \
         SELECT id, payload FROM lake.analytics.events ORDER BY id; \
         SELECT concat( \
             (SELECT count(*) FROM lake.analytics.signups WHERE referrer IS NULL)::VARCHAR, ':', \
             (SELECT count(*) FROM lake.analytics.signups WHERE referrer = 'known')::VARCHAR \
         ) AS null_counts; \
         DETACH lake;"
    );
    let reopen_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&reopen_sql)
            .output(),
    )
    .await
    .expect("DuckDB fresh catalog reopen timed out")
    .expect("duckdb process must start");
    let _ = restart_shutdown_tx.send(());
    let _ = restart_handle.await;
    let reopen_stdout = String::from_utf8_lossy(&reopen_output.stdout);
    let reopen_stderr = String::from_utf8_lossy(&reopen_output.stderr);
    assert!(
        reopen_output.status.success(),
        "DuckDB fresh catalog reopen failed: {}\nstdout: {reopen_stdout}\nstderr: {reopen_stderr}",
        reopen_output.status
    );
    assert!(reopen_stdout.contains("1.1"), "{reopen_stdout}");
    assert!(reopen_stdout.contains("file-backed"), "{reopen_stdout}");
    assert!(reopen_stdout.contains("inlined"), "{reopen_stdout}");
    assert!(reopen_stdout.contains("1:1"), "{reopen_stdout}");
    let restarted_table = restarted_catalog
        .lock()
        .await
        .read_latest()
        .list_data_files(table_id)
        .await
        .unwrap();
    assert_eq!(restarted_table, data_files);
}

#[tokio::test]
#[ignore = "requires DuckDB 2.0, the DuckLake extension, and a loopback listener"]
async fn duckdb_2_0_epoch_partitions_and_variant_inlining_survive_restart() {
    let dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle, catalog) = start_server(&dir, None).await;
    let create_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}', DATA_INLINING_ROW_LIMIT 1); \
         USE lake; \
         CREATE TABLE lake.main.variant_events \
             (id INTEGER, payload VARIANT); \
         INSERT INTO lake.main.variant_events VALUES \
             (1, '{{\"city\":\"Oslo\",\"n\":2}}'::VARIANT); \
         SELECT payload::VARCHAR FROM lake.main.variant_events; \
         CREATE TABLE lake.main.epoch_events PARTITIONED BY (epoch_day(ts)) AS \
             SELECT * FROM (VALUES \
                 (TIMESTAMP '1969-12-31 00:00:00', 1), \
                 (TIMESTAMP '1970-01-01 00:00:00', 2), \
                 (TIMESTAMP '1970-01-02 00:00:00', 3) \
             ) AS rows(ts, id); \
         SELECT concat( \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts < TIMESTAMP '1970-01-01 00:00:00')::VARCHAR, ',', \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts = TIMESTAMP '1970-01-01 00:00:00')::VARCHAR, ',', \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts >= TIMESTAMP '1970-01-02 00:00:00')::VARCHAR \
         ) AS epoch_counts; \
         USE memory; DETACH lake;"
    );
    let create_output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&create_sql)
            .output(),
    )
    .await
    .expect("DuckDB epoch/variant write timed out")
    .expect("duckdb process must start");
    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let create_stderr = String::from_utf8_lossy(&create_output.stderr);
    assert!(
        create_output.status.success(),
        "DuckDB epoch/variant write failed: {}\nstdout: {create_stdout}\nstderr: {create_stderr}",
        create_output.status
    );
    assert!(create_stdout.contains("Oslo"), "{create_stdout}");
    assert!(create_stdout.contains("1,1,1"), "{create_stdout}");

    let (variant_table, epoch_table, epoch_values) = {
        let catalog = catalog.lock().await;
        let reader = catalog.read_latest();
        let schema_id = reader
            .list_schemas()
            .await
            .unwrap()
            .into_iter()
            .find(|schema| schema.schema_name == "main")
            .unwrap()
            .schema_id;
        let tables = reader.list_tables(schema_id).await.unwrap();
        let variant_table = tables
            .iter()
            .find(|table| table.table_name == "variant_events")
            .unwrap()
            .table_id;
        let epoch_table = tables
            .iter()
            .find(|table| table.table_name == "epoch_events")
            .unwrap()
            .table_id;
        let inline_rows = reader.list_inlined_inserts(variant_table).await.unwrap();
        let files = reader.list_data_files(variant_table).await.unwrap();
        assert_eq!(inline_rows.len(), 1);
        assert!(files.is_empty());
        let partitions = reader.list_partition_info(epoch_table).await.unwrap();
        assert_eq!(partitions.len(), 1);
        let columns = reader
            .list_partition_columns(partitions[0].partition_id)
            .await
            .unwrap();
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].transform.as_deref(), Some("epoch_day"));
        let mut epoch_values = reader
            .list_file_partition_values(Some(epoch_table))
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.partition_value.unwrap())
            .collect::<Vec<_>>();
        epoch_values.sort();
        (variant_table, epoch_table, epoch_values)
    };
    assert_eq!(epoch_values, vec!["-1", "0", "1"]);

    let _ = shutdown_tx.send(());
    let _ = handle.await;
    drop(catalog);

    let (restart_port, restart_shutdown_tx, restart_handle, restarted_catalog) =
        start_server(&dir, None).await;
    let reopen_sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={restart_port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         SELECT payload::VARCHAR FROM lake.main.variant_events; \
         SELECT concat( \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts < TIMESTAMP '1970-01-01 00:00:00')::VARCHAR, ',', \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts = TIMESTAMP '1970-01-01 00:00:00')::VARCHAR, ',', \
             (SELECT count(*) FROM lake.main.epoch_events \
                 WHERE ts >= TIMESTAMP '1970-01-02 00:00:00')::VARCHAR \
         ) AS epoch_counts; \
         DETACH lake;"
    );
    let reopen_output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("duckdb")
            .arg("-no-agent")
            .arg("-c")
            .arg(&reopen_sql)
            .output(),
    )
    .await
    .expect("DuckDB epoch/variant reopen timed out")
    .expect("duckdb process must start");
    let _ = restart_shutdown_tx.send(());
    let _ = restart_handle.await;
    let reopen_stdout = String::from_utf8_lossy(&reopen_output.stdout);
    let reopen_stderr = String::from_utf8_lossy(&reopen_output.stderr);
    assert!(
        reopen_output.status.success(),
        "DuckDB epoch/variant reopen failed: {}\nstdout: {reopen_stdout}\nstderr: {reopen_stderr}",
        reopen_output.status
    );
    assert!(reopen_stdout.contains("Oslo"), "{reopen_stdout}");
    assert!(reopen_stdout.contains("1,1,1"), "{reopen_stdout}");
    let restarted_catalog = restarted_catalog.lock().await;
    let reader = restarted_catalog.read_latest();
    let inline_rows = reader.list_inlined_inserts(variant_table).await.unwrap();
    let files = reader.list_data_files(variant_table).await.unwrap();
    assert_eq!(inline_rows.len(), 1);
    assert!(files.is_empty());
    let mut restarted_epoch_values = reader
        .list_file_partition_values(Some(epoch_table))
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.partition_value.unwrap())
        .collect::<Vec<_>>();
    restarted_epoch_values.sort();
    assert_eq!(restarted_epoch_values, vec!["-1", "0", "1"]);
}
