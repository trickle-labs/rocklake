//! IvmOracle: correctness oracle for incremental view maintenance.
//!
//! The oracle validates that the IVM engine's incremental output is equivalent
//! to a full-recompute reference.  It does this by:
//!
//! 1. Parsing the view SQL into an `IvmPlan`
//! 2. Replaying a sequence of DML operations (inserts and deletes) through
//!    both the IVM circuit and a batch reference engine
//! 3. Asserting multiset equivalence of the results after each step
//!
//! ## Usage
//! ```ignore
//! let oracle = IvmOracle::new("SELECT region, COUNT(*) AS cnt FROM sales GROUP BY region");
//! oracle.insert("sales", row!{"region" => "us", "amount" => 100});
//! oracle.insert("sales", row!{"region" => "eu", "amount" => 200});
//! oracle.assert_equivalent("after initial inserts");
//!
//! oracle.delete("sales", row!{"region" => "us", "amount" => 100});
//! oracle.assert_equivalent("after delete");
//! ```
//!
//! ## Join support (v0.14 EC-01)
//! For multi-table views, insert rows into each input table by name.
//! The oracle uses EC-01 asymmetric delta branches for correct join handling.
//! The reference engine performs a full cross-product + filter + GROUP BY to
//! compute the expected result.

use std::collections::HashMap;

use serde_json::Value;

use slateduck_ivm::circuit::ZDelta;
use slateduck_ivm::plan::{AggregateKind, IvmPlan};
use slateduck_ivm::{IvmCircuit, IvmJoinCircuit, JoinStrategy};

/// A DML operation recorded by the oracle.
#[derive(Debug, Clone)]
enum DmlOp {
    Insert { table: String, row: HashMap<String, Value> },
    Delete { table: String, row: HashMap<String, Value> },
}

/// IVM correctness oracle: compares incremental output against batch recompute.
pub struct IvmOracle {
    plan: IvmPlan,
    /// All DML operations in order.
    ops: Vec<DmlOp>,
    /// The IVM circuit under test.
    circuit: IvmCircuit,
    /// Join circuit (if the plan has joins).
    join_circuit: Option<IvmJoinCircuit>,
}

impl IvmOracle {
    /// Create a new oracle from view SQL.
    ///
    /// Panics if the SQL cannot be parsed into an IvmPlan.
    pub fn new(view_sql: &str) -> Self {
        let plan = IvmPlan::parse(view_sql)
            .unwrap_or_else(|e| panic!("IvmOracle: failed to parse view SQL: {e}"));

        let join_circuit = if plan.joins.is_empty() {
            None
        } else {
            let strategies = plan.joins.iter().map(|_| JoinStrategy::Broadcast).collect();
            let left_cols = plan.joins.iter().map(|j| j.left_col.clone()).collect();
            Some(IvmJoinCircuit::new(plan.clone(), strategies, left_cols))
        };

        let circuit = IvmCircuit::new(plan.clone());

        Self {
            plan,
            ops: Vec::new(),
            circuit,
            join_circuit,
        }
    }

    /// Record an INSERT into a table and push it through the IVM circuit.
    pub fn insert(&mut self, table: &str, row: HashMap<String, Value>) {
        self.ops.push(DmlOp::Insert {
            table: table.to_string(),
            row: row.clone(),
        });
        self.push_delta(table, row, 1);
    }

    /// Record a DELETE from a table and push a retraction through the IVM circuit.
    pub fn delete(&mut self, table: &str, row: HashMap<String, Value>) {
        self.ops.push(DmlOp::Delete {
            table: table.to_string(),
            row: row.clone(),
        });
        self.push_delta(table, row, -1);
    }

    /// Push a batch of inserts into a table.
    pub fn insert_batch(&mut self, table: &str, rows: Vec<HashMap<String, Value>>) {
        for row in rows {
            self.insert(table, row);
        }
    }

    /// Assert that the IVM circuit's current output matches a full recompute.
    ///
    /// Panics with a descriptive message if they differ.
    pub fn assert_equivalent(&self, context: &str) {
        let incremental = self.incremental_output();
        let reference = self.reference_output();

        let inc_set = normalize_multiset(&incremental);
        let ref_set = normalize_multiset(&reference);

        if inc_set != ref_set {
            panic!(
                "IvmOracle mismatch ({context}):\n\
                 --- incremental ({} rows) ---\n{}\n\
                 --- reference ({} rows) ---\n{}\n\
                 --- DML history ({} ops) ---\n{}",
                incremental.len(),
                format_rows(&incremental),
                reference.len(),
                format_rows(&reference),
                self.ops.len(),
                format_ops(&self.ops),
            );
        }
    }

    /// Return the IVM circuit's current output (incremental result).
    pub fn incremental_output(&self) -> Vec<HashMap<String, Value>> {
        if let Some(ref join_circuit) = self.join_circuit {
            // For join views, the inner circuit of the join has the aggregation state.
            join_circuit.inner.read_output()
        } else {
            self.circuit.read_output()
        }
    }

    /// Recompute the view from scratch over the current table state.
    pub fn reference_output(&self) -> Vec<HashMap<String, Value>> {
        let tables = self.current_tables();
        reference_compute(&self.plan, &tables)
    }

    /// Return the current materialized rows per table (after applying inserts/deletes).
    fn current_tables(&self) -> HashMap<String, Vec<HashMap<String, Value>>> {
        let mut tables: HashMap<String, Vec<HashMap<String, Value>>> = HashMap::new();
        for op in &self.ops {
            match op {
                DmlOp::Insert { table, row } => {
                    tables.entry(table.clone()).or_default().push(row.clone());
                }
                DmlOp::Delete { table, row } => {
                    let rows = tables.entry(table.clone()).or_default();
                    // Remove the first matching row (multiset semantics).
                    if let Some(pos) = rows.iter().position(|r| r == row) {
                        rows.remove(pos);
                    }
                }
            }
        }
        tables
    }

    /// Push a delta through the appropriate circuit path.
    fn push_delta(&mut self, table: &str, row: HashMap<String, Value>, weight: i64) {
        if self.plan.joins.is_empty() {
            // Single-table: push directly to the aggregation circuit.
            self.circuit.push_batch(&[ZDelta {
                fields: row,
                weight,
            }]);
        } else {
            // Multi-table: route to the correct join input.
            let first_table = self.plan.input_tables.first().cloned().unwrap_or_default();
            if table == first_table {
                // Left side: push through the join and into aggregation.
                if let Some(ref mut join_circuit) = self.join_circuit {
                    join_circuit.push_left_batch(&[(row, weight)]);
                }
            } else {
                // Right side: EC-01 correct handling.
                // When a right-side row is inserted or deleted, we must also
                // produce the join output for all existing left-side rows that
                // match this right-side row.

                // Compute left rows BEFORE borrowing join_circuit.
                let left_rows = self.current_left_rows(&first_table);

                if let Some(ref mut join_circuit) = self.join_circuit {
                    for (idx, join) in self.plan.joins.iter().enumerate() {
                        if join.right_table == table {
                            let right_key_val = row.get(&join.right_col).cloned();

                            // For each matching left row, emit a joined delta.
                            if let Some(ref rkey) = right_key_val {
                                let mut joined_deltas: Vec<ZDelta> = Vec::new();
                                for left_row in &left_rows {
                                    let lkey = left_row.get(&join.left_col);
                                    if lkey == Some(rkey) {
                                        // Merge left + right.
                                        let mut merged = left_row.clone();
                                        for (k, v) in &row {
                                            if !merged.contains_key(k) || k == &join.right_col {
                                                merged.insert(k.clone(), v.clone());
                                            }
                                        }
                                        joined_deltas.push(ZDelta {
                                            fields: merged,
                                            weight,
                                        });
                                    }
                                }
                                if !joined_deltas.is_empty() {
                                    join_circuit.inner.push_batch(&joined_deltas);
                                }
                            }

                            // Update the join state (for future left-side lookups).
                            join_circuit.push_right_delta(idx, row.clone(), &join.right_col, weight);
                        }
                    }
                }
            }
        }
    }

    /// Get current left-side rows (from DML history, not counting the current op).
    fn current_left_rows(&self, table: &str) -> Vec<HashMap<String, Value>> {
        let mut rows: Vec<HashMap<String, Value>> = Vec::new();
        for op in &self.ops {
            match op {
                DmlOp::Insert { table: t, row } if t == table => {
                    rows.push(row.clone());
                }
                DmlOp::Delete { table: t, row } if t == table => {
                    if let Some(pos) = rows.iter().position(|r| r == row) {
                        rows.remove(pos);
                    }
                }
                _ => {}
            }
        }
        rows
    }
}

/// Compute the view result from scratch over the given table state.
fn reference_compute(
    plan: &IvmPlan,
    tables: &HashMap<String, Vec<HashMap<String, Value>>>,
) -> Vec<HashMap<String, Value>> {
    // Start with the rows from the first input table.
    let first_table = match plan.input_tables.first() {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut working_rows: Vec<HashMap<String, Value>> =
        tables.get(first_table).cloned().unwrap_or_default();

    // Apply joins (nested-loop with equality filter).
    for join in &plan.joins {
        let right_rows = tables.get(&join.right_table).cloned().unwrap_or_default();
        let mut joined = Vec::new();
        for left in &working_rows {
            for right in &right_rows {
                let lval = left.get(&join.left_col);
                let rval = right.get(&join.right_col);
                if lval.is_some() && lval == rval {
                    let mut merged = left.clone();
                    // Merge right columns, prefixing on conflict.
                    for (k, v) in right {
                        if merged.contains_key(k) && k != &join.right_col {
                            merged.insert(format!("{}_{}", join.right_table, k), v.clone());
                        } else {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                    joined.push(merged);
                }
            }
        }
        working_rows = joined;
    }

    // Apply GROUP BY + aggregation.
    if plan.group_by_cols.is_empty() && plan.aggregates.is_empty() {
        return working_rows;
    }

    #[allow(clippy::type_complexity)]
    let mut groups: HashMap<String, (HashMap<String, Value>, Vec<&HashMap<String, Value>>)> =
        HashMap::new();

    for row in &working_rows {
        let key = group_key_from_row(row, &plan.group_by_cols);
        let entry = groups.entry(key).or_insert_with(|| {
            let group_vals: HashMap<String, Value> = plan
                .group_by_cols
                .iter()
                .map(|c| (c.clone(), row.get(c).cloned().unwrap_or(Value::Null)))
                .collect();
            (group_vals, Vec::new())
        });
        entry.1.push(row);
    }

    groups
        .into_values()
        .map(|(group_vals, rows)| {
            let mut output = group_vals;
            for agg in &plan.aggregates {
                let val = compute_aggregate(agg, &rows);
                output.insert(agg.output_col.clone(), val);
            }
            output
        })
        .collect()
}

/// Compute a single aggregate over a set of rows.
fn compute_aggregate(
    agg: &slateduck_ivm::plan::Aggregate,
    rows: &[&HashMap<String, Value>],
) -> Value {
    match &agg.kind {
        AggregateKind::Count => Value::Number(serde_json::Number::from(rows.len() as i64)),
        AggregateKind::Sum => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let total: i64 = rows
                .iter()
                .map(|r| {
                    r.get(col)
                        .and_then(|v| match v {
                            Value::Number(n) => n.as_i64(),
                            _ => None,
                        })
                        .unwrap_or(0)
                })
                .sum();
            Value::Number(serde_json::Number::from(total))
        }
        AggregateKind::Avg => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_f64(),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                let sum: f64 = values.iter().sum();
                json_f64(sum / values.len() as f64)
            }
        }
        AggregateKind::Stddev => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_f64(),
                    _ => None,
                })
                .collect();
            if values.len() < 2 {
                Value::Null
            } else {
                let n = values.len() as f64;
                let mean = values.iter().sum::<f64>() / n;
                let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
                json_f64(variance.sqrt())
            }
        }
        AggregateKind::Min => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let min_val = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_f64(),
                    _ => None,
                })
                .fold(f64::INFINITY, f64::min);
            if min_val.is_infinite() {
                Value::Null
            } else {
                json_f64(min_val)
            }
        }
        AggregateKind::Max => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let max_val = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_f64(),
                    _ => None,
                })
                .fold(f64::NEG_INFINITY, f64::max);
            if max_val.is_infinite() {
                Value::Null
            } else {
                json_f64(max_val)
            }
        }
        AggregateKind::BoolAnd => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<bool> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Bool(b) => Some(*b),
                    Value::Number(n) => Some(n.as_i64().unwrap_or(0) != 0),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                Value::Bool(values.iter().all(|&b| b))
            }
        }
        AggregateKind::BoolOr => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<bool> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Bool(b) => Some(*b),
                    Value::Number(n) => Some(n.as_i64().unwrap_or(0) != 0),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                Value::Bool(values.iter().any(|&b| b))
            }
        }
        AggregateKind::BitAnd => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<i64> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_i64(),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                let result = values.iter().fold(!0i64, |acc, &v| acc & v);
                Value::Number(serde_json::Number::from(result))
            }
        }
        AggregateKind::BitOr => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<i64> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_i64(),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                let result = values.iter().fold(0i64, |acc, &v| acc | v);
                Value::Number(serde_json::Number::from(result))
            }
        }
        AggregateKind::BitXor => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<i64> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::Number(n) => n.as_i64(),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                let result = values.iter().fold(0i64, |acc, &v| acc ^ v);
                Value::Number(serde_json::Number::from(result))
            }
        }
        AggregateKind::StringAgg => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<&str> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                Value::String(values.join(","))
            }
        }
        AggregateKind::ArrayAgg => {
            let col = agg.input_col.as_deref().unwrap_or("");
            let values: Vec<Value> = rows
                .iter()
                .filter_map(|r| r.get(col))
                .cloned()
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                Value::Array(values)
            }
        }
    }
}

/// Serialize a group key for a row.
fn group_key_from_row(row: &HashMap<String, Value>, cols: &[String]) -> String {
    let vals: Vec<Value> = cols
        .iter()
        .map(|c| row.get(c).cloned().unwrap_or(Value::Null))
        .collect();
    serde_json::to_string(&vals).unwrap_or_default()
}

/// Normalize a result set into a comparable multiset (sorted JSON strings).
fn normalize_multiset(rows: &[HashMap<String, Value>]) -> Vec<String> {
    let mut normalized: Vec<String> = rows
        .iter()
        .map(|r| {
            let mut pairs: Vec<_> = r.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            serde_json::to_string(&pairs).unwrap_or_default()
        })
        .collect();
    normalized.sort();
    normalized
}

/// Format rows for error output.
fn format_rows(rows: &[HashMap<String, Value>]) -> String {
    rows.iter()
        .map(|r| format!("  {:?}", r))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format DML ops for error output.
fn format_ops(ops: &[DmlOp]) -> String {
    ops.iter()
        .enumerate()
        .map(|(i, op)| match op {
            DmlOp::Insert { table, row } => format!("  [{i}] INSERT {table}: {:?}", row),
            DmlOp::Delete { table, row } => format!("  [{i}] DELETE {table}: {:?}", row),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert f64 to a JSON number value.
fn json_f64(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Convenience macro for building row maps in tests.
///
/// Usage: `row!{"col" => "val", "num" => 42}`
#[macro_export]
macro_rules! row {
    ($($key:expr => $val:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = std::collections::HashMap::new();
        $(
            map.insert($key.to_string(), serde_json::json!($val));
        )*
        map
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_count_star_group_by() {
        let mut oracle =
            IvmOracle::new("SELECT region, COUNT(*) AS cnt FROM sales GROUP BY region");

        oracle.insert("sales", row! {"region" => "us", "amount" => 100});
        oracle.insert("sales", row! {"region" => "us", "amount" => 200});
        oracle.insert("sales", row! {"region" => "eu", "amount" => 50});
        oracle.assert_equivalent("after 3 inserts");

        oracle.insert("sales", row! {"region" => "eu", "amount" => 75});
        oracle.assert_equivalent("after 4th insert");
    }

    #[test]
    fn oracle_sum_aggregate() {
        let mut oracle =
            IvmOracle::new("SELECT dept, SUM(amount) AS total FROM orders GROUP BY dept");

        oracle.insert("orders", row! {"dept" => "eng", "amount" => 100});
        oracle.insert("orders", row! {"dept" => "eng", "amount" => 200});
        oracle.insert("orders", row! {"dept" => "sales", "amount" => 50});
        oracle.assert_equivalent("after inserts");
    }

    #[test]
    fn oracle_delete_retraction() {
        let mut oracle =
            IvmOracle::new("SELECT region, COUNT(*) AS cnt FROM sales GROUP BY region");

        oracle.insert("sales", row! {"region" => "us"});
        oracle.insert("sales", row! {"region" => "us"});
        oracle.insert("sales", row! {"region" => "eu"});
        oracle.assert_equivalent("before delete");

        oracle.delete("sales", row! {"region" => "us"});
        oracle.assert_equivalent("after deleting one us row");
    }

    #[test]
    fn oracle_delete_removes_group() {
        let mut oracle =
            IvmOracle::new("SELECT region, COUNT(*) AS cnt FROM sales GROUP BY region");

        oracle.insert("sales", row! {"region" => "us"});
        oracle.insert("sales", row! {"region" => "eu"});
        oracle.assert_equivalent("two groups");

        oracle.delete("sales", row! {"region" => "eu"});
        oracle.assert_equivalent("eu group removed");
    }

    #[test]
    fn oracle_min_max() {
        let mut oracle =
            IvmOracle::new("SELECT dept, MIN(salary) AS lo, MAX(salary) AS hi FROM emp GROUP BY dept");

        oracle.insert("emp", row! {"dept" => "eng", "salary" => 100});
        oracle.insert("emp", row! {"dept" => "eng", "salary" => 300});
        oracle.insert("emp", row! {"dept" => "eng", "salary" => 200});
        oracle.assert_equivalent("min=100, max=300");

        oracle.delete("emp", row! {"dept" => "eng", "salary" => 300});
        oracle.assert_equivalent("after deleting max");
    }

    #[test]
    fn oracle_join_basic() {
        let mut oracle = IvmOracle::new(
            "SELECT e.dept_id, COUNT(*) AS cnt \
             FROM employees e \
             JOIN departments d ON e.dept_id = d.dept_id \
             GROUP BY e.dept_id",
        );

        // Insert right side first (departments).
        oracle.insert("departments", row! {"dept_id" => 1, "name" => "Engineering"});
        oracle.insert("departments", row! {"dept_id" => 2, "name" => "Sales"});

        // Insert left side (employees).
        oracle.insert("employees", row! {"dept_id" => 1, "name" => "Alice"});
        oracle.insert("employees", row! {"dept_id" => 1, "name" => "Bob"});
        oracle.insert("employees", row! {"dept_id" => 2, "name" => "Carol"});
        oracle.assert_equivalent("join with 3 employees across 2 depts");
    }
}
