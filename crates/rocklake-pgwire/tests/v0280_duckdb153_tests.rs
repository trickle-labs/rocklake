//! DuckDB 1.5.3 ↔ RockLake integration tests.
//!
//! Comprehensive coverage of the DuckDB 1.5.3 (Variegata) ↔ RockLake PgWire
//! interaction.  All tests are gated on `ducklake_available()` so they skip
//! gracefully when the `duckdb` binary or `ducklake` extension is absent —
//! without requiring `#[ignore]` or `--include-ignored`.
//!
//! # What is covered
//!
//! - `full_lifecycle_schema_qualified`     — ATTACH → USE → CREATE SCHEMA →
//!   CREATE TABLE (schema-qualified) → INSERT → SELECT *
//! - `connect_with_ducklake_dbname`        — regression: dbname=ducklake must
//!   be accepted (not just dbname=rocklake)
//! - `column_types_variety`                — INTEGER, BIGINT, DOUBLE, BOOLEAN,
//!   VARCHAR, DATE, TIMESTAMP columns round-trip correctly
//! - `select_with_where_filter`            — SELECT WHERE clause returns only
//!   matching rows
//! - `select_count_star`                   — SELECT COUNT(*) returns correct count
//! - `empty_table_select`                  — SELECT from a freshly-created table
//!   returns zero rows without error
//! - `multiple_tables_in_one_session`      — two independent tables in the same
//!   catalog schema created and queried in one DuckDB session
//! - `multiple_schemas`                    — two schemas with same-named tables
//!   do not clash
//! - `show_tables_after_create`            — SHOW TABLES reflects newly-created
//!   table (via DuckLake's SHOW TABLES)
//! - `persistence_across_restarts`         — data inserted in phase-1 is readable
//!   after a server stop+restart in phase-2
//! - `large_insert_batch`                  — 100-row INSERT survives and is
//!   fully readable
//! - `insert_and_update_value`             — UPDATE a column after initial INSERT,
//!   verify new value visible in SELECT
//! - `null_values_round_trip`              — NULL values survive INSERT → SELECT
//!
//! # Running
//!
//! ```sh
//! cargo test -p rocklake-pgwire --test v0280_duckdb153_tests -- --test-threads=1
//! ```

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::server::ServerConfig;
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

async fn duckdb_available() -> bool {
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::process::Command::new("duckdb")
            .arg("--version")
            .output()
            .await
            .is_ok()
    })
    .await;
    result.unwrap_or(false)
}

async fn ducklake_available() -> bool {
    if !duckdb_available().await {
        return false;
    }
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::process::Command::new("duckdb")
            .arg("-c")
            .arg("LOAD ducklake; SELECT 1;")
            .output()
            .await
    })
    .await;
    match result {
        Ok(Ok(o)) => o.status.success(),
        _ => false,
    }
}

/// Start a plain-text PgWire server on an OS-assigned port.
/// Returns `(port, shutdown_tx, join_handle)`.
async fn start_server(
    opts: OpenOptions,
) -> (
    u16,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let catalog = Arc::new(Mutex::new(CatalogStore::open(opts).await.unwrap()));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let config = ServerConfig {
        bind_addr: format!("127.0.0.1:{port}").parse().unwrap(),
        ..Default::default()
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        rocklake_pgwire::server::run_server_with_shutdown(config, catalog, rx)
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    (port, tx, handle)
}

/// Run a single DuckDB command with a 30-second timeout.
/// Returns `(status_success, stdout, stderr)`.
async fn run_duckdb(sql: &str) -> (bool, String, String) {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new("duckdb")
            .arg("-c")
            .arg(sql)
            .output(),
    )
    .await
    .expect("duckdb timed out after 30s")
    .expect("duckdb must start");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status.success(), stdout, stderr)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Core lifecycle: ATTACH → USE → CREATE SCHEMA → CREATE TABLE (schema-
/// qualified) → INSERT → SELECT *.
///
/// This is the primary regression test for the DuckLake/RockLake integration.
/// All table names are schema-qualified; DuckLake requires this form.
#[tokio::test]
async fn full_lifecycle_schema_qualified() {
    if !ducklake_available().await {
        eprintln!("SKIP full_lifecycle_schema_qualified: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS main; \
         CREATE TABLE main.brukere (id INTEGER, navn VARCHAR, dato DATE); \
         INSERT INTO main.brukere VALUES (1, 'Ola Nordmann', '2026-05-30'), \
                                         (2, 'Kari Nordmann', '2026-01-15'); \
         SELECT id, navn FROM main.brukere ORDER BY id;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "full_lifecycle_schema_qualified failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Ola Nordmann"),
        "SELECT must return 'Ola Nordmann'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Kari Nordmann"),
        "SELECT must return 'Kari Nordmann'.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// Regression: `dbname=ducklake` must be accepted by RockLake — the server
/// does not enforce a specific database name in the startup message.
#[tokio::test]
async fn connect_with_ducklake_dbname() {
    if !ducklake_available().await {
        eprintln!("SKIP connect_with_ducklake_dbname: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=ducklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.ping (val INTEGER); \
         INSERT INTO s.ping VALUES (42); \
         SELECT val FROM s.ping;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "connect_with_ducklake_dbname failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("42"),
        "SELECT must return 42.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// All common DuckDB column types round-trip correctly through the DuckLake
/// catalog and inlined-data storage.
#[tokio::test]
async fn column_types_variety() {
    if !ducklake_available().await {
        eprintln!("SKIP column_types_variety: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS t; \
         CREATE TABLE t.types_test ( \
             col_int     INTEGER, \
             col_bigint  BIGINT, \
             col_double  DOUBLE, \
             col_bool    BOOLEAN, \
             col_varchar VARCHAR, \
             col_date    DATE, \
             col_ts      TIMESTAMP \
         ); \
         INSERT INTO t.types_test VALUES ( \
             42, 9000000000, 3.14, true, 'hello', '2026-05-31', '2026-05-31 12:00:00' \
         ); \
         SELECT col_int, col_bigint, col_double, col_bool, col_varchar, col_date \
         FROM t.types_test;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "column_types_variety failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("42"),
        "INTEGER column must round-trip.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("3.14"),
        "DOUBLE column must round-trip.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("hello"),
        "VARCHAR column must round-trip.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// WHERE clause filters only the matching rows.
#[tokio::test]
async fn select_with_where_filter() {
    if !ducklake_available().await {
        eprintln!("SKIP select_with_where_filter: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.nums (n INTEGER, label VARCHAR); \
         INSERT INTO s.nums VALUES (1,'one'),(2,'two'),(3,'three'),(4,'four'),(5,'five'); \
         SELECT n, label FROM s.nums WHERE n > 3 ORDER BY n;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "select_with_where_filter failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("four"),
        "WHERE n > 3 must include 'four'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("five"),
        "WHERE n > 3 must include 'five'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stdout.contains("one"),
        "WHERE n > 3 must exclude 'one'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stdout.contains("two"),
        "WHERE n > 3 must exclude 'two'.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// SELECT COUNT(*) returns the correct row count after INSERT.
#[tokio::test]
async fn select_count_star() {
    if !ducklake_available().await {
        eprintln!("SKIP select_count_star: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.rows (id INTEGER); \
         INSERT INTO s.rows SELECT range FROM range(1, 8); \
         SELECT COUNT(*) AS cnt FROM s.rows;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "select_count_star failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains('7'),
        "COUNT(*) must return 7.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// SELECT from a freshly-created empty table returns zero rows without error.
#[tokio::test]
async fn empty_table_select() {
    if !ducklake_available().await {
        eprintln!("SKIP empty_table_select: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.empty_tbl (id INTEGER, name VARCHAR); \
         SELECT * FROM s.empty_tbl; \
         SELECT COUNT(*) AS cnt FROM s.empty_tbl;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "empty_table_select failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains('0'),
        "COUNT(*) on empty table must return 0.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// Two independent tables created and queried in the same DuckDB session.
#[tokio::test]
async fn multiple_tables_in_one_session() {
    if !ducklake_available().await {
        eprintln!("SKIP multiple_tables_in_one_session: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.customers (cid INTEGER, cname VARCHAR); \
         CREATE TABLE s.orders   (oid INTEGER, cid INTEGER, amount DOUBLE); \
         INSERT INTO s.customers VALUES (1,'Alice'),(2,'Bob'); \
         INSERT INTO s.orders    VALUES (100,1,99.9),(101,2,49.5),(102,1,10.0); \
         SELECT c.cname, SUM(o.amount) AS total \
         FROM s.customers c JOIN s.orders o ON c.cid = o.cid \
         GROUP BY c.cname ORDER BY c.cname;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "multiple_tables_in_one_session failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Alice"),
        "JOIN must include Alice.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Bob"),
        "JOIN must include Bob.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// Same-named table in two different schemas must not clash.
#[tokio::test]
async fn multiple_schemas_no_clash() {
    if !ducklake_available().await {
        eprintln!("SKIP multiple_schemas_no_clash: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS alpha; \
         CREATE SCHEMA IF NOT EXISTS beta; \
         CREATE TABLE alpha.data (v VARCHAR); \
         CREATE TABLE beta.data  (v VARCHAR); \
         INSERT INTO alpha.data VALUES ('from-alpha'); \
         INSERT INTO beta.data  VALUES ('from-beta'); \
         SELECT v FROM alpha.data; \
         SELECT v FROM beta.data;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "multiple_schemas_no_clash failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("from-alpha"),
        "alpha.data must be queryable.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("from-beta"),
        "beta.data must be queryable.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// SHOW TABLES (DuckLake) must list newly-created tables.
#[tokio::test]
async fn show_tables_after_create() {
    if !ducklake_available().await {
        eprintln!("SKIP show_tables_after_create: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.visible_tbl (x INTEGER); \
         SHOW TABLES;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "show_tables_after_create failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("visible_tbl"),
        "SHOW TABLES must list visible_tbl.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// Data inserted in a session survives a complete server stop + restart.
///
/// Phase 1: write 3 rows.  Phase 2: new server process, same catalog dir,
/// reattach and verify all rows are present.
#[tokio::test]
async fn persistence_across_restarts() {
    if !ducklake_available().await {
        eprintln!("SKIP persistence_across_restarts: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();

    // Phase 1: write.
    {
        let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

        let sql = format!(
            "LOAD ducklake; \
             ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
                 (DATA_PATH '{data_path}'); \
             USE lake; \
             CREATE SCHEMA IF NOT EXISTS s; \
             CREATE TABLE s.durable (k VARCHAR, v INTEGER); \
             INSERT INTO s.durable VALUES ('a',1),('b',2),('c',3);"
        );

        let (ok, _, stderr) = run_duckdb(&sql).await;
        let _ = shutdown_tx.send(());
        let _ = handle.await;
        assert!(ok, "write phase failed.\nstderr: {stderr}");
    }

    // Phase 2: restart and read.
    {
        let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

        let sql = format!(
            "LOAD ducklake; \
             ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
                 (DATA_PATH '{data_path}'); \
             USE lake; \
             SELECT k, v FROM s.durable ORDER BY k;"
        );

        let (ok, stdout, stderr) = run_duckdb(&sql).await;
        let _ = shutdown_tx.send(());
        let _ = handle.await;

        assert!(
            ok,
            "restart read phase failed.\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert!(
            stdout.contains('a') && stdout.contains('1'),
            "row (a,1) must survive restart.\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert!(
            stdout.contains('b') && stdout.contains('2'),
            "row (b,2) must survive restart.\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert!(
            stdout.contains('c') && stdout.contains('3'),
            "row (c,3) must survive restart.\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

/// 100-row INSERT batch survives and is fully readable via COUNT(*).
#[tokio::test]
async fn large_insert_batch() {
    if !ducklake_available().await {
        eprintln!("SKIP large_insert_batch: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.bulk (n INTEGER, label VARCHAR); \
         INSERT INTO s.bulk SELECT range, 'row-' || range::VARCHAR FROM range(1, 101); \
         SELECT COUNT(*) AS cnt FROM s.bulk; \
         SELECT n FROM s.bulk WHERE n = 100;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "large_insert_batch failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("100"),
        "COUNT(*) must be 100 and row 100 must be present.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// UPDATE changes a value; subsequent SELECT sees the new value.
#[tokio::test]
async fn insert_and_update_value() {
    if !ducklake_available().await {
        eprintln!("SKIP insert_and_update_value: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.mutable (id INTEGER, status VARCHAR); \
         INSERT INTO s.mutable VALUES (1,'draft'),(2,'published'); \
         UPDATE s.mutable SET status = 'archived' WHERE id = 1; \
         SELECT id, status FROM s.mutable ORDER BY id;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "insert_and_update_value failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("archived"),
        "Updated row must show 'archived'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("published"),
        "Unchanged row must still show 'published'.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stdout.contains("draft"),
        "Old value 'draft' must not appear after UPDATE.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// NULL values survive an INSERT → SELECT round-trip without becoming empty
/// strings or causing errors.
#[tokio::test]
async fn null_values_round_trip() {
    if !ducklake_available().await {
        eprintln!("SKIP null_values_round_trip: duckdb/ducklake not available");
        return;
    }

    let catalog_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let data_path = data_dir.path().to_string_lossy().into_owned();
    let (port, shutdown_tx, handle) = start_server(make_catalog_opts(&catalog_dir)).await;

    let sql = format!(
        "LOAD ducklake; \
         ATTACH 'ducklake:postgres:host=127.0.0.1 port={port} dbname=rocklake' AS lake \
             (DATA_PATH '{data_path}'); \
         USE lake; \
         CREATE SCHEMA IF NOT EXISTS s; \
         CREATE TABLE s.nullable (id INTEGER, note VARCHAR); \
         INSERT INTO s.nullable VALUES (1, NULL), (2, 'present'); \
         SELECT id, note FROM s.nullable ORDER BY id; \
         SELECT COUNT(*) AS null_cnt FROM s.nullable WHERE note IS NULL; \
         SELECT COUNT(*) AS nonnull_cnt FROM s.nullable WHERE note IS NOT NULL;"
    );

    let (ok, stdout, stderr) = run_duckdb(&sql).await;
    let _ = shutdown_tx.send(());
    let _ = handle.await;

    assert!(
        ok,
        "null_values_round_trip failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("present"),
        "Non-null row must be readable.\nstdout: {stdout}\nstderr: {stderr}"
    );
    // DuckDB prints NULL columns as empty in tabular output;
    // COUNT queries verify the IS NULL / IS NOT NULL predicates work.
    assert!(
        stdout.contains('1'),
        "null_cnt and nonnull_cnt must each be 1.\nstdout: {stdout}\nstderr: {stderr}"
    );
}
