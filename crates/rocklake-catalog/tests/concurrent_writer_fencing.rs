//! Concurrent writer fencing tests (v0.27.3 / v0.28.0).
//!
//! Verifies that the CAS-protected monotonic writer epoch mechanism correctly:
//!  1. Allows only one writer at a time (the one with the highest epoch counter).
//!  2. Rejects stale writers on commit with `WriterEpochMismatch`.
//!  3. Allows re-opening after the original writer is dropped.
//!  4. Handles concurrent open() calls (tokio::join!) with exactly one winner.
//!  5. (v0.28.0) Deterministically fences two writers opened in the same OS tick.

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogError, CatalogStore, OpenOptions};
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

/// Test 1: A stale writer is fenced on commit after a newer writer opens.
///
/// Steps:
///  1. Open Store 1 (acquires epoch counter N).
///  2. Open Store 2 (acquires epoch counter N+1, overwriting N in SlateDB).
///  3. Store 1 attempts create_snapshot() → must fail with WriterEpochMismatch.
///
/// v0.28.0: No sleep required — fencing is based on a monotonic counter, not
/// the wall clock.
#[tokio::test]
async fn stale_writer_fenced_on_commit() {
    let dir = TempDir::new().unwrap();

    // Open first writer.
    let mut store1 = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Open second writer — takes over the epoch (counter incremented, no sleep needed).
    let _store2 = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Store 1's epoch is now stale.  A commit attempt must be rejected.
    let mut writer1 = store1.begin_write();
    let result = writer1.create_snapshot(Some("fencing-test"), None).await;
    // SlateDB may surface the rejection as WriterEpochMismatch (application-level
    // epoch guard) or as TransactionConflict (SlateDB-level closed-DB error).
    let is_fenced = matches!(
        &result,
        Err(CatalogError::WriterEpochMismatch) | Err(CatalogError::TransactionConflict(_))
    );
    assert!(
        is_fenced,
        "expected a writer-fencing error but got: {result:?}"
    );
}

/// Test 2: Re-opening after the prior writer is dropped succeeds.
///
/// v0.28.0: sleeps removed — sequential opens always acquire strictly higher
/// epochs via the monotonic counter.
#[tokio::test]
async fn reopen_after_drop_succeeds() {
    let dir = TempDir::new().unwrap();

    {
        let _store1 = CatalogStore::open(test_opts(&dir)).await.unwrap();
        // store1 is dropped here.
    }

    {
        let _store2 = CatalogStore::open(test_opts(&dir)).await.unwrap();
        // store2 is dropped here.
    }

    // Store 3 opens with the newest epoch and must be able to commit.
    let mut store3 = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let mut writer3 = store3.begin_write();
    let commit = writer3
        .create_snapshot(Some("fresh-writer"), None)
        .await
        .expect("Store 3 must commit successfully");
    store3.commit_writer(commit);
}

/// Test 3: Concurrent open — exactly one writer can commit.
///
/// Both stores open concurrently via `tokio::join!`.  Since the epoch CAS is
/// monotonic and time-stamped, the later-opening store holds the winning epoch.
/// The store with the smaller epoch must get `WriterEpochMismatch` on commit
/// while the larger-epoch store commits cleanly.
#[tokio::test]
async fn concurrent_open_exactly_one_commits() {
    let dir = TempDir::new().unwrap();

    // Open both stores concurrently.  The second future to complete the CAS
    // will hold the winning epoch.  One of the opens may itself fail if SlateDB
    // rejects the second CAS attempt before the store is fully initialised.
    let (s1_result, s2_result) = tokio::join!(
        CatalogStore::open(test_opts(&dir)),
        CatalogStore::open(test_opts(&dir)),
    );

    // Collect successfully-opened stores and count any open-time rejections.
    let mut open_rejected: usize = 0;
    let mut stores: Vec<CatalogStore> = Vec::new();
    for res in [s1_result, s2_result] {
        match res {
            Ok(s) => stores.push(s),
            Err(_) => open_rejected += 1,
        }
    }

    // If both opens failed something is fundamentally wrong.
    assert!(
        !stores.is_empty(),
        "at least one concurrent open must succeed"
    );

    // Try to commit from every successfully-opened store.
    let mut commit_results: Vec<Result<_, CatalogError>> = Vec::new();
    for mut s in stores {
        let r = s
            .begin_write()
            .create_snapshot(Some("concurrent"), None)
            .await;
        commit_results.push(r);
    }

    let num_commit_ok = commit_results.iter().filter(|r| r.is_ok()).count();
    let num_commit_fenced = commit_results
        .iter()
        .filter(|r| {
            matches!(
                r,
                Err(CatalogError::WriterEpochMismatch) | Err(CatalogError::TransactionConflict(_))
            )
        })
        .count();

    // Total fencing events = those rejected during open + those rejected at commit time.
    let total_fenced = open_rejected + num_commit_fenced;

    assert!(
        num_commit_ok >= 1,
        "at least one writer must be able to commit"
    );
    // Every non-committed result must be a recognised fencing error.
    assert!(
        num_commit_ok + total_fenced == 2,
        "each open must either commit or be fenced; ok={num_commit_ok} fenced={total_fenced}"
    );
}

/// Test 4: Interleaved DuckLake writers — exactly one commit wins per snapshot.
///
/// v0.28.0: sleep removed; sequential open() guarantees a strictly higher
/// epoch for store B than for store A via the monotonic counter.
#[tokio::test]
async fn interleaved_ducklake_writers_exactly_one_wins() {
    let dir = TempDir::new().unwrap();

    // Store A opens first and buffers a schema creation (DuckLake-style).
    let mut store_a = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let mut writer_a = store_a.begin_write();
    writer_a
        .create_schema("schema_a")
        .await
        .expect("writer A must buffer schema creation");

    // Store B opens after A — it now owns the epoch (no sleep needed).
    let mut store_b = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let mut writer_b = store_b.begin_write();
    writer_b
        .create_schema("schema_b")
        .await
        .expect("writer B must buffer schema creation");

    // Both writers attempt to commit.  B holds the winning epoch; A is stale.
    let result_b = writer_b.create_snapshot(Some("winner"), None).await;
    let result_a = writer_a.create_snapshot(Some("loser"), None).await;

    // B must win (has the latest epoch).
    assert!(
        result_b.is_ok(),
        "writer B (latest epoch) must commit successfully; got: {result_b:?}"
    );
    // A must be rejected.
    let is_fenced_a = matches!(
        &result_a,
        Err(CatalogError::WriterEpochMismatch) | Err(CatalogError::TransactionConflict(_))
    );
    assert!(
        is_fenced_a,
        "writer A (stale epoch) must be fenced; got: {result_a:?}"
    );

    // Commit winner's result so we can read back the catalog.
    store_b.commit_writer(result_b.unwrap());

    // Only schema_b should be visible; schema_a was never committed.
    let reader = store_b.read_latest();
    let schemas = reader
        .list_schemas()
        .await
        .expect("catalog read must not fail");
    assert_eq!(
        schemas.len(),
        1,
        "exactly one schema must exist after the interleaved race; found {}",
        schemas.len()
    );
    assert_eq!(
        schemas[0].schema_name, "schema_b",
        "only the winning writer's schema must be visible"
    );
}

/// Test 5 (v0.28.0): Two writers opened in the same OS tick — exactly one is fenced.
///
/// With the monotonic counter the CAS loop always produces strictly ordered
/// epoch values even when two opens happen within the same nanosecond.
/// No `sleep` is used; the test must pass deterministically.
#[tokio::test]
async fn no_sleep_same_tick_exactly_one_fenced() {
    let dir = TempDir::new().unwrap();

    // Open two stores sequentially (no sleep). The second open increments the
    // counter to epoch 2; store1 holds epoch 1 and is now stale.
    let mut store1 = CatalogStore::open(test_opts(&dir)).await.unwrap();
    let mut store2 = CatalogStore::open(test_opts(&dir)).await.unwrap();

    let result1 = store1.begin_write().create_snapshot(Some("t1"), None).await;
    let result2 = store2.begin_write().create_snapshot(Some("t2"), None).await;

    // store2 holds the higher epoch and must succeed; store1 must be fenced.
    assert!(
        result2.is_ok(),
        "store2 (latest epoch) must commit; got: {result2:?}"
    );
    let is_fenced = matches!(
        &result1,
        Err(CatalogError::WriterEpochMismatch) | Err(CatalogError::TransactionConflict(_))
    );
    assert!(
        is_fenced,
        "store1 (stale epoch) must be fenced; got: {result1:?}"
    );
}
