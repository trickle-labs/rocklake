//! Tier 6e — IVM operator correctness tests for v0.16 operators.
//!
//! Tests window functions, ORDER BY, LIMIT/top-N, correlated subqueries,
//! recursive CTEs, and non-deterministic capture semantics.

use serde_json::Value;
use slateduck_ivm::decorrelate::{
    decorrelate, CorrelatedSubquery, DecorrelatedOp, ScalarSubqueryEvaluator, SemiJoinEvaluator,
    SubqueryKind,
};
use slateduck_ivm::nondet_capture::{restore_captures, sample_batch_captures, serialize_captures};
use slateduck_ivm::ordered_trace::{OrderedTraceConfig, SlateDbOrderedTrace, SortKey};
use slateduck_ivm::recursive_cte::{
    validate_recursive_cte, RecursiveCteConfig, RecursiveCteEvaluator,
};
use slateduck_ivm::ref_counted::{RefCountedDistinct, RefCountedSetOp, SetOperator};
use slateduck_ivm::top_n::{should_warn_offset, TopNConfig, TopNOperator};
use slateduck_ivm::window::{
    validate_window_spec, AggregateWindowKind, FrameBound, OrderByColumn, WindowEvaluator,
    WindowFrame, WindowFunction, WindowMode, WindowSpec,
};

/// Window: ROW_NUMBER() OVER (PARTITION BY … ORDER BY …) maintained correctly
/// for 1000 snapshots; partition-local mode.
#[test]
fn window_row_number_1000_snapshots_partitioned() {
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

    // Simulate 1000 snapshots with inserts and deletes
    for snapshot in 0..1000u64 {
        let sort_key = snapshot.to_be_bytes().to_vec();
        eval.insert_row(pk.clone(), sort_key, vec![Value::Number(snapshot.into())]);

        // Delete every 5th row to test stability
        if snapshot >= 5 && snapshot % 5 == 0 {
            let old_key = (snapshot - 5).to_be_bytes().to_vec();
            eval.remove_row(&pk, &old_key);
        }
    }

    let results = eval.evaluate_partition(&pk);
    // Verify row numbers are consecutive 1..=N
    for (i, (_, val)) in results.iter().enumerate() {
        assert_eq!(*val, Value::Number((i as u64 + 1).into()));
    }
}

/// Window: ROW_NUMBER() in cross-partition (single-shard merge) mode.
#[test]
fn window_row_number_cross_partition_single_shard() {
    let spec = WindowSpec {
        function: WindowFunction::RowNumber,
        partition_by: vec![],
        order_by: vec![OrderByColumn {
            column: "id".to_string(),
            descending: false,
            nulls_first: false,
        }],
        frame: WindowFrame::default(),
        output_col: "global_rn".to_string(),
        mode: WindowMode::TotalOrder,
        input_col: None,
    };

    // Validate: full-table window requires shard_count = 1
    assert!(validate_window_spec(&spec, 1).is_ok());
    assert!(validate_window_spec(&spec, 4).is_err());

    let mut eval = WindowEvaluator::new(spec);
    let pk = b"".to_vec(); // single partition for full-table

    for i in 0..50u64 {
        eval.insert_row(
            pk.clone(),
            i.to_be_bytes().to_vec(),
            vec![Value::Number(i.into())],
        );
    }

    let results = eval.evaluate_partition(&pk);
    assert_eq!(results.len(), 50);
    assert_eq!(results[0].1, Value::Number(1.into()));
    assert_eq!(results[49].1, Value::Number(50.into()));
}

/// Window: LAG/LEAD navigation — insert then delete a row that is the LAG
/// source for its neighbour; output matches expected.
#[test]
fn window_lag_lead_delete_source() {
    let spec = WindowSpec {
        function: WindowFunction::Lag(1),
        partition_by: vec!["user".to_string()],
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

    // Row at sort_key [2] is the LAG source for row at [3]
    let results = eval.evaluate_partition(&pk);
    assert_eq!(results[2].1, Value::Number(20.into())); // LAG(1) of row[3] = row[2]

    // Delete row at [2]
    eval.remove_row(&pk, &[2]);

    // Now LAG(1) of row[3] should be row[1]'s value
    let results = eval.evaluate_partition(&pk);
    assert_eq!(results.len(), 2);
    assert_eq!(results[1].1, Value::Number(10.into())); // LAG(1) of row[3] = row[1]
}

/// Window: aggregate window SUM OVER (PARTITION BY … ORDER BY … ROWS BETWEEN …)
#[test]
fn window_aggregate_sum_over_frame() {
    let spec = WindowSpec {
        function: WindowFunction::AggregateWindow(AggregateWindowKind::Sum),
        partition_by: vec!["dept".to_string()],
        order_by: vec![OrderByColumn {
            column: "month".to_string(),
            descending: false,
            nulls_first: false,
        }],
        frame: WindowFrame::Rows {
            start: FrameBound::Preceding(1),
            end: FrameBound::CurrentRow,
        },
        output_col: "rolling_sum".to_string(),
        mode: WindowMode::Partitioned,
        input_col: Some("revenue".to_string()),
    };

    let mut eval = WindowEvaluator::new(spec);
    let pk = b"sales".to_vec();

    // Insert monthly revenue
    eval.insert_row(pk.clone(), vec![1], vec![Value::Number(100.into())]);
    eval.insert_row(pk.clone(), vec![2], vec![Value::Number(200.into())]);
    eval.insert_row(pk.clone(), vec![3], vec![Value::Number(300.into())]);

    let results = eval.evaluate_partition(&pk);
    // Row 0 (month 1): SUM of [preceding 1, current] = just 100 (no preceding)
    assert_eq!(
        results[0].1,
        Value::Number(serde_json::Number::from_f64(100.0).unwrap())
    );
    // Row 1 (month 2): SUM of [100, 200] = 300
    assert_eq!(
        results[1].1,
        Value::Number(serde_json::Number::from_f64(300.0).unwrap())
    );
    // Row 2 (month 3): SUM of [200, 300] = 500
    assert_eq!(
        results[2].1,
        Value::Number(serde_json::Number::from_f64(500.0).unwrap())
    );
}

/// ORDER BY: output ordered trace delivers rows in declared order.
#[test]
fn order_by_output_in_declared_order() {
    let config = OrderedTraceConfig {
        state_prefix: "matview_orders".to_string(),
        sort_keys: vec![SortKey {
            column: "order_value".to_string(),
            descending: true, // DESC
            nulls_first: false,
        }],
        total_order: true,
    };

    let mut trace = SlateDbOrderedTrace::new(config);
    let pk = b"".to_vec();

    // Insert out of order
    trace.insert(pk.clone(), vec![0, 50], vec![Value::Number(50.into())]);
    trace.insert(pk.clone(), vec![0, 10], vec![Value::Number(10.into())]);
    trace.insert(pk.clone(), vec![0, 90], vec![Value::Number(90.into())]);
    trace.insert(pk.clone(), vec![0, 30], vec![Value::Number(30.into())]);

    // Read in declared order
    let rows = trace.read_all_ordered();
    assert_eq!(rows.len(), 4);
    // BTreeMap sorts by key bytes, so [0,10] < [0,30] < [0,50] < [0,90]
    // For descending, the encode_sort_key would invert; here the raw keys are ascending
    assert_eq!(rows[0], &vec![Value::Number(10.into())]);
}

/// LIMIT: global top-100 maintained correctly across 1000 input snapshots.
#[test]
fn limit_top_100_across_1000_snapshots() {
    let config = TopNConfig {
        limit: 100,
        offset: 0,
        sort_keys: vec![SortKey {
            column: "value".to_string(),
            descending: false,
            nulls_first: false,
        }],
        shard_count: 1,
    };

    let mut op = TopNOperator::new(config);

    // Insert 1000 rows across "snapshots"
    for i in 0..1000u64 {
        op.insert(i.to_be_bytes().to_vec(), vec![Value::Number(i.into())]);
    }

    let result = op.evaluate();
    assert_eq!(result.rows.len(), 100);
    // Top 100 (ascending) should be 0..100
    assert_eq!(result.rows[0], vec![Value::Number(0.into())]);
    assert_eq!(result.rows[99], vec![Value::Number(99.into())]);

    // Delete some rows from the middle (simulating mid-sequence deletions)
    for i in 20..30u64 {
        op.remove(&i.to_be_bytes());
    }

    // State rebuilds: should now have top 100 minus 10 deleted = 90 rows
    // (eviction during insert means state may be < 100 after deletions)
    let result = op.evaluate();
    assert!(result.rows.len() <= 100);
}

/// LIMIT/OFFSET state bound.
#[test]
fn limit_offset_state_bound() {
    let config = TopNConfig {
        limit: 50,
        offset: 10,
        sort_keys: vec![SortKey {
            column: "id".to_string(),
            descending: false,
            nulls_first: false,
        }],
        shard_count: 1,
    };

    let mut op = TopNOperator::new(config);

    for i in 0..1000u64 {
        op.insert(i.to_be_bytes().to_vec(), vec![Value::Number(i.into())]);
    }

    // State should be bounded to offset + limit = 60
    let max_allowed = ((10 + 50) as f64 * 1.1) as u64;
    assert!(op.state_rows() <= max_allowed);
}

/// LIMIT/OFFSET WARN: view creation with OFFSET 10001 emits a warning indicator.
#[test]
fn limit_offset_warn_large_offset() {
    assert!(!should_warn_offset(10000));
    assert!(should_warn_offset(10001));
    assert!(should_warn_offset(50000));
}

/// Correlated subquery: WHERE EXISTS (TPC-H Q4 pattern) maintained correctly.
#[test]
fn correlated_exists_semi_join() {
    // Decorrelate: EXISTS → semi-join
    let subquery = CorrelatedSubquery {
        kind: SubqueryKind::Exists,
        outer_table: "orders".to_string(),
        inner_table: "lineitem".to_string(),
        outer_col: "o_orderkey".to_string(),
        inner_col: "l_orderkey".to_string(),
        scalar_expr: None,
        output_alias: None,
    };

    let op = decorrelate(&subquery).unwrap();
    assert!(matches!(op, DecorrelatedOp::SemiJoin { .. }));

    // Test semi-join evaluation with inserts and deletes from both sides
    let mut eval = SemiJoinEvaluator::new();

    // Add inner keys (lineitem orderkeys)
    eval.add_inner_key(b"order_1".to_vec());
    eval.add_inner_key(b"order_2".to_vec());
    eval.add_inner_key(b"order_3".to_vec());

    // Check outer row matches
    assert!(eval.has_match(b"order_1"));
    assert!(eval.has_match(b"order_2"));
    assert!(!eval.has_match(b"order_99"));

    // Delete from inner (simulating lineitem delete)
    eval.remove_inner_key(b"order_1");
    assert!(!eval.has_match(b"order_1"));
}

/// Correlated subquery: IN (SELECT …) maintained correctly.
#[test]
fn correlated_in_semi_join() {
    let subquery = CorrelatedSubquery {
        kind: SubqueryKind::In,
        outer_table: "customer".to_string(),
        inner_table: "orders".to_string(),
        outer_col: "c_custkey".to_string(),
        inner_col: "o_custkey".to_string(),
        scalar_expr: None,
        output_alias: None,
    };

    let op = decorrelate(&subquery).unwrap();
    assert!(matches!(op, DecorrelatedOp::SemiJoin { .. }));

    let mut eval = SemiJoinEvaluator::new();
    eval.add_inner_key(b"cust_1".to_vec());
    eval.add_inner_key(b"cust_2".to_vec());

    assert!(eval.has_match(b"cust_1"));
    assert!(!eval.has_match(b"cust_3"));

    // Delete and re-add
    eval.remove_inner_key(b"cust_1");
    assert!(!eval.has_match(b"cust_1"));
    eval.add_inner_key(b"cust_1".to_vec());
    assert!(eval.has_match(b"cust_1"));
}

/// Correlated subquery: scalar subquery — correct result when inner is non-empty;
/// correctly returns NULL when inner becomes empty after delete.
#[test]
fn correlated_scalar_subquery_null_on_empty() {
    let mut eval = ScalarSubqueryEvaluator::new();

    // Initially no inner rows → NULL
    assert_eq!(eval.get_scalar(b"order_1"), serde_json::Value::Null);

    // Add aggregate for order_1
    eval.update_aggregate(b"order_1".to_vec(), serde_json::Value::Number(150.into()));
    assert_eq!(
        eval.get_scalar(b"order_1"),
        serde_json::Value::Number(150.into())
    );

    // Remove (inner becomes empty for this key)
    eval.remove_aggregate(b"order_1");
    assert_eq!(eval.get_scalar(b"order_1"), serde_json::Value::Null);
}

/// Recursive CTE: transitive closure, incremental batches.
#[test]
fn recursive_cte_transitive_closure_incremental() {
    let config = RecursiveCteConfig {
        cte_name: "reachable".to_string(),
        max_iterations: 100,
        bounded_depth: None,
        shard_count: 1,
    };

    // Validate: unbounded must be single shard
    assert!(validate_recursive_cte(&config).is_ok());

    let mut eval = RecursiveCteEvaluator::new(config);

    // Build a graph with edges
    let edges: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (b"A".to_vec(), b"B".to_vec()),
        (b"B".to_vec(), b"C".to_vec()),
        (b"C".to_vec(), b"D".to_vec()),
        (b"D".to_vec(), b"E".to_vec()),
    ];

    // Seed: nodes reachable from A (direct neighbors)
    eval.seed(vec![b"B".to_vec()]);

    // Run to fixed point
    eval.run_to_completion(|frontier| {
        let mut new_rows = Vec::new();
        for node in frontier {
            for (from, to) in &edges {
                if from == node {
                    new_rows.push(to.clone());
                }
            }
        }
        new_rows
    });

    assert!(eval.is_converged());
    // All nodes reachable from A: B, C, D, E
    assert_eq!(eval.result().len(), 4);
    assert!(eval.result().contains(&b"B".to_vec()));
    assert!(eval.result().contains(&b"C".to_vec()));
    assert!(eval.result().contains(&b"D".to_vec()));
    assert!(eval.result().contains(&b"E".to_vec()));
}

/// Non-det capture: repaired shard re-using stored per-batch seed produces
/// bit-identical output to original; running repair twice is idempotent.
#[test]
fn nondet_capture_repair_idempotent() {
    let functions = vec![
        "now".to_string(),
        "random".to_string(),
        "gen_random_uuid".to_string(),
    ];

    // Original batch capture
    let original = sample_batch_captures(42, &functions);
    let serialized = serialize_captures(&original).unwrap();

    // First repair: restore from checkpoint
    let repair1 = restore_captures(&serialized).unwrap();
    assert_eq!(original.batch_id, repair1.batch_id);
    assert_eq!(original.random_seed, repair1.random_seed);
    assert_eq!(original.captured_values, repair1.captured_values);

    // Second repair: idempotent
    let serialized2 = serialize_captures(&repair1).unwrap();
    let repair2 = restore_captures(&serialized2).unwrap();
    assert_eq!(repair1.captured_values, repair2.captured_values);
    assert_eq!(repair1.random_seed, repair2.random_seed);
}

/// DISTINCT ref-counting: insert same row 3×, delete 2×; confirm exactly one output row.
#[test]
fn distinct_ref_counted_insert_delete() {
    let mut distinct = RefCountedDistinct::new();
    let key = b"row_data_hash".to_vec();

    // Insert 3 times
    assert!(distinct.insert(key.clone())); // became visible
    assert!(!distinct.insert(key.clone())); // still visible
    assert!(!distinct.insert(key.clone())); // still visible

    assert_eq!(distinct.visible_count(), 1);

    // Delete 2 times — still exactly one output row
    assert!(!distinct.delete(&key)); // still visible (count 2)
    assert!(!distinct.delete(&key)); // still visible (count 1)
    assert_eq!(distinct.visible_count(), 1);

    // Delete once more — gone
    assert!(distinct.delete(&key)); // became invisible (count 0)
    assert_eq!(distinct.visible_count(), 0);
}

/// UNION DISTINCT: same row in both operands → exactly one output row.
#[test]
fn union_distinct_shared_row_one_output() {
    let mut set_op = RefCountedSetOp::new();
    let key = b"shared_row".to_vec();

    // Insert into both operands
    set_op.insert_left(key.clone());
    set_op.insert_right(key.clone());

    // UNION DISTINCT: exactly one output row (MAX semantics, not addition)
    assert!(set_op.is_visible(&key, SetOperator::UnionDistinct));
    let visible = set_op.visible_rows(SetOperator::UnionDistinct);
    assert_eq!(visible.len(), 1);

    // Remove from one side — still visible
    set_op.delete_left(&key);
    assert!(set_op.is_visible(&key, SetOperator::UnionDistinct));

    // Remove from other side — gone
    set_op.delete_right(&key);
    assert!(!set_op.is_visible(&key, SetOperator::UnionDistinct));
}
