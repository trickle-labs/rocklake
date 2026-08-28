//! v0.47.17 value-level DuckLake 1.0 conformance checks.

use std::sync::Arc;

use futures::StreamExt;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use pgwire::api::results::Response;
use pgwire::messages::data::DataRow;
use tempfile::TempDir;
use tokio::sync::Mutex;

use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::executor;
use rocklake_pgwire::notify::NotifyManager;
use rocklake_pgwire::session::SessionState;
use rocklake_sql::ParamValues;

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

fn decode_text_row(row: DataRow) -> Vec<Option<String>> {
    let mut data = &row.data[..];
    (0..row.field_count)
        .map(|_| {
            assert!(data.len() >= 4, "row field length is truncated");
            let length = i32::from_be_bytes(data[..4].try_into().unwrap());
            data = &data[4..];
            if length < 0 {
                None
            } else {
                let length = length as usize;
                assert!(data.len() >= length, "row field is truncated");
                let value = String::from_utf8(data[..length].to_vec()).unwrap();
                data = &data[length..];
                Some(value)
            }
        })
        .collect()
}

#[tokio::test]
async fn snapshot_changes_row_values_match_ducklake_1_0_contract() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir).await;
    {
        let mut catalog = store.lock().await;
        let mut writer = catalog.begin_write();
        writer.create_schema("analytics").await.unwrap();
        let commit = writer
            .create_snapshot(Some("certifier"), Some("value-level"))
            .await
            .unwrap();
        catalog.commit_writer(commit);
    }

    let mut session = SessionState::new();
    let responses = executor::execute_sql(
        "SELECT * FROM ducklake_snapshot_changes",
        &ParamValues::default(),
        &store,
        &mut session,
        &Arc::new(NotifyManager::new()),
        &Arc::new(Vec::new()),
    )
    .await
    .unwrap();
    let response = responses.into_iter().next().unwrap();
    let Response::Query(query) = response else {
        panic!("expected a query response")
    };

    let fields = query.row_schema();
    let names: Vec<_> = fields.iter().map(|field| field.name()).collect();
    let types: Vec<_> = fields.iter().map(|field| field.datatype().name()).collect();
    assert_eq!(
        names,
        [
            "snapshot_id",
            "changes_made",
            "author",
            "commit_message",
            "commit_extra_info"
        ]
    );
    assert_eq!(types, ["int8", "text", "text", "text", "text"]);

    let mut rows = query.data_rows();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(
        decode_text_row(row),
        vec![
            Some("1".to_string()),
            None,
            Some("certifier".to_string()),
            Some("value-level".to_string()),
            None,
        ]
    );
    assert!(
        rows.next().await.is_none(),
        "one snapshot must produce one row"
    );
}
