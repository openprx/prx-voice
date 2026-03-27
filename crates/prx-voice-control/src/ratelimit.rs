//! Simple in-memory rate limiter for API requests.
//! Uses a sliding window counter per key (tenant or IP).

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limit check result with header values.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Maximum requests per window.
    pub limit: u64,
    /// Remaining requests in the current window.
    pub remaining: u64,
    /// Seconds until the window resets.
    pub reset_after_sec: u64,
}

/// Rate limiter configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Max requests per window.
    pub max_requests: u64,
    /// Window duration.
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
        }
    }
}

struct WindowCounter {
    count: u64,
    window_start: Instant,
}

/// In-memory rate limiter.
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    counters: Arc<Mutex<HashMap<String, WindowCounter>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a request is allowed for the given key.
    /// Returns a `RateLimitInfo` with allow status and header values.
    pub fn check(&self, key: &str) -> RateLimitInfo {
        let mut counters = self.counters.lock();
        let now = Instant::now();

        let counter = counters.entry(key.to_string()).or_insert(WindowCounter {
            count: 0,
            window_start: now,
        });

        // Reset if window expired
        if now.duration_since(counter.window_start) >= self.config.window {
            counter.count = 0;
            counter.window_start = now;
        }

        counter.count += 1;
        let remaining = self.config.max_requests.saturating_sub(counter.count);
        let reset_after = self
            .config
            .window
            .as_secs()
            .saturating_sub(now.duration_since(counter.window_start).as_secs());

        if counter.count > self.config.max_requests {
            RateLimitInfo {
                allowed: false,
                limit: self.config.max_requests,
                remaining: 0,
                reset_after_sec: reset_after,
            }
        } else {
            RateLimitInfo {
                allowed: true,
                limit: self.config.max_requests,
                remaining,
                reset_after_sec: reset_after,
            }
        }
    }

    /// Get current request count for a key (for testing).
    pub fn current_count(&self, key: &str) -> u64 {
        self.counters.lock().get(key).map(|c| c.count).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_within_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
        });
        for _ in 0..5 {
            let info = limiter.check("tenant-1");
            assert!(info.allowed);
        }
    }

    #[test]
    fn blocks_over_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 2,
            window: Duration::from_secs(60),
        });
        limiter.check("t1");
        limiter.check("t1");
        let info = limiter.check("t1");
        assert!(!info.allowed);
        assert_eq!(info.remaining, 0);
        assert_eq!(info.limit, 2);
    }

    #[test]
    fn different_keys_independent() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
        });
        let i1 = limiter.check("k1");
        let i2 = limiter.check("k2");
        assert!(i1.allowed);
        assert!(i2.allowed);

        let i3 = limiter.check("k1");
        assert!(!i3.allowed); // k1 over limit
        let i4 = limiter.check("k2");
        assert!(!i4.allowed); // k2 over limit
    }

    #[test]
    fn remaining_decreases() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
        });
        let i1 = limiter.check("t");
        assert_eq!(i1.remaining, 4);
        let i2 = limiter.check("t");
        assert_eq!(i2.remaining, 3);
    }
}
