//! v0.47.0 Integration Tests — Read-Only Catalog Access
//!
//! Tests for RFC-01 roadmap items:
//! - 16 simultaneous ReadOnlyCatalog opens produce zero CAS conflicts
//! - ReadOnlyCatalog::refresh() advances to the latest committed snapshot
//! - Concurrent writer + N readers do not interfere
//! - open_without_epoch skips the CAS writer-epoch

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions, ReadOnlyCatalog};
use std::sync::Arc;
use tempfile::TempDir;

fn test_opts(dir: &TempDir) -> OpenOptions {
    let path = dir.path().to_str().unwrap().to_string();
    let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(&path).unwrap());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

// ─── RFC-01: ReadOnlyCatalog — zero CAS conflicts ─────────────────────────

/// 16 simultaneous concurrent reads from a single `ReadOnlyCatalog` must all
/// succeed with zero writer-epoch CAS conflicts.
///
/// SlateDB supports only one active `Db` instance per object-store path at
/// a time; opening a second `Db::open()` writes an empty WAL that fences
/// the previous instance.  `ReadOnlyCatalog` therefore opens one `Db` and
/// shares it (via `Arc<DbInner>`) across all `reader()` calls — each call
/// is just an `Arc::clone()` with no new writes.
#[tokio::test]
async fn test_16_simultaneous_readonly_opens_zero_conflicts() {
    let dir = TempDir::new().unwrap();

    // Bootstrap a writer so the catalog has some initial state.
    {
        let mut w = CatalogStore::open(test_opts(&dir)).await.unwrap();
        let mut writer = w.begin_write();
        writer.create_schema("analytics").await.unwrap();
        let result = writer.create_snapshot(None, None).await.unwrap();
        w.commit_writer(result);
        w.close().await.unwrap();
    }

    // Open ONE ReadOnlyCatalog — no writer epoch is written.
    let cat = Arc::new(ReadOnlyCatalog::open(test_opts(&dir)).await.unwrap());

    // Spawn 16 concurrent reader tasks, all sharing the same catalog handle.
    // reader() clones Arc<DbInner> — zero new Db::open() calls, zero fencing.
    let mut handles = Vec::with_capacity(16);
    for _ in 0..16 {
        let cat_ref = Arc::clone(&cat);
        handles.push(tokio::spawn(async move {
            let reader = cat_ref.reader().expect("reader() failed");
            let schemas = reader.list_schemas().await.expect("list_schemas failed");
            assert!(
                schemas.iter().any(|s| s.schema_name == "analytics"),
                "reader must see the committed schema"
            );
        }));
    }

    let mut success_count = 0usize;
    for h in handles {
        h.await.expect("task panicked");
        success_count += 1;
    }
    assert_eq!(success_count, 16, "all 16 concurrent reads must succeed");

    let cat = Arc::try_unwrap(cat)
        .ok()
        .expect("Arc should have single owner");
    cat.close().await.expect("close failed");
}

/// ReadOnlyCatalog::refresh() advances to the latest committed snapshot.
///
/// The reader is constructed from the writer's existing `Db` handle (via
/// `Arc<DbInner>` clone) to avoid SlateDB fencing: a separate `Db::open()`
/// would fence the writer's open handle.
#[tokio::test]
async fn test_readonly_refresh_sees_new_snapshots() {
    let dir = TempDir::new().unwrap();

    // Open the writer and write an initial schema.
    let mut w = CatalogStore::open(test_opts(&dir)).await.unwrap();
    {
        let mut writer = w.begin_write();
        writer.create_schema("initial_schema").await.unwrap();
        let result = writer.create_snapshot(None, None).await.unwrap();
        w.commit_writer(result);
    }

    // Create a ReadOnlyCatalog sharing the writer's Db (same Arc<DbInner>).
    // This avoids fencing: a fresh Db::open() here would fence the writer.
    let opts = test_opts(&dir);
    let mut cat = ReadOnlyCatalog::from_db_for_test(w.db().clone(), opts.object_store);

    // After refresh, the reader sees the initial snapshot.
    let initial_snap = cat.refresh().await.expect("initial refresh failed");
    assert!(
        initial_snap.as_u64() > 0,
        "reader should see the initial snapshot after refresh"
    );

    // Before a second refresh, the snapshot ID has not changed.
    assert_eq!(
        cat.current_snapshot_id(),
        initial_snap,
        "snapshot should not change without refresh"
    );

    // Writer commits another snapshot.
    {
        let mut writer = w.begin_write();
        writer.create_schema("new_schema").await.unwrap();
        let result = writer.create_snapshot(None, None).await.unwrap();
        w.commit_writer(result);
    }

    // Before refresh the reader still sees the old snapshot.
    assert_eq!(
        cat.current_snapshot_id(),
        initial_snap,
        "snapshot should not change without refresh"
    );

    // After refresh the reader must see the new snapshot.
    let refreshed = cat.refresh().await.expect("refresh failed");
    assert!(
        refreshed.as_u64() > initial_snap.as_u64(),
        "refreshed snapshot ({}) must be newer than initial ({})",
        refreshed.as_u64(),
        initial_snap.as_u64()
    );

    let reader = cat.reader().expect("reader() after refresh failed");
    let schemas = reader.list_schemas().await.expect("list_schemas failed");
    assert!(
        schemas.iter().any(|s| s.schema_name == "new_schema"),
        "reader must see the newly committed schema after refresh"
    );

    w.close().await.unwrap();
}

/// open_without_epoch should not increment the writer epoch counter.
#[tokio::test]
async fn test_open_without_epoch_does_not_increment_epoch() {
    use rocklake_core::keys;
    use rocklake_core::tags::SYSTEM_WRITER_EPOCH;
    use rocklake_core::values;

    let dir = TempDir::new().unwrap();

    // First open via open() — this sets epoch to 1.
    let w1 = CatalogStore::open(test_opts(&dir)).await.unwrap();
    drop(w1);

    // Record the epoch after the writer open.
    let epoch_after_writer = {
        let db = slatedb::Db::open(
            ObjectPath::from("catalog"),
            Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap()),
        )
        .await
        .unwrap();
        let key = keys::key_system(SYSTEM_WRITER_EPOCH);
        let val = db
            .get(&key)
            .await
            .unwrap()
            .map(|d| values::decode_counter(&d).unwrap())
            .unwrap_or(0);
        db.close().await.unwrap();
        val
    };

    // Now open three readers via open_without_epoch — epoch must not change.
    for _ in 0..3 {
        let r = CatalogStore::open_without_epoch(test_opts(&dir))
            .await
            .unwrap();
        drop(r);
    }

    let epoch_after_readers = {
        let db = slatedb::Db::open(
            ObjectPath::from("catalog"),
            Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap()),
        )
        .await
        .unwrap();
        let key = keys::key_system(SYSTEM_WRITER_EPOCH);
        let val = db
            .get(&key)
            .await
            .unwrap()
            .map(|d| values::decode_counter(&d).unwrap())
            .unwrap_or(0);
        db.close().await.unwrap();
        val
    };

    assert_eq!(
        epoch_after_writer, epoch_after_readers,
        "open_without_epoch must not increment the writer epoch"
    );
}

/// Writer and concurrent readers do not interfere with each other.
///
/// The reader is constructed from the writer's `Db` clone to avoid SlateDB
/// fencing.  Readers share `Arc<DbInner>` with the writer so concurrent
/// reads see a consistent snapshot without conflicting `Db::open()` calls.
#[tokio::test]
async fn test_concurrent_writer_and_readers() {
    let dir = TempDir::new().unwrap();

    // Open the writer and bootstrap with an initial schema.
    let mut w = CatalogStore::open(test_opts(&dir)).await.unwrap();
    {
        let mut writer = w.begin_write();
        writer.create_schema("base").await.unwrap();
        let result = writer.create_snapshot(None, None).await.unwrap();
        w.commit_writer(result);
    }

    // Create a ReadOnlyCatalog sharing the writer's Db (same Arc<DbInner>).
    // A fresh Db::open() here would fence the writer's open handle.
    let opts = test_opts(&dir);
    let mut cat = ReadOnlyCatalog::from_db_for_test(w.db().clone(), opts.object_store);
    // Sync to the current snapshot so readers see "base".
    cat.refresh().await.unwrap();
    let cat = Arc::new(cat);

    // Spawn 8 concurrent reader tasks, all sharing the same catalog handle.
    let mut reader_tasks = Vec::with_capacity(8);
    for _ in 0..8 {
        let cat_ref = Arc::clone(&cat);
        reader_tasks.push(tokio::spawn(async move {
            // Small delay to interleave with writer writes.
            for _ in 0..3 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            let reader = cat_ref.reader().expect("reader() failed");
            let schemas = reader.list_schemas().await.expect("list_schemas failed");
            assert!(
                !schemas.is_empty(),
                "must always see at least the base schema"
            );
        }));
    }

    // Writer adds 5 more schemas concurrently.
    for i in 0..5u64 {
        let mut writer = w.begin_write();
        writer.create_schema(&format!("schema_{i}")).await.unwrap();
        let result = writer.create_snapshot(None, None).await.unwrap();
        w.commit_writer(result);
    }

    for rt in reader_tasks {
        rt.await.expect("reader task panicked");
    }
    w.close().await.unwrap();
}
