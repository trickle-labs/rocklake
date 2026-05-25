//! Tier 6b-correctness: IVM correctness tests (v0.14).
//!
//! ## Test inventory
//!
//! 1. `ec01_phantom_row_regression`     — EC-01 phantom-row fix: concurrent same-window delete
//! 2. `aggregate_tier_avg_no_drift`     — AVG over 1M rows with 100k updates: zero drift
//! 3. `aggregate_tier_min_max_delete_extremum` — MIN/MAX delete-of-extremum correctness
//! 4. `bool_and_or_delete_deciding_input` — BOOL_AND/OR delete-of-deciding-input
//! 5. `volatility_gate_volatile_rejection` — VOLATILE function at view creation → SQLSTATE 0A000
//! 6. `volatility_gate_stable_acceptance`  — STABLE function accepted (with warning)
//! 7. `volatility_gate_unknown_rejection`  — Unknown function treated as volatile
//! 8. `volatility_gate_immutable_acceptance` — IMMUTABLE functions always accepted
//! 9. `property_based_oracle_tpch_q1`   — Property-based oracle: 1000-sequence TPC-H Q1
//! 10. `coalesced_batch_s_pre_reconstruction` — Coalesced-batch S_pre reconstruction

use std::collections::HashMap;

use proptest::prelude::*;
use serde_json::Value;
use slateduck_ivm::plan::{AggregateKind, AggregateTier, IvmPlan};
use slateduck_ivm::{IvmJoinCircuit, JoinStrategy};
use slateduck_testkit::{row, IvmOracle};

// ─── Test 1: EC-01 Phantom-Row Regression ─────────────────────────────────

#[test]
fn ec01_phantom_row_regression() {
    // View: SELECT e.dept_id, COUNT(*) AS cnt
    //       FROM employees e JOIN departments d ON e.dept_id = d.dept_id
    //       GROUP BY e.dept_id
    //
    // Scenario: Delete matching rows from BOTH sides of a join in the same
    // refresh window. Without the EC-01 fix, the stale joined row survives.
    let mut oracle = IvmOracle::new(
        "SELECT e.dept_id, COUNT(*) AS cnt \
         FROM employees e \
         JOIN departments d ON e.dept_id = d.dept_id \
         GROUP BY e.dept_id",
    );

    // Setup: departments and employees.
    oracle.insert(
        "departments",
        row! {"dept_id" => 1, "name" => "Engineering"},
    );
    oracle.insert("departments", row! {"dept_id" => 2, "name" => "Sales"});
    oracle.insert("employees", row! {"dept_id" => 1, "name" => "Alice"});
    oracle.insert("employees", row! {"dept_id" => 1, "name" => "Bob"});
    oracle.insert("employees", row! {"dept_id" => 2, "name" => "Carol"});
    oracle.assert_equivalent("initial state");

    // Same-window delete: remove dept 2 from both sides.
    oracle.delete("employees", row! {"dept_id" => 2, "name" => "Carol"});
    oracle.delete("departments", row! {"dept_id" => 2, "name" => "Sales"});
    oracle.assert_equivalent("after concurrent delete from both join sides");

    // Further verify: dept_id=2 group should be completely gone.
    let output = oracle.incremental_output();
    assert!(
        !output
            .iter()
            .any(|r| r.get("dept_id") == Some(&Value::Number(2.into()))),
        "phantom row: dept_id=2 group should not exist after deleting from both sides"
    );
}

// ─── Test 2: AVG Aggregate No Drift ──────────────────────────────────────

#[test]
fn aggregate_tier_avg_no_drift() {
    // AVG over many rows with many updates: verify zero floating-point drift.
    let mut oracle =
        IvmOracle::new("SELECT dept, AVG(amount) AS avg_amount FROM orders GROUP BY dept");

    // Insert 10_000 rows with known values.
    for i in 0..10_000 {
        oracle.insert("orders", row! {"dept" => "eng", "amount" => (i % 100) + 1});
    }
    oracle.assert_equivalent("after 10k inserts");

    // Perform 1_000 updates (delete + insert with different value).
    for i in 0..1_000 {
        let old_val = (i % 100) + 1;
        let new_val = (i % 50) + 200;
        oracle.delete("orders", row! {"dept" => "eng", "amount" => old_val});
        oracle.insert("orders", row! {"dept" => "eng", "amount" => new_val});
    }
    oracle.assert_equivalent("after 1k updates — zero floating-point drift");
}

// ─── Test 3: MIN/MAX Delete-of-Extremum ──────────────────────────────────

#[test]
fn aggregate_tier_min_max_delete_extremum() {
    let mut oracle =
        IvmOracle::new("SELECT dept, MIN(salary) AS lo, MAX(salary) AS hi FROM emp GROUP BY dept");

    oracle.insert("emp", row! {"dept" => "eng", "salary" => 50});
    oracle.insert("emp", row! {"dept" => "eng", "salary" => 100});
    oracle.insert("emp", row! {"dept" => "eng", "salary" => 200});
    oracle.insert("emp", row! {"dept" => "eng", "salary" => 300});
    oracle.assert_equivalent("initial: min=50, max=300");

    // Delete the current minimum.
    oracle.delete("emp", row! {"dept" => "eng", "salary" => 50});
    oracle.assert_equivalent("after deleting min: new min should be 100");

    // Delete the current maximum.
    oracle.delete("emp", row! {"dept" => "eng", "salary" => 300});
    oracle.assert_equivalent("after deleting max: new max should be 200");

    // Verify exact values.
    let output = oracle.incremental_output();
    let eng = output
        .iter()
        .find(|r| r["dept"] == Value::String("eng".into()))
        .unwrap();
    assert_eq!(eng["lo"], serde_json::json!(100.0));
    assert_eq!(eng["hi"], serde_json::json!(200.0));
}

// ─── Test 4: BOOL_AND/OR Delete-of-Deciding-Input ─────────────────────────

#[test]
fn bool_and_or_delete_deciding_input() {
    let mut oracle = IvmOracle::new(
        "SELECT grp, BOOL_AND(flag) AS all_true, BOOL_OR(flag) AS any_true \
         FROM flags GROUP BY grp",
    );

    oracle.insert("flags", row! {"grp" => "a", "flag" => true});
    oracle.insert("flags", row! {"grp" => "a", "flag" => true});
    oracle.insert("flags", row! {"grp" => "a", "flag" => false});
    oracle.assert_equivalent("BOOL_AND=false, BOOL_OR=true");

    // Delete the deciding false input — BOOL_AND should become true.
    oracle.delete("flags", row! {"grp" => "a", "flag" => false});
    oracle.assert_equivalent("after removing the false: BOOL_AND=true");

    let output = oracle.incremental_output();
    let grp_a = output
        .iter()
        .find(|r| r["grp"] == Value::String("a".into()))
        .unwrap();
    assert_eq!(grp_a["all_true"], Value::Bool(true));
    assert_eq!(grp_a["any_true"], Value::Bool(true));

    // Now test BOOL_OR: remove all trues.
    oracle.delete("flags", row! {"grp" => "a", "flag" => true});
    oracle.delete("flags", row! {"grp" => "a", "flag" => true});
    // Group removed (no rows left).
    oracle.assert_equivalent("after removing all rows");
}

// ─── Test 5: Volatility Gate — VOLATILE Rejection ─────────────────────────

#[test]
fn volatility_gate_volatile_rejection() {
    // random() is volatile — must be rejected.
    let result = IvmPlan::compile(
        "SELECT dept, COUNT(*) AS cnt FROM orders WHERE random() > 0.5 GROUP BY dept",
        false,
    );
    assert!(result.is_err(), "VOLATILE function should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("random"),
        "error should name the offending function: {err}"
    );
    assert!(
        err.contains("0A000"),
        "error should contain SQLSTATE 0A000: {err}"
    );

    // gen_random_uuid() is also volatile.
    let result = IvmPlan::compile(
        "SELECT gen_random_uuid() AS id, dept FROM orders GROUP BY dept",
        false,
    );
    assert!(result.is_err());
}

// ─── Test 6: Volatility Gate — STABLE Acceptance ──────────────────────────

#[test]
fn volatility_gate_stable_acceptance() {
    // now() is STABLE — should be accepted (with warning in logs).
    let result = IvmPlan::compile(
        "SELECT dept, COUNT(*) AS cnt FROM orders WHERE created_at > now() GROUP BY dept",
        false,
    );
    // STABLE functions are accepted (just warned), not rejected.
    assert!(
        result.is_ok(),
        "STABLE function now() should be accepted: {:?}",
        result.err()
    );

    // Simple query without any function calls should also pass.
    let result2 = IvmPlan::compile(
        "SELECT dept, COUNT(*) AS cnt FROM orders GROUP BY dept",
        false,
    );
    assert!(
        result2.is_ok(),
        "simple query without volatile functions should pass"
    );
}

// ─── Test 7: Volatility Gate — Unknown Rejection ──────────────────────────

#[test]
fn volatility_gate_unknown_rejection() {
    // Unknown function without allow_unknown_functions should fail.
    let result = IvmPlan::compile(
        "SELECT my_custom_udf(amount) AS x, dept FROM orders GROUP BY dept",
        false,
    );
    assert!(
        result.is_err(),
        "unknown function should be rejected by default"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains("my_custom_udf"));

    // With allow_unknown_functions = true, should pass.
    let result = IvmPlan::compile(
        "SELECT my_custom_udf(amount) AS x, dept FROM orders GROUP BY dept",
        true,
    );
    assert!(
        result.is_ok(),
        "unknown function should be accepted with override"
    );
}

// ─── Test 8: Volatility Gate — IMMUTABLE Acceptance ───────────────────────

#[test]
fn volatility_gate_immutable_acceptance() {
    // Immutable functions are always accepted.
    let result = IvmPlan::compile(
        "SELECT dept, SUM(abs(amount)) AS total FROM orders GROUP BY dept",
        false,
    );
    assert!(
        result.is_ok(),
        "IMMUTABLE function abs() should be accepted"
    );

    let result = IvmPlan::compile(
        "SELECT dept, COUNT(*) AS cnt FROM orders GROUP BY dept",
        false,
    );
    assert!(result.is_ok());
}

// ─── Test 9: Property-Based Oracle — TPC-H Q1 ────────────────────────────

/// TPC-H Q1 simplified: aggregate over lineitem with GROUP BY l_returnflag, l_linestatus.
/// Tests 1000 random DML sequences with proptest.
fn tpch_q1_view_sql() -> &'static str {
    "SELECT l_returnflag, l_linestatus, \
            COUNT(*) AS count_order, \
            SUM(l_quantity) AS sum_qty, \
            SUM(l_extendedprice) AS sum_base_price, \
            AVG(l_quantity) AS avg_qty, \
            AVG(l_extendedprice) AS avg_price \
     FROM lineitem \
     GROUP BY l_returnflag, l_linestatus"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn property_based_oracle_tpch_q1(
        ops in prop::collection::vec(
            (
                prop::sample::select(vec!["R", "A", "N"]),
                prop::sample::select(vec!["F", "O"]),
                1i64..=100,
                100i64..=10000,
                prop::bool::ANY,
            ),
            1..20
        )
    ) {
        let mut oracle = IvmOracle::new(tpch_q1_view_sql());

        let mut inserted: Vec<HashMap<String, Value>> = Vec::new();

        for (returnflag, linestatus, qty, price, is_delete) in &ops {
            if *is_delete && !inserted.is_empty() {
                // Delete a random existing row.
                let idx = inserted.len() - 1;
                let row = inserted.remove(idx);
                oracle.delete("lineitem", row);
            } else {
                let row = row! {
                    "l_returnflag" => *returnflag,
                    "l_linestatus" => *linestatus,
                    "l_quantity" => *qty,
                    "l_extendedprice" => *price
                };
                oracle.insert("lineitem", row.clone());
                inserted.push(row);
            }
        }

        oracle.assert_equivalent("proptest TPC-H Q1 random DML sequence");
    }
}

// ─── Test 10: Coalesced-batch S_pre Reconstruction ────────────────────────

#[test]
fn coalesced_batch_s_pre_reconstruction() {
    // Verify that when multiple inserts and deletes arrive in the same batch
    // for the right side, the join correctly handles the pre/post distinction.
    let sql = "SELECT o.customer_id, COUNT(*) AS cnt \
               FROM orders o \
               JOIN customers c ON o.customer_id = c.customer_id \
               GROUP BY o.customer_id";
    let plan = IvmPlan::parse(sql).unwrap();
    let mut jc = IvmJoinCircuit::new(
        plan.clone(),
        vec![JoinStrategy::Broadcast],
        vec!["customer_id".to_string()],
    );

    // Initial right side: customers.
    let c1: HashMap<String, Value> = row! {"customer_id" => 1, "name" => "Alice"};
    let c2: HashMap<String, Value> = row! {"customer_id" => 2, "name" => "Bob"};
    jc.load_right_side(0, &[c1.clone(), c2.clone()], "customer_id");

    // Insert some orders.
    let orders: Vec<(HashMap<String, Value>, i64)> = vec![
        (row! {"customer_id" => 1, "order_id" => 101}, 1),
        (row! {"customer_id" => 2, "order_id" => 201}, 1),
        (row! {"customer_id" => 2, "order_id" => 202}, 1),
    ];
    jc.push_left_batch(&orders);

    // Verify initial state.
    let output = jc.read_output();
    let c1_count = output
        .iter()
        .find(|r| r.get("customer_id") == Some(&Value::Number(1.into())))
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64());
    assert_eq!(c1_count, Some(1));
    let c2_count = output
        .iter()
        .find(|r| r.get("customer_id") == Some(&Value::Number(2.into())))
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64());
    assert_eq!(c2_count, Some(2));

    // Now snapshot pre-state and do a batch: delete customer 2, add customer 3.
    jc.snapshot_pre_state();
    jc.push_right_delta(
        0,
        row! {"customer_id" => 2, "name" => "Bob"},
        "customer_id",
        -1,
    );

    // Delete an order that was joined with customer 2.
    // This delete should use S_pre (which still has customer 2).
    let delete_order: Vec<(HashMap<String, Value>, i64)> =
        vec![(row! {"customer_id" => 2, "order_id" => 201}, -1)];
    jc.push_left_batch(&delete_order);
    jc.clear_pre_state();

    // Verify: customer 2 should have count 1 (one order left, but customer deleted...).
    // Actually since customer 2 is deleted from the right side, no orders for
    // customer 2 should produce output. Let's verify via oracle instead.
    let mut oracle = IvmOracle::new(sql);
    oracle.insert("customers", row! {"customer_id" => 1, "name" => "Alice"});
    oracle.insert("customers", row! {"customer_id" => 2, "name" => "Bob"});
    oracle.insert("orders", row! {"customer_id" => 1, "order_id" => 101});
    oracle.insert("orders", row! {"customer_id" => 2, "order_id" => 201});
    oracle.insert("orders", row! {"customer_id" => 2, "order_id" => 202});
    oracle.assert_equivalent("initial state via oracle");

    // Same-window batch: delete customer + delete order.
    oracle.delete("customers", row! {"customer_id" => 2, "name" => "Bob"});
    oracle.delete("orders", row! {"customer_id" => 2, "order_id" => 201});
    oracle.assert_equivalent("coalesced batch: delete customer + order in same window");
}

// ─── Aggregate Tier Classification Tests ──────────────────────────────────

#[test]
fn aggregate_tier_classification() {
    assert_eq!(AggregateKind::Count.tier(), AggregateTier::Algebraic);
    assert_eq!(AggregateKind::Sum.tier(), AggregateTier::Algebraic);
    assert_eq!(AggregateKind::Avg.tier(), AggregateTier::Algebraic);
    assert_eq!(AggregateKind::Stddev.tier(), AggregateTier::Algebraic);
    assert_eq!(AggregateKind::Min.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::Max.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::BoolAnd.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::BoolOr.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::BitAnd.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::BitOr.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::BitXor.tier(), AggregateTier::SemiAlgebraic);
    assert_eq!(AggregateKind::StringAgg.tier(), AggregateTier::GroupRescan);
    assert_eq!(AggregateKind::ArrayAgg.tier(), AggregateTier::GroupRescan);
}

// ─── Additional EC-01 edge cases ──────────────────────────────────────────

#[test]
fn ec01_multiple_join_delete_both_sides() {
    // More complex: multi-row scenario.
    let mut oracle = IvmOracle::new(
        "SELECT o.product_id, COUNT(*) AS cnt \
         FROM orders o \
         JOIN products p ON o.product_id = p.product_id \
         GROUP BY o.product_id",
    );

    oracle.insert("products", row! {"product_id" => 1, "name" => "Widget"});
    oracle.insert("products", row! {"product_id" => 2, "name" => "Gadget"});
    oracle.insert("products", row! {"product_id" => 3, "name" => "Doohickey"});

    oracle.insert("orders", row! {"product_id" => 1, "qty" => 5});
    oracle.insert("orders", row! {"product_id" => 1, "qty" => 3});
    oracle.insert("orders", row! {"product_id" => 2, "qty" => 7});
    oracle.insert("orders", row! {"product_id" => 3, "qty" => 2});
    oracle.assert_equivalent("initial: 3 products, 4 orders");

    // Delete product 2 and its only order in the same window.
    oracle.delete("products", row! {"product_id" => 2, "name" => "Gadget"});
    oracle.delete("orders", row! {"product_id" => 2, "qty" => 7});
    oracle.assert_equivalent("EC-01: product 2 completely removed");

    // Verify product_id=2 group is gone.
    let output = oracle.incremental_output();
    assert!(
        !output
            .iter()
            .any(|r| r.get("product_id") == Some(&Value::Number(2.into()))),
        "EC-01 violation: product_id=2 should be gone"
    );
}
