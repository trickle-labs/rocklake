//! DuckDB + DuckLake Checkpoint Consolidation Tests
//!
//! Tests the exact scenario: INSERT → CHECKPOINT → SELECT should not duplicate rows.
//! These tests verify that when DuckLake consolidates files during CHECKPOINT,
//! RockLake correctly filters to show only the latest consolidated version.

use object_store::path::Path as ObjectPath;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
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

/// Simulate what DuckLake does: Insert files, then "consolidate" them by creating
/// a new file without marking the old ones as deleted.
#[tokio::test]
async fn test_checkpoint_consolidation_no_duplication() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup: Create schema, table
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("main").await.unwrap();
    let table_id = writer
        .create_table(schema_id, "brukere", Some("az://data/brukere/"))
        .await
        .unwrap();
    writer
        .add_column(table_id, "id", "INTEGER", 0, false, None)
        .await
        .unwrap();
    writer
        .add_column(table_id, "navn", "VARCHAR", 1, true, None)
        .await
        .unwrap();
    let snap1 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1.clone());

    // Phase 1: INSERT 2 rows, create initial files
    let mut writer = store.begin_write();
    let _file1_id = writer
        .register_data_file(
            table_id,
            "s3://bucket/data1.parquet",
            "parquet",
            2,      // 2 rows
            1024,   // file size
        )
        .await
        .unwrap();
    let snap2 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2.clone());
    eprintln!("✓ Phase 1: Registered 2-row file at snapshot 2");

    // Verify: Should see 1 file with 2 rows
    {
        let reader = store.read_at(snap2.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        eprintln!("After INSERT - Files visible: {}", files.len());
        for f in &files {
            eprintln!("  file_id={}, begin_snapshot={:?}, end_snapshot={:?}, rows={}", 
                f.data_file_id, f.begin_snapshot, f.end_snapshot, f.record_count);
        }
        assert_eq!(files.len(), 1, "Should see exactly 1 file after INSERT");
    }

    // Phase 2: CHECKPOINT - Consolidate to a single file (DuckLake behavior)
    // Old file is NOT marked as deleted (this is the problematic DuckLake behavior)
    let mut writer = store.begin_write();
    let _file2_id = writer
        .register_data_file(
            table_id,
            "s3://bucket/data_consolidated.parquet",
            "parquet",
            2,      // Same 2 rows, consolidated
            1024,   // file size
        )
        .await
        .unwrap();
    // NOTE: Old file1 is NOT being marked with end_snapshot here
    // This simulates DuckLake not sending DELETE statements
    let snap3 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap3.clone());
    eprintln!("✓ Phase 2: CHECKPOINT - Created consolidated file");

    // Verify: Should still see only 2 rows, NOT 4 (despite having 2 files)
    {
        let reader = store.read_at(snap3.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        eprintln!("After CHECKPOINT - Files visible: {}", files.len());
        for f in &files {
            eprintln!("  file_id={}, begin_snapshot={:?}, end_snapshot={:?}, rows={}", 
                f.data_file_id, f.begin_snapshot, f.end_snapshot, f.record_count);
        }
        
        // BUG: Without consolidation detection, this would be 2 files
        // FIX: With consolidation detection, this should be 1 file (the newest)
        if files.len() == 2 {
            eprintln!("⚠️  BUG DETECTED: Both old and new files are visible!");
            eprintln!("   This causes row duplication (2 + 2 = 4 rows returned)");
            let total_rows: u64 = files.iter().map(|f| f.record_count).sum();
            eprintln!("   Total rows would be: {}", total_rows);
        } else if files.len() == 1 {
            eprintln!("✓ FIXED: Only newest file is visible (consolidation cleanup working)");
        }
        
        // Assert the fix is in place
        assert_eq!(files.len(), 1, 
            "Should see only 1 file after CHECKPOINT (latest consolidation batch)\n\
             If you see 2 here, consolidation detection isn't working");
    }

    store.close().await.unwrap();
}

/// Extended test: Multiple INSERT + CHECKPOINT cycles
#[tokio::test]
async fn test_multiple_checkpoint_cycles_no_duplication() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Setup
    let mut writer = store.begin_write();
    let schema_id = writer.create_schema("main").await.unwrap();
    let table_id = writer
        .create_table(schema_id, "test_table", Some("az://data/test/"))
        .await
        .unwrap();
    writer
        .add_column(table_id, "id", "INTEGER", 0, false, None)
        .await
        .unwrap();
    let mut snap = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap.clone());

    let mut expected_total_rows = 0u64;

    // Cycle 1: INSERT 2 rows + CHECKPOINT
    {
        let mut writer = store.begin_write();
        expected_total_rows += 2;
        writer
            .register_data_file(
                table_id,
                "s3://bucket/file1.parquet",
                "parquet",
                2,
                1024,
            )
            .await
            .unwrap();
        snap = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
    }

    // Verify after cycle 1
    {
        let reader = store.read_at(snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!("Cycle 1: {} files, {} total rows", files.len(), total);
        assert_eq!(total, expected_total_rows, "Should have {} rows after cycle 1", expected_total_rows);
    }

    // Cycle 2: INSERT 1 more row + CHECKPOINT (consolidates)
    {
        let mut writer = store.begin_write();
        expected_total_rows += 1;
        // Register consolidation: old file + new file
        // This simulates what happens when DuckLake consolidates
        writer
            .register_data_file(
                table_id,
                "s3://bucket/consolidated2.parquet",
                "parquet",
                expected_total_rows, // All rows in new consolidated file
                1024,
            )
            .await
            .unwrap();
        snap = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
    }

    // Verify after cycle 2: Should NOT duplicate
    {
        let reader = store.read_at(snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!("Cycle 2: {} files, {} total rows", files.len(), total);
        
        eprintln!("  Files:");
        for f in &files {
            eprintln!("    id={}, rows={}, begin={:?}", 
                f.data_file_id, f.record_count, f.begin_snapshot);
        }
        
        assert_eq!(total, expected_total_rows, 
            "Should have {} rows after cycle 2 (not duplicated)\n\
             If total is {}, consolidation cleanup isn't working",
            expected_total_rows, expected_total_rows * 2);
    }

    store.close().await.unwrap();
}
