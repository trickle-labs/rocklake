//! End-to-end test: DuckDB + DuckLake + RockLake + Minio/Azure
//!
//! This test simulates the real scenario the user reported:
//! 1. Create a table in DuckLake
//! 2. Insert 2 rows
//! 3. Verify select returns 2 rows
//! 4. Run CHECKPOINT
//! 5. Verify select still returns 2 rows (NOT duplicated)

#![allow(clippy::clone_on_copy)]

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
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

/// Simulates the real DuckLake flow with separate snapshots for INSERT and CHECKPOINT
///
/// This is likely what's happening in the user's scenario where CHECKPOINT happens
/// after the INSERT snapshot is committed.
#[tokio::test]
async fn test_real_checkpoint_with_separate_snapshots() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: Create schema and table
    let table_id = {
        let mut w = store.begin_write();
        let schema_id = w.create_schema("public").await.unwrap();
        let tid = w
            .create_table(schema_id, "brukere", Some("data/"))
            .await
            .unwrap();
        w.add_column(tid, "id", "INTEGER", 0, false, None)
            .await
            .unwrap();
        w.add_column(tid, "navn", "VARCHAR", 1, true, None)
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap);
        tid
    };

    // Transaction 1: INSERT 2 rows (represented as 2 separate files of 1 row each for realistic consolidation)
    println!("📝 Transaction 1: Inserting 2 rows into 2 files");
    let insert_snap = {
        let mut w = store.begin_write();
        let _file1_id = w
            .register_data_file(
                table_id,
                "data/part-00001.parquet",
                "parquet",
                1,   // 1 row
                256, // file size
            )
            .await
            .unwrap();
        let _file2_id = w
            .register_data_file(
                table_id,
                "data/part-00002.parquet",
                "parquet",
                1,   // 1 row
                256, // file size
            )
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!("  ✓ Registered data files at snapshot {}", snap.snapshot_id);
        snap
    };

    // Verify: 2 rows visible after INSERT
    {
        let reader = store.read_at(insert_snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total_rows: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!(
            "After INSERT: {} files, {} total rows",
            files.len(),
            total_rows
        );
        assert_eq!(files.len(), 2, "After INSERT: should see 2 files");
        assert_eq!(total_rows, 2, "After INSERT: should see 2 rows");
    }

    // Transaction 2: CHECKPOINT (consolidates files)
    // In reality, DuckLake would:
    // 1. Read old files (part-00001.parquet, part-00002.parquet)
    // 2. Consolidate them into part-consolidated.parquet
    // 3. Register new consolidated file
    // 4. NOT mark old files as deleted (simulated behavior)
    println!("🔄 Transaction 2: Running CHECKPOINT");
    let checkpoint_snap = {
        let mut w = store.begin_write();

        // Reset table stats next_row_id to 0 temporarily so the consolidated file gets row_id_start = Some(0)!
        w.set_table_stats(table_id, 2, 512, 0).await.unwrap();

        // Register consolidated file with same 2 rows
        let _consolidated_id = w
            .register_data_file(
                table_id,
                "data/part-consolidated-000.parquet",
                "parquet",
                2,   // Same 2 rows, consolidated
                512, // file size
            )
            .await
            .unwrap();

        // NOTE: Old file is NOT marked as deleted here - simulating DuckLake's behavior

        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!(
            "  ✓ Registered consolidated file at snapshot {}",
            snap.snapshot_id
        );
        snap
    };

    // Verify: Should still see 2 rows (NOT 4!)
    {
        let reader = store.read_at(checkpoint_snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total_rows: u64 = files.iter().map(|f| f.record_count).sum();

        eprintln!(
            "After CHECKPOINT: {} files, {} total rows",
            files.len(),
            total_rows
        );
        for (i, f) in files.iter().enumerate() {
            eprintln!(
                "  File {}: id={}, begin={:?}, rows={}",
                i + 1,
                f.data_file_id,
                f.begin_snapshot,
                f.record_count
            );
        }

        // This is the key assertion - should NOT duplicate!
        assert_eq!(
            total_rows,
            2,
            "After CHECKPOINT: should see 2 rows, NOT 4! {} files visible",
            files.len()
        );
    }

    store.close().await.unwrap();
}

/// Additional test: Multiple INSERT+CHECKPOINT cycles
#[tokio::test]
async fn test_multiple_inserts_and_checkpoints() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup
    let table_id = {
        let mut w = store.begin_write();
        let schema_id = w.create_schema("public").await.unwrap();
        let tid = w
            .create_table(schema_id, "orders", Some("data/"))
            .await
            .unwrap();
        w.add_column(tid, "order_id", "INTEGER", 0, false, None)
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap);
        tid
    };

    let mut total_expected = 0u64;
    let mut last_consolidated_file_id = 0u64;
    let _ = last_consolidated_file_id;

    // Cycle 1: INSERT 5 rows + CHECKPOINT
    {
        let mut w = store.begin_write();
        total_expected += 5;
        let file1_id = w
            .register_data_file(table_id, "data/part-1.parquet", "parquet", 5, 512)
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!("Cycle 1: Inserted 5 rows");

        // CHECKPOINT: consolidate part-1.parquet into part-1-consolidated.parquet
        let mut w = store.begin_write();
        let file2_id = w
            .register_data_file(
                table_id,
                "data/part-1-consolidated.parquet",
                "parquet",
                5,
                512,
            )
            .await
            .unwrap();
        // Mark old file as deleted (simulating proper DuckLake cleanup)
        w.mark_data_file_deleted(file1_id).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        last_consolidated_file_id = file2_id;
        eprintln!("Cycle 1: CHECKPOINT completed, old file marked deleted");

        let reader = store.read_at(snap).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!("After cycle 1 CHECKPOINT: {} rows", total);
        assert_eq!(
            total, total_expected,
            "Cycle 1: should have {} rows",
            total_expected
        );
    }

    // Cycle 2: INSERT 3 more rows + CHECKPOINT
    {
        let mut w = store.begin_write();
        total_expected += 3;
        let file3_id = w
            .register_data_file(table_id, "data/part-2.parquet", "parquet", 3, 512)
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!("Cycle 2: Inserted 3 more rows");

        // CHECKPOINT: consolidate previous consolidated file + new file into part-2-consolidated.parquet
        let mut w = store.begin_write();
        let _file4_id = w
            .register_data_file(
                table_id,
                "data/part-2-consolidated.parquet",
                "parquet",
                total_expected,
                512,
            )
            .await
            .unwrap();
        // Mark both old consolidated file AND new insert file as deleted
        w.mark_data_file_deleted(last_consolidated_file_id)
            .await
            .unwrap();
        w.mark_data_file_deleted(file3_id).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!("Cycle 2: CHECKPOINT completed, old files marked deleted");

        let reader = store.read_at(snap).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!(
            "After cycle 2 CHECKPOINT: {} rows, {} files",
            total,
            files.len()
        );
        for (i, f) in files.iter().enumerate() {
            eprintln!(
                "  File {}: id={}, begin={:?}, end={:?}, rows={}",
                i + 1,
                f.data_file_id,
                f.begin_snapshot,
                f.end_snapshot,
                f.record_count
            );
        }
        assert_eq!(
            total, total_expected,
            "Cycle 2: should have {} rows",
            total_expected
        );
    }

    store.close().await.unwrap();
}

/// Verification test: 2 independent inserts with the same row count across different snapshots
/// must both remain visible and not be mistaken for consolidation.
#[tokio::test]
async fn test_independent_unconsolidated_inserts_same_row_count() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: Create schema and table
    let table_id = {
        let mut w = store.begin_write();
        let schema_id = w.create_schema("public").await.unwrap();
        let tid = w
            .create_table(schema_id, "brukere", Some("data/"))
            .await
            .unwrap();
        w.add_column(tid, "id", "INTEGER", 0, false, None)
            .await
            .unwrap();
        w.add_column(tid, "navn", "VARCHAR", 1, true, None)
            .await
            .unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap);
        tid
    };

    // Transaction 1: INSERT 2 rows (first batch, in file1)
    println!("📝 Transaction 1: Inserting 2 rows");
    let _insert_snap_1 = {
        let mut w = store.begin_write();
        let _file1_id = w
            .register_data_file(
                table_id,
                "data/part-00001.parquet",
                "parquet",
                2,   // 2 rows
                512, // file size
            )
            .await
            .unwrap();
        w.update_table_stats(table_id, 2, 2, 512).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        snap
    };

    // Transaction 2: INSERT 2 more rows (second batch, e.g. after CHECKPOINT of inlined, in file2)
    println!("📝 Transaction 2: Inserting another 2 rows into a separate file");
    let insert_snap_2 = {
        let mut w = store.begin_write();
        let _file2_id = w
            .register_data_file(
                table_id,
                "data/part-00002.parquet",
                "parquet",
                2,   // Same 2 row count!
                512, // file size
            )
            .await
            .unwrap();
        w.update_table_stats(table_id, 2, 4, 512).await.unwrap();
        let snap = w.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        snap
    };

    // Verify: Total rows must be 4, as there are 2 separate files with 2 rows each.
    // They must not be mistaken for consolidation!
    {
        let reader = store.read_at(insert_snap_2).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total_rows: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!(
            "After independent inserts: {} files, {} total rows",
            files.len(),
            total_rows
        );
        assert_eq!(files.len(), 2, "Should see 2 files");
        assert_eq!(
            total_rows, 4,
            "Should see 4 rows (2 + 2), NOT 2 due to incorrect consolidation detection"
        );
    }

    store.close().await.unwrap();
}
