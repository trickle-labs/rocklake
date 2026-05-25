//! Ordered trace: extends the base trace with per-partition sort order.
//!
//! `SlateDbOrderedTrace` layers per-partition sort keys on top of the existing
//! persistence layer (v0.15's `SlateDbTrace` or the `SlateDbBatch` adapter).
//! Key layout: `{state_prefix}/ordered/{partition_key}/{sequence}`.
//!
//! Used for: total-order output (ORDER BY), window functions, top-N.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for an ordered trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderedTraceConfig {
    /// State key prefix for this trace.
    pub state_prefix: String,
    /// Sort keys defining the output order.
    pub sort_keys: Vec<SortKey>,
    /// Whether the output is total-ordered (shard_count must be 1).
    pub total_order: bool,
}

/// A single sort key column with direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub column: String,
    pub descending: bool,
    pub nulls_first: bool,
}

/// Per-partition ordered state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderedPartition {
    /// Sorted entries: encoded sort key → row payload.
    pub entries: BTreeMap<Vec<u8>, Vec<Value>>,
}

/// The ordered trace implementation.
#[derive(Debug, Clone)]
pub struct SlateDbOrderedTrace {
    pub config: OrderedTraceConfig,
    /// Per-partition ordered state.
    pub partitions: BTreeMap<Vec<u8>, OrderedPartition>,
    /// Total row count across all partitions.
    pub total_rows: u64,
}

impl SlateDbOrderedTrace {
    /// Create a new ordered trace with the given configuration.
    pub fn new(config: OrderedTraceConfig) -> Self {
        Self {
            config,
            partitions: BTreeMap::new(),
            total_rows: 0,
        }
    }

    /// Insert a row into the ordered trace.
    pub fn insert(&mut self, partition_key: Vec<u8>, sort_key: Vec<u8>, row: Vec<Value>) {
        let partition = self.partitions.entry(partition_key).or_default();
        partition.entries.insert(sort_key, row);
        self.total_rows += 1;
    }

    /// Remove a row from the ordered trace.
    pub fn remove(&mut self, partition_key: &[u8], sort_key: &[u8]) -> Option<Vec<Value>> {
        if let Some(partition) = self.partitions.get_mut(partition_key) {
            if let Some(row) = partition.entries.remove(sort_key) {
                self.total_rows -= 1;
                if partition.entries.is_empty() {
                    self.partitions.remove(partition_key);
                }
                return Some(row);
            }
        }
        None
    }

    /// Read all rows in the declared sort order (for a specific partition).
    pub fn read_partition_ordered(&self, partition_key: &[u8]) -> Vec<&Vec<Value>> {
        self.partitions
            .get(partition_key)
            .map(|p| p.entries.values().collect())
            .unwrap_or_default()
    }

    /// Read all rows across all partitions in global sort order.
    /// Only valid when `total_order = true` (single partition expected).
    pub fn read_all_ordered(&self) -> Vec<&Vec<Value>> {
        self.partitions
            .values()
            .flat_map(|p| p.entries.values())
            .collect()
    }

    /// Get the top-N rows globally (for LIMIT queries).
    pub fn top_n(&self, n: usize) -> Vec<&Vec<Value>> {
        self.read_all_ordered().into_iter().take(n).collect()
    }

    /// Get top-N with offset (for LIMIT/OFFSET queries).
    pub fn top_n_offset(&self, limit: usize, offset: usize) -> Vec<&Vec<Value>> {
        self.read_all_ordered()
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Encode a sort key from row values according to the configured sort keys.
    /// Returns a byte vector suitable for BTreeMap ordering.
    pub fn encode_sort_key(&self, row: &[Value]) -> Vec<u8> {
        let mut key = Vec::new();
        for (i, sk) in self.config.sort_keys.iter().enumerate() {
            let val = row.get(i).unwrap_or(&Value::Null);
            encode_value_for_sort(&mut key, val, sk.descending, sk.nulls_first);
        }
        key
    }

    /// Get the total number of rows.
    pub fn len(&self) -> u64 {
        self.total_rows
    }

    /// Check if the trace is empty.
    pub fn is_empty(&self) -> bool {
        self.total_rows == 0
    }
}

/// Encode a JSON value into a byte sequence suitable for lexicographic ordering.
fn encode_value_for_sort(buf: &mut Vec<u8>, val: &Value, descending: bool, nulls_first: bool) {
    // NULL handling
    if val.is_null() {
        if nulls_first {
            buf.push(if descending { 0xFF } else { 0x00 });
        } else {
            buf.push(if descending { 0x00 } else { 0xFF });
        }
        return;
    }

    // Non-null marker
    buf.push(0x01);

    let raw = match val {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            f.to_be_bytes().to_vec()
        }
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Bool(b) => vec![if *b { 1 } else { 0 }],
        _ => serde_json::to_vec(val).unwrap_or_default(),
    };

    if descending {
        // Invert bytes for descending order
        for b in &raw {
            buf.push(!b);
        }
    } else {
        buf.extend_from_slice(&raw);
    }

    // Terminator
    buf.push(0x00);
}

/// Merge-sorted Parquet writer configuration for total-ordered output tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSortedWriterConfig {
    /// Output path for the Parquet file.
    pub output_path: String,
    /// Sorting columns metadata (for Parquet `sorting_columns`).
    pub sorting_columns: Vec<SortKey>,
    /// Target row group size.
    pub row_group_size: usize,
}

impl Default for MergeSortedWriterConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            sorting_columns: Vec::new(),
            row_group_size: 1_000_000,
        }
    }
}

/// Result of a merge-sorted write operation.
#[derive(Debug, Clone)]
pub struct MergeSortedWriteResult {
    /// Number of rows written.
    pub rows_written: u64,
    /// Number of row groups.
    pub row_groups: u32,
    /// File size in bytes.
    pub file_size_bytes: u64,
}

/// Write rows from the ordered trace as a merge-sorted Parquet file.
/// In a real implementation this would produce actual Parquet;
/// here we compute the metadata that would be produced.
pub fn write_merge_sorted(
    trace: &SlateDbOrderedTrace,
    config: &MergeSortedWriterConfig,
) -> MergeSortedWriteResult {
    let total_rows = trace.len();
    let row_groups = if config.row_group_size > 0 {
        (total_rows as usize).div_ceil(config.row_group_size) as u32
    } else {
        1
    };
    // Estimate ~100 bytes per row for size calculation
    let file_size_bytes = total_rows * 100;

    MergeSortedWriteResult {
        rows_written: total_rows,
        row_groups,
        file_size_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_trace_insert_and_read() {
        let config = OrderedTraceConfig {
            state_prefix: "test".to_string(),
            sort_keys: vec![SortKey {
                column: "id".to_string(),
                descending: false,
                nulls_first: false,
            }],
            total_order: true,
        };

        let mut trace = SlateDbOrderedTrace::new(config);
        let pk = b"".to_vec(); // single partition for total order

        trace.insert(pk.clone(), vec![0, 3], vec![Value::Number(30.into())]);
        trace.insert(pk.clone(), vec![0, 1], vec![Value::Number(10.into())]);
        trace.insert(pk.clone(), vec![0, 2], vec![Value::Number(20.into())]);

        let rows = trace.read_all_ordered();
        assert_eq!(rows.len(), 3);
        // BTreeMap sorts by key, so [0,1] < [0,2] < [0,3]
        assert_eq!(rows[0], &vec![Value::Number(10.into())]);
        assert_eq!(rows[1], &vec![Value::Number(20.into())]);
        assert_eq!(rows[2], &vec![Value::Number(30.into())]);
    }

    #[test]
    fn top_n_returns_first_n() {
        let config = OrderedTraceConfig {
            state_prefix: "test".to_string(),
            sort_keys: vec![SortKey {
                column: "value".to_string(),
                descending: false,
                nulls_first: false,
            }],
            total_order: true,
        };

        let mut trace = SlateDbOrderedTrace::new(config);
        let pk = b"".to_vec();

        for i in 0..100u64 {
            trace.insert(
                pk.clone(),
                i.to_be_bytes().to_vec(),
                vec![Value::Number(i.into())],
            );
        }

        let top5 = trace.top_n(5);
        assert_eq!(top5.len(), 5);
        assert_eq!(top5[0], &vec![Value::Number(0.into())]);
        assert_eq!(top5[4], &vec![Value::Number(4.into())]);
    }

    #[test]
    fn remove_row_from_trace() {
        let config = OrderedTraceConfig {
            state_prefix: "test".to_string(),
            sort_keys: vec![],
            total_order: true,
        };

        let mut trace = SlateDbOrderedTrace::new(config);
        let pk = b"".to_vec();

        trace.insert(pk.clone(), vec![1], vec![Value::Number(10.into())]);
        trace.insert(pk.clone(), vec![2], vec![Value::Number(20.into())]);
        assert_eq!(trace.len(), 2);

        let removed = trace.remove(&pk, &[1]);
        assert_eq!(removed, Some(vec![Value::Number(10.into())]));
        assert_eq!(trace.len(), 1);
    }
}
