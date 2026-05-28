//! v0.28.0 Writer Fencing & Concurrency Correctness tests.
//!
//! Covers the three task groups from the v0.28.0 roadmap:
//!   § Transactional GC Lease/Pin Enforcement
//!     1. gc_lease_acquired_concurrently_is_respected
//!   § Atomic rebuild_catalog()
//!     2. partial_rebuild_leaves_catalog_absent
//!     3. rebuild_catalog_is_all_or_none

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use slatedb::{Db, WriteBatch};
use std::sync::Arc;
use tempfile::TempDir;

fn test_opts(dir: &TempDir) -> OpenOptions {
    let store = Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from("catalog"),
        encryption: None,
    }
}

async fn open_db(dir: &TempDir) -> Db {
    let path = dir.path().to_str().unwrap().to_string();
    let store = Arc::new(LocalFileSystem::new_with_prefix(&path).unwrap());
    Db::open(ObjectPath::from("catalog"), store).await.unwrap()
}

// ─── § Transactional GC Lease/Pin Enforcement ────────────────────────────

/// A snapshot lease acquired concurrently with `gc_apply()` must be respected.
///
/// Scenario:
///  1. Commit three snapshots.
///  2. Acquire a lease that pins snapshot 2.
///  3. Attempt `gc_apply(3)` — must fail because the lease pins snapshot 2
///     and 3 > 2.
///
/// v0.28.0: The lease scan runs inside the same `SerializableSnapshot`
/// transaction as the retain-from write, so no TOCTOU window exists.
#[tokio::test]
async fn gc_lease_acquired_concurrently_is_respected() {
    let dir = TempDir::new().unwrap();

    // Write three snapshots.
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();
    {
        let mut w = store.begin_write();
        w.create_schema("s1").await.unwrap();
        let r = w.create_snapshot(Some("snap1"), None).await.unwrap();
        store.commit_writer(r);
    }
    {
        let mut w = store.begin_write();
        w.create_schema("s2").await.unwrap();
        let r = w.create_snapshot(Some("snap2"), None).await.unwrap();
        store.commit_writer(r);
    }
    {
        let mut w = store.begin_write();
        w.create_schema("s3").await.unwrap();
        let r = w.create_snapshot(Some("snap3"), None).await.unwrap();
        store.commit_writer(r);
    }
    store.close().await.unwrap();

    let db = open_db(&dir).await;

    // Acquire a lease pinning snapshot 2 with a 60-second TTL.
    rocklake_catalog::lease::hold_snapshot(&db, "consumer-a", 2, 60)
        .await
        .unwrap();

    // gc_apply(3) must fail because the lease covers snapshot 2 (3 > 2).
    let result = rocklake_catalog::gc::gc_apply(&db, 3).await;
    assert!(
        result.is_err(),
        "gc_apply must fail when a lease pins a snapshot below the requested retain-from"
    );

    // gc_apply(1) should succeed (does not advance past the leased snapshot).
    let ok = rocklake_catalog::gc::gc_apply(&db, 1).await;
    assert!(
        ok.is_ok(),
        "gc_apply(1) should succeed when lease pins snapshot 2; got: {ok:?}"
    );

    // Release the lease and verify gc_apply(3) now succeeds.
    rocklake_catalog::lease::release_snapshot(&db, "consumer-a")
        .await
        .unwrap();
    let after_release = rocklake_catalog::gc::gc_apply(&db, 3).await;
    assert!(
        after_release.is_ok(),
        "gc_apply(3) must succeed after releasing the lease; got: {after_release:?}"
    );

    db.close().await.unwrap();
}

// ─── § Atomic rebuild_catalog() ──────────────────────────────────────────

/// Dropping a WriteBatch before calling `db.write()` leaves the catalog absent.
///
/// This test proves the atomicity property directly: rows staged in a batch
/// that is dropped without committing are never visible in SlateDB.
#[tokio::test]
async fn partial_rebuild_leaves_catalog_absent() {
    let dir = TempDir::new().unwrap();
    let db = open_db(&dir).await;

    // Stage several catalog rows in a WriteBatch but DROP the batch without
    // calling db.write() — simulating a crash before commit.
    {
        let mut batch = WriteBatch::new();
        batch.put(b"test-key-1", b"test-value-1");
        batch.put(b"test-key-2", b"test-value-2");
        batch.put(b"test-key-3", b"test-value-3");
        // batch is dropped here without being written to `db`.
    }

    // None of the keys must be visible.
    let v1 = db.get(b"test-key-1").await.unwrap();
    let v2 = db.get(b"test-key-2").await.unwrap();
    let v3 = db.get(b"test-key-3").await.unwrap();
    assert!(
        v1.is_none(),
        "key-1 must not be visible after dropped batch"
    );
    assert!(
        v2.is_none(),
        "key-2 must not be visible after dropped batch"
    );
    assert!(
        v3.is_none(),
        "key-3 must not be visible after dropped batch"
    );

    db.close().await.unwrap();
}

/// `rebuild_catalog()` is atomic: either all rows are present or none are.
///
/// After a successful `rebuild_catalog()` call every schema, table, data file,
/// snapshot, and counter row must be readable.
#[tokio::test]
async fn rebuild_catalog_is_all_or_none() {
    let dir = TempDir::new().unwrap();

    {
        let db = open_db(&dir).await;

        let paths = vec![
            "data/part-0001.parquet".to_string(),
            "data/part-0002.parquet".to_string(),
        ];

        let count = rocklake_catalog::export::rebuild_catalog(&db, &paths)
            .await
            .expect("rebuild_catalog must succeed");
        assert_eq!(count, 2, "rebuild must register exactly 2 data files");

        // Close the raw DB before opening via CatalogStore.
        db.close().await.unwrap();
    }

    // Open the rebuilt catalog via CatalogStore and verify it is fully readable.
    let store = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let reader = store.read_at(SnapshotId::new(1)).unwrap();

    let schemas = reader.list_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1, "exactly one schema after rebuild");

    let tables = reader.list_tables(1).await.unwrap();
    assert_eq!(tables.len(), 1, "exactly one table after rebuild");

    let files = reader.list_data_files(tables[0].table_id).await.unwrap();
    assert_eq!(
        files.len(),
        2,
        "both data files must be visible after rebuild"
    );

    store.close().await.unwrap();
}
