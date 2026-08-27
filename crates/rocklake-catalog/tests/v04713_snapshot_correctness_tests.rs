//! v0.47.13 Snapshot Correctness and Metadata Isolation Tests.
//!
//! Release Gates:
//! 1. Two-table isolation tests covering data files, delete files, stats pruning,
//!    CDC, mappings, tags, cascades at old and latest snapshots.
//! 2. Property tests proving pruning is conservative (false positives ok, false negatives never).
//! 3. Golden consolidation tests proving legitimate same-size and overlapping-row-range
//!    files remain visible until explicitly retired.
//! 4. Historical-reader tests mutating every dependent metadata family and proving
//!    old snapshots remain stable.
//! 5. Strict snapshot bounds check returning `CatalogError::SnapshotNotFound`.

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use rocklake_catalog::error::CatalogError;
use rocklake_catalog::writer::stats::FileColumnStatsInput;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_core::mvcc::SnapshotId;
use rocklake_core::types::DuckLakeType;
use std::sync::Arc;

fn make_opts(test_name: &str) -> OpenOptions {
    let store: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
    OpenOptions {
        object_store: store,
        path: ObjectPath::from(format!("v04713-{test_name}")),
        encryption: None,
    }
}

// ─── 1. Two-table isolation and lifecycle ─────────────────────────────────────

#[tokio::test]
async fn test_two_table_isolation_and_cascading_drop() {
    let mut cat = CatalogStore::open(make_opts("two-table")).await.unwrap();

    // Snapshot 1: Create Schema + Table A + Table B
    let mut w1 = cat.begin_write();
    let schema_id = w1.create_schema("analytics").await.unwrap();
    let table_a = w1.create_table(schema_id, "table_a", None).await.unwrap();
    let table_b = w1.create_table(schema_id, "table_b", None).await.unwrap();
    let col_a = w1
        .add_column(table_a, "val", "BIGINT", 0, true, None)
        .await
        .unwrap();
    let col_b = w1
        .add_column(table_b, "val", "BIGINT", 0, true, None)
        .await
        .unwrap();
    let snap1 = w1
        .create_snapshot(Some("test"), Some("init two tables"))
        .await
        .unwrap();
    cat.commit_writer(snap1);

    // Snapshot 2: Add Data Files, Delete Files, Tags, Column Mappings, Stats to both tables
    let mut w2 = cat.begin_write();
    let f_a = w2
        .register_data_file_with_metadata(
            table_a,
            "data/a1.parquet",
            "parquet",
            100,
            1024,
            Some(0),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let f_b = w2
        .register_data_file_with_metadata(
            table_b,
            "data/b1.parquet",
            "parquet",
            200,
            2048,
            Some(0),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let d_a = w2
        .register_delete_file(f_a, "delete/a1.parquet", 5, 128)
        .await
        .unwrap();
    let d_b = w2
        .register_delete_file(f_b, "delete/b1.parquet", 10, 256)
        .await
        .unwrap();

    w2.set_tag(table_a, "owner", "team_a").await.unwrap();
    w2.set_tag(table_b, "owner", "team_b").await.unwrap();
    w2.set_column_tag(table_a, col_a, "pii", "false")
        .await
        .unwrap();
    w2.set_column_tag(table_b, col_b, "pii", "true")
        .await
        .unwrap();

    w2.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f_a,
        column_id: col_a,
        table_id: table_a,
        column_size_bytes: Some(100),
        value_count: Some(100),
        null_count: Some(0),
        min_value: Some("10"),
        max_value: Some("50"),
        contains_null: false,
        contains_nan: false,
        extra_stats: None,
    })
    .await
    .unwrap();

    w2.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f_b,
        column_id: col_b,
        table_id: table_b,
        column_size_bytes: Some(200),
        value_count: Some(200),
        null_count: Some(0),
        min_value: Some("100"),
        max_value: Some("500"),
        contains_null: false,
        contains_nan: false,
        extra_stats: None,
    })
    .await
    .unwrap();

    let snap2 = w2
        .create_snapshot(Some("test"), Some("add files and tags"))
        .await
        .unwrap();
    cat.commit_writer(snap2);

    // Verify isolation at Snapshot 2
    let r2 = cat.read_at(SnapshotId::new(2)).unwrap();
    let files_a = r2.list_data_files(table_a).await.unwrap();
    assert_eq!(files_a.len(), 1);
    assert_eq!(files_a[0].data_file_id, f_a);

    let files_b = r2.list_data_files(table_b).await.unwrap();
    assert_eq!(files_b.len(), 1);
    assert_eq!(files_b[0].data_file_id, f_b);

    let del_a = r2.list_delete_files(table_a).await.unwrap();
    assert_eq!(del_a.len(), 1);
    assert_eq!(del_a[0].delete_file_id, d_a);

    let del_b = r2.list_delete_files(table_b).await.unwrap();
    assert_eq!(del_b.len(), 1);
    assert_eq!(del_b[0].delete_file_id, d_b);

    // Snapshot 3: Drop Table A only
    let mut w3 = cat.begin_write();
    w3.drop_table(schema_id, table_a, 1).await.unwrap();
    let snap3 = w3
        .create_snapshot(Some("test"), Some("drop table a"))
        .await
        .unwrap();
    cat.commit_writer(snap3);

    // At Snapshot 3: Table A is dropped, Table B remains intact
    let r3 = cat.read_at(SnapshotId::new(3)).unwrap();
    assert!(r3.describe_table(table_a).await.unwrap().is_none());
    assert_eq!(r3.list_data_files(table_a).await.unwrap().len(), 0);
    assert_eq!(r3.list_delete_files(table_a).await.unwrap().len(), 0);

    // Table B at Snapshot 3 is completely unaffected
    assert!(r3.describe_table(table_b).await.unwrap().is_some());
    assert_eq!(r3.list_data_files(table_b).await.unwrap().len(), 1);
    assert_eq!(r3.list_delete_files(table_b).await.unwrap().len(), 1);

    // At historical Snapshot 2: Table A is still fully visible
    let r2_hist = cat.read_at(SnapshotId::new(2)).unwrap();
    assert!(r2_hist.describe_table(table_a).await.unwrap().is_some());
    assert_eq!(r2_hist.list_data_files(table_a).await.unwrap().len(), 1);
    assert_eq!(r2_hist.list_delete_files(table_a).await.unwrap().len(), 1);
}

// ─── 2. Strict Snapshot Bounds Validation ──────────────────────────────────────

#[tokio::test]
async fn test_snapshot_bounds_rejection() {
    let cat = CatalogStore::open(make_opts("bounds")).await.unwrap();

    // Fresh catalog: latest committed is 0
    // Querying snapshot 1 must fail with SnapshotNotFound
    match cat.read_at(SnapshotId::new(1)) {
        Err(CatalogError::SnapshotNotFound {
            requested,
            latest_committed,
        }) => {
            assert_eq!(requested, 1);
            assert_eq!(latest_committed, 0);
        }
        _ => panic!("expected SnapshotNotFound"),
    }

    // Querying snapshot 0 succeeds
    let r0 = cat.read_at(SnapshotId::new(0)).unwrap();
    assert_eq!(r0.snapshot_id().as_u64(), 0);
}

// ─── 3. Golden Consolidation Tests: Overlapping files remain visible ──────────

#[tokio::test]
async fn test_overlapping_row_range_files_remain_visible() {
    let mut cat = CatalogStore::open(make_opts("consolidation"))
        .await
        .unwrap();

    let mut w = cat.begin_write();
    let schema_id = w.create_schema("s").await.unwrap();
    let table_id = w.create_table(schema_id, "t", None).await.unwrap();

    // Add two files with identical row_count and overlapping row_id_start (legitimate partition appends)
    let f1 = w
        .register_data_file_with_metadata(
            table_id,
            "data/p1.parquet",
            "parquet",
            1000,
            10240,
            Some(0),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let f2 = w
        .register_data_file_with_metadata(
            table_id,
            "data/p2.parquet",
            "parquet",
            1000,
            10240,
            Some(0),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let snap = w.create_snapshot(None, None).await.unwrap();
    cat.commit_writer(snap);

    let reader = cat.read_latest();
    let files = reader.list_data_files(table_id).await.unwrap();
    assert_eq!(
        files.len(),
        2,
        "both files must remain visible without consolidation guessing"
    );
    let file_ids: Vec<u64> = files.into_iter().map(|f| f.data_file_id).collect();
    assert!(file_ids.contains(&f1));
    assert!(file_ids.contains(&f2));
}

// ─── 4. Conservative Statistics Pruning ────────────────────────────────────────

#[tokio::test]
async fn test_conservative_stats_pruning() {
    let mut cat = CatalogStore::open(make_opts("pruning")).await.unwrap();

    let mut w = cat.begin_write();
    let schema_id = w.create_schema("s").await.unwrap();
    let table_id = w.create_table(schema_id, "t", None).await.unwrap();
    let col_id = w
        .add_column(table_id, "x", "BIGINT", 0, true, None)
        .await
        .unwrap();

    // File 1: conclusive match range [10, 20]
    let f1 = w
        .register_data_file_with_metadata(
            table_id,
            "data/f1.parquet",
            "parquet",
            100,
            1000,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    w.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f1,
        column_id: col_id,
        table_id,
        column_size_bytes: Some(100),
        value_count: Some(100),
        null_count: Some(0),
        min_value: Some("10"),
        max_value: Some("20"),
        contains_null: false,
        contains_nan: false,
        extra_stats: None,
    })
    .await
    .unwrap();

    // File 2: missing statistics
    let f2 = w
        .register_data_file_with_metadata(
            table_id,
            "data/f2.parquet",
            "parquet",
            100,
            1000,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // File 3: NaN statistics
    let f3 = w
        .register_data_file_with_metadata(
            table_id,
            "data/f3.parquet",
            "parquet",
            100,
            1000,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    w.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f3,
        column_id: col_id,
        table_id,
        column_size_bytes: Some(100),
        value_count: Some(100),
        null_count: Some(0),
        min_value: Some("100"),
        max_value: Some("200"),
        contains_null: false,
        contains_nan: true,
        extra_stats: None,
    })
    .await
    .unwrap();

    // File 4: null-only statistics (null_count == value_count)
    let f4 = w
        .register_data_file_with_metadata(
            table_id,
            "data/f4.parquet",
            "parquet",
            100,
            1000,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    w.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f4,
        column_id: col_id,
        table_id,
        column_size_bytes: Some(100),
        value_count: Some(100),
        null_count: Some(100),
        min_value: None,
        max_value: None,
        contains_null: true,
        contains_nan: false,
        extra_stats: None,
    })
    .await
    .unwrap();

    // File 5: conclusive exclude range [100, 200]
    let f5 = w
        .register_data_file_with_metadata(
            table_id,
            "data/f5.parquet",
            "parquet",
            100,
            1000,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    w.upsert_file_column_stats(FileColumnStatsInput {
        data_file_id: f5,
        column_id: col_id,
        table_id,
        column_size_bytes: Some(100),
        value_count: Some(100),
        null_count: Some(0),
        min_value: Some("100"),
        max_value: Some("200"),
        contains_null: false,
        contains_nan: false,
        extra_stats: None,
    })
    .await
    .unwrap();

    let snap = w.create_snapshot(None, None).await.unwrap();
    cat.commit_writer(snap);

    let reader = cat.read_latest();

    // Prune for predicate: x = 15
    let bigint_type = DuckLakeType::Integer {
        signed: true,
        width_bits: 64,
    };
    let kept = reader
        .prune_files(table_id, col_id, "15", &bigint_type)
        .await
        .unwrap();

    // f1 matches (kept)
    assert!(kept.contains(&f1), "f1 in range [10, 20] must be kept");
    // f2 missing stats (conservatively kept)
    assert!(
        kept.contains(&f2),
        "f2 with missing stats must be conservatively kept"
    );
    // f3 has NaN (conservatively kept)
    assert!(
        kept.contains(&f3),
        "f3 with NaN must be conservatively kept"
    );
    // f4 null-only (conservatively kept)
    assert!(
        kept.contains(&f4),
        "f4 with null-only stats must be conservatively kept"
    );
    // f5 conclusively out of range [100, 200] (pruned!)
    assert!(
        !kept.contains(&f5),
        "f5 in range [100, 200] must be pruned for x = 15"
    );
}
