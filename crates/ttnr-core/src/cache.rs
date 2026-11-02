//! On-disk cache of last good stream URLs (`~/.cache/ttnr/resolve.json`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Strategy, StreamDescriptor};

/// TTL used when a descriptor does not set one.
pub const DEFAULT_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    urls: Vec<String>,
    #[serde(default)]
    headers: Vec<(String, String)>,
    /// Seconds since the Unix epoch.
    resolved_at: u64,
    ttl_secs: u64,
    strategy: Strategy,
}

/// Cache of resolved descriptors keyed by [`crate::Station::key`].
/// Thread-safe; persisted as JSON when created with a path.
#[derive(Debug, Default)]
pub struct ResolveCache {
    path: Option<PathBuf>,
    entries: Mutex<BTreeMap<String, Entry>>,
}

impl ResolveCache {
    /// An in-memory cache (tests, or when no cache dir is available).
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Load from `path` (missing or unreadable file = empty cache; a broken
    /// cache must never stop playback).
    pub fn load(path: PathBuf) -> Self {
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| match serde_json::from_slice(&bytes) {
                Ok(entries) => Some(entries),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "ignoring unreadable resolve cache");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries: Mutex::new(entries),
        }
    }

    /// A descriptor for `key` if one is cached and younger than its TTL.
    pub fn get_fresh(&self, key: &str, now: SystemTime) -> Option<StreamDescriptor> {
        let entries = self.lock();
        let e = entries.get(key)?;
        let at = UNIX_EPOCH + Duration::from_secs(e.resolved_at);
        let ttl = Duration::from_secs(e.ttl_secs);
        let age = now.duration_since(at).unwrap_or(Duration::ZERO);
        (age < ttl).then(|| StreamDescriptor {
            urls: e.urls.clone(),
            headers: e.headers.clone(),
            resolved_at: Some(at),
            ttl: Some(ttl),
            ..StreamDescriptor::default()
        })
    }

    /// Remember `d` (resolved by `strategy` at `now`) for `key`.
    pub fn put(&self, key: &str, d: &StreamDescriptor, strategy: Strategy, now: SystemTime) {
        let entry = Entry {
            urls: d.urls.clone(),
            headers: d.headers.clone(),
            resolved_at: now
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            ttl_secs: d.ttl.unwrap_or(DEFAULT_TTL).as_secs(),
            strategy,
        };
        self.lock().insert(key.to_owned(), entry);
        self.save();
    }

    /// Forget `key` (e.g. before a forced refresh).
    pub fn remove(&self, key: &str) {
        if self.lock().remove(key).is_some() {
            self.save();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Entry>> {
        // A poisoned lock only means another thread panicked mid-update of
        // a plain map; the data is still usable.
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Write atomically (temp file + rename); failures are logged only.
    fn save(&self) {
        let Some(path) = &self.path else { return };
        let json = match serde_json::to_vec_pretty(&*self.lock()) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!(error = %e, "cannot serialise resolve cache");
                return;
            }
        };
        let tmp = path.with_extension("json.tmp");
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, json))
            .and_then(|()| std::fs::rename(&tmp, path));
        if let Err(e) = result {
            tracing::warn!(path = %path.display(), error = %e, "cannot write resolve cache");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(url: &str, ttl: Option<Duration>) -> StreamDescriptor {
        StreamDescriptor {
            ttl,
            ..StreamDescriptor::direct(url)
        }
    }

    #[test]
    fn fresh_until_ttl() {
        let c = ResolveCache::in_memory();
        let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
        c.put(
            "bbc:x",
            &desc("http://a", Some(Duration::from_secs(60))),
            Strategy::Discovery,
            t0,
        );
        assert_eq!(
            c.get_fresh("bbc:x", t0 + Duration::from_secs(59))
                .unwrap()
                .urls,
            ["http://a"]
        );
        assert!(c.get_fresh("bbc:x", t0 + Duration::from_secs(60)).is_none());
        assert!(c.get_fresh("other", t0).is_none());
    }

    #[test]
    fn default_ttl_applies() {
        let c = ResolveCache::in_memory();
        let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
        c.put("k", &desc("http://a", None), Strategy::Patterns, t0);
        assert!(
            c.get_fresh("k", t0 + DEFAULT_TTL - Duration::from_secs(1))
                .is_some()
        );
        assert!(c.get_fresh("k", t0 + DEFAULT_TTL).is_none());
    }

    #[test]
    fn persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/resolve.json");
        let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let c = ResolveCache::load(path.clone());
        let mut d = desc("http://a", None);
        d.headers = vec![("X-A".into(), "1".into())];
        c.put("k", &d, Strategy::Discovery, t0);
        let reloaded = ResolveCache::load(path.clone());
        let got = reloaded.get_fresh("k", t0).unwrap();
        assert_eq!(got.urls, ["http://a"]);
        assert_eq!(got.headers, [("X-A".to_owned(), "1".to_owned())]);
        reloaded.remove("k");
        assert!(ResolveCache::load(path).get_fresh("k", t0).is_none());
    }

    #[test]
    fn corrupt_file_is_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resolve.json");
        std::fs::write(&path, b"{not json").unwrap();
        let c = ResolveCache::load(path);
        assert!(c.get_fresh("k", SystemTime::now()).is_none());
    }
}
