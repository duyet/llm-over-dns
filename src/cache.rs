use hickory_server::proto::rr::Record;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Thread-safe in-memory cache for DNS records with TTL support.
#[derive(Debug)]
pub struct DnsCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    records: Vec<Record>,
    expires_at: Instant,
}

impl DnsCache {
    /// Creates a new cache with the given Time-To-Live (TTL).
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
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
        entries.insert(key_lower, entry);
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
}
