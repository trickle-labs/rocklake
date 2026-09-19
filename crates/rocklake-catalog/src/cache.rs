//! Block cache utilization reporting.
//!
//! SlateDB owns the block cache, so this module reports real counters only
//! when a caller supplies them. Inspection without those counters reports
//! unknown observations and keeps the working-set calculation explicitly as an
//! estimate.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub const WORKING_SET_ESTIMATE_BASIS: &str =
    "estimate: 1 KiB per column plus 2 KiB per data-file header";

/// Thread-safe cache statistics counters.
#[derive(Debug, Default)]
pub struct CacheCounters {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
    /// Total bytes currently in cache.
    pub bytes_used: AtomicU64,
    /// Maximum cache capacity in bytes.
    pub capacity_bytes: AtomicU64,
}

pub fn estimated_working_set_bytes(data_file_count: u64, column_count: u64) -> u64 {
    column_count
        .saturating_mul(1024)
        .saturating_add(data_file_count.saturating_mul(2048))
}

impl CacheCounters {
    pub fn new(capacity_mb: u64) -> Arc<Self> {
        let c = Arc::new(Self::default());
        c.capacity_bytes
            .store(capacity_mb * 1024 * 1024, Ordering::Relaxed);
        c
    }

    pub fn record_hit(&self, bytes: u64) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.bytes_used.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_eviction(&self, bytes: u64) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        let current = self.bytes_used.load(Ordering::Relaxed);
        self.bytes_used
            .store(current.saturating_sub(bytes), Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        if hits.saturating_add(misses) == 0 {
            let mut stats = CacheStats::unavailable();
            stats.capacity_bytes = Some(self.capacity_bytes.load(Ordering::Relaxed));
            return stats;
        }
        let mut stats = CacheStats::observed(hits, misses);
        stats.evictions = Some(self.evictions.load(Ordering::Relaxed));
        stats.bytes_used = Some(self.bytes_used.load(Ordering::Relaxed));
        stats.capacity_bytes = Some(self.capacity_bytes.load(Ordering::Relaxed));
        stats
    }
}

/// A point-in-time snapshot of block cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Observed cache hits, when a cache metrics source was attached.
    pub hits: Option<u64>,
    /// Observed cache misses, when a cache metrics source was attached.
    pub misses: Option<u64>,
    /// Observed hit ratio in range [0.0, 1.0], when counters are available.
    pub hit_ratio: Option<f64>,
    /// Observed evictions, when the cache exposes them.
    pub evictions: Option<u64>,
    /// Observed bytes currently in the cache, when exposed by the cache.
    pub bytes_used: Option<u64>,
    /// Configured cache capacity, when exposed by the serving process.
    pub capacity_bytes: Option<u64>,
    /// Estimated working-set size based on catalog counts.
    pub estimated_working_set_bytes: Option<u64>,
    /// Whether cache activity was observed or unavailable.
    pub observation_status: &'static str,
}

impl CacheStats {
    fn observed(hits: u64, misses: u64) -> Self {
        let total = hits.saturating_add(misses);
        Self {
            hits: Some(hits),
            misses: Some(misses),
            hit_ratio: (total > 0).then_some(hits as f64 / total as f64),
            evictions: None,
            bytes_used: None,
            capacity_bytes: None,
            estimated_working_set_bytes: None,
            observation_status: "observed counters",
        }
    }

    fn unavailable() -> Self {
        Self {
            hits: None,
            misses: None,
            hit_ratio: None,
            evictions: None,
            bytes_used: None,
            capacity_bytes: None,
            estimated_working_set_bytes: None,
            observation_status: "unknown",
        }
    }

    pub fn unknown(data_file_count: u64, column_count: u64) -> Self {
        Self {
            hits: None,
            misses: None,
            hit_ratio: None,
            evictions: None,
            bytes_used: None,
            capacity_bytes: None,
            estimated_working_set_bytes: Some(estimated_working_set_bytes(
                data_file_count,
                column_count,
            )),
            observation_status: "unknown",
        }
    }

    /// Print a human-readable cache utilization report.
    pub fn print(&self) {
        println!("Block Cache Utilization");
        println!("=======================");
        println!("  Hits:             {}", display_optional(self.hits));
        println!("  Misses:           {}", display_optional(self.misses));
        println!(
            "  Hit ratio:        {}",
            self.hit_ratio
                .map(|ratio| format!("{:.1}%", ratio * 100.0))
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!("  Evictions:        {}", display_optional(self.evictions));
        println!(
            "  Bytes used:       {}",
            match (self.bytes_used, self.capacity_bytes) {
                (Some(used), Some(capacity)) => format!(
                    "{} MiB / {} MiB",
                    used / (1024 * 1024),
                    capacity / (1024 * 1024)
                ),
                _ => "unknown".to_string(),
            }
        );
        if let Some(bytes) = self.estimated_working_set_bytes {
            println!("  Working set:      {} bytes (estimate)", bytes);
            println!("  Estimate basis:   {WORKING_SET_ESTIMATE_BASIS}");
        }
        println!();
        println!("  Observation source: {}", self.observation_status);
    }
}

fn display_optional(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Build a cache report without claiming observations that were not collected.
pub async fn cache_utilization(
    _cache_size_mb: u64,
    data_file_count: u64,
    column_count: u64,
) -> CacheStats {
    CacheStats::unknown(data_file_count, column_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_counters_hit_ratio() {
        let c = CacheCounters::new(256);
        for _ in 0..80 {
            c.record_hit(1024);
        }
        for _ in 0..20 {
            c.record_miss();
        }
        let stats = c.snapshot();
        assert_eq!(stats.hits, Some(80));
        assert_eq!(stats.misses, Some(20));
        assert!((stats.hit_ratio.unwrap() - 0.8).abs() < 0.01);
    }

    #[test]
    fn empty_cache_counters_are_unknown() {
        let stats = CacheCounters::new(256).snapshot();
        assert!(stats.hits.is_none());
        assert!(stats.misses.is_none());
        assert_eq!(stats.capacity_bytes, Some(256 * 1024 * 1024));
    }

    #[tokio::test]
    async fn cache_utilization_reports_unknown_observations() {
        let stats = cache_utilization(256, 100, 50).await;
        assert!(stats.hit_ratio.is_none());
        assert!(stats.estimated_working_set_bytes.is_some());
    }

    #[tokio::test]
    async fn cache_utilization_keeps_large_working_set_as_estimate() {
        let stats = cache_utilization(256, 1_000_000, 500_000).await;
        assert!(stats.hit_ratio.is_none());
        assert!(stats.estimated_working_set_bytes.unwrap() > 256 * 1024 * 1024);
    }
}
