use std::sync::Arc;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::fault_injection::{FaultInjector, WriteFaultPoint};
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::keys::MetadataScope;
use rocklake_pgwire::executor;
use rocklake_pgwire::notify::NotifyManager;
use rocklake_pgwire::session::SessionState;
use rocklake_sql::ParamValues;
use tempfile::TempDir;
use tokio::sync::Mutex;

async fn open_store(dir: &TempDir) -> Arc<Mutex<CatalogStore>> {
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

async fn execute(
    sql: &str,
    store: &Arc<Mutex<CatalogStore>>,
    session: &mut SessionState,
) -> Result<(), rocklake_pgwire::error::RockLakeError> {
    executor::execute_sql(
        sql,
        &ParamValues::default(),
        store,
        session,
        &Arc::new(NotifyManager::new()),
        &Arc::new(Vec::new()),
    )
    .await
    .map(|_| ())
}

async fn version(store: &Arc<Mutex<CatalogStore>>) -> String {
    store
        .lock()
        .await
        .read_latest()
        .get_metadata(MetadataScope::Global, 0, "version")
        .await
        .unwrap()
        .unwrap()
        .value
}

#[tokio::test]
async fn v11_migration_rolls_back_on_failure_and_can_be_retried() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;
    {
        let mut catalog = store.lock().await;
        let mut writer = catalog.begin_write();
        writer
            .set_metadata(MetadataScope::Global, 0, "version", "1.0")
            .unwrap();
        let committed = writer.create_snapshot(None, None).await.unwrap();
        catalog.commit_writer(committed);
    }

    let mut session = SessionState::new();
    execute("BEGIN", &store, &mut session).await.unwrap();
    execute(
        "ALTER TABLE public.ducklake_schema ADD COLUMN parent_schema_id BIGINT",
        &store,
        &mut session,
    )
    .await
    .unwrap();
    execute(
        "UPDATE public.ducklake_metadata SET value = '1.1' WHERE key = 'version'",
        &store,
        &mut session,
    )
    .await
    .unwrap();
    assert_eq!(version(&store).await, "1.0");
    let injector = FaultInjector::new();
    injector.set_error(
        WriteFaultPoint::BeforeSnapshotCommit,
        "injected migration commit failure",
    );
    let commit_result = execute("COMMIT", &store, &mut session).await;
    injector.clear_all();
    assert!(commit_result.is_err());
    assert_eq!(version(&store).await, "1.0");
    assert_eq!(store.lock().await.read_latest().snapshot_id().as_u64(), 1);
    assert!(!session.in_transaction);

    execute("BEGIN", &store, &mut session).await.unwrap();
    execute(
        "ALTER TABLE public.ducklake_schema ADD COLUMN parent_schema_id BIGINT",
        &store,
        &mut session,
    )
    .await
    .unwrap();
    execute(
        "UPDATE public.ducklake_metadata SET value = '1.1' WHERE key = 'version'",
        &store,
        &mut session,
    )
    .await
    .unwrap();
    execute("COMMIT", &store, &mut session).await.unwrap();
    assert_eq!(version(&store).await, "1.1");
    assert_eq!(store.lock().await.read_latest().snapshot_id().as_u64(), 2);

    assert!(execute(
        "UPDATE public.ducklake_metadata SET value = '1.0' WHERE key = 'version'",
        &store,
        &mut session,
    )
    .await
    .is_err());
    assert_eq!(version(&store).await, "1.1");
    assert_eq!(store.lock().await.read_latest().snapshot_id().as_u64(), 2);
}
