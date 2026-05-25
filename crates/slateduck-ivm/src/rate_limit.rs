//! Per-IP connection and authentication rate limiting.
//!
//! Token-bucket rate limiter with configurable burst for connections,
//! and failed-auth tracking with temporary lockout.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Rate limiter configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum connections per second per IP.
    pub connections_per_sec: u32,
    /// Connection burst allowance.
    pub connection_burst: u32,
    /// Maximum failed auth attempts before lockout.
    pub max_auth_failures: u32,
    /// Window for counting auth failures.
    pub auth_failure_window: Duration,
    /// Lockout duration after exceeding max failures.
    pub lockout_duration: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            connections_per_sec: 10,
            connection_burst: 20,
            max_auth_failures: 5,
            auth_failure_window: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300),
        }
    }
}

/// Token bucket for connection rate limiting.
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
}

/// Auth failure tracking for an IP.
#[derive(Debug, Clone)]
struct AuthFailureTracker {
    failures: Vec<Instant>,
    locked_until: Option<Instant>,
}

impl AuthFailureTracker {
    fn new() -> Self {
        Self {
            failures: Vec::new(),
            locked_until: None,
        }
    }
}

/// Per-IP rate limiter state.
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Per-IP connection rate buckets.
    connection_buckets: HashMap<IpAddr, TokenBucket>,
    /// Per-IP auth failure trackers.
    auth_trackers: HashMap<IpAddr, AuthFailureTracker>,
}

/// Result of a rate limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request allowed.
    Allowed,
    /// Connection rate limit exceeded.
    ConnectionRateLimited,
    /// Auth failure lockout active (SQLSTATE 08004).
    AuthLocked { remaining_secs: u64 },
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            connection_buckets: HashMap::new(),
            auth_trackers: HashMap::new(),
        }
    }

    /// Check if a new connection from this IP is allowed.
    pub fn check_connection(&mut self, ip: IpAddr) -> RateLimitResult {
        // First check auth lockout.
        if let Some(tracker) = self.auth_trackers.get(&ip) {
            if let Some(locked_until) = tracker.locked_until {
                if Instant::now() < locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    return RateLimitResult::AuthLocked {
                        remaining_secs: remaining,
                    };
                }
            }
        }

        // Check connection rate.
        let bucket = self.connection_buckets.entry(ip).or_insert_with(|| {
            TokenBucket::new(
                self.config.connection_burst as f64,
                self.config.connections_per_sec as f64,
            )
        });

        if bucket.try_consume() {
            RateLimitResult::Allowed
        } else {
            RateLimitResult::ConnectionRateLimited
        }
    }

    /// Record a failed authentication attempt.
    pub fn record_auth_failure(&mut self, ip: IpAddr) -> RateLimitResult {
        let window = self.config.auth_failure_window;
        let max_failures = self.config.max_auth_failures;
        let lockout = self.config.lockout_duration;

        let tracker = self
            .auth_trackers
            .entry(ip)
            .or_insert_with(AuthFailureTracker::new);

        let now = Instant::now();

        // Remove old failures outside the window.
        tracker.failures.retain(|t| now.duration_since(*t) < window);

        // Add this failure.
        tracker.failures.push(now);

        // Check if we should lock out.
        if tracker.failures.len() >= max_failures as usize {
            tracker.locked_until = Some(now + lockout);
            tracing::warn!(
                "Rate limit: IP {} locked out for {}s after {} failed auth attempts",
                ip,
                lockout.as_secs(),
                tracker.failures.len(),
            );
            RateLimitResult::AuthLocked {
                remaining_secs: lockout.as_secs(),
            }
        } else {
            RateLimitResult::Allowed
        }
    }

    /// Record a successful authentication (resets failure counter).
    pub fn record_auth_success(&mut self, ip: IpAddr) {
        if let Some(tracker) = self.auth_trackers.get_mut(&ip) {
            tracker.failures.clear();
            tracker.locked_until = None;
        }
    }

    /// Clean up stale entries (call periodically).
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        self.auth_trackers.retain(|_, tracker| {
            // Keep if locked or has recent failures.
            if let Some(locked_until) = tracker.locked_until {
                if now < locked_until {
                    return true;
                }
            }
            !tracker.failures.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn test_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))
    }

    #[test]
    fn connection_rate_limiting() {
        let config = RateLimitConfig {
            connections_per_sec: 2,
            connection_burst: 3,
            ..Default::default()
        };
        let mut limiter = RateLimiter::new(config);
        let ip = test_ip();

        // First 3 should be allowed (burst).
        assert_eq!(limiter.check_connection(ip), RateLimitResult::Allowed);
        assert_eq!(limiter.check_connection(ip), RateLimitResult::Allowed);
        assert_eq!(limiter.check_connection(ip), RateLimitResult::Allowed);

        // 4th should be rate limited.
        assert_eq!(
            limiter.check_connection(ip),
            RateLimitResult::ConnectionRateLimited
        );
    }

    #[test]
    fn auth_failure_lockout() {
        let config = RateLimitConfig {
            max_auth_failures: 3,
            auth_failure_window: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300),
            ..Default::default()
        };
        let mut limiter = RateLimiter::new(config);
        let ip = test_ip();

        // First 2 failures — no lockout.
        assert_eq!(limiter.record_auth_failure(ip), RateLimitResult::Allowed);
        assert_eq!(limiter.record_auth_failure(ip), RateLimitResult::Allowed);

        // 3rd failure — lockout.
        let result = limiter.record_auth_failure(ip);
        assert!(matches!(result, RateLimitResult::AuthLocked { .. }));

        // Subsequent connection attempts should be rejected.
        let result = limiter.check_connection(ip);
        assert!(matches!(result, RateLimitResult::AuthLocked { .. }));
    }

    #[test]
    fn auth_success_resets_failures() {
        let config = RateLimitConfig {
            max_auth_failures: 5,
            ..Default::default()
        };
        let mut limiter = RateLimiter::new(config);
        let ip = test_ip();

        limiter.record_auth_failure(ip);
        limiter.record_auth_failure(ip);
        limiter.record_auth_success(ip);

        // After success, counter should be reset.
        // Need 5 more failures to lock out.
        for _ in 0..4 {
            assert_eq!(limiter.record_auth_failure(ip), RateLimitResult::Allowed);
        }
        let result = limiter.record_auth_failure(ip);
        assert!(matches!(result, RateLimitResult::AuthLocked { .. }));
    }
}
