//! Album release dates for track-list sorting (multi-sort spec §4): fetched
//! from the public OpenAPI in batches, cached in memory and in
//! `cache/album_dates.json`. Release dates do not change, so entries never
//! expire; every failure is fail-open (the dates stay unknown).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::Error;
use serde::{Deserialize, Serialize};

use crate::config::default_cache_dir;

pub const ALBUM_DATES_VERSION: u32 = 1;
pub const ALBUM_DATES_MAX_ENTRIES: usize = 20_000;
/// Ids per public-OpenAPI request, as for track flags.
pub const ALBUM_DATES_BATCH: usize = 20;

/// One cached album. `date: None` with a `fetched_at` stamp is a settled
/// "the API has no date" result, so it is not requested again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumDateEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default)]
    pub fetched_at: u64,
}

/// `%APPDATA%\OxiTide\config\cache\album_dates.json`.
pub fn album_dates_path() -> Option<PathBuf> {
    default_cache_dir().map(|dir| dir.join("album_dates.json"))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskCache {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    albums: HashMap<String, AlbumDateEntry>,
}

pub struct AlbumDatesStore {
    memory: RwLock<HashMap<u64, AlbumDateEntry>>,
    loaded: AtomicBool,
    path: Option<PathBuf>,
}

impl AlbumDatesStore {
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
        if cache.version != ALBUM_DATES_VERSION {
            return;
        }
        let mut memory = self.memory.write().unwrap();
        for (key, entry) in cache.albums {
            if let Ok(id) = key.parse::<u64>() {
                memory.insert(id, entry);
            }
        }
    }

    /// Cached dates for `ids`, in input order; unknown ids are omitted.
    pub fn known(&self, ids: &[u64]) -> Vec<(u64, Option<String>)> {
        self.ensure_loaded();
        let memory = self.memory.read().unwrap();
        ids.iter()
            .filter_map(|id| memory.get(id).map(|entry| (*id, entry.date.clone())))
            .collect()
    }

    /// Fetch uncached ids in batches, merge, and persist once. Auth errors
    /// surface; any other failure stops fetching and returns what is known.
    pub async fn enrich(
        &self,
        client: &TidalClient,
        ids: &[u64],
    ) -> Result<Vec<(u64, Option<String>)>, Error> {
        self.ensure_loaded();
        let missing: Vec<u64> = {
            let memory = self.memory.read().unwrap();
            let mut seen = std::collections::HashSet::new();
            ids.iter()
                .copied()
                .filter(|id| !memory.contains_key(id) && seen.insert(*id))
                .collect()
        };
        let mut fetched = false;
        for chunk in missing.chunks(ALBUM_DATES_BATCH) {
            match client.album_release_dates(chunk).await {
                Ok(rows) => {
                    self.insert(chunk, rows);
                    fetched = true;
                }
                Err(Error::Unauthorized) => return Err(Error::Unauthorized),
                Err(error) => {
                    tracing::warn!("album release-date lookup failed: {error}");
                    break;
                }
            }
        }
        if fetched {
            self.save();
        }
        Ok(self.known(ids))
    }

    /// Store a fetched batch; ids the API omitted are negative-cached so a
    /// re-sort does not request them again.
    fn insert(&self, requested: &[u64], rows: Vec<(u64, Option<String>)>) {
        let now = now_secs();
        let mut memory = self.memory.write().unwrap();
        for id in requested {
            memory.insert(*id, AlbumDateEntry { date: None, fetched_at: now });
        }
        for (id, date) in rows {
            memory.insert(id, AlbumDateEntry { date, fetched_at: now });
        }
    }

    fn save(&self) {
        let Some(path) = self.path.as_ref() else { return };
        let mut memory = self.memory.write().unwrap();
        prune(&mut memory);
        let cache = DiskCache {
            version: ALBUM_DATES_VERSION,
            albums: memory.iter().map(|(id, entry)| (id.to_string(), entry.clone())).collect(),
        };
        let Ok(text) = serde_json::to_string(&cache) else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path)).is_err() {
            tracing::warn!("failed to save album dates at {}", path.display());
        }
    }
}

/// Drop the oldest `fetched_at` entries until the cap is met.
fn prune(map: &mut HashMap<u64, AlbumDateEntry>) {
    if map.len() <= ALBUM_DATES_MAX_ENTRIES {
        return;
    }
    let mut by_age: Vec<(u64, u64)> = map.iter().map(|(id, entry)| (*id, entry.fetched_at)).collect();
    by_age.sort_by_key(|(_, fetched_at)| *fetched_at);
    let excess = map.len() - ALBUM_DATES_MAX_ENTRIES;
    for (id, _) in by_age.into_iter().take(excess) {
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
    use std::sync::atomic::{AtomicU32, AtomicUsize};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-album-dates-test-{}-{}",
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
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    /// One resource per requested id; odd ids have a date, even ids omit it,
    /// and id 7 is left out of the response entirely.
    fn responder(requests: Arc<AtomicUsize>) -> impl Fn(&Request) -> ResponseTemplate {
        move |req: &Request| {
            requests.fetch_add(1, Ordering::SeqCst);
            let data: Vec<_> = req
                .url
                .query_pairs()
                .filter(|(key, _)| key == "filter[id]")
                .map(|(_, value)| value.parse::<u64>().unwrap())
                .filter(|id| *id != 7)
                .map(|id| {
                    let attributes = if id % 2 == 1 {
                        serde_json::json!({"releaseDate": format!("2000-01-{:02}", id % 28 + 1)})
                    } else {
                        serde_json::json!({})
                    };
                    serde_json::json!({"id": id.to_string(), "type": "albums", "attributes": attributes})
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": data}))
        }
    }

    #[tokio::test]
    async fn enrich_batches_caches_and_negative_caches() {
        let server = MockServer::start().await;
        let requests = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums"))
            .respond_with(responder(requests.clone()))
            .mount(&server)
            .await;
        let dir = temp_dir();
        let store = AlbumDatesStore::start(Some(dir.join("album_dates.json")));
        let mut ids: Vec<u64> = (1..=45).collect();
        ids.push(1); // duplicates are requested once

        let dates = store.enrich(&client(&server), &ids).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        assert_eq!(dates.len(), 46);
        assert_eq!(dates[0], (1, Some("2000-01-02".to_string())));
        assert_eq!(dates[1], (2, None));
        assert_eq!(dates[6], (7, None), "an omitted id is settled as unknown");

        store.enrich(&client(&server), &ids).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3, "cached dates were refetched");

        let reloaded = AlbumDatesStore::start(Some(dir.join("album_dates.json")));
        assert_eq!(reloaded.known(&[1, 7]), vec![(1, Some("2000-01-02".to_string())), (7, None)]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn enrich_fails_open_on_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let store = AlbumDatesStore::start(None);
        assert!(store.enrich(&client(&server), &[1, 2]).await.unwrap().is_empty());
    }

    #[test]
    fn version_mismatch_discards_the_disk_cache() {
        let dir = temp_dir();
        let path = dir.join("album_dates.json");
        std::fs::write(&path, r#"{"version":99,"albums":{"5":{"date":"2001-01-01","fetched_at":1}}}"#)
            .unwrap();
        assert!(AlbumDatesStore::start(Some(path)).known(&[5]).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_keeps_the_newest() {
        let mut map: HashMap<u64, AlbumDateEntry> = (0..(ALBUM_DATES_MAX_ENTRIES as u64 + 5))
            .map(|id| (id, AlbumDateEntry { date: None, fetched_at: id }))
            .collect();
        prune(&mut map);
        assert_eq!(map.len(), ALBUM_DATES_MAX_ENTRIES);
        assert!(!map.contains_key(&0));
        assert!(map.contains_key(&(ALBUM_DATES_MAX_ENTRIES as u64 + 4)));
    }
}
