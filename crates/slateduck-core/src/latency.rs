//! Latency baseline measurement tests.
//!
//! These tests measure and verify the latency characteristics of SlateDB
//! operations on InMemory object store as the Phase 0 baseline.

#[cfg(test)]
mod tests {
    use object_store::memory::InMemory;
    use object_store::path::Path as ObjectPath;
    use slatedb::Db;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    async fn open_db(store: Arc<dyn object_store::ObjectStore>, path: &str) -> Db {
        Db::builder(ObjectPath::from(path), store)
            .build()
            .await
            .expect("failed to open DB")
    }

    fn percentile(sorted: &[Duration], p: f64) -> Duration {
        let idx = ((sorted.len() as f64) * p / 100.0) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Measure durable commit latency (put + flush).
    #[tokio::test]
    async fn latency_durable_commit() {
        let store: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
        let db = open_db(store.clone(), "bench/commit").await;

        let mut latencies = Vec::new();
        for i in 0..200 {
            let key = format!("commit_key_{i:04}");
            let start = Instant::now();
            db.put(key.as_bytes(), b"value").await.unwrap();
            db.flush().await.unwrap();
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let p50 = percentile(&latencies, 50.0);
        let p95 = percentile(&latencies, 95.0);
        let p99 = percentile(&latencies, 99.0);

        // InMemory should be well under 500ms for p50 (accounts for CI variability)
        assert!(p50 < Duration::from_millis(500), "p50={p50:?}");
        assert!(p95 < Duration::from_secs(1), "p95={p95:?}");
        assert!(p99 < Duration::from_secs(2), "p99={p99:?}");

        db.close().await.unwrap();
    }

    /// Measure single get latency.
    #[tokio::test]
    async fn latency_single_get() {
        let store: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
        let db = open_db(store.clone(), "bench/get").await;

        // Pre-populate
        for i in 0..100 {
            let key = format!("get_key_{i:04}");
            db.put(key.as_bytes(), b"value").await.unwrap();
        }
        db.flush().await.unwrap();

        let mut latencies = Vec::new();
        for i in 0..100 {
            let key = format!("get_key_{i:04}");
            let start = Instant::now();
            let _ = db.get(key.as_bytes()).await.unwrap();
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let p50 = percentile(&latencies, 50.0);
        let p95 = percentile(&latencies, 95.0);
        let p99 = percentile(&latencies, 99.0);

        assert!(p50 < Duration::from_millis(50), "p50={p50:?}");
        assert!(p95 < Duration::from_millis(100), "p95={p95:?}");
        assert!(p99 < Duration::from_millis(200), "p99={p99:?}");

        db.close().await.unwrap();
    }

    /// Measure prefix scan latency over 10K entries.
    #[tokio::test]
    async fn latency_prefix_scan_10k() {
        let store: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
        let db = open_db(store.clone(), "bench/scan").await;

        // Pre-populate 10K entries
        for batch_start in (0..10000).step_by(500) {
            let mut batch = slatedb::WriteBatch::new();
            for i in batch_start..batch_start + 500 {
                let key = format!("scan_prefix/{i:06}");
                let value = format!("value_{i}");
                batch.put(key.as_bytes(), value.as_bytes());
            }
            db.write(batch).await.unwrap();
        }
        db.flush().await.unwrap();

        // Measure scan time
        let start = Instant::now();
        let mut iter = db.scan_prefix(b"scan_prefix/").await.unwrap();
        let mut count = 0u64;
        while iter.next().await.unwrap().is_some() {
            count += 1;
        }
        let elapsed = start.elapsed();

        assert_eq!(count, 10000);
        // 10K scan should complete in under 5 seconds on InMemory
        assert!(elapsed < Duration::from_secs(5), "scan took {elapsed:?}");

        db.close().await.unwrap();
    }
}
