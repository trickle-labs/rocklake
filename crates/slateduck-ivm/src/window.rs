//! Window function support for IVM.
//!
//! Implements incremental maintenance of window functions:
//! `ROW_NUMBER`, `RANK`, `DENSE_RANK`, `PERCENT_RANK`, `CUME_DIST`, `NTILE`,
//! `LAG`, `LEAD`, `FIRST_VALUE`, `LAST_VALUE`, `NTH_VALUE`,
//! and all aggregate windows (`SUM/AVG/COUNT OVER (PARTITION BY … ORDER BY …)`).
//!
//! ## Design
//! Partition-local windows (PARTITION BY = shard key) are fully parallel.
//! Cross-partition or full-table windows require a single-shard merge stage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Window function kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowFunction {
    RowNumber,
    Rank,
    DenseRank,
    PercentRank,
    CumeDist,
    Ntile(u64),
    Lag(u64),
    Lead(u64),
    FirstValue,
    LastValue,
    NthValue(u64),
    /// Aggregate window: SUM/AVG/COUNT/MIN/MAX over a frame.
    AggregateWindow(AggregateWindowKind),
}

/// Aggregate window function kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateWindowKind {
    Sum,
    Avg,
    Count,
    Min,
    Max,
}

/// Window frame specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowFrame {
    /// `ROWS BETWEEN <start> AND <end>`
    Rows { start: FrameBound, end: FrameBound },
    /// `RANGE BETWEEN <start> AND <end>`
    Range { start: FrameBound, end: FrameBound },
    /// `GROUPS BETWEEN <start> AND <end>`
    Groups { start: FrameBound, end: FrameBound },
}

impl Default for WindowFrame {
    fn default() -> Self {
        Self::Rows {
            start: FrameBound::UnboundedPreceding,
            end: FrameBound::CurrentRow,
        }
    }
}

/// Frame boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameBound {
    UnboundedPreceding,
    Preceding(u64),
    CurrentRow,
    Following(u64),
    UnboundedFollowing,
}

/// Window execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowMode {
    /// Partition key matches shard key — fully parallel.
    Partitioned,
    /// Total-order mode — single shard merge stage required.
    TotalOrder,
}

/// Window operator specification extracted from the SQL plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSpec {
    /// The window function to compute.
    pub function: WindowFunction,
    /// Partition-by columns (may be empty for full-table windows).
    pub partition_by: Vec<String>,
    /// Order-by columns with direction.
    pub order_by: Vec<OrderByColumn>,
    /// Frame specification.
    pub frame: WindowFrame,
    /// Output column name.
    pub output_col: String,
    /// Execution mode: auto-detected from partition key vs shard key.
    pub mode: WindowMode,
    /// Input column for aggregate windows or navigation functions.
    pub input_col: Option<String>,
}

/// ORDER BY column with direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderByColumn {
    pub column: String,
    pub descending: bool,
    pub nulls_first: bool,
}

/// Per-partition state for incremental window maintenance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartitionState {
    /// Sorted rows within this partition (sort key → row data).
    pub rows: BTreeMap<Vec<u8>, Vec<Value>>,
    /// Number of rows in this partition.
    pub row_count: u64,
}

/// Window evaluator: maintains per-partition state and computes window outputs.
#[derive(Debug, Clone)]
pub struct WindowEvaluator {
    pub spec: WindowSpec,
    /// Per-partition state keyed by partition key values.
    pub partitions: BTreeMap<Vec<u8>, PartitionState>,
}

impl WindowEvaluator {
    /// Create a new evaluator for the given window spec.
    pub fn new(spec: WindowSpec) -> Self {
        Self {
            spec,
            partitions: BTreeMap::new(),
        }
    }

    /// Insert a row into the appropriate partition.
    pub fn insert_row(&mut self, partition_key: Vec<u8>, sort_key: Vec<u8>, row: Vec<Value>) {
        let partition = self.partitions.entry(partition_key).or_default();
        partition.rows.insert(sort_key, row);
        partition.row_count += 1;
    }

    /// Remove a row from the appropriate partition.
    pub fn remove_row(&mut self, partition_key: &[u8], sort_key: &[u8]) -> bool {
        if let Some(partition) = self.partitions.get_mut(partition_key) {
            if partition.rows.remove(sort_key).is_some() {
                partition.row_count -= 1;
                if partition.row_count == 0 {
                    self.partitions.remove(partition_key);
                }
                return true;
            }
        }
        false
    }

    /// Compute the window function result for all rows in a partition.
    pub fn evaluate_partition(&self, partition_key: &[u8]) -> Vec<(Vec<u8>, Value)> {
        let Some(partition) = self.partitions.get(partition_key) else {
            return Vec::new();
        };

        let rows: Vec<(&Vec<u8>, &Vec<Value>)> = partition.rows.iter().collect();
        let count = rows.len();
        let mut results = Vec::with_capacity(count);

        for (idx, (sort_key, _row)) in rows.iter().enumerate() {
            let value = match &self.spec.function {
                WindowFunction::RowNumber => Value::Number((idx as u64 + 1).into()),
                WindowFunction::Rank => {
                    // For rank, find position of first row with same sort key
                    let rank = rows
                        .iter()
                        .position(|(sk, _)| sk == sort_key)
                        .unwrap_or(idx)
                        + 1;
                    Value::Number((rank as u64).into())
                }
                WindowFunction::DenseRank => {
                    // Count distinct sort keys up to and including current
                    let mut seen = std::collections::BTreeSet::new();
                    for (sk, _) in rows.iter().take(idx + 1) {
                        seen.insert(*sk);
                    }
                    Value::Number((seen.len() as u64).into())
                }
                WindowFunction::PercentRank => {
                    if count <= 1 {
                        Value::Number(serde_json::Number::from_f64(0.0).unwrap())
                    } else {
                        let rank = rows
                            .iter()
                            .position(|(sk, _)| sk == sort_key)
                            .unwrap_or(idx) as f64;
                        let pct = rank / (count - 1) as f64;
                        Value::Number(serde_json::Number::from_f64(pct).unwrap())
                    }
                }
                WindowFunction::CumeDist => {
                    let num_leq = rows.iter().filter(|(sk, _)| *sk <= *sort_key).count() as f64;
                    let cd = num_leq / count as f64;
                    Value::Number(serde_json::Number::from_f64(cd).unwrap())
                }
                WindowFunction::Ntile(n) => {
                    let bucket = ((idx as u64) * n / count as u64) + 1;
                    Value::Number(bucket.into())
                }
                WindowFunction::Lag(offset) => {
                    let offset = *offset as usize;
                    if idx >= offset {
                        rows[idx - offset].1.first().cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                WindowFunction::Lead(offset) => {
                    let offset = *offset as usize;
                    if idx + offset < count {
                        rows[idx + offset].1.first().cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                WindowFunction::FirstValue => rows
                    .first()
                    .map(|(_, v)| v.first().cloned().unwrap_or(Value::Null))
                    .unwrap_or(Value::Null),
                WindowFunction::LastValue => rows
                    .last()
                    .map(|(_, v)| v.first().cloned().unwrap_or(Value::Null))
                    .unwrap_or(Value::Null),
                WindowFunction::NthValue(n) => {
                    let n = *n as usize;
                    if n > 0 && n <= count {
                        rows[n - 1].1.first().cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                WindowFunction::AggregateWindow(kind) => {
                    compute_aggregate_window(kind, &rows, idx, &self.spec.frame)
                }
            };
            results.push((sort_key.to_vec(), value));
        }

        results
    }

    /// Get the total number of partitions.
    pub fn partition_count(&self) -> usize {
        self.partitions.len()
    }

    /// Get total row count across all partitions.
    pub fn total_rows(&self) -> u64 {
        self.partitions.values().map(|p| p.row_count).sum()
    }
}

/// Compute an aggregate window value for the given row index within the frame.
fn compute_aggregate_window(
    kind: &AggregateWindowKind,
    rows: &[(&Vec<u8>, &Vec<Value>)],
    current_idx: usize,
    frame: &WindowFrame,
) -> Value {
    let (start, end) = resolve_frame_bounds(frame, current_idx, rows.len());

    let values: Vec<f64> = rows[start..=end]
        .iter()
        .filter_map(|(_, row)| row.first())
        .filter_map(|v| match v {
            Value::Number(n) => n.as_f64(),
            _ => None,
        })
        .collect();

    if values.is_empty() {
        return Value::Null;
    }

    match kind {
        AggregateWindowKind::Sum => {
            let sum: f64 = values.iter().sum();
            Value::Number(serde_json::Number::from_f64(sum).unwrap())
        }
        AggregateWindowKind::Avg => {
            let sum: f64 = values.iter().sum();
            let avg = sum / values.len() as f64;
            Value::Number(serde_json::Number::from_f64(avg).unwrap())
        }
        AggregateWindowKind::Count => Value::Number((values.len() as u64).into()),
        AggregateWindowKind::Min => {
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            Value::Number(serde_json::Number::from_f64(min).unwrap())
        }
        AggregateWindowKind::Max => {
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Value::Number(serde_json::Number::from_f64(max).unwrap())
        }
    }
}

/// Resolve frame bounds to concrete row indices.
fn resolve_frame_bounds(frame: &WindowFrame, current: usize, total: usize) -> (usize, usize) {
    let (start_bound, end_bound) = match frame {
        WindowFrame::Rows { start, end }
        | WindowFrame::Range { start, end }
        | WindowFrame::Groups { start, end } => (start, end),
    };

    let start = match start_bound {
        FrameBound::UnboundedPreceding => 0,
        FrameBound::Preceding(n) => current.saturating_sub(*n as usize),
        FrameBound::CurrentRow => current,
        FrameBound::Following(n) => (current + *n as usize).min(total - 1),
        FrameBound::UnboundedFollowing => total - 1,
    };

    let end = match end_bound {
        FrameBound::UnboundedPreceding => 0,
        FrameBound::Preceding(n) => current.saturating_sub(*n as usize),
        FrameBound::CurrentRow => current,
        FrameBound::Following(n) => (current + *n as usize).min(total - 1),
        FrameBound::UnboundedFollowing => total - 1,
    };

    (start, end)
}

/// Validate window specification for IVM compatibility.
pub fn validate_window_spec(spec: &WindowSpec, shard_count: u32) -> Result<(), WindowError> {
    // Full-table windows require shard_count = 1
    if spec.partition_by.is_empty() && shard_count > 1 {
        return Err(WindowError::FullTableWindowRequiresSingleShard);
    }

    // Total-order mode requires shard_count = 1
    if spec.mode == WindowMode::TotalOrder && shard_count > 1 {
        return Err(WindowError::TotalOrderRequiresSingleShard);
    }

    Ok(())
}

/// Errors from window operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WindowError {
    #[error("full-table window (no PARTITION BY) requires shard_count = 1")]
    FullTableWindowRequiresSingleShard,
    #[error("total-order window mode requires shard_count = 1")]
    TotalOrderRequiresSingleShard,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_number_basic() {
        let spec = WindowSpec {
            function: WindowFunction::RowNumber,
            partition_by: vec!["dept".to_string()],
            order_by: vec![OrderByColumn {
                column: "salary".to_string(),
                descending: false,
                nulls_first: false,
            }],
            frame: WindowFrame::default(),
            output_col: "rn".to_string(),
            mode: WindowMode::Partitioned,
            input_col: None,
        };

        let mut eval = WindowEvaluator::new(spec);
        let pk = b"engineering".to_vec();
        eval.insert_row(pk.clone(), vec![0, 1], vec![Value::Number(50000.into())]);
        eval.insert_row(pk.clone(), vec![0, 2], vec![Value::Number(60000.into())]);
        eval.insert_row(pk.clone(), vec![0, 3], vec![Value::Number(70000.into())]);

        let results = eval.evaluate_partition(&pk);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].1, Value::Number(1.into()));
        assert_eq!(results[1].1, Value::Number(2.into()));
        assert_eq!(results[2].1, Value::Number(3.into()));
    }

    #[test]
    fn lag_lead_functions() {
        let spec = WindowSpec {
            function: WindowFunction::Lag(1),
            partition_by: vec!["id".to_string()],
            order_by: vec![OrderByColumn {
                column: "ts".to_string(),
                descending: false,
                nulls_first: false,
            }],
            frame: WindowFrame::default(),
            output_col: "prev_val".to_string(),
            mode: WindowMode::Partitioned,
            input_col: Some("value".to_string()),
        };

        let mut eval = WindowEvaluator::new(spec);
        let pk = b"user1".to_vec();
        eval.insert_row(pk.clone(), vec![1], vec![Value::Number(10.into())]);
        eval.insert_row(pk.clone(), vec![2], vec![Value::Number(20.into())]);
        eval.insert_row(pk.clone(), vec![3], vec![Value::Number(30.into())]);

        let results = eval.evaluate_partition(&pk);
        assert_eq!(results[0].1, Value::Null); // No previous row
        assert_eq!(results[1].1, Value::Number(10.into()));
        assert_eq!(results[2].1, Value::Number(20.into()));
    }

    #[test]
    fn validate_full_table_window_rejects_multi_shard() {
        let spec = WindowSpec {
            function: WindowFunction::RowNumber,
            partition_by: vec![], // No partition = full-table
            order_by: vec![],
            frame: WindowFrame::default(),
            output_col: "rn".to_string(),
            mode: WindowMode::Partitioned,
            input_col: None,
        };
        assert_eq!(
            validate_window_spec(&spec, 4),
            Err(WindowError::FullTableWindowRequiresSingleShard)
        );
        assert!(validate_window_spec(&spec, 1).is_ok());
    }
}
