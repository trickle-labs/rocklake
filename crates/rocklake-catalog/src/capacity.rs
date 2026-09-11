//! Evidence-backed capacity and cost reporting.

#![allow(missing_docs)]

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::cache::estimated_working_set_bytes;
use crate::inspect::InspectResult;

pub const CAPACITY_REPORT_SCHEMA_VERSION: u32 = 1;
const SECONDS_PER_MONTH: f64 = 30.0 * 24.0 * 60.0 * 60.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingFile {
    pub schema_version: u32,
    pub provider: String,
    pub region: String,
    pub effective_date: String,
    pub currency: String,
    pub storage_usd_per_gb_month: f64,
    pub request_cost_per_1000: RequestPricing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPricing {
    pub get: f64,
    pub put: f64,
    pub list: f64,
    pub delete: f64,
}

impl PricingFile {
    pub fn read(path: &Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read pricing file {}: {error}", path.display()))?;
        let pricing: Self = serde_json::from_str(&contents)
            .map_err(|error| format!("invalid pricing file {}: {error}", path.display()))?;
        pricing.validate()?;
        Ok(pricing)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported pricing schema version {}; expected 1",
                self.schema_version
            ));
        }
        if self.provider.is_empty() || self.region.is_empty() || self.currency.is_empty() {
            return Err(
                "pricing provider, region, effective_date, and currency are required".into(),
            );
        }
        chrono::NaiveDate::parse_from_str(&self.effective_date, "%Y-%m-%d")
            .map_err(|_| "pricing effective_date must be YYYY-MM-DD".to_string())?;
        for (name, value) in [
            ("storage_usd_per_gb_month", self.storage_usd_per_gb_month),
            ("get", self.request_cost_per_1000.get),
            ("put", self.request_cost_per_1000.put),
            ("list", self.request_cost_per_1000.list),
            ("delete", self.request_cost_per_1000.delete),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "pricing value {name} must be finite and non-negative"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapacityInput {
    pub read_ops_per_second: f64,
    pub write_ops_per_second: f64,
    pub list_ops_per_second: f64,
    pub delete_ops_per_second: f64,
    pub read_bytes_per_second: u64,
    pub write_bytes_per_second: u64,
    pub catalog_bytes: Option<u64>,
    pub cache_size_mb: u64,
    pub max_sessions: u64,
    pub max_active_scans: u64,
    pub evidence_profile: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapacityReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub catalog: CatalogFacts,
    pub workload: WorkloadFacts,
    pub limits: CapacityLimits,
    pub cache: CacheFacts,
    pub evidence: EvidenceEnvelope,
    pub cost: Option<CostEstimate>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogFacts {
    pub latest_snapshot_id: u64,
    pub snapshot_history: u64,
    pub catalog_bytes: Option<u64>,
    pub schema_count: u64,
    pub table_count: u64,
    pub column_count: u64,
    pub data_file_count: u64,
    pub delete_file_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadFacts {
    pub read_ops_per_second: f64,
    pub write_ops_per_second: f64,
    pub list_ops_per_second: f64,
    pub delete_ops_per_second: f64,
    pub read_bytes_per_second: u64,
    pub write_bytes_per_second: u64,
    pub projected_monthly_requests: RequestCounts,
    pub projected_monthly_bytes: ByteCounts,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestCounts {
    pub get: u64,
    pub put: u64,
    pub list: u64,
    pub delete: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ByteCounts {
    pub read: u64,
    pub write: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapacityLimits {
    pub max_sessions: u64,
    pub max_active_scans: u64,
    pub cache_budget_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheFacts {
    pub estimated_working_set_bytes: u64,
    pub recommended_cache_size_mb: u64,
    pub global_budget_mb: u64,
    pub catalog_budget_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceEnvelope {
    pub profile: String,
    pub max_data_files: u64,
    pub max_read_ops_per_second: f64,
    pub max_write_ops_per_second: f64,
    pub within_envelope: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostEstimate {
    pub provider: String,
    pub region: String,
    pub effective_date: String,
    pub currency: String,
    pub request_cost_usd_per_month: f64,
    pub storage_cost_usd_per_month: Option<f64>,
}

struct Profile {
    max_data_files: u64,
    max_read_ops_per_second: f64,
    max_write_ops_per_second: f64,
}

fn profile(name: &str) -> Option<Profile> {
    match name {
        "small" => Some(Profile {
            max_data_files: 10_000,
            max_read_ops_per_second: 50.0,
            max_write_ops_per_second: 5.0,
        }),
        "medium" => Some(Profile {
            max_data_files: 100_000,
            max_read_ops_per_second: 250.0,
            max_write_ops_per_second: 25.0,
        }),
        "large" => Some(Profile {
            max_data_files: 1_000_000,
            max_read_ops_per_second: 1_000.0,
            max_write_ops_per_second: 100.0,
        }),
        _ => None,
    }
}

pub fn build_report(
    state: &InspectResult,
    input: &CapacityInput,
    pricing: Option<&PricingFile>,
) -> Result<CapacityReport, String> {
    let profile = profile(&input.evidence_profile)
        .ok_or_else(|| "evidence profile must be small, medium, or large".to_string())?;
    for (name, value) in [
        ("read_ops_per_second", input.read_ops_per_second),
        ("write_ops_per_second", input.write_ops_per_second),
        ("list_ops_per_second", input.list_ops_per_second),
        ("delete_ops_per_second", input.delete_ops_per_second),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{name} must be finite and non-negative"));
        }
    }
    if let Some(pricing) = pricing {
        pricing.validate()?;
    }

    let projected_monthly_requests = RequestCounts {
        get: monthly_count(input.read_ops_per_second),
        put: monthly_count(input.write_ops_per_second),
        list: monthly_count(input.list_ops_per_second),
        delete: monthly_count(input.delete_ops_per_second),
    };
    let projected_monthly_bytes = ByteCounts {
        read: monthly_bytes(input.read_bytes_per_second),
        write: monthly_bytes(input.write_bytes_per_second),
    };
    let estimated_working_set_bytes =
        estimated_working_set_bytes(state.data_file_count, state.column_count);
    let recommended_cache_size_mb = estimated_working_set_bytes.div_ceil(1024 * 1024).max(256);
    let within_envelope = state.data_file_count <= profile.max_data_files
        && input.read_ops_per_second <= profile.max_read_ops_per_second
        && input.write_ops_per_second <= profile.max_write_ops_per_second;

    let cost = pricing.map(|pricing| CostEstimate {
        provider: pricing.provider.clone(),
        region: pricing.region.clone(),
        effective_date: pricing.effective_date.clone(),
        currency: pricing.currency.clone(),
        request_cost_usd_per_month: request_cost(
            &projected_monthly_requests,
            &pricing.request_cost_per_1000,
        ),
        storage_cost_usd_per_month: input
            .catalog_bytes
            .map(|bytes| bytes as f64 / 1_000_000_000.0 * pricing.storage_usd_per_gb_month),
    });

    let catalog = CatalogFacts {
        latest_snapshot_id: state.latest_snapshot_id,
        snapshot_history: state.latest_snapshot_id,
        catalog_bytes: input.catalog_bytes,
        schema_count: state.schema_count,
        table_count: state.table_count,
        column_count: state.column_count,
        data_file_count: state.data_file_count,
        delete_file_count: state.delete_file_count,
    };
    let workload = WorkloadFacts {
        read_ops_per_second: input.read_ops_per_second,
        write_ops_per_second: input.write_ops_per_second,
        list_ops_per_second: input.list_ops_per_second,
        delete_ops_per_second: input.delete_ops_per_second,
        read_bytes_per_second: input.read_bytes_per_second,
        write_bytes_per_second: input.write_bytes_per_second,
        projected_monthly_requests,
        projected_monthly_bytes,
    };
    let cache = CacheFacts {
        estimated_working_set_bytes,
        recommended_cache_size_mb,
        global_budget_mb: input.cache_size_mb,
        catalog_budget_mb: input.cache_size_mb,
    };
    let evidence = EvidenceEnvelope {
        profile: input.evidence_profile.clone(),
        max_data_files: profile.max_data_files,
        max_read_ops_per_second: profile.max_read_ops_per_second,
        max_write_ops_per_second: profile.max_write_ops_per_second,
        within_envelope,
    };

    let mut recommendations = Vec::new();
    if !within_envelope {
        recommendations.push(
            "Workload or catalog size exceeds the selected evidence profile; split catalogs or service instances and re-measure.".into(),
        );
    }
    if input.cache_size_mb < recommended_cache_size_mb {
        recommendations.push(format!(
            "Increase the cache budget to at least {recommended_cache_size_mb} MiB or select a smaller catalog working set."
        ));
    }
    if pricing.is_none() {
        recommendations.push(
            "Pass --pricing-file with a dated provider price file to calculate currency estimates."
                .into(),
        );
    }
    if input.read_ops_per_second == 0.0 && input.write_ops_per_second == 0.0 {
        recommendations.push(
            "No request rates were supplied; this report is a structural capacity envelope, not a traffic forecast.".into(),
        );
    }

    Ok(CapacityReport {
        schema_version: CAPACITY_REPORT_SCHEMA_VERSION,
        generated_at: Utc::now().to_rfc3339(),
        catalog,
        workload,
        limits: CapacityLimits {
            max_sessions: input.max_sessions,
            max_active_scans: input.max_active_scans,
            cache_budget_mb: input.cache_size_mb,
        },
        cache,
        evidence,
        cost,
        recommendations,
    })
}

fn monthly_count(per_second: f64) -> u64 {
    (per_second * SECONDS_PER_MONTH).round() as u64
}

fn monthly_bytes(per_second: u64) -> u64 {
    per_second.saturating_mul(30 * 24 * 60 * 60)
}

fn request_cost(counts: &RequestCounts, pricing: &RequestPricing) -> f64 {
    (counts.get as f64 * pricing.get
        + counts.put as f64 * pricing.put
        + counts.list as f64 * pricing.list
        + counts.delete as f64 * pricing.delete)
        / 1000.0
}

impl CapacityReport {
    pub fn print(&self) {
        println!("RockLake Capacity Report");
        println!("========================");
        println!("Catalog files: {}", self.catalog.data_file_count);
        println!("Snapshot history: {}", self.catalog.snapshot_history);
        println!("Evidence profile: {}", self.evidence.profile);
        println!("Within envelope: {}", self.evidence.within_envelope);
        println!(
            "Cache budget: {} MiB (recommended: {} MiB)",
            self.cache.global_budget_mb, self.cache.recommended_cache_size_mb
        );
        println!(
            "Projected monthly requests: GET {} / PUT {} / LIST {} / DELETE {}",
            self.workload.projected_monthly_requests.get,
            self.workload.projected_monthly_requests.put,
            self.workload.projected_monthly_requests.list,
            self.workload.projected_monthly_requests.delete
        );
        if let Some(cost) = &self.cost {
            println!(
                "Estimated monthly request cost: {} {:.4} ({}, {}, effective {})",
                cost.currency,
                cost.request_cost_usd_per_month,
                cost.provider,
                cost.region,
                cost.effective_date
            );
            if let Some(storage) = cost.storage_cost_usd_per_month {
                println!(
                    "Estimated monthly storage cost: {} {:.4}",
                    cost.currency, storage
                );
            }
        }
        for recommendation in &self.recommendations {
            println!("Recommendation: {recommendation}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> InspectResult {
        InspectResult {
            latest_snapshot_id: 10,
            schema_version: 1,
            snapshot_time: String::new(),
            next_snapshot_id: 11,
            next_catalog_id: 1,
            next_file_id: 101,
            schema_count: 1,
            table_count: 2,
            column_count: 20,
            data_file_count: 100,
            delete_file_count: 0,
            retain_from: 0,
            writer_epoch: 1,
            format_version: 1,
        }
    }

    #[test]
    fn report_projects_rates_and_costs() {
        let pricing = PricingFile {
            schema_version: 1,
            provider: "test".into(),
            region: "test-1".into(),
            effective_date: "2026-09-11".into(),
            currency: "USD".into(),
            storage_usd_per_gb_month: 1.0,
            request_cost_per_1000: RequestPricing {
                get: 1.0,
                put: 2.0,
                list: 3.0,
                delete: 4.0,
            },
        };
        let report = build_report(
            &state(),
            &CapacityInput {
                read_ops_per_second: 1.0,
                write_ops_per_second: 2.0,
                catalog_bytes: Some(1_000_000_000),
                cache_size_mb: 256,
                max_sessions: 50,
                max_active_scans: 25,
                evidence_profile: "small".into(),
                ..CapacityInput::default()
            },
            Some(&pricing),
        )
        .unwrap();

        assert_eq!(report.workload.projected_monthly_requests.get, 2_592_000);
        assert_eq!(report.workload.projected_monthly_requests.put, 5_184_000);
        assert_eq!(report.cost.unwrap().storage_cost_usd_per_month, Some(1.0));
        assert!(report.evidence.within_envelope);
    }

    #[test]
    fn pricing_rejects_negative_values() {
        let pricing = PricingFile {
            schema_version: 1,
            provider: "test".into(),
            region: "test-1".into(),
            effective_date: "2026-09-11".into(),
            currency: "USD".into(),
            storage_usd_per_gb_month: -1.0,
            request_cost_per_1000: RequestPricing {
                get: 0.0,
                put: 0.0,
                list: 0.0,
                delete: 0.0,
            },
        };
        assert!(pricing.validate().is_err());
    }
}
