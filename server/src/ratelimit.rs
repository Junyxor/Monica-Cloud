use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::error::{ApiResult, Error};

const WINDOW: Duration = Duration::from_secs(60);

/// Fixed one-minute window per key (client IP), for login-style endpoints.
pub struct RateLimiter {
    per_minute: u32,
    buckets: DashMap<String, (Instant, u32)>,
}

impl RateLimiter {
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            buckets: DashMap::new(),
        }
    }

    pub fn check(&self, key: &str) -> ApiResult<()> {
        if self.per_minute == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let mut bucket = self.buckets.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(bucket.0) >= WINDOW {
            *bucket = (now, 0);
        }
        bucket.1 += 1;
        if bucket.1 > self.per_minute {
            return Err(Error::RateLimited);
        }
        Ok(())
    }

    pub fn cleanup(&self) {
        let now = Instant::now();
        self.buckets.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit() {
        let limiter = RateLimiter::new(2);
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_ok());
        assert!(limiter.check("ip").is_err());
        assert!(limiter.check("other").is_ok());
    }
}
