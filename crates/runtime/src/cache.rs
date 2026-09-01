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

/// In-memory thread-safe query cache with TTL and tag invalidation.
#[derive(Debug)]
pub struct InMemoryCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    tag_index: RwLock<HashMap<String, HashSet<String>>>,
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
            entries: RwLock::new(HashMap::new()),
            tag_index: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    /// Prunes expired entries from the cache.
    pub fn prune_expired(&self) {
        let now = Instant::now();
        let mut expired_keys = Vec::new();

        if let Ok(entries) = self.entries.read() {
            for (key, entry) in entries.iter() {
                if let Some(exp) = entry.expires_at {
                    if exp <= now {
                        expired_keys.push(key.clone());
                    }
                }
            }
        }

        if !expired_keys.is_empty() {
            if let Ok(mut entries) = self.entries.write() {
                if let Ok(mut tag_index) = self.tag_index.write() {
                    for key in expired_keys {
                        if let Some(entry) = entries.remove(&key) {
                            for tag in entry.tags {
                                if let Some(keys) = tag_index.get_mut(&tag) {
                                    keys.remove(&key);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl QueryCache for InMemoryCache {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(key)?;
        if let Some(expires_at) = entry.expires_at {
            if Instant::now() >= expires_at {
                return None;
            }
        }
        Some(entry.data.clone())
    }

    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>, tags: &[&str]) {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let tag_strings: Vec<String> = tags.iter().map(|s| (*s).to_string()).collect();

        if let Ok(mut entries) = self.entries.write() {
            // Evict if at capacity
            if entries.len() >= self.max_entries && !entries.contains_key(key) {
                if let Some(first_key) = entries.keys().next().cloned() {
                    if let Some(old) = entries.remove(&first_key) {
                        if let Ok(mut tag_index) = self.tag_index.write() {
                            for tag in old.tags {
                                if let Some(set) = tag_index.get_mut(&tag) {
                                    set.remove(&first_key);
                                }
                            }
                        }
                    }
                }
            }

            let entry = CacheEntry {
                data: value,
                expires_at,
                tags: tag_strings.clone(),
            };
            entries.insert(key.to_string(), entry);
        }

        if let Ok(mut tag_index) = self.tag_index.write() {
            for tag in tag_strings {
                tag_index.entry(tag).or_default().insert(key.to_string());
            }
        }
    }

    fn invalidate_tag(&self, tag: &str) {
        let keys_to_remove: Vec<String> = {
            if let Ok(mut tag_index) = self.tag_index.write() {
                tag_index
                    .remove(tag)
                    .map(|s| s.into_iter().collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };

        if !keys_to_remove.is_empty() {
            let mut other_tags_to_prune: Vec<(String, String)> = Vec::new();
            if let Ok(mut entries) = self.entries.write() {
                for key in &keys_to_remove {
                    if let Some(entry) = entries.remove(key) {
                        for t in entry.tags {
                            if t != tag {
                                other_tags_to_prune.push((t, key.clone()));
                            }
                        }
                    }
                }
            }
            if !other_tags_to_prune.is_empty() {
                if let Ok(mut tag_index) = self.tag_index.write() {
                    for (t, k) in other_tags_to_prune {
                        if let Some(set) = tag_index.get_mut(&t) {
                            set.remove(&k);
                            if set.is_empty() {
                                tag_index.remove(&t);
                            }
                        }
                    }
                }
            }
        }
    }

    fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
        if let Ok(mut tag_index) = self.tag_index.write() {
            tag_index.clear();
        }
    }

    fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

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
        while set_at.elapsed() <= TTL {
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
        assert!(cache.tag_index.read().unwrap().get("admins").is_none());
    }

    #[test]
    fn cache_secondary_tag_cleanup() {
        let cache = InMemoryCache::new(100);
        cache.set("shared", b"shared_data".to_vec(), None, &["t1", "t2", "t3"]);
        assert_eq!(cache.tag_index.read().unwrap().len(), 3);

        cache.invalidate_tag("t2");
        assert_eq!(cache.get("shared"), None);
        assert_eq!(cache.tag_index.read().unwrap().len(), 0);
    }
}
