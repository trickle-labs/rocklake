use futures::{StreamExt, TryStreamExt};
use object_store::memory::InMemory;
use object_store::path::Path;
use rocklake_catalog::{CatalogStore, FileColumnStatsInput, OpenOptions, MAX_METADATA_PAGE_SIZE};
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

async fn collect_pages<T, F>(mut next: F) -> Vec<T>
where
    F: for<'a> FnMut(
        Option<&'a str>,
    ) -> futures::future::BoxFuture<
        'a,
        rocklake_catalog::CatalogResult<rocklake_catalog::MetadataPage<T>>,
    >,
{
    let mut token = None;
    let mut rows = Vec::new();
    loop {
        let page = next(token.as_deref()).await.unwrap();
        rows.extend(page.rows);
        token = page.continuation_token;
        if token.is_none() {
            return rows;
        }
    }
}

#[tokio::test]
async fn v0513_metadata_pages_and_streams_have_matching_rows() {
    let mut store = catalog("v0513-bounded-metadata").await;
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("main").await.unwrap();
    let table_id = writer
        .create_table(schema_id, "events", None)
        .await
        .unwrap();
    let column_id = writer
        .add_column(table_id, "id", "int64", 0, false, None)
        .await
        .unwrap();
    let first_file = writer
        .register_data_file(table_id, "first.parquet", "parquet", 10, 100)
        .await
        .unwrap();
    let second_file = writer
        .register_data_file(table_id, "second.parquet", "parquet", 20, 200)
        .await
        .unwrap();
    for file_id in [first_file, second_file] {
        writer
            .upsert_file_column_stats(FileColumnStatsInput {
                table_id,
                column_id,
                data_file_id: file_id,
                contains_null: false,
                min_value: Some("1"),
                max_value: Some("20"),
                contains_nan: false,
                column_size_bytes: Some(100),
                value_count: Some(10),
                null_count: Some(0),
                extra_stats: None,
            })
            .await
            .unwrap();
    }
    writer
        .register_delete_file(first_file, "delete.parquet", 1, 10)
        .await
        .unwrap();
    let commit = writer
        .create_snapshot(Some("test"), Some("bounded metadata"))
        .await
        .unwrap();
    store.commit_writer(commit);

    let reader = store.read_latest();
    let data_ids = reader
        .list_data_files(table_id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.data_file_id)
        .collect::<Vec<_>>();
    let mut data_stream = reader.stream_data_files(table_id).await.unwrap();
    let streamed_data_ids = {
        let mut ids = Vec::new();
        while let Some(row) = data_stream.next().await {
            ids.push(row.unwrap().data_file_id);
        }
        ids
    };
    let paged_data_ids = {
        let mut token = None;
        let mut ids = Vec::new();
        loop {
            let page = reader
                .list_data_files_paged(table_id, 1, token.as_deref())
                .await
                .unwrap();
            ids.extend(page.files.into_iter().map(|row| row.data_file_id));
            token = page.continuation_token;
            if token.is_none() {
                break ids;
            }
        }
    };
    assert_eq!(data_ids, streamed_data_ids);
    assert_eq!(data_ids, paged_data_ids);

    let delete_stream = reader.stream_delete_files(table_id).await.unwrap();
    let streamed_delete = delete_stream.try_collect::<Vec<_>>().await.unwrap();
    let delete_reader = reader.clone();
    let paged_delete = collect_pages(move |token| {
        let reader = delete_reader.clone();
        Box::pin(async move { reader.list_delete_files_paged(table_id, 1, token).await })
    })
    .await;
    assert_eq!(streamed_delete, paged_delete);

    let stats_stream = reader
        .stream_file_column_stats(table_id, column_id)
        .await
        .unwrap();
    let streamed_stats = stats_stream.try_collect::<Vec<_>>().await.unwrap();
    let stats_reader = reader.clone();
    let paged_stats = collect_pages(move |token| {
        let reader = stats_reader.clone();
        Box::pin(async move {
            reader
                .list_file_column_stats_paged(table_id, column_id, 1, token)
                .await
        })
    })
    .await;
    assert_eq!(streamed_stats, paged_stats);

    let snapshot_stream = reader.stream_snapshot_changes().await.unwrap();
    let streamed_snapshots = snapshot_stream.try_collect::<Vec<_>>().await.unwrap();
    let snapshot_reader = reader.clone();
    let paged_snapshots = collect_pages(move |token| {
        let reader = snapshot_reader.clone();
        Box::pin(async move { reader.list_snapshot_changes_paged(1, token).await })
    })
    .await;
    assert_eq!(streamed_snapshots, paged_snapshots);
    assert!(reader
        .list_delete_files_paged(table_id, MAX_METADATA_PAGE_SIZE + 1, None)
        .await
        .is_err());
    let verification = rocklake_catalog::verify::verify_catalog(store.db())
        .await
        .unwrap();
    assert!(
        verification.is_ok(),
        "catalog verification failed: {verification:?}"
    );
}
