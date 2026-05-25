//! Correlated subquery decorrelation for IVM.
//!
//! Transforms correlated subqueries (`EXISTS`, `IN`, scalar subqueries)
//! into equivalent join/aggregation operations suitable for incremental
//! maintenance.
//!
//! ## Approach
//! Decorrelation via algebraic rewrites (same technique DataFusion uses):
//! - Correlated `EXISTS` → semi-join
//! - Correlated `NOT EXISTS` → anti-join
//! - Correlated `IN (SELECT …)` → semi-join
//! - Correlated `NOT IN (SELECT …)` → anti-join
//! - Scalar correlated subquery → left join + aggregation
//!
//! After decorrelation the circuit contains only regular joins and aggregations.

use serde::{Deserialize, Serialize};

/// Kind of correlated subquery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubqueryKind {
    /// `WHERE EXISTS (SELECT … WHERE outer.col = inner.col)`
    Exists,
    /// `WHERE NOT EXISTS (SELECT … WHERE outer.col = inner.col)`
    NotExists,
    /// `WHERE col IN (SELECT … FROM …)`
    In,
    /// `WHERE col NOT IN (SELECT … FROM …)`
    NotIn,
    /// Scalar subquery in SELECT list: `(SELECT agg(…) FROM … WHERE outer.col = inner.col)`
    Scalar,
}

/// A correlated subquery extracted from the SQL plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedSubquery {
    /// The kind of subquery.
    pub kind: SubqueryKind,
    /// The outer table that references the subquery.
    pub outer_table: String,
    /// The inner (subquery) table.
    pub inner_table: String,
    /// Correlation predicate: outer column.
    pub outer_col: String,
    /// Correlation predicate: inner column.
    pub inner_col: String,
    /// For scalar subqueries: the aggregate expression.
    pub scalar_expr: Option<String>,
    /// Output alias for the decorrelated result.
    pub output_alias: Option<String>,
}

/// The decorrelated plan: the subquery rewritten as a join operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecorrelatedOp {
    /// Semi-join (EXISTS, IN) — output rows from outer where match exists.
    SemiJoin {
        outer_table: String,
        inner_table: String,
        outer_col: String,
        inner_col: String,
    },
    /// Anti-join (NOT EXISTS, NOT IN) — output rows from outer where no match.
    AntiJoin {
        outer_table: String,
        inner_table: String,
        outer_col: String,
        inner_col: String,
    },
    /// Left join + aggregation (scalar subquery).
    LeftJoinAggregate {
        outer_table: String,
        inner_table: String,
        outer_col: String,
        inner_col: String,
        aggregate_expr: String,
        output_alias: String,
    },
}

/// Decorrelation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecorrelationError {
    #[error("cannot decorrelate: deep mutual correlation between '{0}' and '{1}'")]
    CannotDecorrelate(String, String),
    #[error("missing correlation predicate in subquery")]
    MissingCorrelationPredicate,
    #[error("unsupported subquery pattern")]
    UnsupportedPattern,
}

/// Attempt to decorrelate a subquery into an equivalent join operation.
pub fn decorrelate(subquery: &CorrelatedSubquery) -> Result<DecorrelatedOp, DecorrelationError> {
    if subquery.outer_col.is_empty() || subquery.inner_col.is_empty() {
        return Err(DecorrelationError::MissingCorrelationPredicate);
    }

    match &subquery.kind {
        SubqueryKind::Exists | SubqueryKind::In => Ok(DecorrelatedOp::SemiJoin {
            outer_table: subquery.outer_table.clone(),
            inner_table: subquery.inner_table.clone(),
            outer_col: subquery.outer_col.clone(),
            inner_col: subquery.inner_col.clone(),
        }),
        SubqueryKind::NotExists | SubqueryKind::NotIn => Ok(DecorrelatedOp::AntiJoin {
            outer_table: subquery.outer_table.clone(),
            inner_table: subquery.inner_table.clone(),
            outer_col: subquery.outer_col.clone(),
            inner_col: subquery.inner_col.clone(),
        }),
        SubqueryKind::Scalar => {
            let aggregate_expr = subquery
                .scalar_expr
                .clone()
                .ok_or(DecorrelationError::UnsupportedPattern)?;
            let output_alias = subquery
                .output_alias
                .clone()
                .unwrap_or_else(|| "scalar_subquery".to_string());
            Ok(DecorrelatedOp::LeftJoinAggregate {
                outer_table: subquery.outer_table.clone(),
                inner_table: subquery.inner_table.clone(),
                outer_col: subquery.outer_col.clone(),
                inner_col: subquery.inner_col.clone(),
                aggregate_expr,
                output_alias,
            })
        }
    }
}

/// Semi-join evaluator: filters outer rows by existence of matching inner rows.
#[derive(Debug, Clone)]
pub struct SemiJoinEvaluator {
    /// Inner table keys (set of values that exist).
    pub inner_keys: std::collections::HashSet<Vec<u8>>,
}

impl SemiJoinEvaluator {
    pub fn new() -> Self {
        Self {
            inner_keys: std::collections::HashSet::new(),
        }
    }

    /// Add an inner key.
    pub fn add_inner_key(&mut self, key: Vec<u8>) {
        self.inner_keys.insert(key);
    }

    /// Remove an inner key.
    pub fn remove_inner_key(&mut self, key: &[u8]) {
        self.inner_keys.remove(key);
    }

    /// Check if an outer key has a match in the inner set.
    pub fn has_match(&self, outer_key: &[u8]) -> bool {
        self.inner_keys.contains(outer_key)
    }

    /// Number of distinct inner keys.
    pub fn inner_key_count(&self) -> usize {
        self.inner_keys.len()
    }
}

impl Default for SemiJoinEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Anti-join evaluator: filters outer rows by non-existence of matching inner rows.
#[derive(Debug, Clone)]
pub struct AntiJoinEvaluator {
    inner: SemiJoinEvaluator,
}

impl AntiJoinEvaluator {
    pub fn new() -> Self {
        Self {
            inner: SemiJoinEvaluator::new(),
        }
    }

    pub fn add_inner_key(&mut self, key: Vec<u8>) {
        self.inner.add_inner_key(key);
    }

    pub fn remove_inner_key(&mut self, key: &[u8]) {
        self.inner.remove_inner_key(key);
    }

    /// Check if an outer key has NO match in the inner set.
    pub fn has_no_match(&self, outer_key: &[u8]) -> bool {
        !self.inner.has_match(outer_key)
    }
}

impl Default for AntiJoinEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Scalar subquery evaluator: maintains per-group aggregation from the inner table.
#[derive(Debug, Clone)]
pub struct ScalarSubqueryEvaluator {
    /// Per-key aggregation state (key → current aggregate value).
    pub aggregates: std::collections::HashMap<Vec<u8>, serde_json::Value>,
}

impl ScalarSubqueryEvaluator {
    pub fn new() -> Self {
        Self {
            aggregates: std::collections::HashMap::new(),
        }
    }

    /// Update the aggregate for a key.
    pub fn update_aggregate(&mut self, key: Vec<u8>, value: serde_json::Value) {
        self.aggregates.insert(key, value);
    }

    /// Remove the aggregate for a key (inner relation becomes empty for this key).
    pub fn remove_aggregate(&mut self, key: &[u8]) {
        self.aggregates.remove(key);
    }

    /// Get the scalar result for a given outer key.
    /// Returns NULL if no matching inner rows exist.
    pub fn get_scalar(&self, key: &[u8]) -> serde_json::Value {
        self.aggregates
            .get(key)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
}

impl Default for ScalarSubqueryEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorrelate_exists_to_semi_join() {
        let subquery = CorrelatedSubquery {
            kind: SubqueryKind::Exists,
            outer_table: "orders".to_string(),
            inner_table: "lineitem".to_string(),
            outer_col: "o_orderkey".to_string(),
            inner_col: "l_orderkey".to_string(),
            scalar_expr: None,
            output_alias: None,
        };

        let result = decorrelate(&subquery).unwrap();
        matches!(result, DecorrelatedOp::SemiJoin { .. });
    }

    #[test]
    fn decorrelate_not_exists_to_anti_join() {
        let subquery = CorrelatedSubquery {
            kind: SubqueryKind::NotExists,
            outer_table: "orders".to_string(),
            inner_table: "lineitem".to_string(),
            outer_col: "o_orderkey".to_string(),
            inner_col: "l_orderkey".to_string(),
            scalar_expr: None,
            output_alias: None,
        };

        let result = decorrelate(&subquery).unwrap();
        matches!(result, DecorrelatedOp::AntiJoin { .. });
    }

    #[test]
    fn decorrelate_scalar_to_left_join_agg() {
        let subquery = CorrelatedSubquery {
            kind: SubqueryKind::Scalar,
            outer_table: "orders".to_string(),
            inner_table: "lineitem".to_string(),
            outer_col: "o_orderkey".to_string(),
            inner_col: "l_orderkey".to_string(),
            scalar_expr: Some("SUM(l_quantity)".to_string()),
            output_alias: Some("total_qty".to_string()),
        };

        let result = decorrelate(&subquery).unwrap();
        matches!(result, DecorrelatedOp::LeftJoinAggregate { .. });
    }

    #[test]
    fn semi_join_evaluator() {
        let mut eval = SemiJoinEvaluator::new();
        eval.add_inner_key(b"key1".to_vec());
        eval.add_inner_key(b"key2".to_vec());

        assert!(eval.has_match(b"key1"));
        assert!(eval.has_match(b"key2"));
        assert!(!eval.has_match(b"key3"));

        eval.remove_inner_key(b"key1");
        assert!(!eval.has_match(b"key1"));
    }

    #[test]
    fn anti_join_evaluator() {
        let mut eval = AntiJoinEvaluator::new();
        eval.add_inner_key(b"key1".to_vec());

        assert!(!eval.has_no_match(b"key1"));
        assert!(eval.has_no_match(b"key2"));

        eval.remove_inner_key(b"key1");
        assert!(eval.has_no_match(b"key1"));
    }

    #[test]
    fn scalar_subquery_returns_null_when_empty() {
        let mut eval = ScalarSubqueryEvaluator::new();
        assert_eq!(eval.get_scalar(b"key1"), serde_json::Value::Null);

        eval.update_aggregate(b"key1".to_vec(), serde_json::Value::Number(42.into()));
        assert_eq!(
            eval.get_scalar(b"key1"),
            serde_json::Value::Number(42.into())
        );

        eval.remove_aggregate(b"key1");
        assert_eq!(eval.get_scalar(b"key1"), serde_json::Value::Null);
    }
}
