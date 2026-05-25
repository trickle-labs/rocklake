//! Incremental delta optimizations.
//!
//! - Change-buffer compaction: cancel INSERT/DELETE pairs on same row_id.
//! - Predicate pushdown into delta scan.
//! - Semi-join key pre-filter.
//! - Append-only fast path.
//! - Auto sort-by on join and group-by keys.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// A delta event with row_id and weight.
#[derive(Debug, Clone)]
pub struct DeltaEvent {
    pub row_id: String,
    pub row: HashMap<String, Value>,
    /// +1 for INSERT, -1 for DELETE.
    pub weight: i32,
}

/// Result of change-buffer compaction.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Events after compaction.
    pub events: Vec<DeltaEvent>,
    /// Number of pairs cancelled.
    pub pairs_cancelled: usize,
    /// Total events before compaction.
    pub total_before: usize,
}

/// Compact a delta batch by cancelling INSERT/DELETE pairs on the same row_id.
///
/// Consecutive `(INSERT row_id=X) + (DELETE row_id=X)` pairs within the same
/// batch cancel out, cutting buffer size 50–90% on high-update workloads.
pub fn compact_change_buffer(events: Vec<DeltaEvent>) -> CompactionResult {
    let total_before = events.len();

    // Group by row_id and sum weights.
    let mut by_row_id: HashMap<String, Vec<DeltaEvent>> = HashMap::new();
    for event in events {
        by_row_id
            .entry(event.row_id.clone())
            .or_default()
            .push(event);
    }

    let mut result_events = Vec::new();
    let mut pairs_cancelled = 0;

    for (_row_id, row_events) in by_row_id {
        let net_weight: i32 = row_events.iter().map(|e| e.weight).sum();

        if net_weight == 0 {
            // Fully cancelled.
            pairs_cancelled += row_events.len() / 2;
        } else if let Some(last) = row_events.last() {
            // Keep the net effect (last event with adjusted weight).
            let mut kept = last.clone();
            kept.weight = net_weight;
            result_events.push(kept);
            if row_events.len() > 1 {
                pairs_cancelled += (row_events.len() - 1) / 2;
            }
        }
    }

    CompactionResult {
        events: result_events,
        pairs_cancelled,
        total_before,
    }
}

/// Compaction ratio: pairs_cancelled / total_events.
pub fn compaction_ratio(result: &CompactionResult) -> f64 {
    if result.total_before == 0 {
        return 0.0;
    }
    (result.pairs_cancelled * 2) as f64 / result.total_before as f64
}

/// A filter predicate for pushdown.
#[derive(Debug, Clone)]
pub struct FilterPredicate {
    pub column: String,
    pub op: FilterOp,
    pub value: Value,
}

/// Filter operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// Apply predicate pushdown to a delta batch.
///
/// Only rows matching the predicate are kept, reducing delta bytes read.
pub fn apply_predicate_pushdown(
    events: Vec<DeltaEvent>,
    predicates: &[FilterPredicate],
) -> Vec<DeltaEvent> {
    events
        .into_iter()
        .filter(|event| {
            predicates.iter().all(|pred| {
                if let Some(val) = event.row.get(&pred.column) {
                    evaluate_predicate(val, &pred.op, &pred.value)
                } else {
                    false
                }
            })
        })
        .collect()
}

/// Evaluate a single predicate against a value.
fn evaluate_predicate(actual: &Value, op: &FilterOp, expected: &Value) -> bool {
    match op {
        FilterOp::Eq => actual == expected,
        FilterOp::Ne => actual != expected,
        FilterOp::Lt => compare_values(actual, expected) == Some(std::cmp::Ordering::Less),
        FilterOp::Gt => compare_values(actual, expected) == Some(std::cmp::Ordering::Greater),
        FilterOp::Le => {
            matches!(
                compare_values(actual, expected),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        }
        FilterOp::Ge => {
            matches!(
                compare_values(actual, expected),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )
        }
    }
}

/// Compare two JSON values.
fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64().partial_cmp(&b.as_f64()),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Extract distinct join keys from a delta batch (semi-join pre-filter).
///
/// For `delta_orders ⋈ customers`, project DISTINCT join_key from the delta
/// side first and use it as the probe set.
pub fn extract_distinct_keys(events: &[DeltaEvent], key_column: &str) -> HashSet<Value> {
    events
        .iter()
        .filter_map(|e| e.row.get(key_column).cloned())
        .collect()
}

/// Apply semi-join key pre-filter to the probe side.
///
/// Only rows whose join key appears in `probe_keys` are returned.
pub fn apply_semi_join_filter(
    probe_rows: Vec<HashMap<String, Value>>,
    key_column: &str,
    probe_keys: &HashSet<Value>,
) -> Vec<HashMap<String, Value>> {
    probe_rows
        .into_iter()
        .filter(|row| {
            row.get(key_column)
                .map(|k| probe_keys.contains(k))
                .unwrap_or(false)
        })
        .collect()
}

/// Append-only fast path detector.
#[derive(Debug, Clone)]
pub struct AppendOnlyDetector {
    /// Number of consecutive insert-only batches.
    pub consecutive_insert_only: u64,
    /// Threshold to activate fast path.
    pub threshold: u64,
    /// Whether fast path is currently active.
    pub active: bool,
}

impl AppendOnlyDetector {
    pub fn new(threshold: u64) -> Self {
        Self {
            consecutive_insert_only: 0,
            threshold,
            active: false,
        }
    }

    /// Record a batch. Returns whether fast path is active after this batch.
    pub fn record_batch(&mut self, has_deletes: bool) -> bool {
        if has_deletes {
            self.consecutive_insert_only = 0;
            self.active = false;
        } else {
            self.consecutive_insert_only += 1;
            if self.consecutive_insert_only >= self.threshold {
                self.active = true;
            }
        }
        self.active
    }
}

/// Sort keys for Parquet output files.
#[derive(Debug, Clone)]
pub struct SortKeyConfig {
    /// Column names to sort by (from GROUP BY and equi-join keys).
    pub sort_keys: Vec<String>,
}

impl SortKeyConfig {
    /// Auto-populate sort keys from a plan's GROUP BY and join keys.
    pub fn from_plan_keys(group_by_cols: &[String], join_keys: &[String]) -> Self {
        let mut keys: Vec<String> = group_by_cols.to_vec();
        for jk in join_keys {
            if !keys.contains(jk) {
                keys.push(jk.clone());
            }
        }
        Self { sort_keys: keys }
    }

    /// Check if sort keys are configured.
    pub fn has_keys(&self) -> bool {
        !self.sort_keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(row_id: &str, weight: i32, data: &[(&str, Value)]) -> DeltaEvent {
        let row: HashMap<String, Value> = data
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        DeltaEvent {
            row_id: row_id.to_string(),
            row,
            weight,
        }
    }

    #[test]
    fn change_buffer_compaction_cancels_pairs() {
        let events = vec![
            make_event("r1", 1, &[("id", Value::from(1))]),
            make_event("r1", -1, &[("id", Value::from(1))]),
            make_event("r2", 1, &[("id", Value::from(2))]),
        ];

        let result = compact_change_buffer(events);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.pairs_cancelled, 1);
        assert!(compaction_ratio(&result) >= 0.5);
    }

    #[test]
    fn predicate_pushdown_filters_rows() {
        let events = vec![
            make_event("r1", 1, &[("status", Value::from("active"))]),
            make_event("r2", 1, &[("status", Value::from("inactive"))]),
            make_event("r3", 1, &[("status", Value::from("active"))]),
        ];

        let predicates = vec![FilterPredicate {
            column: "status".to_string(),
            op: FilterOp::Eq,
            value: Value::from("active"),
        }];

        let filtered = apply_predicate_pushdown(events, &predicates);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn semi_join_key_extraction() {
        let events = vec![
            make_event("r1", 1, &[("customer_id", Value::from(10))]),
            make_event("r2", 1, &[("customer_id", Value::from(20))]),
            make_event("r3", 1, &[("customer_id", Value::from(10))]),
        ];

        let keys = extract_distinct_keys(&events, "customer_id");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&Value::from(10)));
        assert!(keys.contains(&Value::from(20)));
    }

    #[test]
    fn semi_join_filter_reduces_probe_side() {
        let probe_rows = vec![
            [
                ("customer_id".to_string(), Value::from(10)),
                ("name".to_string(), Value::from("Alice")),
            ]
            .into(),
            [
                ("customer_id".to_string(), Value::from(30)),
                ("name".to_string(), Value::from("Bob")),
            ]
            .into(),
            [
                ("customer_id".to_string(), Value::from(20)),
                ("name".to_string(), Value::from("Carol")),
            ]
            .into(),
        ];

        let mut keys = HashSet::new();
        keys.insert(Value::from(10));
        keys.insert(Value::from(20));

        let filtered = apply_semi_join_filter(probe_rows, "customer_id", &keys);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn append_only_detection() {
        let mut detector = AppendOnlyDetector::new(5);
        for _ in 0..4 {
            assert!(!detector.record_batch(false));
        }
        assert!(detector.record_batch(false));
        assert!(detector.active);

        // Delete reverts to full mode.
        assert!(!detector.record_batch(true));
        assert!(!detector.active);
    }

    #[test]
    fn sort_key_auto_population() {
        let config = SortKeyConfig::from_plan_keys(
            &["dept".to_string(), "region".to_string()],
            &["dept".to_string(), "emp_id".to_string()],
        );
        assert_eq!(config.sort_keys, vec!["dept", "region", "emp_id"]);
    }
}
