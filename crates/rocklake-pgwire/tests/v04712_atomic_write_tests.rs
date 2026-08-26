//! v0.47.12 atomic PG-wire write regressions.

use std::sync::Arc;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use tokio::sync::Mutex;

use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::executor;
use rocklake_pgwire::notify::NotifyManager;
use rocklake_pgwire::session::SessionState;
use rocklake_sql::ParamValues;

async fn store(dir: &tempfile::TempDir) -> Arc<Mutex<CatalogStore>> {
    let object_store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let catalog = CatalogStore::open(OpenOptions {
        object_store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    })
    .await
    .unwrap();
    Arc::new(Mutex::new(catalog))
}

fn notify() -> Arc<NotifyManager> {
    Arc::new(NotifyManager::new())
}

fn extensions() -> Arc<Vec<String>> {
    Arc::new(vec!["pgtrickle".to_string()])
}

#[tokio::test]
async fn statement_error_aborts_the_pending_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = store(&dir).await;
    let mut session = SessionState::new();

    executor::execute_sql(
        "BEGIN",
        &ParamValues::default(),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &ParamValues::new(vec![Some("discarded".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();

    let result = executor::execute_sql(
        "INSERT INTO ducklake_data_file (table_id, path, file_format, row_count, file_size_bytes) VALUES ($1, $2, $3, $4, $5)",
        &ParamValues::new(vec![Some("1".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await;
    assert!(result.is_err());
    assert!(!session.in_transaction);
    assert!(session.pending_txn.is_empty());

    let catalog = catalog.lock().await;
    assert!(catalog
        .read_latest()
        .list_schemas()
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn metadata_rollback_does_not_publish_rows() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = store(&dir).await;
    let mut session = SessionState::new();

    executor::execute_sql(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &ParamValues::new(vec![Some("public".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "INSERT INTO ducklake_table (schema_id, table_name) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("1".to_string()), Some("events".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "BEGIN",
        &ParamValues::default(),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "INSERT INTO ducklake_partition_info (table_id, partition_id) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("2".to_string()), Some("7".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "ROLLBACK",
        &ParamValues::default(),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();

    let catalog = catalog.lock().await;
    assert!(catalog
        .read_latest()
        .list_partition_info(2)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn extended_data_file_fields_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = store(&dir).await;
    let mut session = SessionState::new();

    executor::execute_sql(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &ParamValues::new(vec![Some("public".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "INSERT INTO ducklake_table (schema_id, table_name) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("1".to_string()), Some("events".to_string())]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();
    executor::execute_sql(
        "INSERT INTO ducklake_data_file (table_id, path, file_format, record_count, file_size_bytes, footer_size, encryption_key, partition_id, mapping_id, partial_max) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &ParamValues::new(vec![
            Some("2".to_string()),
            Some("data/events/part.parquet".to_string()),
            Some("parquet".to_string()),
            Some("10".to_string()),
            Some("1024".to_string()),
            Some("64".to_string()),
            Some("secret".to_string()),
            Some("7".to_string()),
            Some("8".to_string()),
            Some("99".to_string()),
        ]),
        &catalog,
        &mut session,
        &notify(),
        &extensions(),
    )
    .await
    .unwrap();

    let catalog = catalog.lock().await;
    let files = catalog.read_latest().list_data_files(2).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].footer_size, Some(64));
    assert_eq!(files[0].encryption_key.as_deref(), Some("secret"));
    assert_eq!(files[0].partition_id, Some(7));
    assert_eq!(files[0].mapping_id, Some(8));
    assert_eq!(files[0].partial_max.as_deref(), Some("99"));
}
