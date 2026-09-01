use futures::StreamExt;
use object_store::memory::InMemory;
use object_store::path::Path;
use rocklake_catalog::{CatalogStore, OpenOptions, MAX_DATA_FILE_PAGE_SIZE};
use rocklake_core::mvcc::SnapshotId;
use std::sync::Arc;

async fn catalog(name: &str) -> CatalogStore {
    CatalogStore::open(OpenOptions {
        object_store: Arc::new(InMemory::new()),
        path: Path::from(name),
        encryption: None,
    })
    .await
    .unwrap()
}

async fn table_with_files(name: &str, count: usize) -> (CatalogStore, u64, SnapshotId) {
    let mut store = catalog(name).await;
    let mut writer = store.begin_write();
    let schema = writer.create_schema("main").await.unwrap();
    let table = writer.create_table(schema, "files", None).await.unwrap();
    for i in 0..count {
        let path = format!("part-{i}.parquet");
        writer
            .register_data_file_with_metadata(
                table, &path, "parquet", 1, 1, None, None, None, None, None,
            )
            .await
            .unwrap();
    }
    let snapshot = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snapshot);
    (store, table, snapshot.snapshot_id)
}

#[tokio::test]
async fn pages_traverse_once_and_validate_context() {
    let (store, table, snapshot) = table_with_files("v051-pages", 5).await;
    let reader = store.read_at(snapshot).unwrap();
    let mut token = None;
    let mut first_token = None;
    let mut ids = Vec::new();
    for _ in 0..3 {
        let page = reader
            .list_data_files_paged(table, 2, token.as_deref())
            .await
            .unwrap();
        ids.extend(page.files.iter().map(|file| file.data_file_id));
        token = page.continuation_token;
        if first_token.is_none() {
            first_token = token.clone();
        }
        if token.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 5);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));

    let err = reader
        .list_data_files_paged(table, 3, first_token.as_deref())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("does not match"));
    assert!(reader
        .list_data_files_paged(table, 2, Some("not-a-token"))
        .await
        .is_err());
    assert!(reader.list_data_files_paged(table, 0, None).await.is_err());
    assert!(reader
        .list_data_files_paged(table, MAX_DATA_FILE_PAGE_SIZE + 1, None)
        .await
        .is_err());
    let exact = reader.list_data_files_paged(table, 5, None).await.unwrap();
    assert_eq!(exact.files.len(), 5);
    assert!(exact.continuation_token.is_none());
}

#[tokio::test]
async fn historical_snapshot_and_stream_are_bounded() {
    let (mut store, table, first_snapshot) = table_with_files("v051-history", 1).await;
    let mut writer = store.begin_write();
    writer
        .register_data_file_with_metadata(
            table,
            "part-later.parquet",
            "parquet",
            1,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let second_snapshot = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(second_snapshot);

    let historical = store.read_at(first_snapshot).unwrap();
    assert_eq!(historical.list_data_files(table).await.unwrap().len(), 1);

    let mut stream = store
        .read_at(second_snapshot.snapshot_id)
        .unwrap()
        .stream_data_files(table)
        .await
        .unwrap();
    assert!(stream.next().await.unwrap().is_ok());
    drop(stream);
}
