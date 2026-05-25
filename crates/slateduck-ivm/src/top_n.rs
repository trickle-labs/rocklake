//! Top-N materialized views: `LIMIT N [OFFSET M]` with incremental maintenance.
//!
//! Implements DBSP-style `top_k` operator: maintains a bounded sorted heap of N
//! candidates across updates. Each shard maintains local top-N; a merge shard
//! selects the global top-N from `shard_count × N` candidates.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ordered_trace::{OrderedTraceConfig, SlateDbOrderedTrace, SortKey};

/// Top-N configuration extracted from the SQL plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopNConfig {
    /// Maximum number of rows to maintain.
    pub limit: u64,
    /// Offset (number of rows to skip). State cost = O(offset + limit).
    pub offset: u64,
    /// Sort keys determining the ordering.
    pub sort_keys: Vec<SortKey>,
    /// Number of shards. Sharded top-N: each shard maintains local top-N;
    /// merge selects global top-N from `shard_count × N` candidates.
    pub shard_count: u32,
}

/// Top-N operator state.
#[derive(Debug, Clone)]
pub struct TopNOperator {
    pub config: TopNConfig,
    /// The underlying ordered trace maintaining sorted state.
    pub trace: SlateDbOrderedTrace,
}

/// Result of a top-N evaluation.
#[derive(Debug, Clone)]
pub struct TopNResult {
    /// The output rows (exactly min(limit, available) rows).
    pub rows: Vec<Vec<Value>>,
    /// Number of candidate rows tracked in state.
    pub state_rows: u64,
}

impl TopNOperator {
    /// Create a new top-N operator.
    pub fn new(config: TopNConfig) -> Self {
        let trace_config = OrderedTraceConfig {
            state_prefix: "topn".to_string(),
            sort_keys: config.sort_keys.clone(),
            total_order: true,
        };
        Self {
            config,
            trace: SlateDbOrderedTrace::new(trace_config),
        }
    }

    /// Insert a row. If the trace exceeds the candidate limit (offset + limit),
    /// the worst candidate is evicted.
    pub fn insert(&mut self, sort_key: Vec<u8>, row: Vec<Value>) {
        let pk = b"".to_vec(); // Single partition for global top-N
        self.trace.insert(pk, sort_key, row);

        // Evict if we exceed the candidate limit
        let max_candidates = self.config.offset + self.config.limit;
        while self.trace.len() > max_candidates {
            // Remove the last (worst) entry
            if let Some(partition) = self.trace.partitions.get_mut(b"".as_slice()) {
                if let Some(last_key) = partition.entries.keys().next_back().cloned() {
                    partition.entries.remove(&last_key);
                    self.trace.total_rows -= 1;
                }
            }
        }
    }

    /// Remove a row from the candidate set.
    pub fn remove(&mut self, sort_key: &[u8]) -> Option<Vec<Value>> {
        self.trace.remove(b"", sort_key)
    }

    /// Get the current top-N result (respecting offset).
    pub fn evaluate(&self) -> TopNResult {
        let rows: Vec<Vec<Value>> = self
            .trace
            .top_n_offset(self.config.limit as usize, self.config.offset as usize)
            .into_iter()
            .cloned()
            .collect();

        TopNResult {
            rows,
            state_rows: self.trace.len(),
        }
    }

    /// Get the number of candidate rows tracked in state.
    pub fn state_rows(&self) -> u64 {
        self.trace.len()
    }

    /// Check if the state is at capacity.
    pub fn at_capacity(&self) -> bool {
        self.trace.len() >= self.config.offset + self.config.limit
    }
}

/// Validate a LIMIT/OFFSET configuration.
pub fn validate_top_n(config: &TopNConfig) -> Result<(), TopNError> {
    if config.sort_keys.is_empty() {
        return Err(TopNError::LimitRequiresOrderBy);
    }
    if config.offset > 0 && config.sort_keys.is_empty() {
        return Err(TopNError::OffsetRequiresOrderByAndLimit);
    }
    Ok(())
}

/// Check if a LIMIT/OFFSET configuration should emit a warning.
pub fn should_warn_offset(offset: u64) -> bool {
    offset > 10000
}

/// Errors from top-N validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TopNError {
    #[error("LIMIT requires ORDER BY; error if absent")]
    LimitRequiresOrderBy,
    #[error("OFFSET requires ORDER BY + LIMIT")]
    OffsetRequiresOrderByAndLimit,
}

/// Merge local top-N results from multiple shards into global top-N.
pub fn merge_shard_top_n(
    shard_results: &[TopNResult],
    limit: u64,
    offset: u64,
    sort_keys: &[SortKey],
) -> TopNResult {
    // Collect all candidates from all shards
    let mut all_rows: Vec<(Vec<u8>, Vec<Value>)> = Vec::new();

    for result in shard_results {
        for row in &result.rows {
            let key = encode_row_sort_key(row, sort_keys);
            all_rows.push((key, row.clone()));
        }
    }

    // Sort by encoded key
    all_rows.sort_by(|(a, _), (b, _)| a.cmp(b));

    // Apply offset and limit
    let rows: Vec<Vec<Value>> = all_rows
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(|(_, row)| row)
        .collect();

    let state_rows = rows.len() as u64;
    TopNResult { rows, state_rows }
}

/// Encode a row's sort key values into a comparable byte vector.
fn encode_row_sort_key(row: &[Value], sort_keys: &[SortKey]) -> Vec<u8> {
    let mut key = Vec::new();
    for (i, sk) in sort_keys.iter().enumerate() {
        let val = row.get(i).unwrap_or(&Value::Null);
        // Simple encoding: serialize to JSON bytes
        let bytes = serde_json::to_vec(val).unwrap_or_default();
        if sk.descending {
            for b in &bytes {
                key.push(!b);
            }
        } else {
            key.extend_from_slice(&bytes);
        }
        key.push(0x00); // separator
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_n_maintains_bounded_state() {
        let config = TopNConfig {
            limit: 5,
            offset: 0,
            sort_keys: vec![SortKey {
                column: "value".to_string(),
                descending: false,
                nulls_first: false,
            }],
            shard_count: 1,
        };

        let mut op = TopNOperator::new(config);

        // Insert 100 rows
        for i in 0..100u64 {
            op.insert(i.to_be_bytes().to_vec(), vec![Value::Number(i.into())]);
        }

        // State should be bounded to limit (5)
        assert_eq!(op.state_rows(), 5);
        assert!(op.at_capacity());

        // Evaluate: should get top 5
        let result = op.evaluate();
        assert_eq!(result.rows.len(), 5);
        assert_eq!(result.rows[0], vec![Value::Number(0.into())]);
        assert_eq!(result.rows[4], vec![Value::Number(4.into())]);
    }

    #[test]
    fn top_n_with_offset() {
        let config = TopNConfig {
            limit: 3,
            offset: 2,
            sort_keys: vec![SortKey {
                column: "id".to_string(),
                descending: false,
                nulls_first: false,
            }],
            shard_count: 1,
        };

        let mut op = TopNOperator::new(config);

        for i in 0..10u64 {
            op.insert(i.to_be_bytes().to_vec(), vec![Value::Number(i.into())]);
        }

        // State bounded to offset + limit = 5
        assert_eq!(op.state_rows(), 5);

        let result = op.evaluate();
        assert_eq!(result.rows.len(), 3);
        // Skip first 2, take 3: [2, 3, 4]
        assert_eq!(result.rows[0], vec![Value::Number(2.into())]);
        assert_eq!(result.rows[2], vec![Value::Number(4.into())]);
    }

    #[test]
    fn validate_limit_requires_order_by() {
        let config = TopNConfig {
            limit: 10,
            offset: 0,
            sort_keys: vec![], // Missing ORDER BY
            shard_count: 1,
        };
        assert_eq!(
            validate_top_n(&config),
            Err(TopNError::LimitRequiresOrderBy)
        );
    }

    #[test]
    fn warn_on_large_offset() {
        assert!(!should_warn_offset(100));
        assert!(!should_warn_offset(10000));
        assert!(should_warn_offset(10001));
    }
}
