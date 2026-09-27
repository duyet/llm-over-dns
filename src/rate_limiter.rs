use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Hard ceiling on how many source addresses are tracked at once.
///
/// A bucket is allocated for every source address ever seen, and the only reaping
/// is the periodic [`IpRateLimiter::cleanup`] the server runs every 300 seconds
/// — so between sweeps the table grows by one entry per unseen source, without
/// limit. UDP source addresses are trivially spoofable, the same fact the global
/// LLM concurrency ceiling is built on, so a flood of fabricated sources would
/// otherwise size the limiter's own bookkeeping and force every sweep to walk
/// the whole table under the write lock. Each entry costs on the order of 60
/// bytes, so this ceiling caps the table at a few megabytes. Past it, requests
/// from addresses not already tracked are shed rather than queued, matching the
/// server's existing rule.
pub const MAX_TRACKED_CLIENTS: usize = 65_536;

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
    /// Returns `true` if allowed, `false` if rate-limited, including when the
    /// request is shed because the client table is at
    /// [`MAX_TRACKED_CLIENTS`].
    pub fn check_allowed(&self, ip: IpAddr) -> bool {
        if self.refill_rate <= 0.0 || self.max_tokens <= 0.0 {
            // Disabled
            return true;
        }

        let mut clients = self.clients.lock().unwrap();

        // Sample the clock *inside* the critical section. Sampled before the
        // lock, two threads can both read a time and then serialise on the mutex,
        // so the second writer stores a timestamp earlier than the first wrote.
        // Elapsed time then saturates to zero for that call and rewinds the
        // stored clock, so the next call is credited refill that has already
        // elapsed. Under contention from a single source — precisely the abusive
        // case this limiter exists for — the effective allowed rate drifts above
        // the configured limit.
        let now = Instant::now();

        self.consume(&mut clients, ip, now)
    }

    /// Charge one request from `ip` against its bucket, as of `now`.
    ///
    /// The caller must hold `clients` and must sample `now` after acquiring it.
    fn consume(
        &self,
        clients: &mut HashMap<IpAddr, TokenBucket>,
        ip: IpAddr,
        now: Instant,
    ) -> bool {
        // Refuse to allocate a bucket once the table is full, shedding the
        // request instead of growing the table: an attacker spoofing source
        // addresses would otherwise get to pick the limiter's memory ceiling.
        // Addresses already tracked keep their bucket, so this only ever denies
        // sources the limiter has never seen.
        if clients.len() >= MAX_TRACKED_CLIENTS && !clients.contains_key(&ip) {
            return false;
        }

        let bucket = clients.entry(ip).or_insert_with(|| TokenBucket {
            tokens: self.max_tokens,
            last_update: now,
        });

        // Refill tokens based on time elapsed, ignoring a clock sample that
        // appears to run backwards: crediting the negative elapsed time, and
        // rewinding `last_update` to match, would hand out tokens for time that
        // has not happened yet and let the next call refill from the rewound
        // baseline.
        if now > bucket.last_update {
            let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
            bucket.tokens = (bucket.tokens + elapsed * self.refill_rate).min(self.max_tokens);
            bucket.last_update = now;
        }

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

    #[test]
    fn test_backdated_sample_never_rewinds_last_update() {
        // Regression: the clock used to be read before the lock, so a second
        // thread could store a timestamp earlier than the one already written.
        // The elapsed time then saturates to zero and rewinds `last_update`,
        // which credits the next call with refill time already spent — the
        // effective rate drifts above the configured limit. A back-dated sample
        // must be ignored outright.
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let limiter = IpRateLimiter::new(5.0, 1.0);

        // A bucket whose clock reads an hour into the future, as if another
        // thread's earlier sample landed here.
        let mut clients = HashMap::new();
        clients.insert(
            ip,
            TokenBucket {
                tokens: 0.0,
                last_update: Instant::now() + Duration::from_secs(3600),
            },
        );
        let seeded = clients[&ip].last_update;

        assert!(!limiter.consume(&mut clients, ip, Instant::now()));

        let bucket = &clients[&ip];
        assert_eq!(bucket.tokens, 0.0, "back-dated sample credited refill");
        assert!(
            bucket.last_update >= seeded,
            "back-dated sample rewound the stored timestamp"
        );
    }

    #[test]
    fn test_last_update_moves_forwards_under_contention() {
        // The same defect, observed end to end: with the clock sampled before the
        // lock, concurrent writers from one source publish an out-of-order
        // sequence of timestamps. Sampling inside the critical section makes every
        // published value non-decreasing. Reads are taken under the lock, so a
        // rewind is visible to the observer rather than lost in a race.
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let limiter = IpRateLimiter::new(1000.0, 10.0);

        std::thread::scope(|scope| {
            let observer = scope.spawn(|| {
                let mut highest: Option<Instant> = None;
                for _ in 0..2_000 {
                    let clients = limiter.clients.lock().unwrap();
                    if let Some(last_update) = clients.get(&ip).map(|bucket| bucket.last_update) {
                        if let Some(previous) = highest {
                            assert!(
                                last_update >= previous,
                                "last_update moved backwards: {previous:?} -> {last_update:?}"
                            );
                        }
                        highest = Some(last_update);
                    }
                    drop(clients);
                    std::thread::yield_now();
                }
            });

            for _ in 0..4 {
                scope.spawn(|| {
                    for _ in 0..2_000 {
                        limiter.check_allowed(ip);
                    }
                });
            }

            observer.join().unwrap();
        });
    }

    #[test]
    fn test_client_table_is_bounded() {
        // Regression: one bucket per source address, reaped only every 300s, so
        // a flood of spoofed sources could grow the table without limit. Unseen
        // addresses are shed once the ceiling is reached, while addresses already
        // tracked keep being served from their bucket.
        let limiter = IpRateLimiter::new(1.0, 1.0);

        // Fill the table, one fresh source address per request.
        for i in 0..MAX_TRACKED_CLIENTS {
            assert!(
                limiter.check_allowed(source_ip(i)),
                "source {i} was shed early"
            );
        }

        // The next unseen source is refused instead of allocated.
        assert!(!limiter.check_allowed(source_ip(MAX_TRACKED_CLIENTS)));
        assert_eq!(limiter.clients.lock().unwrap().len(), MAX_TRACKED_CLIENTS);

        // A source already in the table is still served from its bucket.
        assert!(!limiter.check_allowed(source_ip(0)));
    }

    /// Builds a distinct address for each index, within 10.0.0.0/8.
    fn source_ip(index: usize) -> IpAddr {
        let offset = u32::try_from(index).expect("test index fits in u32");
        IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + offset))
    }
}
