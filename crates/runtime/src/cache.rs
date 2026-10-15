//! Query Result Caching with TTL and Tag Invalidation.
//!
//! Provides the [`QueryCache`] trait and [`InMemoryCache`] implementation
//! supporting time-to-live expiration, capacity limits, and tag-based invalidation.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Trait for query result cache backends (e.g. In-memory, Redis).
pub trait QueryCache: Send + Sync {
    /// Retrieves a cached entry if present and not expired.
    fn get(&self, key: &str) -> Option<Vec<u8>>;

    /// Stores an entry with an optional TTL and associated cache tags.
    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>, tags: &[&str]);

    /// Invalidates all entries associated with the given tag.
    fn invalidate_tag(&self, tag: &str);

    /// Invalidates all entries associated with any of the given tags.
    fn invalidate_tags(&self, tags: &[&str]) {
        for tag in tags {
            self.invalidate_tag(tag);
        }
    }

    /// Clears all entries from the cache.
    fn clear(&self);

    /// Returns the number of active cached entries.
    fn len(&self) -> usize;

    /// Returns `true` if the cache has no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    data: Vec<u8>,
    expires_at: Option<Instant>,
    tags: Vec<String>,
}

#[derive(Debug, Default)]
struct CacheState {
    entries: HashMap<String, CacheEntry>,
    tag_index: HashMap<String, HashSet<String>>,
}

impl CacheState {
    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.entries.remove(key) {
            for tag in entry.tags {
                if let Some(keys) = self.tag_index.get_mut(&tag) {
                    keys.remove(key);
                    if keys.is_empty() {
                        self.tag_index.remove(&tag);
                    }
                }
            }
        }
    }
}

/// In-memory thread-safe query cache with TTL and tag invalidation.
#[derive(Debug)]
pub struct InMemoryCache {
    // Entries and tags form one state: every mutation must be atomic across both.
    state: RwLock<CacheState>,
    max_entries: usize,
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl InMemoryCache {
    /// Creates a new in-memory cache with the given maximum capacity.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            state: RwLock::new(CacheState::default()),
            max_entries,
        }
    }

    /// Prunes expired entries from the cache.
    pub fn prune_expired(&self) {
        if let Ok(mut state) = self.state.write() {
            let now = Instant::now();
            let expired: Vec<String> = state
                .entries
                .iter()
                .filter(|(_, entry)| entry.expires_at.is_some_and(|exp| exp <= now))
                .map(|(key, _)| key.clone())
                .collect();
            for key in expired {
                state.remove(&key);
            }
        }
    }
}

impl QueryCache for InMemoryCache {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let state = self.state.read().ok()?;
        let entry = state.entries.get(key)?;
        if entry.expires_at.is_some_and(|exp| Instant::now() >= exp) {
            return None;
        }
        Some(entry.data.clone())
    }

    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>, tags: &[&str]) {
        if self.max_entries == 0 {
            return;
        }
        if let Ok(mut state) = self.state.write() {
            state.remove(key);
            if state.entries.len() >= self.max_entries {
                if let Some(victim) = state.entries.keys().next().cloned() {
                    state.remove(&victim);
                }
            }
            let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_owned()).collect();
            for tag in &tags {
                state
                    .tag_index
                    .entry(tag.clone())
                    .or_default()
                    .insert(key.to_owned());
            }
            state.entries.insert(
                key.to_owned(),
                CacheEntry {
                    data: value,
                    expires_at: ttl.and_then(|d| Instant::now().checked_add(d)),
                    tags,
                },
            );
        }
    }

    fn invalidate_tag(&self, tag: &str) {
        if let Ok(mut state) = self.state.write() {
            if let Some(keys) = state.tag_index.remove(tag) {
                for key in keys {
                    state.remove(&key);
                }
            }
        }
    }

    fn clear(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CacheState::default();
        }
    }

    fn len(&self) -> usize {
        let now = Instant::now();
        self.state
            .read()
            .map(|state| {
                state
                    .entries
                    .values()
                    .filter(|e| e.expires_at.is_none_or(|exp| exp > now))
                    .count()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn replacing_an_entry_replaces_its_tags() {
        let cache = InMemoryCache::new(10);
        cache.set("k", vec![1], None, &["users"]);
        cache.set("k", vec![2], None, &["posts"]);
        cache.invalidate_tag("users");
        assert_eq!(cache.get("k"), Some(vec![2]));
        cache.invalidate_tag("posts");
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn concurrent_mutations_keep_entries_and_tags_consistent() {
        let cache = std::sync::Arc::new(InMemoryCache::new(8));
        std::thread::scope(|scope| {
            for worker in 0..4 {
                let cache = &cache;
                scope.spawn(move || {
                    for i in 0..1000 {
                        let key = format!("k{}", i % 16);
                        cache.set(&key, vec![worker], Some(Duration::ZERO), &["expired"]);
                        cache.prune_expired();
                        cache.set(&key, vec![worker], None, &["live"]);
                        cache.invalidate_tag("expired");
                        if i % 17 == 0 {
                            cache.clear();
                        }
                    }
                });
            }
        });
        let state = cache.state.read().unwrap();
        assert!(state.entries.len() <= 8);
        for (key, entry) in &state.entries {
            for tag in &entry.tags {
                assert!(
                    state
                        .tag_index
                        .get(tag)
                        .is_some_and(|keys| keys.contains(key))
                );
            }
        }
        for (tag, keys) in &state.tag_index {
            assert!(!keys.is_empty());
            for key in keys {
                assert!(
                    state
                        .entries
                        .get(key)
                        .is_some_and(|entry| entry.tags.contains(tag))
                );
            }
        }
    }

    #[test]
    fn zero_capacity_does_not_store_entries() {
        let cache = InMemoryCache::new(0);
        cache.set("k", vec![1], None, &["users"]);
        assert!(cache.is_empty());
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn expired_entries_are_not_counted_and_refresh_survives_pruning() {
        let cache = InMemoryCache::new(10);
        cache.set("k", vec![1], Some(Duration::ZERO), &["old"]);
        assert_eq!(cache.len(), 0);
        cache.set("k", vec![2], None, &["new"]);
        cache.prune_expired();
        cache.invalidate_tag("old");
        assert_eq!(cache.get("k"), Some(vec![2]));
    }

    #[test]
    fn cache_set_and_get() {
        let cache = InMemoryCache::new(100);
        cache.set("k1", b"v1".to_vec(), None, &["users"]);
        assert_eq!(cache.get("k1"), Some(b"v1".to_vec()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn cache_ttl_expiration() {
        // A short TTL plus a single `sleep` is a race on a loaded machine: the
        // scheduler can hold the thread past the "still live" assertion. Use a
        // generous TTL and gate every assertion on the *measured* elapsed time
        // rather than on the sleep having been accurate.
        const TTL: Duration = Duration::from_millis(500);

        let cache = InMemoryCache::new(100);
        let set_at = Instant::now();
        cache.set("k1", b"v1".to_vec(), Some(TTL), &[]);

        // Only assert liveness if we genuinely are still inside the window.
        if set_at.elapsed() < TTL {
            assert_eq!(cache.get("k1"), Some(b"v1".to_vec()));
        }

        // Sleep until the deadline has definitely passed, however long that takes.
        // Use a small grace period to account for the `Instant::now()` sampled
        // inside `cache.set` being slightly after `set_at`.
        const GRACE: Duration = Duration::from_millis(100);
        while set_at.elapsed() <= TTL + GRACE {
            sleep(Duration::from_millis(10));
        }
        assert_eq!(cache.get("k1"), None);
    }

    #[test]
    fn cache_tag_invalidation() {
        let cache = InMemoryCache::new(100);
        cache.set("user_1", b"data1".to_vec(), None, &["users"]);
        cache.set("user_2", b"data2".to_vec(), None, &["users", "admins"]);
        cache.set("post_1", b"data3".to_vec(), None, &["posts"]);

        assert_eq!(cache.len(), 3);
        cache.invalidate_tag("users");
        assert_eq!(cache.get("user_1"), None);
        assert_eq!(cache.get("user_2"), None);
        assert_eq!(cache.get("post_1"), Some(b"data3".to_vec()));

        // Ensure "admins" tag index was pruned when user_2 was removed
        assert!(
            !cache
                .state
                .read()
                .unwrap()
                .tag_index
                .contains_key("admins")
        );
    }

    #[test]
    fn cache_secondary_tag_cleanup() {
        let cache = InMemoryCache::new(100);
        cache.set("shared", b"shared_data".to_vec(), None, &["t1", "t2", "t3"]);
        assert_eq!(cache.state.read().unwrap().tag_index.len(), 3);

        cache.invalidate_tag("t2");
        assert_eq!(cache.get("shared"), None);
        assert_eq!(cache.state.read().unwrap().tag_index.len(), 0);
    }
}
