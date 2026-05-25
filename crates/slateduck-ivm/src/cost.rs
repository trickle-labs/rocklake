//! Cost guardrails: estimation, budgets, and freshness degradation.
//!
//! Users need visibility and protection before they get an unexpected S3 bill.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Per-view cost budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBudget {
    /// Monthly cost limit in USD (from `WITH (monthly_cost_limit = '$50')`).
    pub monthly_limit_usd: f64,
    /// Whether to degrade freshness when over budget instead of alerting only.
    pub degrade_freshness_on_budget: bool,
    /// Current estimated monthly cost in USD.
    pub estimated_monthly_cost_usd: f64,
}

/// Cost estimation parameters.
#[derive(Debug, Clone)]
pub struct CostEstimateParams {
    /// Input rate: batches per hour (from recent snapshot commit frequency).
    pub batches_per_hour: f64,
    /// Shard count for this matview.
    pub shard_count: u32,
    /// Freshness target duration.
    pub freshness: Duration,
    /// Empirical cost per million rows (USD) — from cost model benchmarks.
    pub cost_per_million_rows_usd: f64,
    /// State amplification factor (accumulated state size / delta size).
    pub state_amplification: f64,
    /// Average rows per batch.
    pub avg_rows_per_batch: f64,
}

/// Cost estimate result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Estimated monthly S3 API cost in USD.
    pub monthly_cost_usd: f64,
    /// Estimated S3 PUTs per month.
    pub monthly_puts: u64,
    /// Estimated S3 GETs per month.
    pub monthly_gets: u64,
    /// Whether cost exceeds the budget.
    pub over_budget: bool,
    /// Recommended freshness (may be wider than target if degrading).
    pub recommended_freshness: Duration,
}

/// Alert severity for cost events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostAlertLevel {
    /// Within budget — no alert.
    None,
    /// Approaching budget (> budget but < 2× budget).
    Warning,
    /// Cost ceiling exceeded (> 2× budget) — burst scenario.
    Critical,
}

impl CostBudget {
    /// Create a new budget with the given monthly limit.
    pub fn new(monthly_limit_usd: f64, degrade_freshness: bool) -> Self {
        Self {
            monthly_limit_usd,
            degrade_freshness_on_budget: degrade_freshness,
            estimated_monthly_cost_usd: 0.0,
        }
    }

    /// Check alert level based on current estimated cost.
    pub fn alert_level(&self) -> CostAlertLevel {
        if self.estimated_monthly_cost_usd <= self.monthly_limit_usd {
            CostAlertLevel::None
        } else if self.estimated_monthly_cost_usd <= self.monthly_limit_usd * 2.0 {
            CostAlertLevel::Warning
        } else {
            CostAlertLevel::Critical
        }
    }

    /// Update the estimated cost.
    pub fn update_estimate(&mut self, estimate: &CostEstimate) {
        self.estimated_monthly_cost_usd = estimate.monthly_cost_usd;
    }

    /// Check if freshness should be degraded.
    pub fn should_degrade_freshness(&self) -> bool {
        self.degrade_freshness_on_budget && self.estimated_monthly_cost_usd > self.monthly_limit_usd
    }
}

impl CostEstimateParams {
    /// Estimate monthly cost based on parameters.
    pub fn estimate(&self) -> CostEstimate {
        let hours_per_month = 730.0;
        let batches_per_month = self.batches_per_hour * hours_per_month;
        let rows_per_month = batches_per_month * self.avg_rows_per_batch;

        // PUTs: one per flush per shard (flush frequency based on freshness).
        let flushes_per_hour = 3600.0 / self.freshness.as_secs_f64();
        let monthly_puts = (flushes_per_hour * hours_per_month * self.shard_count as f64) as u64;

        // GETs: one per read per shard per batch (compaction reads).
        let monthly_gets =
            (batches_per_month * self.shard_count as f64 * self.state_amplification) as u64;

        // Cost: $0.005 per 1000 PUTs, $0.0004 per 1000 GETs (S3 Standard pricing).
        let put_cost = monthly_puts as f64 * 0.000005;
        let get_cost = monthly_gets as f64 * 0.0000004;
        let row_cost = (rows_per_month / 1_000_000.0) * self.cost_per_million_rows_usd;
        let monthly_cost_usd = put_cost + get_cost + row_cost;

        CostEstimate {
            monthly_cost_usd,
            monthly_puts,
            monthly_gets,
            over_budget: false,
            recommended_freshness: self.freshness,
        }
    }
}

/// Compute degraded freshness: widen from target toward 60s proportionally to over-budget ratio.
pub fn compute_degraded_freshness(
    target_freshness: Duration,
    budget_limit: f64,
    estimated_cost: f64,
) -> Duration {
    if estimated_cost <= budget_limit {
        return target_freshness;
    }

    let max_freshness = Duration::from_secs(60);
    let ratio = (estimated_cost / budget_limit).min(12.0); // Cap degradation at 12×.
    let degraded_secs = target_freshness.as_secs_f64() * ratio;
    let clamped = degraded_secs.min(max_freshness.as_secs_f64());
    Duration::from_secs_f64(clamped)
}

/// Format cost estimate for EXPLAIN MATERIALIZED VIEW output.
pub fn format_cost_explanation(estimate: &CostEstimate, budget: Option<&CostBudget>) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Estimated monthly S3 cost: ${:.2}",
        estimate.monthly_cost_usd
    ));
    lines.push(format!("  PUTs/month: {}", estimate.monthly_puts));
    lines.push(format!("  GETs/month: {}", estimate.monthly_gets));

    if let Some(b) = budget {
        lines.push(format!("  Budget: ${:.2}/month", b.monthly_limit_usd));
        match b.alert_level() {
            CostAlertLevel::None => lines.push("  Status: within budget".to_string()),
            CostAlertLevel::Warning => {
                lines.push("  Status: WARNING - approaching budget".to_string())
            }
            CostAlertLevel::Critical => {
                lines.push("  Status: CRITICAL - exceeds 2× budget".to_string())
            }
        }
    }

    lines.join("\n")
}

/// Per-worker cost tracking metric.
#[derive(Debug, Clone, Default)]
pub struct WorkerCostMetrics {
    /// matview → estimated monthly cost.
    pub per_matview: std::collections::HashMap<u64, f64>,
}

impl WorkerCostMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, matview_id: u64, cost: f64) {
        self.per_matview.insert(matview_id, cost);
    }

    pub fn total_monthly_cost(&self) -> f64 {
        self.per_matview.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_estimate_basic() {
        let params = CostEstimateParams {
            batches_per_hour: 100.0,
            shard_count: 4,
            freshness: Duration::from_secs(5),
            cost_per_million_rows_usd: 0.01,
            state_amplification: 2.0,
            avg_rows_per_batch: 1000.0,
        };
        let estimate = params.estimate();
        assert!(estimate.monthly_cost_usd > 0.0);
        assert!(estimate.monthly_puts > 0);
        assert!(estimate.monthly_gets > 0);
    }

    #[test]
    fn cost_budget_alert_levels() {
        let mut budget = CostBudget::new(50.0, false);
        assert_eq!(budget.alert_level(), CostAlertLevel::None);

        budget.estimated_monthly_cost_usd = 60.0;
        assert_eq!(budget.alert_level(), CostAlertLevel::Warning);

        budget.estimated_monthly_cost_usd = 110.0;
        assert_eq!(budget.alert_level(), CostAlertLevel::Critical);
    }

    #[test]
    fn freshness_degradation() {
        let target = Duration::from_secs(5);
        // Within budget — no degradation.
        let result = compute_degraded_freshness(target, 50.0, 40.0);
        assert_eq!(result, target);

        // Over budget — degrades proportionally.
        let result = compute_degraded_freshness(target, 50.0, 100.0);
        assert!(result > target);
        assert!(result <= Duration::from_secs(60));
    }

    #[test]
    fn worker_cost_metrics() {
        let mut metrics = WorkerCostMetrics::new();
        metrics.update(1, 10.0);
        metrics.update(2, 20.0);
        assert_eq!(metrics.total_monthly_cost(), 30.0);
    }
}
