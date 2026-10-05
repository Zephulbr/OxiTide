//! Content-blocking flags (spec §6.5): `explicit` observed from v1 Track
//! DTOs, `ai` fetched from the public OpenAPI, cached in memory and in
//! `cache/track_flags.json`. Every failure is fail-open (QoL-D11).
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::flags::TrackFlags;
use oxitide_tidal::Error;
use serde::{Deserialize, Serialize};

use crate::config::{default_cache_dir, ContentConfig};

pub const TRACK_FLAGS_VERSION: u32 = 1;
pub const TRACK_FLAGS_MAX_ENTRIES: usize = 2_000;
/// Ids per public-OpenAPI request (spec §6.5).
pub const TRACK_FLAGS_BATCH: usize = 20;

/// One cached entry. `fetched_at == 0` means no OpenAPI lookup has succeeded
/// for this id yet (an explicit-only observation), so a later enrich still
/// fetches it; a successful fetch stores the timestamp even when `ai` is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackFlagEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<bool>,
    #[serde(default)]
    pub fetched_at: u64,
}

/// True when the active filters hide this track. Unknown values never block.
pub fn is_blocked(explicit: Option<bool>, ai: Option<bool>, content: &ContentConfig) -> bool {
    (content.block_explicit && explicit == Some(true))
        || (content.block_ai && ai == Some(true))
}

/// Index of the original `start` track inside the kept list, or the position
/// of the next kept track, clamped to the last kept item.
pub fn remap_start(ids: &[u64], kept: &[u64], start: usize) -> usize {
    if kept.is_empty() || start >= ids.len() {
        return 0;
    }
    if let Some(index) = kept.iter().position(|id| *id == ids[start]) {
        return index;
    }
    let before = ids[..start].iter().filter(|id| kept.contains(id)).count();
    before.min(kept.len() - 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneOutcome {
    /// Blocked indices, highest first.
    pub remove: Vec<usize>,
    /// Current index in the *kept* list; meaningful when `current_removed`.
    pub new_current: Option<usize>,
    pub current_removed: bool,
}

/// Decide how to prune `blocked` items without touching playback unless the
/// current track itself is blocked (then `new_current` points at the kept
/// replacement: the next kept item, else the previous one, else `None`).
pub fn plan_prune(len: usize, current_index: Option<usize>, blocked: &[bool]) -> PruneOutcome {
    let mut remove: Vec<usize> = blocked
        .iter()
        .take(len)
        .enumerate()
        .filter(|(_, blocked)| **blocked)
        .map(|(index, _)| index)
        .collect();
    remove.reverse();
    let current_removed = current_index.is_some_and(|index| blocked.get(index) == Some(&true));
    let new_current = if current_removed {
        let current = current_index.unwrap_or(0);
        let next = (current..len).find(|index| !blocked.get(*index).copied().unwrap_or(false));
        match next {
            Some(index) => Some(kept_before(blocked, index)),
            None => (0..current)
                .rev()
                .find(|index| !blocked.get(*index).copied().unwrap_or(false))
                .map(|index| kept_before(blocked, index)),
        }
    } else {
        current_index
    };
    PruneOutcome { remove, new_current, current_removed }
}

fn kept_before(blocked: &[bool], index: usize) -> usize {
    blocked[..index].iter().filter(|blocked| !**blocked).count()
}

/// `%APPDATA%\OxiTide\cache\track_flags.json`.
pub fn track_flags_path() -> Option<PathBuf> {
    default_cache_dir().map(|dir| dir.join("track_flags.json"))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskCache {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    tracks: HashMap<String, TrackFlagEntry>,
}

/// Flag cache with a lazy, version-gated disk file. Reads are cheap; writes
/// happen only after successful enrichment batches.
pub struct TrackFlagsStore {
    memory: RwLock<HashMap<u64, TrackFlagEntry>>,
    loaded: AtomicBool,
    path: Option<PathBuf>,
}

impl TrackFlagsStore {
    pub fn start(path: Option<PathBuf>) -> Self {
        Self { memory: RwLock::new(HashMap::new()), loaded: AtomicBool::new(false), path }
    }

    fn ensure_loaded(&self) {
        if self.loaded.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(path) = self.path.as_ref() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let Ok(cache) = serde_json::from_str::<DiskCache>(&text) else { return };
        if cache.version != TRACK_FLAGS_VERSION {
            return;
        }
        let mut memory = self.memory.write().unwrap();
        for (key, entry) in cache.tracks {
            if let Ok(id) = key.parse::<u64>() {
                memory.insert(id, entry);
            }
        }
    }

    pub fn get(&self, track_id: u64) -> Option<TrackFlagEntry> {
        self.ensure_loaded();
        self.memory.read().unwrap().get(&track_id).copied()
    }

    /// Cached entries for `ids`, in input order; unknown ids are omitted.
    pub fn known(&self, ids: &[u64]) -> Vec<(u64, TrackFlagEntry)> {
        self.ensure_loaded();
        let memory = self.memory.read().unwrap();
        ids.iter()
            .filter_map(|id| memory.get(id).map(|entry| (*id, *entry)))
            .collect()
    }

    /// Record the v1 `explicit` observation. New entries keep `fetched_at = 0`
    /// so a later enrichment still runs.
    pub fn record_explicit(&self, track_id: u64, explicit: bool) {
        self.ensure_loaded();
        let mut memory = self.memory.write().unwrap();
        let entry = memory.entry(track_id).or_insert(TrackFlagEntry {
            explicit: None,
            ai: None,
            fetched_at: 0,
        });
        entry.explicit = Some(explicit);
    }

    /// Fetch unknown ids in batches of `TRACK_FLAGS_BATCH`, merge results, and
    /// persist once. Auth errors surface; every other failure stops fetching
    /// quietly and returns the known flags (fail open).
    pub async fn enrich(
        &self,
        client: &TidalClient,
        ids: &[u64],
    ) -> Result<Vec<(u64, TrackFlagEntry)>, Error> {
        self.ensure_loaded();
        let missing: Vec<u64> = {
            let memory = self.memory.read().unwrap();
            ids.iter()
                .copied()
                .filter(|id| memory.get(id).is_none_or(|entry| entry.fetched_at == 0))
                .collect()
        };
        let mut fetched = false;
        for chunk in missing.chunks(TRACK_FLAGS_BATCH) {
            match client.track_flags(chunk).await {
                Ok(rows) => {
                    self.insert_rows(rows);
                    fetched = true;
                }
                Err(Error::Unauthorized) => return Err(Error::Unauthorized),
                Err(error) => {
                    tracing::warn!("track flags lookup failed: {error}");
                    break;
                }
            }
        }
        if fetched {
            self.save();
        }
        Ok(self.known(ids))
    }

    fn insert_rows(&self, rows: Vec<(u64, TrackFlags)>) {
        let now = now_secs();
        let mut memory = self.memory.write().unwrap();
        for (id, flags) in rows {
            let entry = memory.entry(id).or_insert(TrackFlagEntry {
                explicit: None,
                ai: None,
                fetched_at: 0,
            });
            if flags.explicit.is_some() {
                entry.explicit = flags.explicit;
            }
            if flags.ai.is_some() {
                entry.ai = flags.ai;
            }
            entry.fetched_at = now;
        }
        prune(&mut memory);
    }

    fn save(&self) {
        let Some(path) = self.path.as_ref() else { return };
        let memory = self.memory.read().unwrap();
        let cache = DiskCache {
            version: TRACK_FLAGS_VERSION,
            tracks: memory
                .iter()
                .map(|(id, entry)| (id.to_string(), *entry))
                .collect(),
        };
        let Ok(text) = serde_json::to_string(&cache) else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path)).is_err() {
            tracing::warn!("failed to save track flags at {}", path.display());
        }
    }
}

/// Drop the oldest `fetched_at` entries until the cap is met.
fn prune(map: &mut HashMap<u64, TrackFlagEntry>) {
    if map.len() <= TRACK_FLAGS_MAX_ENTRIES {
        return;
    }
    let mut by_age: Vec<(u64, u64)> =
        map.iter().map(|(id, entry)| (*id, entry.fetched_at)).collect();
    by_age.sort_by_key(|(_, fetched_at)| *fetched_at);
    for (id, _) in by_age {
        if map.len() <= TRACK_FLAGS_MAX_ENTRIES {
            break;
        }
        map.remove(&id);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::auth::SessionInfo;
    use oxitide_tidal::token_store::{MemoryStore, TokenSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-flags-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 1,
            },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    /// Answers every /openapi/v2/tracks request with one resource per
    /// requested id; `ai` is true for even ids, explicit for all.
    fn flag_responder(requests: Arc<AtomicUsize>) -> impl Fn(&Request) -> ResponseTemplate {
        move |req: &Request| {
            requests.fetch_add(1, Ordering::SeqCst);
            let ids: Vec<u64> = req
                .url
                .query_pairs()
                .filter(|(key, _)| key == "filter[id]")
                .map(|(_, value)| value.parse().unwrap())
                .collect();
            let data: Vec<_> = ids
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "id": id.to_string(),
                        "type": "tracks",
                        "attributes": {"explicit": true, "ai": id % 2 == 0}
                    })
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": data}))
        }
    }

    #[test]
    fn is_blocked_respects_each_toggle_and_fails_open() {
        let content = ContentConfig { block_explicit: true, block_ai: false };
        assert!(is_blocked(Some(true), Some(true), &content));
        assert!(!is_blocked(None, Some(true), &content));
        assert!(!is_blocked(Some(false), Some(false), &content));

        let ai_only = ContentConfig { block_explicit: false, block_ai: true };
        assert!(is_blocked(Some(true), Some(true), &ai_only));
        assert!(!is_blocked(Some(true), None, &ai_only));

        let off = ContentConfig::default();
        assert!(!is_blocked(Some(true), Some(true), &off));
    }

    #[test]
    fn remap_start_follows_the_original_track_or_the_next_kept_one() {
        assert_eq!(remap_start(&[1, 2, 3], &[1, 2, 3], 2), 2);
        assert_eq!(remap_start(&[1, 2, 3], &[2, 3], 0), 0);
        assert_eq!(remap_start(&[1, 2, 3], &[1, 3], 1), 1);
        assert_eq!(remap_start(&[1, 2, 3], &[1, 2], 2), 1);
        assert_eq!(remap_start(&[1, 2, 3], &[1], 2), 0);
        assert_eq!(remap_start(&[1, 2], &[1], 99), 0);
    }

    #[test]
    fn plan_prune_removes_descending_and_keeps_a_kept_current() {
        let outcome = plan_prune(4, Some(1), &[true, false, true, false]);
        assert_eq!(outcome.remove, vec![2, 0]);
        assert!(!outcome.current_removed);
        assert_eq!(outcome.new_current, Some(1));
    }

    #[test]
    fn plan_prune_advances_to_the_next_kept_or_the_previous_one() {
        let next = plan_prune(4, Some(1), &[true, true, false, false]);
        assert!(next.current_removed);
        assert_eq!(next.remove, vec![1, 0]);
        assert_eq!(next.new_current, Some(0), "first kept item is index 0 of the kept list");

        let mid = plan_prune(3, Some(1), &[false, true, false]);
        assert!(mid.current_removed);
        assert_eq!(mid.new_current, Some(1));

        let previous = plan_prune(4, Some(3), &[false, true, true, true]);
        assert!(previous.current_removed);
        assert_eq!(previous.new_current, Some(0));

        let empty = plan_prune(2, Some(0), &[true, true]);
        assert!(empty.current_removed);
        assert_eq!(empty.new_current, None);
    }

    #[tokio::test]
    async fn enrich_batches_in_twenties_and_caches() {
        let server = MockServer::start().await;
        let requests = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(flag_responder(requests.clone()))
            .mount(&server)
            .await;
        let dir = temp_dir();
        let store = TrackFlagsStore::start(Some(dir.join("track_flags.json")));
        let ids: Vec<u64> = (1..=45).collect();

        let entries = store.enrich(&client(&server), &ids).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        assert_eq!(entries.len(), 45);
        assert_eq!(store.get(2).unwrap().ai, Some(true));
        assert_eq!(store.get(1).unwrap().ai, Some(false));

        let again = store.enrich(&client(&server), &ids).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3, "cached flags were refetched");
        assert_eq!(again.len(), 45);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn enrich_fails_open_on_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let store = TrackFlagsStore::start(None);
        let out = store.enrich(&client(&server), &[1, 2]).await.unwrap();
        assert!(out.is_empty());
        assert!(store.get(1).is_none());
    }

    #[test]
    fn record_explicit_merges_without_clobbering_ai() {
        let store = TrackFlagsStore::start(None);
        store.record_explicit(1, false);
        assert_eq!(store.get(1).unwrap().explicit, Some(false));
        store.record_explicit(1, true);
        assert_eq!(store.get(1).unwrap().explicit, Some(true));
    }

    #[test]
    fn disk_cache_round_trips_and_version_mismatch_is_discarded() {
        let dir = temp_dir();
        let path = dir.join("track_flags.json");
        {
            let store = TrackFlagsStore::start(Some(path.clone()));
            store.record_explicit(5, true);
            let entry = TrackFlagEntry { explicit: Some(true), ai: Some(true), fetched_at: 9 };
            *store.memory.write().unwrap() = [(5, entry)].into_iter().collect();
            store.save();
        }
        let loaded = TrackFlagsStore::start(Some(path.clone()));
        let entry = loaded.get(5).expect("entry");
        assert_eq!(entry.explicit, Some(true));
        assert_eq!(entry.ai, Some(true));

        std::fs::write(&path, r#"{"version":99,"tracks":{"5":{"ai":true}}}"#).unwrap();
        let stale = TrackFlagsStore::start(Some(path));
        assert!(stale.get(5).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_keeps_the_newest_by_fetched_at() {
        let mut map: HashMap<u64, TrackFlagEntry> = HashMap::new();
        for id in 0..(TRACK_FLAGS_MAX_ENTRIES as u64 + 10) {
            map.insert(
                id,
                TrackFlagEntry { explicit: None, ai: Some(false), fetched_at: id },
            );
        }
        prune(&mut map);
        assert_eq!(map.len(), TRACK_FLAGS_MAX_ENTRIES);
        assert!(!map.contains_key(&0));
        assert!(map.contains_key(&u64::try_from(TRACK_FLAGS_MAX_ENTRIES).unwrap()));
    }
}
