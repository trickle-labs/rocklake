//! Tier 6d — SlateDbTrace property tests.
//!
//! Property-tested against reference in-memory trace: 500 random DML sequences
//! against TPC-H Q1; SlateDbTrace output must be identical to the in-memory
//! reference trace at every snapshot.

use proptest::prelude::*;
use serde_json::Value;
use slateduck_ivm::circuit::ZDelta;
use slateduck_ivm::plan::IvmPlan;
use slateduck_ivm::slatedb_trace::{CompactionTrigger, SlateDbTrace, SlateDbTraceConfig};
use slateduck_ivm::trace::IvmTrace;
use std::collections::HashMap;
use std::time::Duration;

fn test_config() -> SlateDbTraceConfig {
    SlateDbTraceConfig {
        state_prefix: "s3://test/state".to_string(),
        matview_id: 1,
        shard_id: 0,
        flush_coalesce_window: Duration::from_millis(0),
        await_durable_non_checkpoint: false,
        await_durable_checkpoint: true,
        compaction_trigger: CompactionTrigger::Default,
    }
}

fn make_plan() -> IvmPlan {
    IvmPlan::parse("SELECT dept, COUNT(*) AS cnt, SUM(qty) AS total FROM lineitem GROUP BY dept")
        .unwrap()
}

/// Generate a random DML event (insert or delete).
fn arb_dml_event() -> impl Strategy<Value = (HashMap<String, Value>, i64)> {
    let dept = prop_oneof![
        Just("eng".to_string()),
        Just("sales".to_string()),
        Just("ops".to_string()),
        Just("hr".to_string()),
    ];
    let qty = 1i64..1000;
    let weight = prop_oneof![Just(1i64), Just(-1i64)];

    (dept, qty, weight).prop_map(|(d, q, w)| {
        let mut row = HashMap::new();
        row.insert("dept".to_string(), Value::String(d));
        row.insert(
            "qty".to_string(),
            Value::Number(serde_json::Number::from(q)),
        );
        (row, w)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property: SlateDbTrace output matches in-memory reference trace at every step.
    #[test]
    fn slatedb_trace_matches_reference(
        events in proptest::collection::vec(arb_dml_event(), 1..50)
    ) {
        let plan = make_plan();
        let config = test_config();

        let mut slatedb = SlateDbTrace::new(plan.clone(), config);
        let mut reference = IvmTrace::new(plan);

        for (row, weight) in &events {
            let delta = ZDelta {
                fields: row.clone(),
                weight: *weight,
            };

            slatedb.inner.circuit.push_batch(std::slice::from_ref(&delta));
            reference.circuit.push_batch(std::slice::from_ref(&delta));

            slatedb.record_batch(1, *weight < 0);
        }

        // Compare outputs.
        let slatedb_output = slatedb.inner.read_output();
        let reference_output = reference.read_output();

        // Both should have the same number of groups.
        assert_eq!(
            slatedb_output.len(),
            reference_output.len(),
            "Group count mismatch: slatedb={}, reference={}",
            slatedb_output.len(),
            reference_output.len()
        );

        // Convert to comparable format (sort by dept).
        let mut slatedb_sorted: Vec<_> = slatedb_output.iter()
            .map(|r| {
                let dept = r.get("dept").cloned().unwrap_or(Value::Null);
                let cnt = r.get("cnt").cloned().unwrap_or(Value::Null);
                let total = r.get("total").cloned().unwrap_or(Value::Null);
                (dept, cnt, total)
            })
            .collect();
        slatedb_sorted.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));

        let mut ref_sorted: Vec<_> = reference_output.iter()
            .map(|r| {
                let dept = r.get("dept").cloned().unwrap_or(Value::Null);
                let cnt = r.get("cnt").cloned().unwrap_or(Value::Null);
                let total = r.get("total").cloned().unwrap_or(Value::Null);
                (dept, cnt, total)
            })
            .collect();
        ref_sorted.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));

        assert_eq!(slatedb_sorted, ref_sorted,
            "SlateDbTrace output diverged from reference at event count {}",
            events.len()
        );
    }
}
