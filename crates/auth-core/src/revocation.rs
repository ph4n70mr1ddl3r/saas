use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Maximum time a revoked token JTI stays in cache before eviction.
/// Should be >= the maximum token lifetime (24h) to ensure expired tokens
/// are also caught during revocation checks.
const REVOCATION_TTL: Duration = Duration::from_secs(48 * 3600); // 48 hours

/// In-memory cache of revoked token JTIs with TTL-based eviction.
/// Updated by subscribing to `iam.token.revoked` events via NATS.
#[derive(Clone, Default)]
pub struct RevocationCache {
    /// Maps JTI -> instant when it was revoked.
    revoked: Arc<Mutex<HashMap<String, Instant>>>,
}

impl RevocationCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a token with the given JTI has been revoked.
    pub fn is_revoked(&self, jti: &str) -> bool {
        let cache = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
        match cache.get(jti) {
            Some(revoked_at) => revoked_at.elapsed() < REVOCATION_TTL,
            None => false,
        }
    }

    /// Mark a token JTI as revoked.
    pub fn revoke(&self, jti: String) {
        let mut cache = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(jti, Instant::now());
    }

    /// Get the number of currently revoked tokens (for diagnostics).
    /// Evicts stale entries as part of the count.
    pub fn len(&self) -> usize {
        let mut cache = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
        Self::evict_stale(&mut cache);
        cache.len()
    }

    /// Check if the cache is empty. Evicts stale entries first.
    pub fn is_empty(&self) -> bool {
        let mut cache = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
        Self::evict_stale(&mut cache);
        cache.is_empty()
    }

    /// Remove entries older than REVOCATION_TTL to bound memory usage.
    fn evict_stale(cache: &mut HashMap<String, Instant>) {
        cache.retain(|_, revoked_at| revoked_at.elapsed() < REVOCATION_TTL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = RevocationCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_revoke_and_check() {
        let cache = RevocationCache::new();
        assert!(!cache.is_revoked("jti-001"));

        cache.revoke("jti-001".into());
        assert!(cache.is_revoked("jti-001"));
        assert!(!cache.is_revoked("jti-002"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_multiple_revocations() {
        let cache = RevocationCache::new();
        cache.revoke("jti-a".into());
        cache.revoke("jti-b".into());
        cache.revoke("jti-c".into());

        assert!(cache.is_revoked("jti-a"));
        assert!(cache.is_revoked("jti-b"));
        assert!(cache.is_revoked("jti-c"));
        assert!(!cache.is_revoked("jti-d"));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_duplicate_revoke_is_idempotent() {
        let cache = RevocationCache::new();
        cache.revoke("jti-x".into());
        cache.revoke("jti-x".into());

        assert!(cache.is_revoked("jti-x"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_clone_shares_state() {
        let cache = RevocationCache::new();
        let clone = cache.clone();

        cache.revoke("jti-shared".into());
        assert!(clone.is_revoked("jti-shared"));
    }

    #[test]
    fn test_stale_entry_evicted_on_len() {
        let cache = RevocationCache::new();
        // Insert an entry with a manually-set past timestamp
        {
            let mut inner = cache.revoked.lock().unwrap_or_else(|e| e.into_inner());
            inner.insert("jti-old".into(), Instant::now() - REVOCATION_TTL - Duration::from_secs(1));
        }
        // The stale entry should not be counted as revoked
        assert!(!cache.is_revoked("jti-old"));
        // len() triggers eviction
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_mutex_poisoning_recovery() {
        let cache = RevocationCache::new();
        cache.revoke("jti-before-poison".into());

        // Simulate a poisoned mutex by forcing a panic while holding the lock
        let cache_clone = cache.clone();
        let handle = std::thread::spawn(move || {
            let _lock = cache_clone.revoked.lock().unwrap();
            panic!("intentional test panic");
        });
        let _ = handle.join();

        // Should still be usable after poisoning
        assert!(cache.is_revoked("jti-before-poison"));
        cache.revoke("jti-after-poison".into());
        assert!(cache.is_revoked("jti-after-poison"));
    }
}
