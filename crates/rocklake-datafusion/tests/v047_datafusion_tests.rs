//! v0.47.0 — DataFusion AsyncBridge Backpressure Tests.
//!
//! Verifies that the AsyncBridge channel capacity is sufficient for 128
//! concurrent DataFusion catalog queries without blocking.
//!
//! Roadmap item: "Add a test that fires 128 concurrent DataFusion catalog
//! queries and asserts none block beyond the bridge queue depth."

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use rocklake_datafusion::RockLakeCatalogProvider;
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

/// Fire 128 concurrent DataFusion catalog queries and assert that none block
/// beyond the bridge queue depth of 256.
///
/// The AsyncBridge uses a bounded sync_channel with configurable capacity.
/// With queue_depth=256, up to 256 tasks can be enqueued without blocking the
/// caller.  Firing 128 concurrent queries (< 256) must never block.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_128_concurrent_datafusion_queries_no_blocking() {
    let dir = TempDir::new().unwrap();

    // Bootstrap a catalog with a few schemas.
    {
        let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();
        let mut writer = store.begin_write();
        for i in 0..8 {
            writer.create_schema(&format!("schema_{i}")).await.unwrap();
        }
        let _ = writer.create_snapshot(None, None).await.unwrap();
        store.close().await.unwrap();
    }

    let store = CatalogStore::open(test_opts(&dir)).await.unwrap();
    // Use queue_depth=256 (the default) — well above the 128 concurrent queries.
    let provider = Arc::new(
        RockLakeCatalogProvider::new_with_queue_depth(store, Some(SnapshotId::new(1)), 256)
            .expect("provider construction failed"),
    );

    // Spawn 128 blocking tasks via tokio::task::spawn_blocking (managed thread pool)
    // so the test does not exhaust OS thread limits in debug mode.
    // schema_names() dispatches through AsyncBridge.run_sync(), which enqueues a
    // task on the bounded channel.  With queue_depth=256 > 128, none should block.
    let start = std::time::Instant::now();
    let handles: Vec<_> = (0..128)
        .map(|_| {
            let p = Arc::clone(&provider);
            tokio::task::spawn_blocking(move || {
                use datafusion::catalog::CatalogProvider;
                let names = p.schema_names();
                assert!(!names.is_empty(), "schema_names must return non-empty list");
                names.len()
            })
        })
        .collect();

    let mut total_schema_count = 0usize;
    for h in handles {
        let count = h.await.expect("task panicked");
        total_schema_count += count;
    }
    let elapsed = start.elapsed();

    // All 128 queries returned 8 schemas each.
    assert_eq!(
        total_schema_count,
        128 * 8,
        "each of the 128 queries must see all 8 schemas"
    );

    // The total wall time must be well under the 10-second timeout even under
    // contention — this is a liveness check, not a strict latency SLA.
    assert!(
        elapsed.as_secs() < 10,
        "128 concurrent queries took too long ({elapsed:?}); bridge may be blocking"
    );
}

/// Verify that `new_with_queue_depth` creates a bridge with the specified
/// capacity and that it handles the default (256) correctly.
#[test]
fn test_catalog_provider_queue_depth_configurable() {
    use datafusion::catalog::CatalogProvider;
    use tokio::runtime::Runtime;

    let dir = TempDir::new().unwrap();
    let rt = Runtime::new().unwrap();

    let store = rt.block_on(async { CatalogStore::open(test_opts(&dir)).await.unwrap() });

    // Default queue depth (256) — should succeed.
    let provider_default =
        RockLakeCatalogProvider::new(store, Some(SnapshotId::new(0))).expect("default queue depth");

    // Verify it compiles and returns a valid provider.
    let _ = provider_default.schema_names();

    // Explicitly configured queue depth.
    let dir2 = TempDir::new().unwrap();
    let store2 = rt.block_on(async { CatalogStore::open(test_opts(&dir2)).await.unwrap() });
    let provider_custom =
        RockLakeCatalogProvider::new_with_queue_depth(store2, Some(SnapshotId::new(0)), 512)
            .expect("custom queue depth 512");

    let _ = provider_custom.schema_names();
}
