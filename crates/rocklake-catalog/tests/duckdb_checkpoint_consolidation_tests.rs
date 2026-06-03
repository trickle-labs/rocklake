//! DuckDB + DuckLake Checkpoint Consolidation Tests
//!
//! Tests the exact scenario: INSERT → CHECKPOINT → SELECT should not duplicate rows.
//! These tests verify that when DuckLake consolidates files during CHECKPOINT,
//! RockLake correctly filters to show only the latest consolidated version.

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

/// Simulate DuckLake CHECKPOINT: INSERT → CHECKPOINT → should not duplicate rows.
/// 
/// DuckLake's CHECKPOINT operation consolidates files in 5 phases:
/// 1. Read old files
/// 2. Compact them
/// 3. Write consolidated file
/// 4. Register consolidated file in catalog
/// 5. Cleanup: Mark old files with end_snapshot (this is what we test here)
///
/// Without proper end_snapshot marking in Phase 5, both old and new files would be visible,
/// causing row duplication. This test verifies that mark_data_file_deleted() correctly
/// marks files as deleted so MVCC filtering excludes them.
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

    // Phase 1: INSERT 2 rows, create initial file
    let mut writer = store.begin_write();
    let file1_id = writer
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
    eprintln!("✓ Phase 1: Registered file1 (id={}, 2 rows) at snapshot {}", file1_id, snap2.snapshot_id);

    // Verify: Should see 1 file with 2 rows
    {
        let reader = store.read_at(snap2.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        eprintln!("After INSERT - Files visible: {}", files.len());
        assert_eq!(files.len(), 1, "Should see exactly 1 file after INSERT");
    }

    // Phase 2-5: CHECKPOINT - Consolidate to a single file
    // This simulates DuckLake's CHECKPOINT phases:
    // Phase 2-4: Register new consolidated file
    // Phase 5: Mark old file as deleted
    let mut writer = store.begin_write();
    
    // Phase 5: Mark old file as deleted (end_snapshot set by mark_data_file_deleted)
    writer.mark_data_file_deleted(file1_id).await.unwrap();
    eprintln!("✓ Phase 5a: Marked file1 (id={}) as deleted", file1_id);
    
    // Phase 2-4: Register new consolidated file (has same 2 rows)
    let file2_id = writer
        .register_data_file(
            table_id,
            "s3://bucket/data_consolidated.parquet",
            "parquet",
            2,      // Same 2 rows, consolidated
            1024,   // file size
        )
        .await
        .unwrap();
    eprintln!("✓ Phase 5b: Registered file2 (id={}, 2 rows) as consolidated version", file2_id);
    
    let snap3 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap3.clone());
    eprintln!("✓ CHECKPOINT complete at snapshot {}", snap3.snapshot_id);

    // Verify: Should still see only 2 rows, NOT 4
    // file1 is invisible due to end_snapshot being set by mark_data_file_deleted
    // Only file2 is visible
    {
        let reader = store.read_at(snap3.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        eprintln!("After CHECKPOINT - Files visible: {}", files.len());
        for f in &files {
            eprintln!("  file_id={}, begin_snapshot={:?}, end_snapshot={:?}, rows={}", 
                f.data_file_id, f.begin_snapshot, f.end_snapshot, f.record_count);
        }
        
        // Should see only 1 file (file2) because file1 has end_snapshot set
        assert_eq!(files.len(), 1, 
            "Should see exactly 1 file after CHECKPOINT consolidation");
        
        // Verify it's the new consolidated file
        assert_eq!(files[0].data_file_id, file2_id);
        assert_eq!(files[0].record_count, 2, "Consolidated file should still have 2 rows");
        
        // Verify the old file is not visible (has end_snapshot)
        assert!(files[0].end_snapshot.is_none(), "Active file should not have end_snapshot set");
        
        eprintln!("✓ VERIFIED: No duplication - only consolidated file visible");
    }

    store.close().await.unwrap();
}

/// Test: Consolidation requires proper end_snapshot marking by DuckLake.
/// When DuckLake performs CHECKPOINT:
/// 1. Old files should be marked as deleted (end_snapshot set)
/// 2. New consolidated file is registered
/// 3. MVCC filtering (based on begin/end_snapshot) ensures only active files are visible
/// 
/// This test simulates the real consolidation scenario where old files are properly marked.
#[tokio::test]
async fn test_consolidation_same_snapshot_duplication() {
    let dir = TempDir::new().unwrap();
    let mut store = CatalogStore::open(test_opts(&dir)).await.unwrap();

    // Phase 1: Initial INSERT creates file1
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
    let snap1 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap1.clone());
    
    // Phase 2: Register file1 (2-row file from insert)
    let mut writer = store.begin_write();
    let file1_id = writer
        .register_data_file(table_id, "s3://bucket/file1.parquet", "parquet", 2, 1024)
        .await
        .unwrap();
    let snap2 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap2.clone());
    eprintln!("Phase 2: Registered file1 id={} at snapshot {}", file1_id, snap2.snapshot_id);

    // Phase 3: CHECKPOINT consolidates file1 into file2
    // DuckLake would:
    // 1. Mark file1 as deleted (end_snapshot set to current snapshot)
    // 2. Register consolidated file2 with the same data
    let mut writer = store.begin_write();
    
    // Mark file1 as deleted (DuckLake's cleanup phase marks old files)
    writer.mark_data_file_deleted(file1_id).await.unwrap();
    
    // Register new consolidated file
    let file2_id = writer
        .register_data_file(table_id, "s3://bucket/consolidated.parquet", "parquet", 2, 1024)
        .await
        .unwrap();
    let snap3 = writer.create_snapshot(None, None).await.unwrap();
    store.commit_writer(snap3.clone());
    eprintln!("Phase 3: Deleted file1, registered file2 id={} at snapshot {}", file2_id, snap3.snapshot_id);

    // Verify at snap3: only file2 should be visible (file1 is deleted)
    // MVCC filtering based on begin_snapshot/end_snapshot ensures file1 is excluded
    {
        let reader = store.read_at(snap3.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        eprintln!("At snapshot {}: {} visible files", snap3.snapshot_id, files.len());
        for f in &files {
            eprintln!("  file_id={}, begin={:?}, end={:?}, records={}", 
                f.data_file_id, f.begin_snapshot, f.end_snapshot, f.record_count);
        }
        
        // Should see only 1 file (file2) because file1 is deleted (end_snapshot is set)
        assert_eq!(files.len(), 1, "At snapshot {}: expected 1 file but got {}", snap3.snapshot_id, files.len());
        assert_eq!(files[0].data_file_id, file2_id);
        assert_eq!(files[0].record_count, 2);
    }

    store.close().await.unwrap();
}

/// Extended test: Multiple INSERT + CHECKPOINT cycles with consolidation
/// 
/// Simulates realistic DuckLake behavior over time:
/// - Cycle 1: Insert 2 rows → CHECKPOINT consolidates (no old files to consolidate yet)
/// - Cycle 2: Insert 1 more row → CHECKPOINT consolidates old file(s) with new rows
/// 
/// Each CHECKPOINT properly marks old files as deleted, so MVCC filtering keeps
/// the total row count correct without duplication.
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
    let mut file_ids: Vec<u64> = Vec::new();

    // Cycle 1: INSERT 2 rows
    let file1_id = {
        let mut writer = store.begin_write();
        expected_total_rows += 2;
        let fid = writer
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
        eprintln!("Cycle 1: Inserted 2 rows (file_id={})", fid);
        fid
    };
    file_ids.push(file1_id);

    // Verify after cycle 1
    {
        let reader = store.read_at(snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!("Cycle 1 verified: {} files, {} total rows", files.len(), total);
        assert_eq!(total, expected_total_rows, "Should have {} rows after cycle 1", expected_total_rows);
        assert_eq!(files.len(), 1, "Should have 1 file after cycle 1");
    }

    // Cycle 2: INSERT 1 more row + CHECKPOINT consolidates file1 into consolidated2
    // CHECKPOINT phases:
    // - Mark file1 as deleted (end_snapshot set)
    // - Register new consolidated file with all 3 rows
    let file2_id = {
        let mut writer = store.begin_write();
        expected_total_rows += 1;
        
        // Mark file1 as deleted (Phase 5 of CHECKPOINT)
        writer.mark_data_file_deleted(file1_id).await.unwrap();
        eprintln!("Cycle 2: Marked file1 (id={}) as deleted", file1_id);
        
        // Register consolidated file with all rows
        let fid = writer
            .register_data_file(
                table_id,
                "s3://bucket/consolidated2.parquet",
                "parquet",
                expected_total_rows, // All rows consolidated: 2 + 1 = 3
                1024,
            )
            .await
            .unwrap();
        snap = writer.create_snapshot(None, None).await.unwrap();
        store.commit_writer(snap.clone());
        eprintln!("Cycle 2: CHECKPOINT completed, registered consolidated file (id={}) with {} rows", fid, expected_total_rows);
        fid
    };
    file_ids.push(file2_id);

    // Verify after cycle 2: Should NOT duplicate
    {
        let reader = store.read_at(snap.clone()).unwrap();
        let files = reader.list_data_files(table_id).await.unwrap();
        let total: u64 = files.iter().map(|f| f.record_count).sum();
        eprintln!("Cycle 2 verified: {} files, {} total rows", files.len(), total);
        
        eprintln!("  Files:");
        for f in &files {
            eprintln!("    id={}, rows={}, begin={:?}, end={:?}", 
                f.data_file_id, f.record_count, f.begin_snapshot, f.end_snapshot);
        }
        
        // Should see only the consolidated file (file2), not file1
        assert_eq!(files.len(), 1, "After cycle 2: should have only 1 file (consolidated)");
        assert_eq!(files[0].data_file_id, file2_id, "Should see consolidated file2");
        
        // Total rows should be 3 (not 2+3=5 from duplication)
        assert_eq!(total, expected_total_rows, 
            "Should have {} rows after cycle 2 (no duplication). \
             File1 should be invisible due to end_snapshot marking", 
            expected_total_rows);
    }

    store.close().await.unwrap();
}
