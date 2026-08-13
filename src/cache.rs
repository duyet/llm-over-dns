use hickory_server::proto::rr::Record;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Thread-safe in-memory cache for DNS records with TTL support.
#[derive(Debug)]
pub struct DnsCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
    /// Hard ceiling on retained entries. 0 means unbounded.
    max_entries: usize,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    records: Vec<Record>,
    expires_at: Instant,
}

impl DnsCache {
    /// Creates an unbounded cache with the given Time-To-Live (TTL).
    pub fn new(ttl: Duration) -> Self {
        Self::with_capacity(ttl, 0)
    }

    /// Creates a cache with the given TTL and a ceiling on retained entries.
    ///
    /// `max_entries` of 0 leaves the cache unbounded. Every distinct query name
    /// inserts an entry that survives a full TTL, so an unbounded cache lets a
    /// stream of unique names grow memory without limit.
    pub fn with_capacity(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
            max_entries,
        }
    }

    /// Retrieves cached DNS records for a given key if they exist and are not expired.
    pub async fn get(&self, key: &str) -> Option<Vec<Record>> {
        let key_lower = key.to_lowercase();
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(&key_lower) {
            if Instant::now() < entry.expires_at {
                return Some(entry.records.clone());
            }
        }
        None
    }

    /// Caches the given records for the specified key.
    pub async fn insert(&self, key: &str, records: Vec<Record>) {
        if self.ttl.is_zero() {
            return;
        }
        let key_lower = key.to_lowercase();
        let expires_at = Instant::now() + self.ttl;
        let entry = CacheEntry {
            records,
            expires_at,
        };
        let mut entries = self.entries.write().await;

        // Make room before growing past the ceiling. The periodic cleanup only
        // runs every 30s, which is far slower than an attacker can insert.
        if self.max_entries > 0
            && entries.len() >= self.max_entries
            && !entries.contains_key(&key_lower)
        {
            let now = Instant::now();
            entries.retain(|_, e| now < e.expires_at);

            // Still full of live entries: evict whichever expires soonest so the
            // cache keeps turning over instead of freezing on its first 10k keys.
            if entries.len() >= self.max_entries {
                if let Some(soonest) = entries
                    .iter()
                    .min_by_key(|(_, e)| e.expires_at)
                    .map(|(k, _)| k.clone())
                {
                    entries.remove(&soonest);
                }
            }
        }

        entries.insert(key_lower, entry);
    }

    /// Number of entries currently retained, including any not yet cleaned up.
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Returns true when no entries are retained.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Clears all entries from the cache.
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// Removes expired entries from the cache.
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        let now = Instant::now();
        entries.retain(|_, entry| now < entry.expires_at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_server::proto::rr::rdata::TXT;
    use hickory_server::proto::rr::{Name, RData};

    fn create_test_record(name_str: &str, text: &str) -> Record {
        let name = Name::from_utf8(name_str).unwrap();
        let txt = TXT::new(vec![text.to_string()]);
        Record::from_rdata(name, 300, RData::TXT(txt))
    }

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = DnsCache::new(Duration::from_secs(10));
        let record = create_test_record("example.com.", "hello");
        let records = vec![record];

        cache.insert("example.com", records.clone()).await;

        let cached = cache.get("example.com").await;
        assert!(cached.is_some());
        let cached_records = cached.unwrap();
        assert_eq!(cached_records.len(), 1);
        assert_eq!(cached_records[0].name.to_string(), "example.com.");

        if let RData::TXT(txt) = &cached_records[0].data {
            assert_eq!(&*txt.txt_data[0], b"hello");
        } else {
            panic!("Expected TXT record data");
        }
    }

    #[tokio::test]
    async fn test_cache_case_insensitivity() {
        let cache = DnsCache::new(Duration::from_secs(10));
        let record = create_test_record("example.com.", "hello");
        cache.insert("ExAmPlE.CoM", vec![record]).await;

        let cached = cache.get("eXaMpLe.cOm").await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = DnsCache::new(Duration::from_millis(50));
        let record = create_test_record("example.com.", "hello");
        cache.insert("example.com", vec![record]).await;

        // Check immediately
        assert!(cache.get("example.com").await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(cache.get("example.com").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_zero_ttl() {
        let cache = DnsCache::new(Duration::from_secs(0));
        let record = create_test_record("example.com.", "hello");
        cache.insert("example.com", vec![record]).await;

        assert!(cache.get("example.com").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = DnsCache::new(Duration::from_secs(10));
        let record = create_test_record("example.com.", "hello");
        cache.insert("example.com", vec![record]).await;

        assert!(cache.get("example.com").await.is_some());
        cache.clear().await;
        assert!(cache.get("example.com").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let cache = DnsCache::new(Duration::from_millis(20));
        let record1 = create_test_record("example.com.", "hello");
        cache.insert("example.com", vec![record1]).await;

        let record2 = create_test_record("test.com.", "world");
        cache.insert("test.com", vec![record2]).await;

        tokio::time::sleep(Duration::from_millis(30)).await;
        cache.cleanup().await;

        // Both should be gone after cleanup
        assert!(cache.get("example.com").await.is_none());
        assert!(cache.get("test.com").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_respects_capacity_under_unique_keys() {
        // The exhaustion scenario: a stream of distinct query names. Without a
        // ceiling every one is retained for a full TTL.
        let cache = DnsCache::with_capacity(Duration::from_secs(300), 8);

        for i in 0..500 {
            let key = format!("unique-query-{i}.example.com");
            cache
                .insert(&key, vec![create_test_record("example.com.", "x")])
                .await;
        }

        assert!(
            cache.len().await <= 8,
            "cache grew past its ceiling: {} entries",
            cache.len().await
        );
    }

    #[tokio::test]
    async fn test_cache_keeps_turning_over_when_full() {
        // Once full of unexpired entries the cache must still admit new keys,
        // otherwise it freezes on whichever keys happened to arrive first.
        let cache = DnsCache::with_capacity(Duration::from_secs(300), 4);

        for i in 0..4 {
            cache
                .insert(
                    &format!("old-{i}"),
                    vec![create_test_record("example.com.", "old")],
                )
                .await;
        }
        cache
            .insert("fresh", vec![create_test_record("example.com.", "fresh")])
            .await;

        assert!(cache.get("fresh").await.is_some(), "new key was rejected");
        assert!(cache.len().await <= 4);
    }

    #[tokio::test]
    async fn test_cache_updating_existing_key_does_not_evict() {
        // Overwriting a present key does not grow the map, so it must not
        // trigger eviction of some other entry.
        let cache = DnsCache::with_capacity(Duration::from_secs(300), 2);
        cache
            .insert("a", vec![create_test_record("example.com.", "1")])
            .await;
        cache
            .insert("b", vec![create_test_record("example.com.", "2")])
            .await;
        cache
            .insert("a", vec![create_test_record("example.com.", "3")])
            .await;

        assert!(cache.get("a").await.is_some());
        assert!(cache.get("b").await.is_some(), "unrelated key was evicted");
    }

    #[tokio::test]
    async fn test_cache_new_is_unbounded() {
        // DnsCache::new keeps its original unbounded behaviour.
        let cache = DnsCache::new(Duration::from_secs(300));
        for i in 0..100 {
            cache
                .insert(
                    &format!("k{i}"),
                    vec![create_test_record("example.com.", "x")],
                )
                .await;
        }
        assert_eq!(cache.len().await, 100);
    }
}
