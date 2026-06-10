use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A thread-safe Token Bucket rate limiter for IP addresses.
#[derive(Debug)]
pub struct IpRateLimiter {
    clients: Mutex<HashMap<IpAddr, TokenBucket>>,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

impl IpRateLimiter {
    /// Creates a new rate limiter.
    ///
    /// * `refill_rate` - How many requests are allowed per second (e.g., 5.0).
    /// * `burst_limit` - Maximum burst allowed (e.g., 10.0).
    pub fn new(refill_rate: f64, burst_limit: f64) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            max_tokens: burst_limit,
            refill_rate,
        }
    }

    /// Check if a request from the given IP address is allowed.
    ///
    /// Returns `true` if allowed, `false` if rate-limited.
    pub fn check_allowed(&self, ip: IpAddr) -> bool {
        if self.refill_rate <= 0.0 || self.max_tokens <= 0.0 {
            // Disabled
            return true;
        }

        let now = Instant::now();
        let mut clients = self.clients.lock().unwrap();

        let bucket = clients.entry(ip).or_insert_with(|| TokenBucket {
            tokens: self.max_tokens,
            last_update: now,
        });

        // Refill tokens based on time elapsed
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        bucket.last_update = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Cleans up old inactive IP buckets to prevent memory leaks.
    pub fn cleanup(&self, inactive_duration: Duration) {
        let mut clients = self.clients.lock().unwrap();
        let now = Instant::now();
        clients.retain(|_, bucket| now.duration_since(bucket.last_update) < inactive_duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_rate_limiter_allows_burst() {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let limiter = IpRateLimiter::new(1.0, 3.0); // 1 token/sec, burst 3

        // First 3 requests should be allowed immediately
        assert!(limiter.check_allowed(ip));
        assert!(limiter.check_allowed(ip));
        assert!(limiter.check_allowed(ip));

        // 4th request should be blocked
        assert!(!limiter.check_allowed(ip));
    }

    #[test]
    fn test_rate_limiter_refills() {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let limiter = IpRateLimiter::new(5.0, 1.0); // 5 tokens/sec, burst 1

        assert!(limiter.check_allowed(ip));
        assert!(!limiter.check_allowed(ip)); // Empty now

        // Wait 250ms -> should refill ~1.25 tokens
        std::thread::sleep(Duration::from_millis(250));
        assert!(limiter.check_allowed(ip));
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // Disabled via 0 refill rate
        let limiter = IpRateLimiter::new(0.0, 1.0);
        for _ in 0..10 {
            assert!(limiter.check_allowed(ip));
        }

        // Disabled via 0 burst limit
        let limiter2 = IpRateLimiter::new(1.0, 0.0);
        for _ in 0..10 {
            assert!(limiter2.check_allowed(ip));
        }
    }

    #[test]
    fn test_rate_limiter_isolation() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
        let limiter = IpRateLimiter::new(1.0, 1.0);

        assert!(limiter.check_allowed(ip1));
        assert!(!limiter.check_allowed(ip1)); // ip1 is limited

        // ip2 should still be allowed since buckets are isolated
        assert!(limiter.check_allowed(ip2));
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
        let limiter = IpRateLimiter::new(1.0, 1.0);

        assert!(limiter.check_allowed(ip1));

        // Wait 200ms and check ip2 to make its last_update newer.
        std::thread::sleep(Duration::from_millis(200));
        assert!(limiter.check_allowed(ip2));

        // Clean up buckets inactive for > 100ms. ip1 is now ~200ms old (deleted);
        // ip2 was just touched. The 100ms threshold leaves a wide margin against
        // scheduler jitter between touching ip2 and running cleanup, so ip2 is
        // reliably kept (a tighter threshold made this test flaky under load).
        limiter.cleanup(Duration::from_millis(100));

        // Total clients should be 1 (only ip2 remains active/recent)
        let clients = limiter.clients.lock().unwrap();
        assert_eq!(clients.len(), 1);
        assert!(clients.contains_key(&ip2));
        assert!(!clients.contains_key(&ip1));
    }
}
