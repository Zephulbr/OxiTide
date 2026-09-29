//! Lyrics fetch + cache: TIDAL legacy first, lrclib fallback (Track 4a spec
//! §5.2, §7, §8). Only confirmed misses are negative-cached; transient
//! failures return `None` for the attempt and stay retryable.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::lyrics::TidalLyrics;

use crate::lrc::{parse_lrc, split_plain, LyricLine};

const CACHE_MAX_ENTRIES: usize = 2_000;
const DURATION_TOLERANCE_SECS: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsDto {
    pub track_id: u64,
    pub source: String, // "tidal" | "lrclib"
    pub synced: bool,
    pub rtl: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricsRequest {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,
    lyrics: Option<LyricsDto>,
}

#[derive(Default, Serialize, Deserialize)]
struct DiskCache {
    tracks: HashMap<u64, CacheEntry>,
}

/// The lrclib record fields this feature reads.
#[derive(Debug, Deserialize)]
struct LrclibRecord {
    #[serde(default, rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: Option<f64>,
}

enum LyricsOutcome {
    Found(LyricsDto),
    Missing,
    Failed,
}

pub struct LyricsService {
    http: reqwest::Client,
    base_url: String,
    memory: Mutex<HashMap<u64, CacheEntry>>,
    disk_path: PathBuf,
}

impl LyricsService {
    pub fn new(http: reqwest::Client, lrclib_url: impl Into<String>, disk_path: PathBuf) -> Self {
        if let Some(parent) = disk_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let memory = std::fs::read_to_string(&disk_path)
            .ok()
            .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
            .map(|cache| cache.tracks)
            .unwrap_or_default();
        Self {
            http,
            base_url: lrclib_url.into().trim_end_matches('/').to_string(),
            memory: Mutex::new(memory),
            disk_path,
        }
    }

    pub async fn get(&self, client: &TidalClient, req: &LyricsRequest) -> Option<LyricsDto> {
        if let Some(entry) = self.memory.lock().unwrap().get(&req.track_id) {
            return entry.lyrics.clone();
        }
        match self.fetch(client, req).await {
            LyricsOutcome::Found(dto) => {
                self.store(req.track_id, Some(dto.clone()));
                Some(dto)
            }
            LyricsOutcome::Missing => {
                self.store(req.track_id, None);
                None
            }
            LyricsOutcome::Failed => None, // transient: retry on a later attempt
        }
    }

    async fn fetch(&self, client: &TidalClient, req: &LyricsRequest) -> LyricsOutcome {
        match client.track_lyrics(req.track_id).await {
            Ok(Some(raw)) => {
                if let Some(dto) = dto_from_tidal(req.track_id, raw) {
                    return LyricsOutcome::Found(dto);
                }
            }
            Ok(None) => {}
            Err(error) => tracing::warn!("tidal lyrics failed for {}: {error}", req.track_id),
        }
        self.lrclib_exact(req).await
    }

    async fn lrclib_exact(&self, req: &LyricsRequest) -> LyricsOutcome {
        let mut params: Vec<(&str, String)> = vec![
            ("track_name", req.title.clone()),
            ("artist_name", req.artist.clone()),
            ("duration", req.duration_secs.to_string()),
        ];
        if let Some(album) = req.album.as_deref().filter(|album| !album.is_empty()) {
            params.push(("album_name", album.to_string()));
        }
        match self.get_json::<LrclibRecord>("api/get", &params).await {
            Ok(Some(record)) => match dto_from_lrclib(req.track_id, record) {
                Some(dto) => LyricsOutcome::Found(dto),
                None => LyricsOutcome::Missing,
            },
            Ok(None) => self.lrclib_search(req).await,
            Err(()) => LyricsOutcome::Failed,
        }
    }

    async fn lrclib_search(&self, req: &LyricsRequest) -> LyricsOutcome {
        let params = vec![
            ("track_name", req.title.clone()),
            ("artist_name", req.artist.clone()),
        ];
        match self
            .get_json::<Vec<LrclibRecord>>("api/search", &params)
            .await
        {
            Ok(Some(records)) => {
                let want = req.duration_secs as f64;
                let found = records.into_iter().find(|record| {
                    !record.instrumental
                        && record
                            .duration
                            .map(|duration| (duration - want).abs() <= DURATION_TOLERANCE_SECS)
                            .unwrap_or(false)
                });
                match found.and_then(|record| dto_from_lrclib(req.track_id, record)) {
                    Some(dto) => LyricsOutcome::Found(dto),
                    None => LyricsOutcome::Missing,
                }
            }
            Ok(None) => LyricsOutcome::Missing,
            Err(()) => LyricsOutcome::Failed,
        }
    }

    /// GET a JSON body, retrying once after a `429`'s `Retry-After`.
    /// `Ok(None)` is a 404; `Err(())` is any other failure.
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Option<T>, ()> {
        for attempt in 0..2 {
            let response = self
                .http
                .get(format!("{}/{}", self.base_url, path))
                .header(
                    reqwest::header::USER_AGENT,
                    concat!("OxiTide/", env!("CARGO_PKG_VERSION")),
                )
                .query(params)
                .send()
                .await
                .map_err(|error| {
                    tracing::warn!("lrclib request failed: {error}");
                })?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                tokio::time::sleep(retry_after(&response)).await;
                continue;
            }
            let status = response.status();
            if status.is_success() {
                return response.json::<T>().await.map(Some).map_err(|error| {
                    tracing::warn!("lrclib decode failed: {error}");
                });
            }
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            tracing::warn!("lrclib returned {status}");
            return Err(());
        }
        Err(())
    }

    fn store(&self, track_id: u64, lyrics: Option<LyricsDto>) {
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        self.memory
            .lock()
            .unwrap()
            .insert(track_id, CacheEntry { fetched_at, lyrics });
        self.save_disk();
    }

    fn save_disk(&self) {
        let cache = {
            let mut memory = self.memory.lock().unwrap();
            if memory.len() > CACHE_MAX_ENTRIES {
                let mut entries: Vec<(u64, CacheEntry)> = memory.drain().collect();
                entries.sort_by_key(|entry| std::cmp::Reverse(entry.1.fetched_at));
                entries.truncate(CACHE_MAX_ENTRIES);
                memory.extend(entries);
            }
            DiskCache {
                tracks: memory.clone(),
            }
        };
        let Ok(text) = serde_json::to_string(&cache) else {
            return;
        };
        let tmp = self.disk_path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &self.disk_path);
        }
    }
}

/// True when at least one line carries visible text; a synced body of only
/// timestamps (or metadata) is a miss, not an empty panel.
fn has_text(lines: &[LyricLine]) -> bool {
    lines.iter().any(|line| !line.text.trim().is_empty())
}

fn dto_from_tidal(track_id: u64, raw: TidalLyrics) -> Option<LyricsDto> {
    let rtl = raw.right_to_left;
    if !raw.subtitles.trim().is_empty() {
        let lines = parse_lrc(&raw.subtitles);
        if has_text(&lines) {
            return Some(LyricsDto {
                track_id,
                source: "tidal".into(),
                synced: true,
                rtl,
                lines,
            });
        }
    }
    if !raw.lyrics.trim().is_empty() {
        let lines = split_plain(&raw.lyrics);
        if has_text(&lines) {
            return Some(LyricsDto {
                track_id,
                source: "tidal".into(),
                synced: false,
                rtl,
                lines,
            });
        }
    }
    None
}

fn dto_from_lrclib(track_id: u64, record: LrclibRecord) -> Option<LyricsDto> {
    if record.instrumental {
        return None;
    }
    if let Some(synced) = record.synced_lyrics.as_deref() {
        let lines = parse_lrc(synced);
        if has_text(&lines) {
            return Some(LyricsDto {
                track_id,
                source: "lrclib".into(),
                synced: true,
                rtl: false,
                lines,
            });
        }
    }
    let plain = record.plain_lyrics.as_deref()?;
    if plain.trim().is_empty() {
        return None;
    }
    let lines = split_plain(plain);
    if !has_text(&lines) {
        return None;
    }
    Some(LyricsDto {
        track_id,
        source: "lrclib".into(),
        synced: false,
        rtl: false,
        lines,
    })
}

fn retry_after(response: &reqwest::Response) -> std::time::Duration {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|secs| std::time::Duration::from_secs(secs.min(5)))
        .unwrap_or(std::time::Duration::from_secs(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::auth::SessionInfo;
    use oxitide_tidal::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn tidal_client(server: &MockServer) -> TidalClient {
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

    fn request(track_id: u64) -> LyricsRequest {
        LyricsRequest {
            track_id,
            title: "The Chain".into(),
            artist: "Fleetwood Mac".into(),
            album: Some("Rumours".into()),
            duration_secs: 271,
        }
    }

    fn synced_body(text: &str) -> serde_json::Value {
        serde_json::json!({
            "syncedLyrics": text,
            "plainLyrics": "plain fallback",
            "instrumental": false,
            "duration": 271
        })
    }

    fn temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oxitide-lyrics-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn tidal_hit_is_parsed_and_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7,
                "lyrics": "plain",
                "subtitles": "[00:01.00] synced line",
                "lyricsProvider": "TIDAL",
                "isRightToLeft": false
            })))
            .expect(1)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let client = tidal_client(&tidal);
        let first = service.get(&client, &request(7)).await.unwrap();
        let second = service.get(&client, &request(7)).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(first.source, "tidal");
        assert!(first.synced);
        assert_eq!(first.lines[0].time_ms, Some(1_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn unsynced_tidal_lyrics_map_to_plain_lines() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "one\ntwo", "subtitles": ""
            })))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.source, "tidal");
        assert!(!dto.synced);
        assert_eq!(dto.lines.len(), 2);
        assert_eq!(dto.lines[0].time_ms, None);
        assert_eq!(dto.lines[0].text, "one");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn timestamp_only_tidal_lyrics_fall_back_to_lrclib() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "", "subtitles": "[00:01.00]"
            })))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(synced_body("[00:02.00] real words")),
            )
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.source, "lrclib");
        assert_eq!(dto.lines[0].text, "real words");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn timestamp_only_lrclib_synced_falls_back_to_plain() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "syncedLyrics": "[00:01.00]", "plainLyrics": "plain words",
                "instrumental": false, "duration": 271
            })))
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert!(!dto.synced);
        assert_eq!(dto.lines[0].text, "plain words");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn negative_cache_survives_a_restart() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&lrclib)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let client = tidal_client(&tidal);
        let first = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        assert_eq!(first.get(&client, &request(7)).await, None);
        let second = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        assert_eq!(second.get(&client, &request(7)).await, None);
        drop(tidal);
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn empty_tidal_lyrics_fall_back_to_lrclib() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "", "subtitles": ""
            })))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(synced_body("[00:02.00] from lrclib")),
            )
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.source, "lrclib");
        assert_eq!(dto.lines[0].text, "from lrclib");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn tidal_error_falls_through_to_lrclib() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(synced_body("[00:01.00] hi")))
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.source, "lrclib");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn exact_miss_searches_and_filters_by_duration() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&lrclib)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"syncedLyrics": "[00:01.00] wrong", "instrumental": false, "duration": 100.0},
                {"syncedLyrics": "[00:02.00] right", "instrumental": false, "duration": 271.0}
            ])))
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.lines[0].text, "right");
        assert_eq!(dto.lines[0].time_ms, Some(2_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn instrumental_exact_record_is_a_confirmed_miss() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "syncedLyrics": null, "plainLyrics": null, "instrumental": true, "duration": 271
            })))
            .mount(&lrclib)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!([synced_body("[00:01.00] good")])),
            )
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        // A search hit would return lyrics; an instrumental exact match must not search.
        assert_eq!(service.get(&tidal_client(&tidal), &request(7)).await, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn confirmed_miss_is_negative_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&lrclib)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let client = tidal_client(&tidal);
        assert_eq!(service.get(&client, &request(7)).await, None);
        assert_eq!(service.get(&client, &request(7)).await, None);
        drop(tidal);
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn lrclib_failure_is_not_cached() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .expect(2)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(500))
            .expect(2)
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let client = tidal_client(&tidal);
        assert_eq!(service.get(&client, &request(7)).await, None);
        assert_eq!(service.get(&client, &request(7)).await, None);
        drop(tidal);
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn lrclib_429_retries_once_after_retry_after() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200).set_body_json(synced_body("[00:01.00] hello"))
                }
            }
        };
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(responder)
            .expect(2)
            .mount(&lrclib)
            .await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let dto = service
            .get(&tidal_client(&tidal), &request(7))
            .await
            .unwrap();
        assert_eq!(dto.source, "lrclib");
        assert_eq!(dto.lines[0].time_ms, Some(1_000));
        drop(lrclib);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn disk_cache_survives_a_restart() {
        let tidal = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trackId": 7, "lyrics": "", "subtitles": "[00:01.00] synced"
            })))
            .expect(1)
            .mount(&tidal)
            .await;
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let client = tidal_client(&tidal);
        let first_service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let first = first_service.get(&client, &request(7)).await.unwrap();
        let second_service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let second = second_service.get(&client, &request(7)).await.unwrap();
        assert_eq!(first, second);
        drop(tidal);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_entries() {
        let lrclib = MockServer::start().await;
        let dir = temp_dir();
        let service = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        {
            let mut memory = service.memory.lock().unwrap();
            for id in 0..=2_000u64 {
                memory.insert(
                    id,
                    CacheEntry {
                        fetched_at: id,
                        lyrics: None,
                    },
                );
            }
        }
        service.save_disk();
        let reloaded = LyricsService::new(
            reqwest::Client::new(),
            lrclib.uri(),
            dir.join("lyrics.json"),
        );
        let memory = reloaded.memory.lock().unwrap();
        assert_eq!(memory.len(), 2_000);
        assert!(!memory.contains_key(&0));
        assert!(memory.contains_key(&2_000));
        drop(memory);
        let _ = std::fs::remove_dir_all(dir);
    }
}
