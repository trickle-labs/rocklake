//! v0.47.13 PG-Wire Snapshot Correctness and Transaction Isolation Tests.

use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use pgwire::api::results::Response;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::executor;
use rocklake_pgwire::notify::NotifyManager;
use rocklake_pgwire::session::SessionState;
use rocklake_sql::ParamValues;

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

fn nm() -> Arc<NotifyManager> {
    Arc::new(NotifyManager::new())
}

fn ext() -> Arc<Vec<String>> {
    Arc::new(vec![])
}

async fn exec_sql(
    sql: &'static str,
    params: &ParamValues,
    store: &Arc<Mutex<CatalogStore>>,
    session: &mut SessionState,
) -> Result<Vec<Response<'static>>, rocklake_pgwire::error::RockLakeError> {
    executor::execute_sql(sql, params, store, session, &nm(), &ext()).await
}

#[tokio::test]
async fn test_pgwire_transaction_snapshot_isolation() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;

    let mut session1 = SessionState::new();
    let mut session2 = SessionState::new();

    // Snapshot 1: Create a schema and table via session 1
    exec_sql("BEGIN", &ParamValues::default(), &store, &mut session1)
        .await
        .unwrap();
    exec_sql(
        "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
        &ParamValues::new(vec![Some("public".into())]),
        &store,
        &mut session1,
    )
    .await
    .unwrap();
    exec_sql(
        "INSERT INTO ducklake_table (schema_id, table_name) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("1".into()), Some("t1".into())]),
        &store,
        &mut session1,
    )
    .await
    .unwrap();
    exec_sql(
        "INSERT INTO ducklake_snapshot (author, message) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("author".into()), Some("snap1".into())]),
        &store,
        &mut session1,
    )
    .await
    .unwrap();
    exec_sql("COMMIT", &ParamValues::default(), &store, &mut session1)
        .await
        .unwrap();

    // Session 2 begins transaction at snapshot 1
    exec_sql("BEGIN", &ParamValues::default(), &store, &mut session2)
        .await
        .unwrap();

    // Session 1 commits snapshot 2 adding table t2
    exec_sql("BEGIN", &ParamValues::default(), &store, &mut session1)
        .await
        .unwrap();
    exec_sql(
        "INSERT INTO ducklake_table (schema_id, table_name) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("1".into()), Some("t2".into())]),
        &store,
        &mut session1,
    )
    .await
    .unwrap();
    exec_sql(
        "INSERT INTO ducklake_snapshot (author, message) VALUES ($1, $2)",
        &ParamValues::new(vec![Some("author".into()), Some("snap2".into())]),
        &store,
        &mut session1,
    )
    .await
    .unwrap();
    exec_sql("COMMIT", &ParamValues::default(), &store, &mut session1)
        .await
        .unwrap();

    // Session 2 inside its transaction (pinned to snapshot 1) reads tables
    // It should see only t1 (1 table), not t2!
    let resp2 = exec_sql(
        "SELECT * FROM ducklake_table",
        &ParamValues::default(),
        &store,
        &mut session2,
    )
    .await
    .unwrap();

    let count2 = match resp2.into_iter().next().unwrap() {
        Response::Query(qr) => {
            use futures::StreamExt;
            let stream = qr.data_rows();
            futures::pin_mut!(stream);
            let mut c = 0;
            while stream.next().await.is_some() {
                c += 1;
            }
            c
        }
        _ => panic!("expected query response"),
    };
    assert_eq!(
        count2, 1,
        "Session 2 in snapshot 1 must only see t1, not t2"
    );

    exec_sql("COMMIT", &ParamValues::default(), &store, &mut session2)
        .await
        .unwrap();

    // Now outside transaction, session 2 reads latest and sees both tables (2 tables)
    let resp_latest = exec_sql(
        "SELECT * FROM ducklake_table",
        &ParamValues::default(),
        &store,
        &mut session2,
    )
    .await
    .unwrap();

    let count_latest = match resp_latest.into_iter().next().unwrap() {
        Response::Query(qr) => {
            use futures::StreamExt;
            let stream = qr.data_rows();
            futures::pin_mut!(stream);
            let mut c = 0;
            while stream.next().await.is_some() {
                c += 1;
            }
            c
        }
        _ => panic!("expected query response"),
    };
    assert_eq!(
        count_latest, 2,
        "Session 2 after COMMIT must see both tables at latest snapshot"
    );
}
