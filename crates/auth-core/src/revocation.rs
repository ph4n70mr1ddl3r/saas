use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// In-memory cache of revoked token JTIs.
/// Updated by subscribing to `iam.token.revoked` events via NATS.
#[derive(Clone, Default)]
pub struct RevocationCache {
    revoked: Arc<Mutex<HashSet<String>>>,
}

impl RevocationCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a token with the given JTI has been revoked.
    pub fn is_revoked(&self, jti: &str) -> bool {
        self.revoked.lock().unwrap().contains(jti)
    }

    /// Mark a token JTI as revoked.
    pub fn revoke(&self, jti: String) {
        self.revoked.lock().unwrap().insert(jti);
    }

    /// Get the number of currently revoked tokens (for diagnostics).
    pub fn len(&self) -> usize {
        self.revoked.lock().unwrap().len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.revoked.lock().unwrap().is_empty()
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
}
