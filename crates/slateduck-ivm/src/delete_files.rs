//! Delete-file support for IVM.
//!
//! Input source emits `(row, -1)` updates for rows covered by delete files.
//! Aggregations correctly subtract deleted rows.

use std::collections::HashMap;

use serde_json::Value;

/// A delete event from a DuckLake delete file.
#[derive(Debug, Clone)]
pub struct DeleteEvent {
    /// The row being deleted (column → value).
    pub row: HashMap<String, Value>,
    /// The row_id or file + offset identifying the deleted row.
    pub row_id: String,
}

/// Result of processing delete files.
#[derive(Debug, Clone)]
pub struct DeleteBatch {
    /// Rows to retract (weight = -1).
    pub retractions: Vec<HashMap<String, Value>>,
    /// Whether a full refresh is recommended (large delete campaign).
    pub recommend_full_refresh: bool,
    /// Number of delete files processed.
    pub files_processed: u32,
}

/// Threshold above which we recommend a REFRESH FULL for non-monoidal aggregates.
const LARGE_DELETE_THRESHOLD: usize = 10_000;

/// Process delete files and produce retraction events.
///
/// Each deleted row is emitted as a `(row, -1)` weight event suitable for
/// the IVM circuit.
pub fn process_delete_files(
    deleted_rows: Vec<HashMap<String, Value>>,
    files_processed: u32,
) -> DeleteBatch {
    let recommend_full = deleted_rows.len() > LARGE_DELETE_THRESHOLD;

    if recommend_full {
        tracing::warn!(
            "Large delete campaign ({} rows from {} files): REFRESH ... FULL recommended for non-monoidal aggregates",
            deleted_rows.len(),
            files_processed,
        );
    }

    DeleteBatch {
        retractions: deleted_rows,
        recommend_full_refresh: recommend_full,
        files_processed,
    }
}

/// Check if a delete batch can be handled incrementally for a given aggregate type.
///
/// Monoidal (algebraic) aggregates can always handle deletes incrementally.
/// Non-monoidal aggregates (STRING_AGG, ARRAY_AGG) need REFRESH FULL for large campaigns.
pub fn can_handle_incrementally(aggregate_tier: &str, delete_count: usize) -> bool {
    match aggregate_tier {
        "Algebraic" => true,
        "SemiAlgebraic" => true, // Rescan on extremum delete, but still incremental.
        "GroupRescan" => delete_count <= LARGE_DELETE_THRESHOLD,
        _ => delete_count <= LARGE_DELETE_THRESHOLD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_delete_batch() {
        let rows: Vec<HashMap<String, Value>> = (0..5)
            .map(|i| {
                let mut row = HashMap::new();
                row.insert("id".to_string(), Value::from(i));
                row
            })
            .collect();

        let batch = process_delete_files(rows, 1);
        assert_eq!(batch.retractions.len(), 5);
        assert!(!batch.recommend_full_refresh);
    }

    #[test]
    fn large_delete_recommends_full_refresh() {
        let rows: Vec<HashMap<String, Value>> = (0..10_001)
            .map(|i| {
                let mut row = HashMap::new();
                row.insert("id".to_string(), Value::from(i));
                row
            })
            .collect();

        let batch = process_delete_files(rows, 10);
        assert!(batch.recommend_full_refresh);
    }

    #[test]
    fn incremental_handling_by_tier() {
        assert!(can_handle_incrementally("Algebraic", 100_000));
        assert!(can_handle_incrementally("SemiAlgebraic", 100_000));
        assert!(can_handle_incrementally("GroupRescan", 5_000));
        assert!(!can_handle_incrementally("GroupRescan", 20_000));
    }
}
