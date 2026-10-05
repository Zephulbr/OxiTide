# Track 10.5 Content Blocking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block explicit and AI-labeled tracks client-side across every enqueue/playback path, with a Settings Content tab, grayed-out rows with `E`/`AI` badges, a local enrichment cache (`cache/track_flags.json`), and restore filtering from `session.json` — always failing open on unknown flags.

**Architecture:** `TidalClient::track_flags` wraps the public v2 `tracks` JSON:API (repeated `filter[id]`, ids are strings) reusing `get_openapi`. A new app module `track_flags.rs` owns the pure flag math (`is_blocked`, `remap_start`, `plan_prune`), an in-memory + lazy-loaded disk cache, and a batched (`≤20/request`), fail-open async `enrich`. `play_track_ids` records `explicit` from the Track DTOs it already fetches and filters after enrichment; a new `apply_content_filters` command clears (explicit) or prunes (AI) the live queue using only existing engine commands. `get_restored_session` filters persisted `SessionItem.explicit/ai`, and the session saver captures cached flags at save time. The UI adds a singleton `trackFlags` store (async enrichment, `filterPlayable`), blocked-row presentation in `TrackTable`, disabled menu items, and filtering at every UI `play_tracks`/`queueAdd` call site.

**Tech Stack:** Rust (std only — no new crates; `serde_json` map cache, `wiremock` tests, `MemoryStore` for keyring), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` (§5.1, §5.3, §6.5, §7, §8; decisions QoL-D2, QoL-D11, QoL-D12)

**Spec refinements (flagged for review):**

1. **No engine changes.** The spec keeps engine additions to 10.1/10.2 (QoL-D12). Queue prune therefore uses existing commands: non-current blocked items are removed with `EngineCmd::Remove` from highest index to lowest (audio is untouched because the current item does not move), and when the **current** item is blocked the queue is replaced once via `EngineCmd::PlayTracks` (playing) or `EngineCmd::RestoreSession` (paused, position 0) so playback advances to the nearest kept track without restarting kept tracks.
2. **`get_track_flags` fails open.** Only an auth failure surfaces (`fail_tidal` → `session_expired`); every other fetch error returns cached/known flags and logs a warning, so a network hiccup never blocks playback (QoL-D11).
3. **Badges mark blocked rows only.** `E`/`AI` appear on rows the active filters hide, per §6.5's enforcement wording; an explicit track with `block_explicit` off shows no badge.
4. **History rows fail open on explicit.** `history.jsonl` records carry no flags; the History play path enriches AI by id but treats explicit as unknown (never blocked) rather than changing the 10.3 record format.
5. **`track_flags.json` layout.** Wrapper object `{version, tracks: {id: {explicit?, ai?, fetched_at}}}` (artwork's `palettes` pattern); spec §5.1's flat sketch cannot carry `version` alongside numeric keys without ambiguity.
6. **Content tab placement:** between Playback and Connections.

## Global Constraints

- Windows-first; no CI. Every check is manual, run by the implementer.
- Gates after every Rust task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`. Gates after every UI task: `npm --prefix ui run check`, `npm --prefix ui test`.
- **No new dependencies.** HTTP tests use `wiremock` only and never hit the live TIDAL API; keyring tests use `MemoryStore`.
- Flags are looked up asynchronously and batched (`TRACK_FLAGS_BATCH = 20`); nothing on the 500 ms position path does disk I/O.
- Unknown `explicit`/`ai` values never block playback or enqueue. Enrichment failures keep known flags and leave unknown ones unknown.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`trackId` ↔ `track_id`). Keep `ui/src/lib/api.ts` in sync by hand.
- Never log track titles, tokens, or cache contents; warnings name paths and reasons only.
- Svelte 5 runes only; components consume semantic tokens only; icons are inlined Lucide paths in `Icon.svelte`.
- Do not edit `AGENTS.md` in this plan — the track completion note owns AGENTS.md updates (spec §10).
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **Enrichment failure must never prevent playback, enqueue, or list rendering.** Pinned by `enrich_fails_open_on_server_error`, `resolve_playable_items_does_not_enrich_when_filters_are_off` (Tasks 3, 5), `filterPlayable keeps everything when filters are off`, `ensure fails open on error` (Task 8), and smoke step 5.
- **Batching and caching: one request per 20 unknown ids, none for cached ids.** Pinned by `enrich_batches_in_twenties_and_caches`, `cached_flags_are_not_refetched` (Task 3) and `same-tick ensures share one request` (Task 8).
- **Enabling the AI filter while the blocked track is playing advances to the nearest kept track without restarting it, and removing non-current blocked items does not interrupt audio.** Pinned by `plan_prune_*` (Task 3) and smoke steps 7–8.
- **Restore drops blocked items, remaps the current index, and keeps a sane position.** Pinned by `filter_session_queue_*` (Task 6).
- **Blocked tracks cannot be played from rows, double-click, menus, queue-add, or Play all/Shuffle.** Pinned by the `blocked` menu test (Task 9) and smoke steps 4, 6, 9 (no component test harness exists in this repo).

---

## File Structure

**Rust — `crates/oxitide-tidal`**

- Create `crates/oxitide-tidal/src/flags.rs` — `TrackFlags`, `TidalClient::track_flags`, JSON:API DTOs, wiremock tests.
- Modify `crates/oxitide-tidal/src/lib.rs` — `pub mod flags;`.

**Rust — `crates/oxitide-app`**

- Create `crates/oxitide-app/src/track_flags.rs` — `TrackFlagEntry`, `is_blocked`, `remap_start`, `plan_prune`, `TrackFlagsStore`, disk cache, tests.
- Modify `crates/oxitide-app/src/config.rs` — `ContentConfig`, `Config.content`, tests.
- Modify `crates/oxitide-app/src/state.rs` — `track_flags` field + accessors + test.
- Modify `crates/oxitide-app/src/commands.rs` — `TrackFlagDto`, four commands, filtering in `play_track_ids`, `queue_items_from_ids` flag recording, tests.
- Modify `crates/oxitide-app/src/session.rs` — flag capture at save, `filter_session_queue`, tests.
- Modify `crates/oxitide-app/src/main.rs` — module, store creation, saver wiring, `generate_handler!`.

**UI**

- Modify `ui/src/lib/api.ts` — `TrackFlag`, `Config.content`, four wrappers.
- Modify `ui/src/lib/settingsTabs.ts` + `ui/src/lib/settingsTabs.test.ts` — Content tab.
- Create `ui/src/lib/trackFlags.svelte.ts` + `ui/src/lib/trackFlags.test.ts` — store, `remapStart`.
- Modify `ui/src/lib/menus.ts` + `ui/src/lib/menus.test.ts` — `blocked` context disables playback items.
- Modify `ui/src/lib/components/TrackTable.svelte` — enrichment effect, gray-out, badges, blocked play/queue-add.
- Modify `ui/src/views/Settings.svelte` — Content panel.
- Modify `ui/src/App.svelte` — store init/reset.
- Modify play paths: `ui/src/views/Album.svelte`, `Playlist.svelte`, `Mix.svelte`, `Artist.svelte`, `ArtistAll.svelte`, `Explore.svelte`, `CollectionTracks.svelte`, `History.svelte`, `ui/src/lib/radio.ts`.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-05-track10-5-content-blocking-smoke.md` — human GUI checklist.

---

### Task 1: TidalClient `track_flags`

**Files:**
- Create: `crates/oxitide-tidal/src/flags.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs:1-18` (module list)

**Interfaces:**
- Consumes: `TidalClient::get_openapi` (`client.rs:332`), `TidalClient::session` (used by `catalog.rs:99`).
- Produces: `oxitide_tidal::flags::TrackFlags { explicit: Option<bool>, ai: Option<bool> }` and `TidalClient::track_flags(&self, ids: &[u64]) -> Result<Vec<(u64, TrackFlags)>>` — used by Task 3's `enrich`.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-tidal/src/flags.rs` with only the test module and the imports the tests need (the implementation comes in Step 3; until then `cargo test` fails to compile because `track_flags` does not exist — that is the expected failure):

```rust
//! Public-OpenAPI track flags: the `ai` label and a second source for
//! `explicit`. One JSON:API call can request many ids.

#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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

    #[tokio::test]
    async fn parses_flags_and_keeps_response_order() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "1", "type": "tracks", "attributes": {"explicit": true, "ai": true}},
                    {"id": "2", "type": "tracks", "attributes": {"explicit": false}}
                ]
            })))
            .mount(&server)
            .await;
        let flags = client(&server).track_flags(&[2, 1]).await.unwrap();
        assert_eq!(
            flags,
            vec![
                (1, super::TrackFlags { explicit: Some(true), ai: Some(true) }),
                (2, super::TrackFlags { explicit: Some(false), ai: None }),
            ]
        );
    }

    #[tokio::test]
    async fn sends_one_filter_param_per_id() {
        let server = MockServer::start().await;
        let responder = |req: &Request| {
            let ids: Vec<String> = req
                .url
                .query_pairs()
                .filter(|(key, _)| key == "filter[id]")
                .map(|(_, value)| value.into_owned())
                .collect();
            if ids == vec!["1".to_string(), "2".to_string()] {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []}))
            } else {
                ResponseTemplate::new(500)
            }
        };
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(responder)
            .mount(&server)
            .await;
        let out = client(&server).track_flags(&[1, 2]).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn missing_attributes_are_unknown_and_junk_ids_are_skipped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "7", "type": "tracks"},
                    {"id": "not-a-number", "type": "tracks", "attributes": {"ai": true}}
                ]
            })))
            .mount(&server)
            .await;
        let flags = client(&server).track_flags(&[7]).await.unwrap();
        assert_eq!(flags, vec![(7, super::TrackFlags { explicit: None, ai: None })]);
    }

    #[tokio::test]
    async fn empty_ids_skip_the_request() {
        let server = MockServer::start().await;
        let flags = client(&server).track_flags(&[]).await.unwrap();
        assert!(flags.is_empty());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-tidal track_flags`
Expected: FAIL to compile with "no method named `track_flags`" / "cannot find type `TrackFlags`".

- [ ] **Step 3: Write the minimal implementation**

Prepend these items to `crates/oxitide-tidal/src/flags.rs` (before the test module):

```rust
use serde::Deserialize;

use crate::client::TidalClient;
use crate::Result;

/// Flags for one track as the public v2 `tracks` attributes report them.
/// `None` means the attribute was absent; callers treat unknown as not blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackFlags {
    pub explicit: Option<bool>,
    pub ai: Option<bool>,
}

impl TidalClient {
    /// Flags for a caller-chosen batch of ids. Response order is preserved;
    /// ids that fail to parse are omitted, and unknown attributes stay `None`.
    pub async fn track_flags(&self, ids: &[u64]) -> Result<Vec<(u64, TrackFlags)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut params: Vec<(&str, String)> =
            vec![("countryCode", self.session().country_code.clone())];
        params.extend(ids.iter().map(|id| ("filter[id]", id.to_string())));
        let raw: OpenApiTracks = self.get_openapi("tracks", &params).await?;
        Ok(raw
            .data
            .into_iter()
            .filter_map(|resource| {
                let id = resource.id.parse::<u64>().ok()?;
                let attributes = resource.attributes.unwrap_or_default();
                Some((id, TrackFlags { explicit: attributes.explicit, ai: attributes.ai }))
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct OpenApiTracks {
    #[serde(default)]
    data: Vec<OpenApiTrack>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrack {
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Option<OpenApiTrackAttributes>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenApiTrackAttributes {
    #[serde(default)]
    explicit: Option<bool>,
    #[serde(default)]
    ai: Option<bool>,
}
```

Add `pub mod flags;` to `crates/oxitide-tidal/src/lib.rs` after `pub mod feed;` (keep the list alphabetical).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-tidal track_flags`
Expected: PASS (4 tests).

- [ ] **Step 5: Run the crate gates**

Run: `cargo test -p oxitide-tidal`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-tidal/src/flags.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): look up track flags on the public OpenAPI host"
```

---

### Task 2: `ContentConfig`

**Files:**
- Modify: `crates/oxitide-app/src/config.rs:9-19` (Config struct), after `HistoryConfig` (~line 242)
- Test: `crates/oxitide-app/src/config.rs` (test module)

**Interfaces:**
- Produces: `crate::config::ContentConfig { block_explicit: bool, block_ai: bool }` (both `false` by default) on `Config.content` — consumed by Tasks 3–6, 8, 10.

- [ ] **Step 1: Write the failing tests**

Add to the config test module (after `pre_history_file_fills_defaults`):

```rust
    #[test]
    fn content_defaults_are_off() {
        let config = Config::default();
        assert!(!config.content.block_explicit);
        assert!(!config.content.block_ai);
    }

    #[test]
    fn content_round_trips_as_documented_toml() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.content.block_explicit = true;
        config.content.block_ai = true;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[content]"));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }
```

Also extend `partial_file_fills_defaults` (config.rs:397-425) with two assertions next to the history ones:

```rust
        assert!(!config.content.block_explicit);
        assert!(!config.content.block_ai);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app content_`
Expected: FAIL to compile with "no field `content`".

- [ ] **Step 3: Write the minimal implementation**

Add the field to `Config` (config.rs:9-19):

```rust
    pub history: HistoryConfig,
    pub content: ContentConfig,
```

Add the section after `HistoryConfig`:

```rust
/// Content blocking (spec §6.5): client-side filters for explicit and
/// AI-labeled tracks. Both default off.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentConfig {
    pub block_explicit: bool,
    pub block_ai: bool,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app content_`, `cargo test -p oxitide-app partial_file_fills_defaults`
Expected: PASS.

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/config.rs
git commit -m "feat(app): add the content config section"
```

---

### Task 3: `track_flags` module — flag math, cache, batching

**Files:**
- Create: `crates/oxitide-app/src/track_flags.rs`
- Modify: `crates/oxitide-app/src/main.rs:3-18` (module list)

**Interfaces:**
- Consumes: `crate::config::{Config, ContentConfig, default_cache_dir}`, `oxitide_tidal::client::TidalClient`, `oxitide_tidal::flags::TrackFlags`, `oxitide_tidal::Error`.
- Produces (used by Tasks 4–6): `TrackFlagEntry { explicit: Option<bool>, ai: Option<bool>, fetched_at: u64 }`, `is_blocked(explicit, ai, &ContentConfig) -> bool`, `remap_start(ids, kept, start) -> usize`, `plan_prune(len, current_index, blocked) -> PruneOutcome`, `track_flags_path() -> Option<PathBuf>`, `TrackFlagsStore::{start, get, known, record_explicit, enrich}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/track_flags.rs` with only the test module and `use super::*;` dependencies (implementation in Step 3):

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app track_flags`
Expected: FAIL to compile ("cannot find type `TrackFlagsStore`" etc.).

- [ ] **Step 3: Write the implementation**

Prepend these items to `crates/oxitide-app/src/track_flags.rs` (before the test module):

```rust
//! Content-blocking flags (spec §6.5): `explicit` observed from v1 Track
//! DTOs, `ai` fetched from the public OpenAPI, cached in memory and in
//! `cache/track_flags.json`. Every failure is fail-open (QoL-D11).

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
```

Add `mod track_flags;` to `crates/oxitide-app/src/main.rs` between `mod state;` and `mod window;`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app track_flags`
Expected: PASS (9 tests).

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green. (If clippy flags `is_none_or`, keep it — it is stable here; otherwise use `map_or(true, ...)`.)

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/track_flags.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add the track flag cache and batching"
```

---

### Task 4: `AppState` wiring, `get_track_flags`, and content setters

**Files:**
- Modify: `crates/oxitide-app/src/state.rs:30-57` (fields), accessors near `history` (~148-162), tests
- Modify: `crates/oxitide-app/src/commands.rs` (DTO + commands)
- Modify: `crates/oxitide-app/src/main.rs` (store creation ~187, `generate_handler!` ~352-377)

**Interfaces:**
- Consumes: `TrackFlagsStore` from Task 3.
- Produces: `TrackFlagDto { track_id: u64, explicit: Option<bool>, ai: Option<bool> }`; commands `get_track_flags`, `set_block_explicit`, `set_block_ai`; `AppState::{track_flags, set_track_flags}` — consumed by Tasks 5–6 and UI Task 7.

- [ ] **Step 1: Write the failing test**

Add to the state test module (mirror `history_starts_unset_and_is_set_once`, state.rs:339-350):

```rust
    #[test]
    fn track_flags_starts_unset_and_is_set_once() {
        let state = AppState::default();
        assert!(state.track_flags().is_none());
        let store = Arc::new(crate::track_flags::TrackFlagsStore::start(None));
        state.set_track_flags(store);
        assert!(state.track_flags().is_some());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app track_flags_starts_unset`
Expected: FAIL to compile ("no method named `track_flags`").

- [ ] **Step 3: Implement the state, command, DTO, and wiring**

In `state.rs`, add the field after `pub history: ...`:

```rust
    /// Content-blocking flag cache; `None` only before startup wiring.
    pub track_flags: Mutex<Option<Arc<crate::track_flags::TrackFlagsStore>>>,
```

Add accessors after the history accessors:

```rust
    pub fn set_track_flags(&self, store: Arc<crate::track_flags::TrackFlagsStore>) {
        *self.track_flags.lock().unwrap() = Some(store);
    }

    pub fn track_flags(&self) -> Option<Arc<crate::track_flags::TrackFlagsStore>> {
        self.track_flags.lock().unwrap().clone()
    }
```

In `commands.rs`, add the DTO near `RestoredSessionDto`:

```rust
/// One track's cached flags as the UI enrichment store expects them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackFlagDto {
    pub track_id: u64,
    pub explicit: Option<bool>,
    pub ai: Option<bool>,
}
```

Add the commands next to the history commands:

```rust
#[tauri::command]
pub async fn get_track_flags(
    state: State<'_, AppState>,
    ids: Vec<u64>,
) -> Result<Vec<TrackFlagDto>, String> {
    let Some(store) = state.track_flags() else {
        return Ok(Vec::new());
    };
    let entries = match require_client(&state) {
        Ok(client) => store
            .enrich(&client, &ids)
            .await
            .map_err(|e| state.fail_tidal(e))?,
        // Not logged in: cached flags only; unknown stays unknown.
        Err(_) => store.known(&ids),
    };
    Ok(entries
        .into_iter()
        .map(|(track_id, entry)| TrackFlagDto {
            track_id,
            explicit: entry.explicit,
            ai: entry.ai,
        })
        .collect())
}

#[tauri::command]
pub fn set_block_explicit(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.content.block_explicit = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_block_ai(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.content.block_ai = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

Register all three in `main.rs`'s `generate_handler!` right after `commands::set_history_max_entries,`:

```rust
            commands::get_track_flags,
            commands::set_block_explicit,
            commands::set_block_ai,
```

In `main.rs`, after `state.set_config(config);` (line 187) and before the session block:

```rust
    let track_flags = std::sync::Arc::new(track_flags::TrackFlagsStore::start(
        track_flags::track_flags_path(),
    ));
    state.set_track_flags(track_flags.clone());
```

Also add the serde-shape test to `commands.rs` tests (next to the other DTO tests):

```rust
    #[test]
    fn track_flag_dto_serializes_camel_case_free_fields() {
        let dto = TrackFlagDto { track_id: 7, explicit: Some(true), ai: None };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json, serde_json::json!({"track_id": 7, "explicit": true, "ai": null}));
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app track_flags`, `cargo test -p oxitide-app track_flag_dto`
Expected: PASS.

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose track flags and content toggles"
```

---

### Task 5: Rust-side playback filtering and `apply_content_filters`

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs:1084-1172` (`queue_items_from_ids`, `play_track_ids`), queue commands (~1327-1358)
- Modify: `crates/oxitide-app/src/commands.rs` tests (update 2 calls, add tests)
- Modify: `crates/oxitide-app/src/main.rs` (`generate_handler!`)

**Interfaces:**
- Consumes: `TrackFlagsStore`, `is_blocked`, `remap_start`, `plan_prune` (Task 3); `ContentConfig` (Task 2).
- Produces: `resolve_playable_items(&TidalClient, &[u64], Option<&TrackFlagsStore>, &ContentConfig) -> Result<(Vec<QueueItem>, Vec<u64>)>`; command `apply_content_filters`.

- [ ] **Step 1: Write the failing tests**

Update the two existing `queue_items_from_ids` tests (commands.rs:2109-2164) to pass `None` as the new third argument:

```rust
        let items = queue_items_from_ids(&tidal_client(&server), &[2, 1], None)
            .await
            .unwrap();
```

and in `queue_items_from_ids_fails_fast_on_a_mid_list_error`:

```rust
        let result = queue_items_from_ids(&tidal_client(&server), &[1, 2], None).await;
```

Add these new tests to the commands test module:

```rust
    fn content(block_explicit: bool, block_ai: bool) -> crate::config::ContentConfig {
        crate::config::ContentConfig { block_explicit, block_ai }
    }

    fn v1_track(id: u64, explicit: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id, "title": format!("T{id}"), "duration": 100,
            "artists": [{"id": 1, "name": "A"}], "explicit": explicit
        })
    }

    #[tokio::test]
    async fn resolve_playable_items_filters_explicit_without_enrichment() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, true)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(2, false)))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) = resolve_playable_items(
            &client,
            &[1, 2],
            Some(&store),
            &content(true, false),
        )
        .await
        .unwrap();

        assert_eq!(kept, vec![2]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].track_id, 2);
        assert_eq!(store.get(2).unwrap().explicit, Some(false));
    }

    #[tokio::test]
    async fn resolve_playable_items_does_not_enrich_when_filters_are_off() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1], Some(&store), &content(false, false))
                .await
                .unwrap();
        assert_eq!(kept, vec![1]);
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn resolve_playable_items_filters_ai_after_enrichment() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(2, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "1", "type": "tracks", "attributes": {"explicit": false, "ai": true}},
                    {"id": "2", "type": "tracks", "attributes": {"explicit": false, "ai": false}}
                ]
            })))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1, 2], Some(&store), &content(false, true))
                .await
                .unwrap();

        assert_eq!(kept, vec![2]);
        assert_eq!(items[0].track_id, 2);
    }

    #[tokio::test]
    async fn resolve_playable_items_fails_open_when_enrichment_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(v1_track(1, false)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let store = crate::track_flags::TrackFlagsStore::start(None);
        let client = tidal_client(&server);
        let (items, kept) =
            resolve_playable_items(&client, &[1], Some(&store), &content(false, true))
                .await
                .unwrap();

        assert_eq!(kept, vec![1], "unknown ai must not block");
        assert_eq!(items.len(), 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app resolve_playable`
Expected: FAIL to compile ("cannot find function `resolve_playable_items`"; existing tests fail on arity).

- [ ] **Step 3: Implement the filter, the prune command, and the wiring**

In `commands.rs`, change `queue_items_from_ids` to accept the store and record explicit observations (replace the helper at 1084-1109):

```rust
/// Build queue items for `ids`, preserving input order, with at most four
/// TIDAL requests in flight. Fails on the first fetch error. `flags` receives
/// every observed `explicit` value so session saves and later filters can use it.
async fn queue_items_from_ids(
    client: &TidalClient,
    ids: &[u64],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
) -> Result<Vec<QueueItem>, Error> {
    let mut results: Vec<Option<QueueItem>> = vec![None; ids.len()];
    let mut pending = ids.iter().copied().enumerate();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let Some((index, id)) = pending.next() else {
            break;
        };
        let client = client.clone();
        set.spawn(async move { (index, client.track(id).await) });
    }
    while let Some(joined) = set.join_next().await {
        let (index, result) = joined.map_err(|e| Error::Task(e.to_string()))?;
        let track = result?;
        if let Some(store) = flags {
            store.record_explicit(track.id, track.explicit);
        }
        results[index] = Some(queue_item_from_track(&track));
        if let Some((next_index, next_id)) = pending.next() {
            let client = client.clone();
            set.spawn(async move { (next_index, client.track(next_id).await) });
        }
    }
    Ok(results.into_iter().flatten().collect())
}

/// Fetch the requested tracks and drop the ones the active content filters
/// hide. Unknown flags never block. Returns the kept items and their ids so
/// callers can remap a start index.
async fn resolve_playable_items(
    client: &TidalClient,
    ids: &[u64],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
    content: &crate::config::ContentConfig,
) -> Result<(Vec<QueueItem>, Vec<u64>), Error> {
    let items = queue_items_from_ids(client, ids, flags).await?;
    if !content.block_explicit && !content.block_ai {
        return Ok((items, ids.to_vec()));
    }
    let known: std::collections::HashMap<u64, crate::track_flags::TrackFlagEntry> = match flags {
        Some(store) => {
            if content.block_ai {
                store.enrich(client, ids).await?;
            }
            store.known(ids).into_iter().collect()
        }
        None => std::collections::HashMap::new(),
    };
    let mut kept_items = Vec::new();
    let mut kept_ids = Vec::new();
    for item in items {
        let entry = known.get(&item.track_id);
        let blocked = crate::track_flags::is_blocked(
            entry.and_then(|entry| entry.explicit),
            entry.and_then(|entry| entry.ai),
            content,
        );
        if !blocked {
            kept_ids.push(item.track_id);
            kept_items.push(item);
        }
    }
    Ok((kept_items, kept_ids))
}
```

Replace `play_track_ids` (1144-1155) with:

```rust
/// Play a list of track ids (resolved server-side), replacing the queue.
/// Content filters are enforced here as a backstop for every UI path.
#[tauri::command]
pub async fn play_track_ids(
    state: State<'_, AppState>,
    ids: Vec<u64>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Err("no tracks".to_string());
    }
    let client = require_client(&state)?;
    let content = state.config().content;
    let store = state.track_flags();
    let (items, kept_ids) =
        resolve_playable_items(&client, &ids, store.as_deref(), &content)
            .await
            .map_err(|e| state.fail_tidal(e))?;
    if items.is_empty() {
        return Err("all tracks are hidden by your content filters".to_string());
    }
    let start = crate::track_flags::remap_start(&ids, &kept_ids, start);
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items }
    } else {
        EngineCmd::PlayTracks { items, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}
```

Add the prune/clear command after `queue_clear` (1337-1343):

```rust
/// Enforce the current content filters on the live queue: `block_explicit`
/// clears it (official parity), `block_ai` removes only AI-labeled items and
/// advances playback when the current track is removed. No-op when both
/// filters are off or nothing is blocked.
#[tauri::command]
pub async fn apply_content_filters(state: State<'_, AppState>) -> Result<(), String> {
    let content = state.config().content;
    if !content.block_explicit && !content.block_ai {
        return Ok(());
    }
    let Some(player) = state.player() else {
        return Ok(());
    };
    if content.block_explicit {
        return player
            .engine
            .handle(EngineCmd::Clear)
            .map_err(|e| e.to_string());
    }
    let snapshot = player.engine.state();
    if snapshot.queue.is_empty() {
        return Ok(());
    }
    let ids: Vec<u64> = snapshot.queue.iter().map(|item| item.track_id).collect();
    let entries = match state.track_flags() {
        Some(store) => match require_client(&state) {
            Ok(client) => store
                .enrich(&client, &ids)
                .await
                .map_err(|e| state.fail_tidal(e))?,
            Err(_) => store.known(&ids),
        },
        None => Vec::new(),
    };
    let known: std::collections::HashMap<u64, crate::track_flags::TrackFlagEntry> =
        entries.into_iter().collect();
    let blocked: Vec<bool> = snapshot
        .queue
        .iter()
        .map(|item| {
            let entry = known.get(&item.track_id);
            crate::track_flags::is_blocked(
                entry.and_then(|entry| entry.explicit),
                entry.and_then(|entry| entry.ai),
                &content,
            )
        })
        .collect();
    let outcome = crate::track_flags::plan_prune(snapshot.queue.len(), snapshot.current_index, &blocked);
    if outcome.remove.is_empty() {
        return Ok(());
    }
    if outcome.current_removed {
        let keep: Vec<QueueItem> = snapshot
            .queue
            .iter()
            .zip(blocked.iter())
            .filter(|(_, blocked)| !**blocked)
            .map(|(item, _)| item.clone())
            .collect();
        if keep.is_empty() {
            return player
                .engine
                .handle(EngineCmd::Clear)
                .map_err(|e| e.to_string());
        }
        let start = outcome.new_current.unwrap_or(0);
        let cmd = if snapshot.playing {
            EngineCmd::PlayTracks { items: keep, start }
        } else {
            EngineCmd::RestoreSession {
                items: keep,
                start,
                position_ms: 0,
                shuffle: snapshot.shuffle,
                repeat: snapshot.repeat,
            }
        };
        player.engine.handle(cmd).map_err(|e| e.to_string())?;
    } else {
        for index in outcome.remove {
            player
                .engine
                .handle(EngineCmd::Remove(index))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
```

Register the command in `main.rs` after `commands::queue_clear,`:

```rust
            commands::apply_content_filters,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app resolve_playable`, `cargo test -p oxitide-app queue_items_from_ids`
Expected: PASS.

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): filter blocked tracks on playback and queue toggles"
```

---

### Task 6: Session flag capture and restore filtering

**Files:**
- Modify: `crates/oxitide-app/src/session.rs` (`SessionSaver`, `session_from_state`, `write_snapshot`, new `filter_session_queue`, tests)
- Modify: `crates/oxitide-app/src/commands.rs:1378-1391` (`get_restored_session`)
- Modify: `crates/oxitide-app/src/main.rs:188-193` (saver wiring)

**Interfaces:**
- Consumes: `TrackFlagsStore` (Task 3), `is_blocked`, `remap_start` semantics implemented inline.
- Produces: `SessionSaver::set_flags(Arc<TrackFlagsStore>)`; `filter_session_queue(items, current_index, position_ms, &ContentConfig) -> FilteredQueue { items, current_index, position_ms }` — consumed by `get_restored_session`.

- [ ] **Step 1: Write the failing tests**

Add to the session test module (fixtures `item(id)` and `temp_dir()`/`path` helpers already exist around session.rs:310-353; reuse them):

```rust
    fn content(block_explicit: bool, block_ai: bool) -> crate::config::ContentConfig {
        crate::config::ContentConfig { block_explicit, block_ai }
    }

    fn flagged(id: u64, explicit: Option<bool>, ai: Option<bool>) -> SessionItem {
        SessionItem { explicit, ai, ..item(id) }
    }

    #[test]
    fn filter_session_queue_drops_blocked_and_remaps_the_dropped_current() {
        let queue = vec![
            flagged(1, Some(true), None),
            flagged(2, None, Some(true)),
            flagged(3, None, None),
        ];
        // The current item (index 1, track 2) is AI-blocked and must be replaced
        // by the next kept item (track 3), with the position reset.
        let filtered = filter_session_queue(queue, Some(1), 5_000, &content(false, true));
        assert_eq!(
            filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(filtered.current_index, Some(1));
        assert_eq!(filtered.position_ms, 0);
    }

    #[test]
    fn filter_session_queue_falls_back_to_the_previous_kept_current() {
        let queue = vec![flagged(9, None, None), flagged(2, None, Some(true))];
        let filtered = filter_session_queue(queue, Some(1), 5_000, &content(false, true));
        assert_eq!(filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(), vec![9]);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 0);
    }

    #[test]
    fn filter_session_queue_keeps_position_when_the_current_item_survives() {
        let queue = vec![flagged(1, Some(true), None), flagged(2, Some(false), None)];
        let filtered = filter_session_queue(queue, Some(1), 42_000, &content(true, false));
        assert_eq!(filtered.items.iter().map(|item| item.track_id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 42_000);
    }

    #[test]
    fn filter_session_queue_passes_through_when_filters_are_off() {
        let queue = vec![flagged(1, Some(true), Some(true))];
        let filtered = filter_session_queue(queue, Some(0), 1_000, &ContentConfig::default());
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.current_index, Some(0));
        assert_eq!(filtered.position_ms, 1_000);
    }

    #[test]
    fn session_from_state_captures_cached_flags() {
        let store = crate::track_flags::TrackFlagsStore::start(None);
        store.record_explicit(3, true);
        let state = playback_state(&[3], Some(0), 0);
        let session = session_from_state(&state, None, 100, Some(&store));
        assert_eq!(session.queue[0].explicit, Some(true));
        assert_eq!(session.queue[0].ai, None);
    }

    #[test]
    fn saver_persists_flags_set_after_start() {
        let dir = temp_dir();
        let path = dir.join("session.json");
        let store = std::sync::Arc::new(crate::track_flags::TrackFlagsStore::start(None));
        store.record_explicit(1, true);
        let saver = SessionSaver::start(path.clone(), Duration::from_secs(60));
        saver.set_flags(store);
        saver.record(&EngineEvent::State(playback_state(&[1], Some(0), 0)));
        saver.flush_now();
        let session = load(&path).expect("session");
        assert_eq!(session.queue[0].explicit, Some(true));
        let _ = std::fs::remove_dir_all(dir);
    }
```

`playback_state(ids, current, position_ms)` already exists in the session test module (`session.rs:406-434`); reuse it instead of adding another helper.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app filter_session_queue`
Expected: FAIL to compile ("cannot find function `filter_session_queue`", `set_flags`).

- [ ] **Step 3: Implement flag capture and filtering**

In `session.rs`, add imports and imports for the types used:

```rust
use std::sync::{Arc, Mutex};

use crate::config::ContentConfig;
use crate::track_flags::{is_blocked, TrackFlagsStore};
```

Change `SessionSaver` to carry a late-bound flags source:

```rust
pub struct SessionSaver {
    tx: Sender<SaverMsg>,
    flags: Arc<Mutex<Option<Arc<TrackFlagsStore>>>>,
}

impl SessionSaver {
    pub fn start(path: PathBuf, debounce: Duration) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let flags = Arc::new(Mutex::new(None));
        let worker_flags = flags.clone();
        thread::Builder::new()
            .name("oxitide-session".into())
            .spawn(move || saver_main(path, debounce, worker_flags, rx))
            .expect("spawn session saver");
        Self { tx, flags }
    }

    /// Wire the flag cache after both services exist (main.rs startup).
    pub fn set_flags(&self, source: Arc<TrackFlagsStore>) {
        *self.flags.lock().unwrap() = Some(source);
    }
    // record / set_play_source / flush_now / clear unchanged
}
```

Change `session_from_state` and `write_snapshot`:

```rust
fn session_from_state(
    state: &PlaybackState,
    play_source: Option<String>,
    saved_at: u64,
    flags: Option<&TrackFlagsStore>,
) -> Session {
    Session {
        version: SESSION_VERSION,
        queue: state
            .queue
            .iter()
            .map(|item| {
                let mut saved = SessionItem::from(item);
                if let Some(entry) = flags.and_then(|store| store.get(item.track_id)) {
                    saved.explicit = entry.explicit;
                    saved.ai = entry.ai;
                }
                saved
            })
            .collect(),
        current_index: state.current_index,
        position_ms: state.position_ms,
        shuffle: state.shuffle,
        repeat: state.repeat,
        play_source,
        saved_at,
    }
}

fn write_snapshot(
    path: &Path,
    snapshot: Option<&PlaybackState>,
    play_source: &Option<String>,
    flags: &Arc<Mutex<Option<Arc<TrackFlagsStore>>>>,
) {
    let Some(state) = snapshot else { return };
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let source = flags.lock().unwrap().clone();
    let session = session_from_state(state, play_source.clone(), saved_at, source.as_deref());
    if let Err(e) = save(path, &session) {
        tracing::warn!("failed to save session: {e}");
    }
}
```

Update `saver_main`'s signature to `fn saver_main(path: PathBuf, debounce: Duration, flags: Arc<Mutex<Option<Arc<TrackFlagsStore>>>>, rx: Receiver<SaverMsg>)` and both `write_snapshot(...)` call sites to pass `&flags`.

Add the restore filter at module scope:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredQueue {
    pub items: Vec<SessionItem>,
    pub current_index: Option<usize>,
    pub position_ms: u64,
}

/// Drop persisted items the active filters hide (unknown flags pass), then
/// remap the current index: the same item when kept, else the next kept one,
/// else the previous kept one. Dropping the current item resets position.
pub fn filter_session_queue(
    queue: Vec<SessionItem>,
    current_index: Option<usize>,
    position_ms: u64,
    content: &ContentConfig,
) -> FilteredQueue {
    let blocked: Vec<bool> = queue
        .iter()
        .map(|item| is_blocked(item.explicit, item.ai, content))
        .collect();
    if !blocked.iter().any(|blocked| *blocked) {
        return FilteredQueue { items: queue, current_index, position_ms };
    }
    let kept: Vec<usize> = (0..queue.len()).filter(|index| !blocked[*index]).collect();
    let current_kept = current_index
        .and_then(|index| blocked.get(index).copied())
        .is_some_and(|blocked| !blocked);
    let position_ms = if current_kept { position_ms } else { 0 };
    let Some(current) = current_index else {
        let items = queue
            .into_iter()
            .zip(blocked.iter())
            .filter(|(_, blocked)| !**blocked)
            .map(|(item, _)| item)
            .collect();
        return FilteredQueue { items, current_index: None, position_ms };
    };
    let new_current = if current_kept {
        kept.iter().position(|index| *index == current)
    } else {
        kept.iter().position(|index| *index >= current).or(Some(kept.len().saturating_sub(1)))
    };
    let new_current = new_current.filter(|_| !kept.is_empty());
    let items = queue
        .into_iter()
        .zip(blocked.iter())
        .filter(|(_, blocked)| !**blocked)
        .map(|(item, _)| item)
        .collect();
    FilteredQueue { items, current_index: new_current, position_ms }
}
```

(`position_ms` handling: when the kept current maps back to the original index, keep the position; any remap means a different track, so reset to 0. The `filter_session_queue_*` tests pin this.)

Update `get_restored_session` in `commands.rs`:

```rust
#[tauri::command]
pub fn get_restored_session(state: State<'_, AppState>) -> Result<Option<RestoredSessionDto>, String> {
    let Some(path) = crate::session::session_path() else {
        return Ok(None);
    };
    let Some(session) = crate::session::load(&path) else {
        return Ok(None);
    };
    let content = state.config().content;
    let filtered = crate::session::filter_session_queue(
        session.queue,
        session.current_index,
        session.position_ms,
        &content,
    );
    Ok(Some(RestoredSessionDto {
        current_index: filtered.current_index.unwrap_or(0),
        position_ms: filtered.position_ms,
        shuffle: session.shuffle,
        repeat: session.repeat,
        play_source: session.play_source,
        queue: filtered.items.into_iter().map(QueueItem::from).collect(),
    }))
}
```

Wire the saver in `main.rs`, replacing the session block (188-193):

```rust
    if let Some(path) = session::session_path() {
        let saver = std::sync::Arc::new(session::SessionSaver::start(
            path,
            session::SESSION_DEBOUNCE,
        ));
        saver.set_flags(track_flags);
        state.set_session_saver(saver);
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app filter_session_queue`, `cargo test -p oxitide-app session_from_state_captures`, `cargo test -p oxitide-app saver_persists_flags`
Expected: PASS.

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app/src/session.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): filter restored sessions and persist cached flags"
```

---

### Task 7: UI API surface and Content tab registration

**Files:**
- Modify: `ui/src/lib/api.ts` (Track flag DTO, Config.content, wrappers)
- Modify: `ui/src/lib/settingsTabs.ts` + `ui/src/lib/settingsTabs.test.ts`

**Interfaces:**
- Produces: `TrackFlag { track_id, explicit, ai }`; `api.getTrackFlags/setBlockExplicit/setBlockAi/applyContentFilters`; `Config.content`; `SettingsTab` includes `"content"` — consumed by Tasks 8–11.

- [ ] **Step 1: Write the failing test**

Update `ui/src/lib/settingsTabs.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { nextTab } from "./settingsTabs";

describe("nextTab", () => {
  it("steps forward", () => {
    expect(nextTab("appearance", 1)).toBe("playback");
  });
  it("steps into the content tab after playback", () => {
    expect(nextTab("playback", 1)).toBe("content");
  });
  it("steps backward", () => {
    expect(nextTab("storage", -1)).toBe("connections");
  });
  it("wraps forward from the last tab", () => {
    expect(nextTab("system", 1)).toBe("appearance");
  });
  it("wraps backward from the first tab", () => {
    expect(nextTab("appearance", -1)).toBe("system");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/settingsTabs.test.ts`
Expected: FAIL — `nextTab("playback", 1)` is `"connections"`.

- [ ] **Step 3: Implement the API types and tab**

In `ui/src/lib/api.ts`, add after the `Track` interface (`:11`):

```ts
export interface TrackFlag {
  track_id: number;
  explicit: boolean | null;
  ai: boolean | null;
}
```

In the `Config` interface, after `history: { enabled: boolean; max_entries: number };`:

```ts
  content: { block_explicit: boolean; block_ai: boolean };
```

Add wrappers next to the history wrappers (`:364-365`):

```ts
  getTrackFlags: (ids: number[]) => invoke<TrackFlag[]>("get_track_flags", { ids }),
  setBlockExplicit: (enabled: boolean) => invoke<void>("set_block_explicit", { enabled }),
  setBlockAi: (enabled: boolean) => invoke<void>("set_block_ai", { enabled }),
  applyContentFilters: () => invoke<void>("apply_content_filters"),
```

In `ui/src/lib/settingsTabs.ts`, insert `"content"` after `"playback"`:

```ts
export const SETTINGS_TABS = [
  "appearance",
  "playback",
  "content",
  "connections",
  "storage",
  "system",
] as const;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/settingsTabs.test.ts`, `npm --prefix ui run check`
Expected: PASS / no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/settingsTabs.ts ui/src/lib/settingsTabs.test.ts
git commit -m "feat(ui): add the content API surface and settings tab"
```

---

### Task 8: `trackFlags` store and App wiring

**Files:**
- Create: `ui/src/lib/trackFlags.svelte.ts`, `ui/src/lib/trackFlags.test.ts`
- Modify: `ui/src/App.svelte` (init and both reset blocks)

**Interfaces:**
- Consumes: `api.getTrackFlags`, `api.getConfig` (Task 7).
- Produces: singleton `trackFlags` — `readonly blockExplicit`, `readonly blockAi`, `init()`, `setContentFlags`, `recordTracks`, `ensure`, `isBlocked`, `blockReasons`, `filterPlayable`, `reset`; pure `remapStart` — consumed by Tasks 9–11.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/trackFlags.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createTrackFlags, remapStart, type TrackLike } from "./trackFlags.svelte";

vi.mock("./api", () => ({
  api: {
    getConfig: vi.fn(),
    getTrackFlags: vi.fn(),
  },
}));

function track(id: number, explicit = false): TrackLike {
  return { id, explicit };
}

beforeEach(() => {
  vi.mocked(api.getConfig).mockResolvedValue({
    content: { block_explicit: false, block_ai: false },
  } as never);
  vi.mocked(api.getTrackFlags).mockReset();
});

describe("remapStart", () => {
  it("follows the original track or the next kept one", () => {
    expect(remapStart([1, 2, 3], [1, 2, 3], 2)).toBe(2);
    expect(remapStart([1, 2, 3], [2, 3], 0)).toBe(0);
    expect(remapStart([1, 2, 3], [1, 3], 1)).toBe(1);
    expect(remapStart([1, 2, 3], [1, 2], 2)).toBe(1);
    expect(remapStart([1, 2], [1], 99)).toBe(0);
  });
});

describe("filterPlayable", () => {
  it("keeps everything and skips the network when filters are off", async () => {
    const store = createTrackFlags();
    const result = await store.filterPlayable([track(1, true), track(2)], 1);
    expect(result?.items.map((t) => t.id)).toEqual([1, 2]);
    expect(result?.start).toBe(1);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });

  it("drops explicit tracks when only that filter is on", async () => {
    const store = createTrackFlags();
    store.setContentFlags(true, false);
    const result = await store.filterPlayable([track(1, true), track(2)], 1);
    expect(result?.items.map((t) => t.id)).toEqual([2]);
    expect(result?.start).toBe(0);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });

  it("enriches and drops AI tracks when that filter is on", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([
      { track_id: 1, explicit: false, ai: true },
      { track_id: 2, explicit: false, ai: false },
    ]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    const result = await store.filterPlayable([track(1), track(2)], 0);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
    expect(vi.mocked(api.getTrackFlags).mock.calls[0][0].sort()).toEqual([1, 2]);
    expect(result?.items.map((t) => t.id)).toEqual([2]);
  });

  it("returns null when everything is filtered", async () => {
    const store = createTrackFlags();
    store.setContentFlags(true, false);
    expect(await store.filterPlayable([track(1, true)], 0)).toBeNull();
  });

  it("fails open when enrichment rejects", async () => {
    vi.mocked(api.getTrackFlags).mockRejectedValue(new Error("offline"));
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    const result = await store.filterPlayable([track(1), track(2)], 0);
    expect(result?.items.map((t) => t.id)).toEqual([1, 2]);
  });
});

describe("ensure", () => {
  it("shares one request between same-tick callers", async () => {
    vi.mocked(api.getTrackFlags).mockResolvedValue([
      { track_id: 1, explicit: false, ai: false },
      { track_id: 2, explicit: false, ai: false },
    ]);
    const store = createTrackFlags();
    store.setContentFlags(false, true);
    await Promise.all([store.ensure([1]), store.ensure([2])]);
    expect(api.getTrackFlags).toHaveBeenCalledTimes(1);
    const batch = vi.mocked(api.getTrackFlags).mock.calls[0][0].sort();
    expect(batch).toEqual([1, 2]);
  });

  it("does nothing when the AI filter is off", async () => {
    const store = createTrackFlags();
    await store.ensure([1, 2]);
    expect(api.getTrackFlags).not.toHaveBeenCalled();
  });
});

describe("init and reset", () => {
  it("loads the toggles from config and clears on reset", async () => {
    vi.mocked(api.getConfig).mockResolvedValue({
      content: { block_explicit: true, block_ai: true },
    } as never);
    const store = createTrackFlags();
    await store.init();
    expect(store.blockExplicit).toBe(true);
    expect(store.blockAi).toBe(true);
    store.reset();
    expect(store.blockExplicit).toBe(false);
    expect(store.blockAi).toBe(false);
    expect(store.isBlocked({ id: 1, explicit: true })).toBe(false);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/trackFlags.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the store**

Create `ui/src/lib/trackFlags.svelte.ts`:

```ts
import { SvelteMap } from "svelte/reactivity";
import { api } from "./api";

export interface TrackFlagInfo {
  explicit: boolean | null;
  ai: boolean | null;
}

/// Anything with an id and, when known from the Track DTO, an explicit flag.
export interface TrackLike {
  id: number;
  explicit?: boolean;
}

export interface Playable<T> {
  items: T[];
  start: number;
}

export interface TrackFlagsStore {
  readonly blockExplicit: boolean;
  readonly blockAi: boolean;
  init(): Promise<void>;
  setContentFlags(blockExplicit: boolean, blockAi: boolean): void;
  recordTracks(tracks: readonly TrackLike[]): void;
  ensure(ids: readonly number[]): Promise<void>;
  isBlocked(track: TrackLike): boolean;
  blockReasons(track: TrackLike): ("explicit" | "ai")[];
  filterPlayable<T extends TrackLike>(
    tracks: readonly T[],
    start: number,
  ): Promise<Playable<T> | null>;
  reset(): void;
}

/// Index of the original start track in the kept list, or of the next kept
/// track, clamped to the last kept item.
export function remapStart(ids: readonly number[], kept: readonly number[], start: number): number {
  if (kept.length === 0 || start >= ids.length) return 0;
  const exact = kept.indexOf(ids[start]);
  if (exact >= 0) return exact;
  const before = ids.slice(0, start).filter((id) => kept.includes(id)).length;
  return Math.min(before, kept.length - 1);
}

export function createTrackFlags(): TrackFlagsStore {
  let blockExplicit = $state(false);
  let blockAi = $state(false);
  const flags = new SvelteMap<number, TrackFlagInfo>();
  const pending = new Set<number>();
  let flushing: Promise<void> | null = null;

  function known(id: number): boolean {
    return flags.has(id) && flags.get(id)!.ai !== null;
  }

  async function init(): Promise<void> {
    try {
      const config = await api.getConfig();
      blockExplicit = config.content.block_explicit;
      blockAi = config.content.block_ai;
    } catch {
      // Defaults stay off; enforcement then matches a fresh config.
    }
  }

  function setContentFlags(explicit: boolean, ai: boolean) {
    blockExplicit = explicit;
    blockAi = ai;
  }

  function recordTracks(tracks: readonly TrackLike[]) {
    for (const track of tracks) {
      if (track.explicit === undefined) continue;
      const entry = flags.get(track.id);
      if (entry?.explicit === track.explicit) continue;
      flags.set(track.id, { explicit: track.explicit, ai: entry?.ai ?? null });
    }
  }

  async function ensure(ids: readonly number[]): Promise<void> {
    if (!blockAi) return;
    for (const id of ids) if (!known(id)) pending.add(id);
    if (pending.size === 0) {
      await flushing;
      return;
    }
    if (!flushing) {
      flushing = (async () => {
        // Let same-tick callers join this batch before it is snapshotted.
        await Promise.resolve();
        const batch = [...pending];
        pending.clear();
        try {
          const rows = await api.getTrackFlags(batch);
          for (const row of rows) {
            const entry = flags.get(row.track_id);
            flags.set(row.track_id, {
              explicit: row.explicit ?? entry?.explicit ?? null,
              ai: row.ai ?? entry?.ai ?? null,
            });
          }
        } catch {
          // Fail open: unknown flags never block playback.
        } finally {
          flushing = null;
        }
      })();
    }
    await flushing;
    if (pending.size > 0) await ensure([]);
  }

  function blockReasons(track: TrackLike): ("explicit" | "ai")[] {
    const entry = flags.get(track.id);
    const reasons: ("explicit" | "ai")[] = [];
    if (blockExplicit && entry?.explicit === true) reasons.push("explicit");
    if (blockAi && entry?.ai === true) reasons.push("ai");
    return reasons;
  }

  function isBlocked(track: TrackLike): boolean {
    return blockReasons(track).length > 0;
  }

  async function filterPlayable<T extends TrackLike>(
    tracks: readonly T[],
    start: number,
  ): Promise<Playable<T> | null> {
    recordTracks(tracks);
    if (blockAi) await ensure(tracks.map((track) => track.id));
    const items = tracks.filter((track) => !isBlocked(track));
    if (items.length === 0) return null;
    const kept = items.map((track) => track.id);
    return { items, start: remapStart(tracks.map((track) => track.id), kept, start) };
  }

  function reset() {
    blockExplicit = false;
    blockAi = false;
    flags.clear();
    pending.clear();
    flushing = null;
  }

  return {
    get blockExplicit() {
      return blockExplicit;
    },
    get blockAi() {
      return blockAi;
    },
    init,
    setContentFlags,
    recordTracks,
    ensure,
    isBlocked,
    blockReasons,
    filterPlayable,
    reset,
  };
}

/// One app-wide instance; views and TrackTable consume it directly.
export const trackFlags = createTrackFlags();
```

Wire `App.svelte`: add `import { trackFlags } from "./lib/trackFlags.svelte";`, add an effect after the appearance effect (`App.svelte:216-218`):

```svelte
  $effect(() => {
    void trackFlags.init();
  });
```

Add `trackFlags.reset();` next to every `favorites.reset();` (App.svelte:161 and :381).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/trackFlags.test.ts`, `npm --prefix ui run check`
Expected: PASS / no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/trackFlags.svelte.ts ui/src/lib/trackFlags.test.ts ui/src/App.svelte
git commit -m "feat(ui): add the track flags store"
```

---

### Task 9: TrackTable gray-out, badges, and blocked actions

**Files:**
- Modify: `ui/src/lib/menus.ts` + `ui/src/lib/menus.test.ts`
- Modify: `ui/src/lib/components/TrackTable.svelte`

**Interfaces:**
- Consumes: `trackFlags` singleton (Task 8).
- Produces: `TrackMenuContext.blocked?: boolean`; blocked-row presentation and gated enqueue actions.

- [ ] **Step 1: Write the failing test**

Add to `ui/src/lib/menus.test.ts`:

```ts
  it("disables playback actions for a blocked track but keeps the rest", () => {
    const items = trackMenuItems(baseCtx({ blocked: true }));
    const byId = new Map(items.map((i) => [i.id, i]));
    expect(byId.get("play")?.disabled).toBe(true);
    expect(byId.get("radio")?.disabled).toBe(true);
    expect(byId.get("add-queue")?.disabled).toBe(true);
    expect(byId.get("favorite")?.disabled).toBeFalsy();
    expect(byId.get("copy-title")?.disabled).toBeFalsy();
    expect(byId.get("add-playlist")?.disabled).toBeFalsy();
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — `blocked` is not in `TrackMenuContext` (TypeScript) and items stay enabled.

- [ ] **Step 3: Implement the menu flag and the TrackTable behavior**

In `menus.ts`, add to `TrackMenuContext`:

```ts
  /// True when the active content filters hide this track.
  blocked?: boolean;
```

and update the base items:

```ts
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[] {
  const playbackBlocked = ctx.blocked === true;
  const items: MenuItem[] = [
    { id: "play", label: "Play", disabled: playbackBlocked, run: () => void ctx.play() },
    {
      id: "radio",
      label: "Track Radio",
      disabled: playbackBlocked,
      run: () => ctx.startRadio(),
    },
    {
      id: "add-queue",
      label: "Add to queue",
      disabled: playbackBlocked,
      run: () => ctx.addToQueue(),
    },
    { id: "add-playlist", label: "Add to playlist", run: () => ctx.addToPlaylist() },
    {
      id: "favorite",
      label: ctx.favorites.hasTrack(ctx.track.id) ? "Unfavorite" : "Favorite",
      run: () => void ctx.favorites.toggleTrack(ctx.track.id),
    },
    { id: "copy-title", label: "Copy title", run: () => ctx.copyTitle() },
  ];
```

In `TrackTable.svelte`:

1. Import the singleton next to the `appearance` import (`:12`):

```ts
  import { trackFlags } from "../trackFlags.svelte";
```

2. Add a blocked-actions helper with the toast copy (below `onRowDoubleClick`, `:136`):

```ts
  function blockedNotice(title: string) {
    toasts.show(
      `"${title}" is hidden by your content filters. Change them in Settings → Content.`,
      { severity: "info" },
    );
  }

  function queueOne(t: Track) {
    if (trackFlags.isBlocked(t)) {
      blockedNotice(t.title);
      return;
    }
    void api.queueAdd([trackToQueueItem(t)]);
  }
```

3. Replace `play()` (`:110-121`) so blocked rows are rejected before `onPlay`, and the fallback filters the visible list:

```ts
  async function play(index: number) {
    const target = rows[index];
    if (target && trackFlags.isBlocked(target)) {
      blockedNotice(target.title);
      return;
    }
    if (onPlay) {
      loadingAll = true;
      try {
        await onPlay(index);
      } finally {
        loadingAll = false;
      }
      return;
    }
    const prepared = await trackFlags.filterPlayable(rows, index);
    if (!prepared) {
      blockedNotice(target?.title ?? "This track");
      return;
    }
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }
```

4. Add the enrichment effect after `const columns = ...` (`:80`):

```ts
  // Rows render immediately; AI flags land asynchronously and only when the
  // AI filter is on (the store's `ensure` is a no-op otherwise).
  $effect(() => {
    trackFlags.recordTracks(slice);
    void trackFlags.ensure(slice.map((t) => t.id));
  });
```

5. In the row (`:188-247`), compute reasons and gate the menu; keep the existing `draggable`/`data-drag-index`/`ondblclick` attributes unchanged between `class:playing` and `use:menuFor`:

```svelte
        {#each slice as t, i (`${t.id}-${range.first + i}`)}
          {@const index = range.first + i}
          {@const reasons = trackFlags.blockReasons(t)}
          <tr
            class="row"
            class:blocked={reasons.length > 0}
            class:dragging={dragFrom === index}
            class:playing={playingTrackId !== null && t.id === playingTrackId}
            draggable={canDrag}
            data-drag-index={canDrag ? index : undefined}
            ondblclick={(event) => onRowDoubleClick(event, index)}
            use:menuFor={() =>
              trackMenuItems({
                track: t,
                index,
                trackCount: rows.length,
                favorites,
                blocked: reasons.length > 0,
                play: () => play(index),
                startRadio: () => void playTrackRadio(t.id, toasts),
                addToQueue: () => queueOne(t),
                addToPlaylist: () => onAddToPlaylist?.(t),
                copyTitle: () => void copyText(t.title),
                playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
              })}
          >
```

6. Extend the title cell and the queue button (inside the same row markup):

```svelte
          <td class="title-cell"><div class="title-text">
            {t.title}
            {#if reasons.includes("explicit")}<span class="badge block-badge" title="Hidden: explicit">E</span>{/if}
            {#if reasons.includes("ai")}<span class="badge block-badge" title="Hidden: AI-generated">AI</span>{/if}
          </div></td>
```

```svelte
            <button
              class="reveal"
              onclick={() => queueOne(t)}
              aria-label="Add to queue"
              title="Add to queue"
            ><Icon name="plus" size={14} /></button>
```

7. Add the styles next to the existing badge rules (`:434-455`):

```css
  .row.blocked .title-text,
  .row.blocked .cell-text {
    opacity: 0.45;
  }
  .row.blocked .title-text {
    text-decoration: line-through;
    text-decoration-color: var(--line);
  }
  .block-badge {
    margin-left: 6px;
    border-color: color-mix(in srgb, var(--warning) 55%, transparent);
    color: var(--warning);
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`, `npm --prefix ui run check`
Expected: PASS / no errors. (`TrackTable` has no component tests; Task 13's smoke note covers it.)

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/components/TrackTable.svelte
git commit -m "feat(ui): gray out blocked rows and gate their actions"
```

---

### Task 10: Settings Content tab

**Files:**
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: `api.setBlockExplicit/setBlockAi/applyContentFilters`, `trackFlags.setContentFlags`, `ConfirmDialog` (already imported).

- [ ] **Step 1: Add labels, state, and handlers**

In `Settings.svelte`, import the store next to the other imports:

```ts
  import { trackFlags } from "../lib/trackFlags.svelte";
```

Add `content: "Content",` to `TAB_LABELS` (`:154-160`) after `playback`.

Add state next to `confirmingHistoryClear` (search for it; it is declared near the other `$state` flags):

```ts
  let confirmingBlockExplicit = $state(false);
```

Add handlers next to `setHistoryEnabled` (`:340-344`):

```ts
  function applyBlockExplicit(enabled: boolean) {
    confirmingBlockExplicit = false;
    if (!config) return;
    const next = { ...config, content: { ...config.content, block_explicit: enabled } };
    config = next;
    api
      .setBlockExplicit(enabled)
      .then(async () => {
        trackFlags.setContentFlags(enabled, next.content.block_ai);
        if (enabled) await api.applyContentFilters();
      })
      .catch(apply);
  }

  function setBlockExplicit(enabled: boolean) {
    if (!config) return;
    if (enabled && !config.content.block_explicit) {
      confirmingBlockExplicit = true;
      return;
    }
    applyBlockExplicit(enabled);
  }

  function setBlockAi(enabled: boolean) {
    if (!config) return;
    const next = { ...config, content: { ...config.content, block_ai: enabled } };
    config = next;
    api
      .setBlockAi(enabled)
      .then(async () => {
        trackFlags.setContentFlags(next.content.block_explicit, enabled);
        if (enabled) await api.applyContentFilters();
      })
      .catch(apply);
  }
```

In `load()` (`Settings.svelte:232-255`), after `config = next;` (`:251`), sync the store so enforcement matches the loaded file even if `trackFlags.init()` ran first:

```ts
      trackFlags.setContentFlags(next.content.block_explicit, next.content.block_ai);
```

- [ ] **Step 2: Add the panel**

Insert this branch between the playback panel's closing `{:else if tab === "connections"}` — i.e. immediately before `{:else if tab === "connections"}`:

```svelte
  {:else if tab === "content"}
    <div id="settings-panel-content" role="tabpanel" aria-labelledby="settings-tab-content">
      <h3>Content filters</h3>
      <p class="muted">
        OxiTide enforces these client-side: TIDAL does not expose its own filters to
        third-party clients, and blocked tracks can still appear in TIDAL recommendations.
        Flagged rows are grayed out and skipped; unknown flags always play.
      </p>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.content.block_explicit}
              onchange={(e) => setBlockExplicit((e.target as HTMLInputElement).checked)}
            />
            <span>Block explicit content</span>
          </label>
          <span class="muted">
            Hides explicit tracks and blocks them from playing. Enabling clears the current
            queue, matching the official app.
          </span>
        </div>
      </div>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.content.block_ai}
              onchange={(e) => setBlockAi((e.target as HTMLInputElement).checked)}
            />
            <span>Block AI-generated content</span>
          </label>
          <span class="muted">
            Hides tracks TIDAL labels as AI-generated and removes them from the queue.
            Labels may be missing for some tracks or regions; those play normally.
          </span>
        </div>
      </div>
      {#if confirmingBlockExplicit}
        <ConfirmDialog
          title="Block explicit content"
          message="This clears the current queue, matching the TIDAL app. Playback will stop."
          onconfirm={() => applyBlockExplicit(true)}
          oncancel={() => (confirmingBlockExplicit = false)}
        />
      {/if}
    </div>
```

- [ ] **Step 3: Verify**

Run: `npm --prefix ui run check`, `npm --prefix ui test`
Expected: no errors, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/Settings.svelte
git commit -m "feat(ui): add the Content settings tab"
```

---

### Task 11: Filter list-view play paths (Album, Playlist, Mix, Explore, ArtistAll, CollectionTracks)

**Files:**
- Modify: `ui/src/views/Album.svelte:79-94`
- Modify: `ui/src/views/Playlist.svelte:241-263`
- Modify: `ui/src/views/Mix.svelte:60-75`
- Modify: `ui/src/views/Explore.svelte:121-136`
- Modify: `ui/src/views/ArtistAll.svelte:164-179`
- Modify: `ui/src/views/CollectionTracks.svelte:87-109`

**Interfaces:**
- Consumes: `trackFlags.filterPlayable` (Task 8).
- Pattern (repeated below per view): import the singleton, await `filterPlayable`, toast on `null`, and pass `prepared.items`/`prepared.start` to `api.playTracks`.

- [ ] **Step 1: Update Album.svelte**

Add the import next to the other `../lib` imports:

```ts
  import { trackFlags } from "../lib/trackFlags.svelte";
```

Replace `playFrom`/`playAll`/`shuffleAll` (`:79-94`):

```ts
  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(view.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
```

(`playFrom` is passed to `TrackTable onPlay`; an async function returning a promise is compatible.)

- [ ] **Step 2: Update Playlist.svelte**

Add the import, then replace `playFrom`/`playAll`/`shuffleAll` (`:241-263`):

```ts
  async function playFrom(index: number) {
    const clicked = view.items[index];
    await pager.ensureAll();
    if (pager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(id);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
```

- [ ] **Step 3: Update Mix.svelte**

Add the import, then replace the three functions (`:60-75`):

```ts
  async function playAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    if (view.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }

  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(view.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }
```

- [ ] **Step 4: Update Explore.svelte**

Add the import, then replace the three functions (`:121-136`) with the same pattern against `resultsView.items`:

```ts
  async function playFrom(index: number) {
    const prepared = await trackFlags.filterPlayable(resultsView.items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    if (resultsView.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(resultsView.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    if (resultsView.items.length === 0) return;
    const prepared = await trackFlags.filterPlayable(resultsView.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
```

- [ ] **Step 5: Update ArtistAll.svelte**

Add the import, then replace `playFrom`/`playAll` (`:164-179`):

```ts
  async function playFrom(index: number) {
    const clicked = view.items[index];
    await pager.ensureAll();
    if (pager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll(shuffle = false) {
    await pager.ensureAll();
    if (pager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, shuffle);
  }
```

- [ ] **Step 6: Update CollectionTracks.svelte**

Add the import, then replace the three functions (`:87-109`):

```ts
  async function playFrom(index: number) {
    const clicked = view.items[index];
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    const prepared = await trackFlags.filterPlayable(view.items, start);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  }

  async function shuffleAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const prepared = await trackFlags.filterPlayable(view.items, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api.playTracks(prepared.items.map(trackToQueueItem), 0, true);
  }
```

- [ ] **Step 7: Verify**

Run: `npm --prefix ui run check`, `npm --prefix ui test`
Expected: no errors, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add ui/src/views/Album.svelte ui/src/views/Playlist.svelte ui/src/views/Mix.svelte ui/src/views/Explore.svelte ui/src/views/ArtistAll.svelte ui/src/views/CollectionTracks.svelte
git commit -m "feat(ui): filter blocked tracks in list-view play paths"
```

---

### Task 12: Filter Artist, History, and radio play paths

**Files:**
- Modify: `ui/src/views/Artist.svelte:112-124`
- Modify: `ui/src/views/History.svelte:74-81`
- Modify: `ui/src/lib/radio.ts:6-22`

**Interfaces:**
- Consumes: `trackFlags.filterPlayable` (Task 8).
- History rows carry no `explicit`, so that path fails open on explicit and enriches AI by id (spec refinement 4).

- [ ] **Step 1: Update Artist.svelte**

Add the import, then replace `queueTop`/`playFrom` (`:112-124`):

```ts
  async function queueTop(startIndex: number, shuffle = false) {
    if (topTracks.length === 0) return;
    const prepared = await trackFlags.filterPlayable(topTracks, startIndex);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start, shuffle);
  }

  function playFrom(index: number) {
    // `view.items` is a prefix of `topTracks` until a filter/sort engages;
    // sourceIndex maps the visible row back to the queue's source order.
    const source = view.sourceIndex(index);
    if (source < 0) return;
    void queueTop(source);
  }
```

- [ ] **Step 2: Update History.svelte**

Add the import, then replace `playEntry` (`:74-81`):

```ts
  async function playEntry(entry: HistoryEntry) {
    const index = entries.indexOf(entry);
    if (index < 0) return;
    // History records carry no flags: explicit fails open, AI is enriched by id.
    const items = entries.map((e) => ({ entry: e, id: e.track_id, explicit: undefined }));
    const prepared = await trackFlags.filterPlayable(items, index);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    void api
      .playTracks(prepared.items.map((item) => historyToQueueItem(item.entry)), prepared.start)
      .catch((e) => {
        toasts.show(String(e), { severity: "error" });
      });
  }
```

- [ ] **Step 3: Update radio.ts**

Add the import and replace the play call inside `playRadio` (`:6-22`):

```ts
import { trackFlags } from "./trackFlags.svelte";

async function playRadio(fetchTracks: () => Promise<Track[]>, toasts: Toasts): Promise<void> {
  try {
    const tracks = await fetchTracks();
    if (tracks.length === 0) {
      toasts.show("No radio tracks available.", { severity: "info" });
      return;
    }
    const prepared = await trackFlags.filterPlayable(tracks, 0);
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(prepared.items.map(trackToQueueItem), prepared.start);
  } catch (e) {
    const message =
      String(e) === "forbidden"
        ? "Radio isn't available for this track right now"
        : errorMessage(e);
    toasts.show(message, { severity: "error" });
  }
}
```

- [ ] **Step 4: Verify**

Run: `npm --prefix ui test -- src/lib/radio.test.ts`, `npm --prefix ui run check`
Expected: PASS / no errors. The existing test mock needs no `getTrackFlags`/`getConfig` entries: the store starts with both filters off, so `filterPlayable` returns the list unchanged without calling the API.

- [ ] **Step 5: Commit**

```bash
git add ui/src/views/Artist.svelte ui/src/views/History.svelte ui/src/lib/radio.ts
git commit -m "feat(ui): filter blocked tracks in artist, history, and radio paths"
```

---

### Task 13: Smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-05-track10-5-content-blocking-smoke.md`

- [ ] **Step 1: Write the checklist**

```markdown
# Track 10.5 content blocking — GUI smoke checklist

Human, real account, Windows. Record pass/fail per step; steps 3–5 and 8 need a
second launch. Steps 7–8 need a real track flagged `explicit` or AI-labeled.

1. Settings opens on Appearance; the tab strip now reads
   Appearance · Playback · Content · Connections · Storage · System, and
   left/right keyboard navigation reaches Content.
2. Content shows both toggles off on a fresh config; copy mentions
   client-side enforcement and unknown flags playing.
3. Enable "Block explicit content": the confirm dialog appears; Confirm clears
   the queue (toast/queue rail empties) and playback stops. Cancel leaves the
   queue untouched.
4. With the filter on, an explicit track row is grayed/struck with an `E`
   badge; clicking its row or double-clicking shows the Settings toast and
   does not start playback; the context menu's Play / Track Radio / Add to
   queue are disabled; Add to playlist and Favorite still work.
5. Disable the filter: rows return to normal, and previously blocked tracks
   play again (turning a filter off is non-retroactive).
6. Block AI: first load may flash un-grayed rows, then AI rows gray out with
   an `AI` badge; "Play all" on that list starts at the first unblocked track
   with the corrected start index; "Add to queue" on a blocked row toasts.
7. Play a list whose current (first) track is AI-labeled, then enable Block
   AI: playback advances to the nearest unblocked track without restarting
   it, and the queue no longer lists the blocked item.
8. Pause mid-queue with a blocked item *after* the current one, enable Block
   AI: the queue drops it, the current track keeps playing at position, and
   toggling Block AI off does not re-add it.
9. Play from Home/Explore cards (`play_track_ids`): blocked tracks are
   skipped server-side; a fully blocked card errors with the toast instead of
   playing.
10. Restart the app with an explicit track as the saved current track and
    Block explicit on: restore comes back paused on the next kept track at
    position 0, and the queue no longer contains blocked items.
11. Offline check: disconnect the network, open a list view with Block AI on;
    rows render (unknown flags play), no error toast storm, playback works.
12. `%APPDATA%\OxiTide\cache\track_flags.json` exists after step 6/7 and its
    entries contain numeric keys with `explicit`/`ai`.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-05-track10-5-content-blocking-smoke.md
git commit -m "docs: add the Track 10.5 content-blocking smoke checklist"
```

---

## Self-Review Notes

- **Spec coverage:** §6.5 Settings Content tab (Task 10), flags via public v2 with ≤20 batching + memory/disk cache + fail-open (Tasks 1, 3), explicit from Track DTOs (Tasks 5, 8), gray-out + `E`/`AI` badges + Settings toast (Task 9), every enqueue path (`play_track_ids` Rust-side Task 5; UI `play_tracks` Tasks 11–12; menus/queue/dblclick/fallback Task 9), toggle clear/prune + advance (Tasks 5, 10), restore filtering from `session.json` (Task 6), no boot-time API storm (lazy disk load in Task 3; restore reads persisted flags in Task 6), PageItem cards limitation (documented in Task 13 step 9 and the Settings copy).
- **Deferred scope untouched:** no love work, no engine API changes.
