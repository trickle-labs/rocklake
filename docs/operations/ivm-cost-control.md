# IVM Cost Control

This guide covers cost management for incremental materialized views (IMVs).

## Cost Modes

SlateDuck supports five cost modes that control the trade-off between freshness
and operational cost:

| Mode | Freshness | S3 Costs | Use Case |
|------|-----------|----------|----------|
| `standard` | Default | Moderate | General workloads |
| `spot` | Variable | Low | Non-critical analytics |
| `conservative` | Relaxed | Minimal | Cost-sensitive environments |
| `balanced` | Moderate | Moderate | Production defaults |
| `latency` | Aggressive | Higher | Real-time dashboards |

## Configuration

Set the cost mode at view creation time:

```sql
CREATE INCREMENTAL MATERIALIZED VIEW revenue_by_dept
  WITH (cost_mode = 'balanced', freshness = '5m')
AS SELECT dept, SUM(amount) FROM orders GROUP BY dept;
```

## Cost Budget

Set a monthly cost budget to prevent runaway spending:

```sql
ALTER MATERIALIZED VIEW revenue_by_dept
  SET (cost_budget_monthly_usd = 50.0);
```

When the budget is approached, SlateDuck automatically degrades freshness
proportionally to stay within budget.

## EXPLAIN MATERIALIZED VIEW

Inspect cost estimates for a view:

```sql
EXPLAIN MATERIALIZED VIEW revenue_by_dept;
```

Output includes:
- Estimated monthly S3 PUT/GET costs
- Flush frequency and coalescing ratio
- Change-buffer compaction effectiveness
- Predicted vs actual cost over last 7 days

## Cost Alerts

SlateDuck emits cost alerts at three levels:

- **Info**: Cost trending above estimate (>120% of budget)
- **Warning**: Cost at 80% of monthly budget
- **Critical**: Cost exceeded monthly budget

Alerts are surfaced through:
- `SHOW MATERIALIZED VIEWS` output
- Prometheus metrics (`slateduck_ivm_cost_monthly_usd`)
- Doctor report (`slateduck-ivm doctor`)

## Freshness Degradation

When cost approaches the budget limit, freshness is widened:

```
effective_freshness = base_freshness × (1 + overshoot_ratio)
```

This ensures the view continues to be maintained (never stops entirely)
while reducing S3 operation frequency.

## Monitoring

Key metrics for cost monitoring:

```
slateduck_ivm_s3_puts_total
slateduck_ivm_s3_gets_total
slateduck_ivm_flush_coalesce_ratio
slateduck_ivm_cost_estimate_monthly_usd
slateduck_ivm_cost_budget_remaining_usd
```
