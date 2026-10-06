# Track 9.3 — Polish Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the bound polish bundle: feed-card enrichment (instant play, once-played highlight, gray-out, faster Play all), settings-write rollback, the hotkey capture/save race fix, artist-page leftovers (mixes, per-section retries, bio links), and the small cosmetics (playlist sort honesty, Purchases card language, scrobbled indicator).

**Architecture:** `pages.rs` enriches `PageItem` with embedded track metadata; a new `play_page_items` command consumes it and only resolves items whose metadata is incomplete, keeping the Rust content-filter backstop. `PageSections`/`MediaCard` grow blocking/highlight presentation fed by the existing `trackFlags` store. A small `optimistic` helper plus an `appearance` rollback fix make settings writes honest. A Rust mutation lock serializes both hotkey commands (so re-registration can never interleave), and a new `hotkeyCapture` controller sequences capture teardown strictly before save. Artist leftovers touch `Artist.svelte`, `menus.ts`, and a `bio_segments` parser. Cosmetics touch `collectionSort`, `Purchases.svelte`, and a scrobble success event.

**Tech Stack:** Rust (serde, Tauri commands, wiremock), Svelte 5 runes, TypeScript, Vitest, JSON:API via `openapi.tidal.com/v2`.

**Spec:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` (§5.4, §6.3, §7, §8, §9, T9-D6, T9-D8, T9-D9)

## Global Constraints

- **9.0/9.1 carry-in first:** before starting Task 1, read the 9.0 smoke-triage note and the 9.1 smoke result. Defects that touch a surface this plan changes are folded in here only with owner sign-off (T9-D9); anything else is a recorded follow-up. If 9.0's triage note does not exist yet, stop and ask — this plan assumes it ran.
- **Task 0 decides** the playlist sort path (real added-date vs relabel to service order) and confirms which page-payload fields are real. The enrichment code is identical either way (missing fields fall back); only the playlist task branches.
- Items without complete metadata keep today's per-item resolve path (spec §5.4 graceful degradation). Videos stay inert (PM-D6).
- The Rust content-filter backstop stays authoritative: the fast path records observed `explicit`, applies explicit/AI filters, and remaps `start` identically to `play_track_ids`.
- Flags stay batched (≤20/request), fail-open, through the existing `get_track_flags` store; no new remote origins.
- PM-D1: re-run S5 (scroll), S6 (10k filter/sort/album) and S7 (10k queue build) under unchanged budgets; record results in the smoke note.
- The hotkey fix must make stale registration impossible: capture teardown is awaited strictly before the save invoke; a regression test pins the order.
- New strings stay component-local (i18n is a later track).
- Scripted gates after every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.

## Review Focus

1. **Partial payloads:** a page track with some metadata missing (title/duration/album/artists) must still queue correctly through the fallback resolve — never a queue entry with an empty title or zero duration from the fast path.
2. **Flags landing late or failing:** cards gray out when enrichment arrives, fail open, and never block playback on their own; the Rust backstop decides.
3. **Mixed or duplicate sections:** Play all sends only playable tracks in server order; highlight matches by numeric id and never highlights a non-track card.
4. **Hotkey save during capture, or chord captured before capture engages:** exactly one registration of the new binding ends up active; Escape/cancel restores the previous registrations; no double-dispatch on a fast first press.
5. **Playlist payload without dates:** Date mode must not reorder unexpectedly; when relabeled, the order shown matches the service order and the copy says so.
6. **Scrobble success for a non-current track:** the badge must never appear on the currently playing row unless that row was scrobbled.

---

### Task 0: Spike and decision note

**Files:**
- Modify (temporary, reverted in this task): `crates/oxitide-app/src/commands.rs`, `crates/oxitide-app/src/main.rs`
- Create: `docs/superpowers/plans/notes/2026-10-06-track9-3-api-research.md`

**Interfaces:**
- Produces: the spike note (raw payloads + decisions) that Tasks 1, 7, 8 follow.
- Consumes: `TidalClient::get` (`client.rs:97`), `TidalClient::get_openapi` (`client.rs:334`), `require_client`, `state.fail_tidal`, and the existing `get_collection_items` command.

- [ ] **Step 1: Read the carry-in notes and record any deltas**

Read the 9.0 smoke-triage note and the 9.1 smoke note. Note in the research note's "carry-in" section any defect that lands on a surface this plan touches (feed cards, settings toggles, hotkeys, artist page, playlists, purchases, now playing). Do not expand scope without owner sign-off (T9-D9).

- [ ] **Step 2: Add the temporary probe commands**

In `commands.rs` (reuse the temporary-probe pattern from the 9.2 spike, if its probe commit is still in the branch when this runs, adapt rather than duplicate):

```rust
/// TEMPORARY (Track 9.3 spike): raw v1 page probe, removed after the research
/// note lands.
#[tauri::command]
pub async fn debug_get_v1(
    state: State<'_, AppState>,
    path: String,
    params: Vec<(String, String)>,
) -> Result<serde_json::Value, String> {
    let client = require_client(&state)?;
    let params: Vec<(&str, String)> = params
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    client.get(&path, &params).await.map_err(|e| state.fail_tidal(e))
}

/// TEMPORARY (Track 9.3 spike): raw artist-profile probe for the bio markup.
#[tauri::command]
pub async fn debug_get_openapi(
    state: State<'_, AppState>,
    path: String,
    include: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = require_client(&state)?;
    let mut params: Vec<(String, String)> = vec![(
        "countryCode".to_string(),
        client.session().country_code.clone(),
    )];
    if let Some(include) = include {
        params.push(("include".to_string(), include));
    }
    let params: Vec<(&str, String)> = params
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    client
        .get_openapi(&path, &params)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

Register both in `main.rs` `generate_handler![...]`.

- [ ] **Step 3: Probe with the dev app**

Run: `npm --prefix ui run app:dev` (real account). In the WebView2 devtools console:

```js
// Page payloads: which track fields are real? (also run pages/explore)
await window.__TAURI_INTERNALS__.invoke("debug_get_v1", { path: "pages/home", params: [["deviceType","BROWSER"],["platform","WEB"]] })
// Playlist dates: do collection entries carry timestamps today?
await window.__TAURI_INTERNALS__.invoke("get_collection_items", { folderId: "root" })
// Bio markup: pick an artist with a biography
await window.__TAURI_INTERNALS__.invoke("debug_get_openapi", { path: "artists/<ARTIST_ID>", include: "biography" })
```

Capture: (1) a `TRACK_LIST`/`pagedList` track item's fields (`duration`, `explicit`, `audioQuality`, `artists`, `album.title`); (2) whether `created`/`lastItemAddedAt`/`lastUpdated` are non-null on any collection playlist; (3) the raw biography text and whether it contains `[wimpLink artistId="…"]` links.

- [ ] **Step 4: Write the research note**

Create `docs/superpowers/plans/notes/2026-10-06-track9-3-api-research.md` (adjust the date to the run day): status/authority (spec §5.4, §6.3, T9-D6), the carry-in section from Step 1, the captured payload excerpts, and decisions:

- **Page metadata:** which of `{duration, explicit, audioQuality, artists[].name, album.title}` are present on real track items. Fields absent are still parsed as optional and simply trigger the resolve fallback.
- **Playlist dates:** present → Task 8 implements real added-date sorting; absent → Task 8 relabels the toggle to "Service order" and keeps API order.
- **Bio links:** `wimpLink artistId="…"` present → Task 7 ships artist navigation; otherwise Task 7 ships structured parsing but records that links were not observed, and external links stay on the social chips.

- [ ] **Step 5: Remove the temporary probes and commit the note**

Delete both probe commands and their `generate_handler!` lines.

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: green; no probe left in `git diff`.

```bash
git add docs/superpowers/plans/notes/2026-10-06-track9-3-api-research.md
git commit -m "docs: spike the page payloads, playlist dates, and bio markup"
```

---

### Task 1: Page DTO enrichment

**Files:**
- Modify: `crates/oxitide-tidal/src/pages.rs` (`PageItem` lines 6–19, `RawItem` lines 69–105, `RawAlbum` lines 129–133, `page_item_from_raw` lines 236–281, `page_item_from_value` line 311, tests)
- Test: `crates/oxitide-tidal/src/pages.rs` (existing `mod tests`, lines 409+)

**Interfaces:**
- Produces: `PageItem` gains `artists: Vec<String>`, `duration: Option<u32>`, `explicit: Option<bool>`, `audio_quality: Option<String>`, `album_title: Option<String>` (all serialized verbatim to the UI).
- Consumes: existing `RawItem` parsing; Task 0's field confirmation.

- [ ] **Step 1: Write the failing parsing tests**

Add to `pages.rs` `mod tests` (copy the `client(&server)` helper at lines 418–436):

```rust
    #[tokio::test]
    async fn home_track_items_carry_embedded_metadata() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "TRACK_LIST", "title": "New", "pagedList": {"items": [
                    {"id": 5, "title": "Song", "duration": 213, "explicit": true,
                     "audioQuality": "HIRES_LOSSLESS",
                     "artists": [{"id": 1, "name": "Band"}, {"id": 2, "name": "Guest"}],
                     "album": {"id": 9, "title": "Record", "cover": "c-1"}}
                ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        let item = &sections[0].items[0];
        assert_eq!(item.duration, Some(213));
        assert_eq!(item.explicit, Some(true));
        assert_eq!(item.audio_quality.as_deref(), Some("HIRES_LOSSLESS"));
        assert_eq!(item.album_title.as_deref(), Some("Record"));
        assert_eq!(item.artists, vec!["Band".to_string(), "Guest".to_string()]);
    }

    #[tokio::test]
    async fn home_track_items_without_metadata_keep_defaults() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "TRACK_LIST", "title": "New", "items": [
                    {"id": 5, "title": "Bare"}
                ]}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).home_page().await.unwrap();
        let item = &sections[0].items[0];
        assert_eq!(item.duration, None);
        assert_eq!(item.explicit, None);
        assert_eq!(item.audio_quality, None);
        assert_eq!(item.album_title, None);
        assert!(item.artists.is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal home_track_items_carry_embedded_metadata`
Expected: FAIL — no `duration` field on `PageItem`.

- [ ] **Step 3: Implement the DTO and mapping**

Extend `PageItem` with the five fields (after `artwork`):

```rust
    /// Track metadata when the page payload carries it (spec §5.4); absent
    /// fields make playback fall back to a per-item resolve.
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_title: Option<String>,
```

Extend `RawItem` (after `artists`):

```rust
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub explicit: Option<bool>,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
```

Extend `RawAlbum`:

```rust
#[derive(Debug, Deserialize)]
struct RawAlbum {
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}
```

Replace `page_item_from_raw` (lines 236–281) with the enriched version:

```rust
fn page_item_from_raw(module_type: &str, mut raw: RawItem) -> PageItem {
    if let Some(inner) = raw.item.take() {
        let wrapper_type = std::mem::take(&mut raw.item_type);
        raw = *inner;
        if raw.item_type.is_empty() || is_playlist_subtype(&wrapper_type, &raw.item_type) {
            raw.item_type = wrapper_type;
        }
    }
    let id = item_id(&raw);
    let item_type = if raw.item_type.is_empty() {
        module_type.to_string()
    } else {
        raw.item_type
    };
    let artists: Vec<String> = raw
        .artists
        .iter()
        .map(|artist| artist.name.clone())
        .filter(|name| !name.is_empty())
        .collect();
    let subtitle = raw.subtitle.or(raw.description).or_else(|| {
        if item_type.to_uppercase().contains("TRACK") {
            (!artists.is_empty()).then(|| artists.join(", "))
        } else {
            None
        }
    });
    let album_title = raw.album.as_ref().and_then(|album| album.title.clone());
    PageItem {
        id,
        title: raw.title,
        subtitle,
        item_type,
        artwork: raw
            .cover
            .or(raw.square_image)
            .or(raw.image)
            .or(raw.picture)
            .or_else(|| raw.album.as_ref().and_then(|album| album.cover.clone()))
            .or(raw.image_id)
            .or_else(|| {
                raw.images
                    .and_then(|images| images.medium.or(images.large).or(images.small))
                    .and_then(|image| image.url)
            }),
        artists,
        duration: raw.duration,
        explicit: raw.explicit,
        audio_quality: raw.audio_quality,
        album_title,
    }
}
```

In `page_item_from_value` (line 311), add defaults to the constructor: `artists: Vec::new(), duration: None, explicit: None, audio_quality: None, album_title: None,`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal pages`, then `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: all green (existing page tests still pass; `page_item_from_value` callers compile).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/pages.rs
git commit -m "feat(tidal): enrich page track items with embedded metadata"
```

---

### Task 2: Instant-play backend (`play_page_items`)

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (new DTO near `QueueItem`; refactor `resolve_playable_items` lines 1094–1185; new command near `play_track_ids` lines 1187–1218; tests in `mod tests`)
- Modify: `crates/oxitide-app/src/main.rs` (register command)
- Modify: `ui/src/lib/api.ts` (`PageTrackInput` interface; `playPageItems` wrapper)

**Interfaces:**
- Produces: `PageTrackDto`, `queue_items_from_page_items`, `filter_playable_items`, command `play_page_items`; TS `PageTrackInput`, `api.playPageItems(items, start, shuffle)`.
- Consumes: `queue_items_from_ids`, `queue_item_from_track`, `TrackFlagsStore::record_explicit`, `filter_playable_items`'s `remap_start` (unchanged, `commands.rs:1209`).

- [ ] **Step 1: Write the failing backend tests**

In `commands.rs` `mod tests` (helpers `tidal_client` at 2389 and `content` at 2473):

```rust
    fn page_track(id: u64, complete: bool) -> PageTrackDto {
        if complete {
            PageTrackDto {
                id,
                title: Some(format!("T{id}")),
                artists: Some(vec!["A".into()]),
                album: Some("B".into()),
                duration: Some(200),
                cover: Some("c".into()),
                explicit: Some(false),
            }
        } else {
            PageTrackDto {
                id,
                title: None,
                artists: None,
                album: None,
                duration: None,
                cover: None,
                explicit: None,
            }
        }
    }

    fn queue_row(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 200,
            cover: None,
        }
    }

    #[tokio::test]
    async fn queue_items_from_page_items_skips_fetch_for_complete_metadata() {
        let server = MockServer::start().await;
        // No /tracks/{id} mock is mounted; any fetch would 404 and fail the call.
        let items =
            queue_items_from_page_items(&tidal_client(&server), &[page_track(1, true)], None)
                .await
                .unwrap();
        assert_eq!(items[0].track_id, 1);
        assert_eq!(items[0].title, "T1");
        assert_eq!(items[0].artist, "A");
        assert_eq!(items[0].album, "B");
        assert_eq!(items[0].duration_secs, 200);
        assert_eq!(items[0].cover.as_deref(), Some("c"));
    }

    #[tokio::test]
    async fn queue_items_from_page_items_fetches_only_incomplete_metadata() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2, "title": "Fetched", "duration": 180,
                "artists": [{"id": 5, "name": "Artist"}],
                "album": {"id": 7, "title": "Album", "cover": "cover-2"}
            })))
            .mount(&server)
            .await;
        let items = queue_items_from_page_items(
            &tidal_client(&server),
            &[page_track(1, true), page_track(2, false)],
            None,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track_id, 1);
        assert_eq!(items[1].track_id, 2);
        assert_eq!(items[1].title, "Fetched");
        assert_eq!(items[1].duration_secs, 180);
    }

    #[tokio::test]
    async fn queue_items_from_page_items_records_embedded_explicit() {
        let server = MockServer::start().await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        let mut track = page_track(1, true);
        track.explicit = Some(true);
        queue_items_from_page_items(&tidal_client(&server), &[track], Some(&store))
            .await
            .unwrap();
        let known = store.known(&[1]);
        assert_eq!(known[0].1.explicit, Some(true));
    }

    #[tokio::test]
    async fn filter_playable_items_blocks_and_reports_kept_ids() {
        let server = MockServer::start().await;
        let store = crate::track_flags::TrackFlagsStore::start(None);
        store.record_explicit(2, true);
        let (kept, ids) = filter_playable_items(
            &tidal_client(&server),
            &[1, 2],
            vec![queue_row(1), queue_row(2)],
            Some(&store),
            &content(true, false),
        )
        .await
        .unwrap();
        assert_eq!(ids, vec![1]);
        assert_eq!(kept.len(), 1);
    }

    #[tokio::test]
    async fn filter_playable_items_enriches_ai_flags_fail_open() {
        let server = MockServer::start().await;
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
        let (kept, ids) = filter_playable_items(
            &tidal_client(&server),
            &[1, 2],
            vec![queue_row(1), queue_row(2)],
            Some(&store),
            &content(false, true),
        )
        .await
        .unwrap();
        assert_eq!(ids, vec![2]);
        assert_eq!(kept[0].track_id, 2);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app queue_items_from_page_items`
Expected: FAIL — `PageTrackDto` / `queue_items_from_page_items` not found.

- [ ] **Step 3: Implement the DTO, helpers, and command**

In `commands.rs`, near the `QueueItem` DTO:

```rust
/// One page-feed track as the UI sends it for instant playback: embedded
/// metadata is used when complete, otherwise the id is resolved server-side.
#[derive(Debug, Clone, Deserialize)]
pub struct PageTrackDto {
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artists: Option<Vec<String>>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub explicit: Option<bool>,
}

impl PageTrackDto {
    /// The queue item when the payload carried enough metadata; `None` asks
    /// the caller to resolve the track by id.
    fn embedded(&self) -> Option<QueueItem> {
        let title = self.title.as_deref().filter(|title| !title.is_empty())?;
        let artists = self.artists.as_ref().filter(|artists| !artists.is_empty())?;
        let duration = self.duration.filter(|duration| *duration > 0)?;
        Some(QueueItem {
            track_id: self.id,
            title: title.to_string(),
            artist: artists.join(", "),
            album: self.album.clone().unwrap_or_default(),
            duration_secs: duration,
            cover: self.cover.clone(),
        })
    }
}

/// Queue items for page-feed tracks, preserving order. Complete embedded
/// metadata skips the per-track API call (spec §5.4); incomplete entries fall
/// back to `queue_items_from_ids`, so a partial payload still plays.
async fn queue_items_from_page_items(
    client: &TidalClient,
    items: &[PageTrackDto],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
) -> Result<Vec<QueueItem>, Error> {
    let missing: Vec<u64> = items
        .iter()
        .filter(|item| item.embedded().is_none())
        .map(|item| item.id)
        .collect();
    let mut resolved: std::collections::HashMap<u64, QueueItem> = if missing.is_empty() {
        std::collections::HashMap::new()
    } else {
        queue_items_from_ids(client, &missing, flags)
            .await?
            .into_iter()
            .map(|item| (item.track_id, item))
            .collect()
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(entry) = item.embedded() {
            if let (Some(store), Some(explicit)) = (flags, item.explicit) {
                store.record_explicit(item.id, explicit);
            }
            out.push(entry);
        } else if let Some(entry) = resolved.remove(&item.id) {
            out.push(entry);
        } else {
            return Err(Error::NotFound(format!("tracks/{}", item.id)));
        }
    }
    Ok(out)
}
```

Split the existing `resolve_playable_items` (now that fetching and filtering are separate):

```rust
/// Fetch the requested tracks and filter them; kept for the id-only path.
async fn resolve_playable_items(
    client: &TidalClient,
    ids: &[u64],
    flags: Option<&crate::track_flags::TrackFlagsStore>,
    content: &crate::config::ContentConfig,
) -> Result<(Vec<QueueItem>, Vec<u64>), Error> {
    let items = queue_items_from_ids(client, ids, flags).await?;
    filter_playable_items(client, ids, items, flags, content).await
}

/// Apply the active content filters to already-resolved queue items. Unknown
/// flags never block; AI enrichment uses `ids` to batch-fetch flags.
async fn filter_playable_items(
    client: &TidalClient,
    ids: &[u64],
    items: Vec<QueueItem>,
    flags: Option<&crate::track_flags::TrackFlagsStore>,
    content: &crate::config::ContentConfig,
) -> Result<(Vec<QueueItem>, Vec<u64>), Error> {
    if !content.block_explicit && !content.block_ai {
        let kept_ids = items.iter().map(|item| item.track_id).collect();
        return Ok((items, kept_ids));
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

Add the command after `play_track_ids` (~line 1218):

```rust
/// Play page-feed tracks, using embedded metadata and resolving only the
/// items whose payload was incomplete. Content filters are enforced here as
/// the backstop for every UI path.
#[tauri::command]
pub async fn play_page_items(
    state: State<'_, AppState>,
    items: Vec<PageTrackDto>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("no tracks".to_string());
    }
    let client = require_client(&state)?;
    let content = state.config().content;
    let store = state.track_flags();
    let ids: Vec<u64> = items.iter().map(|item| item.id).collect();
    let queue = queue_items_from_page_items(&client, &items, store.as_deref())
        .await
        .map_err(|e| state.fail_tidal(e))?;
    let (queue, kept_ids) = filter_playable_items(&client, &ids, queue, store.as_deref(), &content)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    if queue.is_empty() {
        return Err("all tracks are hidden by your content filters".to_string());
    }
    let start = crate::track_flags::remap_start(&ids, &kept_ids, start);
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items: queue }
    } else {
        EngineCmd::PlayTracks { items: queue, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}
```

Register `commands::play_page_items,` in `main.rs` after `commands::play_track_ids,`.

In `api.ts`, after `QueueItem`:

```ts
export interface PageTrackInput {
  id: number;
  title?: string | null;
  artists?: string[] | null;
  album?: string | null;
  duration?: number | null;
  cover?: string | null;
  explicit?: boolean | null;
}
```

and after `playTrackIds` (line 354):

```ts
  playPageItems: (items: PageTrackInput[], start: number, shuffle = false) =>
    invoke<void>("play_page_items", { items, start, shuffle }),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app queue_items_from_page_items`, `cargo test -p oxitide-app filter_playable_items`, then `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: all green, including the pre-existing `resolve_playable_items_*` tests (the refactor keeps behavior).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts
git commit -m "feat(playback): play page items from embedded metadata"
```

---

### Task 3: Feed cards — instant play, gray-out, highlight

**Files:**
- Modify: `ui/src/lib/api.ts` (`PageItem` interface line 45)
- Modify: `ui/src/lib/media.ts` (add `pageTrackInput`)
- Test: `ui/src/lib/media.test.ts`
- Modify: `ui/src/lib/components/PageSections.svelte` (script lines 1–62, markup lines 64–110)
- Modify: `ui/src/lib/components/MediaCard.svelte` (props lines 8–20, roots lines 55–64, styles)
- Modify: `ui/src/views/Home.svelte` (line 69), `ui/src/views/Explore.svelte` (line 296), `ui/src/views/Section.svelte` (line 58), `ui/src/views/Mixes.svelte` (line 48)
- Modify: `ui/src/App.svelte` (routes lines 545/552/556)

**Interfaces:**
- Produces: `pageTrackInput(item)`, `PageSections` props `player?`, `MediaCard` props `playing`/`blocked`.
- Consumes: `api.playPageItems` (Task 2), `trackFlags.recordTracks/ensure/blockReasons` (`trackFlags.svelte.ts`), `player.state.current`.

- [ ] **Step 1: Write the failing helper test**

In `media.test.ts` add:

```ts
import { pageTrackInput } from "./media";

describe("pageTrackInput", () => {
  it("maps complete enriched items for instant play", () => {
    const input = pageTrackInput({
      id: "5", title: "Song", subtitle: "Band", item_type: "TRACK",
      artwork: "c-1", duration: 213, explicit: true, audio_quality: "LOSSLESS",
      album_title: "Record", artists: ["Band", "Guest"],
    });
    expect(input).toEqual({
      id: 5, title: "Song", artists: ["Band", "Guest"], album: "Record",
      duration: 213, cover: "c-1", explicit: true,
    });
  });

  it("sends null for missing metadata so the backend resolves it", () => {
    const input = pageTrackInput({
      id: "5", title: "Bare", subtitle: null, item_type: "TRACK", artwork: null,
    });
    expect(input.artists).toBeNull();
    expect(input.album).toBeNull();
    expect(input.duration).toBeNull();
    expect(input.cover).toBeNull();
    expect(input.explicit).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/media.test.ts`
Expected: FAIL — `pageTrackInput` not exported.

- [ ] **Step 3: Implement the type, helper, and card presentation**

In `api.ts`, replace the `PageItem` interface (line 45):

```ts
export interface PageItem {
  id: string; title: string; subtitle?: string | null; item_type: string; artwork?: string | null;
  /// Embedded track metadata when the feed payload carries it (Track 9.3).
  artists?: string[] | null;
  duration?: number | null;
  explicit?: boolean | null;
  audio_quality?: string | null;
  album_title?: string | null;
}
```

In `media.ts` (import `PageTrackInput` from `./api`):

```ts
/// Playable page item → the payload for `playPageItems`. Per-field nulls mark
/// metadata the payload did not carry; the backend resolves those ids.
export function pageTrackInput(item: PageItem): PageTrackInput {
  return {
    id: Number(item.id),
    title: item.title || null,
    artists: item.artists && item.artists.length > 0 ? item.artists : null,
    album: item.album_title ?? null,
    duration: item.duration ?? null,
    cover: item.artwork ?? null,
    explicit: item.explicit ?? null,
  };
}
```

In `MediaCard.svelte`, add `playing = false` and `blocked = false` props with the doc comments from Review Focus, apply `class:playing` / `class:blocked` on both roots (button line 56–61 and div line 63), and add CSS:

```css
  .media.playing {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .media.playing .title {
    color: var(--accent);
  }
  .media.blocked .title,
  .media.blocked .sub {
    opacity: 0.45;
  }
  .media.blocked .title {
    text-decoration: line-through;
    text-decoration-color: var(--line);
  }
  .media.blocked .art-clip :global(img) {
    opacity: 0.55;
  }
```

- [ ] **Step 4: Rewire `PageSections`**

Replace the `<script>` (lines 1–62) with:

```svelte
<script lang="ts">
  import { api, type PageItem, type PageSection } from "../api";
  import { errorMessage } from "../errors";
  import { classifySection, isPlayableTrack, pageTrackInput } from "../media";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Player } from "../player.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import type { Toasts } from "../toasts.svelte";
  import Carousel from "./Carousel.svelte";
  import Icon from "./Icon.svelte";
  import MediaCard from "./MediaCard.svelte";

  let {
    sections,
    nav,
    toasts,
    source,
    player = null,
    uncapped = false,
    showTitle = true,
  }: {
    sections: PageSection[];
    nav: Nav;
    toasts: Toasts;
    source: "home" | "explore" | "forYou";
    player?: Player | null;
    uncapped?: boolean;
    showTitle?: boolean;
  } = $props();

  const TILE_CAP = 12;
  const LIST_CAP = 8;

  let busy = $state(false);

  const playingId = $derived(player?.state.current?.track_id ?? null);

  function asTrackLike(item: PageItem) {
    return { id: Number(item.id), explicit: item.explicit ?? undefined };
  }

  function isPlaying(item: PageItem): boolean {
    return isPlayableTrack(item) && Number(item.id) === playingId;
  }

  function isBlocked(item: PageItem): boolean {
    return isPlayableTrack(item) && trackFlags.blockReasons(asTrackLike(item)).length > 0;
  }

  function feedTracks(items: PageItem[]): PageItem[] {
    return items.filter(isPlayableTrack);
  }

  function play(items: PageItem[], start: number) {
    playSource.set(null);
    return api.playPageItems(items.map(pageTrackInput), start);
  }

  function activate(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    if (route) return () => nav.goto(route);
    if (isPlayableTrack(item)) {
      return () => {
        void play([item], 0).catch((e) =>
          toasts.show(errorMessage(e), { severity: "error" }),
        );
      };
    }
    return undefined;
  }

  async function playAll(items: PageItem[]) {
    if (busy) return;
    busy = true;
    try {
      await play(items, 0);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  /// Visible track items across every rendered section; the flags store
  /// batches them (≤20/request, fail-open) and is a no-op when the filters and
  /// labels are off.
  const visibleTracks = $derived.by(() => {
    const tracks: PageItem[] = [];
    for (const section of sections) {
      const treatment = classifySection(section.kind, section.items);
      const effective = uncapped && treatment === "carousel" ? "tiles" : treatment;
      const cap = effective === "list" ? LIST_CAP : TILE_CAP;
      const shown = uncapped ? section.items : section.items.slice(0, cap);
      tracks.push(...feedTracks(shown));
    }
    return tracks;
  });

  $effect(() => {
    trackFlags.recordTracks(visibleTracks.map(asTrackLike));
    void trackFlags.ensure(visibleTracks.map((item) => Number(item.id)));
  });
</script>
```

In the markup, rename the per-section `ids` to `tracks` and pass the new card props:

- `{@const ids = effective === "list" ? playableTrackIds(section.items) : []}` → `{@const tracks = effective === "list" ? feedTracks(section.items) : []}`
- `{#if showTitle || ids.length > 0 || capped}` → `{#if showTitle || tracks.length > 0 || capped}`
- `{#if ids.length > 0}` → `{#if tracks.length > 0}` and `onclick={() => playAll(ids)}` → `onclick={() => playAll(tracks)}`
- Both `MediaCard` call sites gain `playing={isPlaying(item)} blocked={isBlocked(item)}`.

- [ ] **Step 5: Thread `player` through the views**

- `Home.svelte`: add `player: Player` to props (import the type) and pass `{player}` into `<PageSections ...>` at line 69.
- `Explore.svelte`: pass `{player}` into `<PageSections ...>` at line 296 (already a prop).
- `Section.svelte`: add `player: Player` prop and pass it at line 58.
- `Mixes.svelte`: add `player: Player` prop and pass it at line 48.
- `App.svelte`: `<Home {nav} {player} {toasts} />` (line 545), `<Mixes {nav} {player} {toasts} />` (552), `<Section source={...} {nav} {player} {toasts} />` (556).

- [ ] **Step 6: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `cargo test --workspace`.
Expected: green. Manual: blocked tracks gray out on Home/Explore when a filter is on; the playing card is accent-highlighted; clicking a track card plays with no per-track `/tracks/{id}` fetch (watch devtools network); Play all works for track lists.

```bash
git add ui/src/lib/api.ts ui/src/lib/media.ts ui/src/lib/media.test.ts ui/src/lib/components/PageSections.svelte ui/src/lib/components/MediaCard.svelte ui/src/views/Home.svelte ui/src/views/Explore.svelte ui/src/views/Section.svelte ui/src/views/Mixes.svelte ui/src/App.svelte
git commit -m "feat(ui): enrich feed cards with instant play, gray-out, and highlight"
```

---

### Task 4: Settings rollback

**Files:**
- Create: `ui/src/lib/optimistic.ts`
- Test: `ui/src/lib/optimistic.test.ts`
- Modify: `ui/src/lib/appearance.svelte.ts` (`setOneko` line 173; interface line 84)
- Test: `ui/src/lib/appearance.test.ts` (oneko describe, line 73)
- Modify: `ui/src/views/Settings.svelte` (handlers lines 339–355)

**Interfaces:**
- Produces: `optimistic(value, next, set, persist, onError)`, rollback-aware `appearance.setOneko`.
- Consumes: `api.setOneko` / `api.setSmtc` / `api.setCloseToTray`, `toasts` via `apply` (`Settings.svelte:255`).

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/optimistic.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { optimistic } from "./optimistic";

describe("optimistic", () => {
  it("keeps the new value when persistence succeeds", async () => {
    let value = false;
    const set = (next: boolean) => (value = next);
    await optimistic(value, true, set, async () => {});
    expect(value).toBe(true);
  });

  it("reverts and reports when persistence fails", async () => {
    let value = false;
    const set = (next: boolean) => (value = next);
    const onError = vi.fn();
    const error = new Error("disk full");
    await optimistic(value, true, set, async () => Promise.reject(error), onError);
    expect(value).toBe(false);
    expect(onError).toHaveBeenCalledWith(error);
  });
});
```

In `appearance.test.ts` (inside the `oneko setting` describe), add:

```ts
  it("setOneko rolls back when persistence fails", async () => {
    vi.mocked(api.setOneko).mockRejectedValueOnce(new Error("disk full"));
    await expect(appearance.setOneko(true)).rejects.toThrow("disk full");
    expect(appearance.oneko).toBe(false);
  });
```

(If the file's `beforeEach` does not already reset `setOneko` to resolve, add `vi.mocked(api.setOneko).mockResolvedValue(undefined)` there.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/optimistic.test.ts src/lib/appearance.test.ts`
Expected: FAIL — `./optimistic` missing; the setOneko rollback assertion fails (the value stays flipped today).

- [ ] **Step 3: Implement the helper and the appearance rollback**

Create `ui/src/lib/optimistic.ts`:

```ts
/// Flip a value optimistically, persist, and revert on failure. Kept as a
/// standalone helper so settings handlers share one rollback shape and the
/// behavior is testable without component infra.
export async function optimistic<T>(
  value: T,
  next: T,
  set: (value: T) => void,
  persist: () => Promise<void>,
  onError?: (error: unknown) => void,
): Promise<void> {
  const previous = value;
  set(next);
  try {
    await persist();
  } catch (error) {
    set(previous);
    onError?.(error);
  }
}
```

In `appearance.svelte.ts` replace the inline `setOneko` (line 173) with a rollback-aware function (and return it):

```ts
  /// Local-first like the other setters, but a failed persist reverts the
  /// flag so the UI never disagrees with the stored config.
  async function setOneko(enabled: boolean): Promise<void> {
    const previous = oneko;
    oneko = enabled;
    try {
      await api.setOneko(enabled);
    } catch (error) {
      oneko = previous;
      throw error;
    }
  }
```

```ts
    setOneko,
```

- [ ] **Step 4: Apply the same shape to the config toggles**

In `Settings.svelte` import `{ optimistic } from "../lib/optimistic";` and replace the three handlers:

```ts
  function setOneko(enabled: boolean) {
    if (!config) return;
    const previous = config.ui.oneko;
    config = { ...config, ui: { ...config.ui, oneko: enabled } };
    appearance.setOneko(enabled).catch((e) => {
      config = { ...config, ui: { ...config.ui, oneko: previous } };
      apply(e);
    });
  }

  function setSmtc(enabled: boolean) {
    if (!config) return;
    void optimistic(
      config.integrations.smtc,
      enabled,
      (value) => {
        if (config) {
          config = { ...config, integrations: { ...config.integrations, smtc: value } };
        }
      },
      () => api.setSmtc(enabled),
      apply,
    );
  }

  function setCloseToTray(enabled: boolean) {
    if (!config) return;
    void optimistic(
      config.ui.close_to_tray,
      enabled,
      (value) => {
        if (config) config = { ...config, ui: { ...config.ui, close_to_tray: value } };
      },
      () => api.setCloseToTray(enabled),
      apply,
    );
  }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/optimistic.test.ts src/lib/appearance.test.ts`, then `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/optimistic.ts ui/src/lib/optimistic.test.ts ui/src/lib/appearance.svelte.ts ui/src/lib/appearance.test.ts ui/src/views/Settings.svelte
git commit -m "fix(settings): roll back failed toggles"
```

---

### Task 5: Hotkey capture/save ordering

**Files:**
- Modify: `crates/oxitide-app/src/hotkeys.rs` (add the mutation lock; tests in `mod tests` line 235)
- Modify: `crates/oxitide-app/src/commands.rs` (`set_hotkey` line 1977, `set_hotkey_capture` line 2006)
- Create: `ui/src/lib/hotkeyCapture.svelte.ts`
- Test: `ui/src/lib/hotkeyCapture.test.ts`
- Modify: `ui/src/views/Settings.svelte` (state line 68, effect lines 194–213, handlers lines 357–381, markup ~1036–1058)

**Interfaces:**
- Produces: Rust `hotkeys::with_mutation_lock` (serializes config writes + re-registration across both commands — the backend guarantee), and `createHotkeyCapture({ setCapture, save, report })` with `recording`, `active`, `start(action)`, `cancel()`, `finish(action, accelerator, replace?)` (the client guarantee: teardown strictly before save, listener only after engagement).
- Consumes: `api.setHotkeyCapture`, `api.setHotkey`, `acceleratorFromEvent`, `conflictFor`.

- [ ] **Step 1: Write the failing Rust serialization test**

In `hotkeys.rs` `mod tests`:

```rust
    #[test]
    fn mutations_are_serialized() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let inside = Arc::new(AtomicBool::new(false));
        let overlapped = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let inside = inside.clone();
            let overlapped = overlapped.clone();
            handles.push(std::thread::spawn(move || {
                with_mutation_lock(|| {
                    if inside.swap(true, Ordering::SeqCst) {
                        overlapped.store(true, Ordering::SeqCst);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    inside.store(false, Ordering::SeqCst);
                });
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(!overlapped.load(Ordering::SeqCst));
    }
```

- [ ] **Step 2: Write the failing UI ordering tests**

Create `ui/src/lib/hotkeyCapture.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createHotkeyCapture } from "./hotkeyCapture.svelte";

function setup() {
  const calls: string[] = [];
  const capture = createHotkeyCapture({
    setCapture: vi.fn(async (active: boolean) => {
      calls.push(`capture:${active}`);
    }),
    save: vi.fn(async () => {
      calls.push("save");
    }),
    report: vi.fn(),
  });
  return { capture, calls };
}

function setupPendingCapture() {
  const calls: string[] = [];
  let releaseCapture: () => void = () => {};
  const capture = createHotkeyCapture({
    setCapture: vi.fn((active: boolean) => {
      calls.push(`capture:${active}`);
      if (!active) return Promise.resolve();
      return new Promise<void>((resolve) => (releaseCapture = resolve));
    }),
    save: vi.fn(async () => {
      calls.push("save");
    }),
    report: vi.fn(),
  });
  return { capture, calls, release: () => releaseCapture() };
}

describe("hotkey capture", () => {
  it("engages capture before reporting active", async () => {
    const { capture, calls } = setup();
    await capture.start("play_pause");
    expect(calls).toEqual(["capture:true"]);
    expect(capture.active).toBe(true);
    expect(capture.recording).toBe("play_pause");
  });

  it("orders capture teardown strictly before save", async () => {
    const { capture, calls } = setup();
    await capture.start("play_pause");
    await capture.finish("play_pause", "Ctrl+Alt+KeyP");
    expect(calls).toEqual(["capture:true", "capture:false", "save"]);
    expect(capture.recording).toBeNull();
  });

  it("finishes while capture engagement is still in flight", async () => {
    const { capture, calls, release } = setupPendingCapture();
    const starting = capture.start("next");
    await Promise.resolve(); // engagement reached the backend and is pending
    const finishing = capture.finish("next", "Ctrl+Alt+KeyN");
    release();
    await starting;
    await finishing;
    expect(calls).toEqual(["capture:true", "capture:false", "save"]);
  });

  it("cancels an in-flight engagement without leaving capture on", async () => {
    const { capture, calls, release } = setupPendingCapture();
    const starting = capture.start("prev");
    await Promise.resolve(); // engagement reached the backend and is pending
    capture.cancel();
    release();
    await starting;
    await Promise.resolve();
    expect(calls).toEqual(["capture:true", "capture:false"]);
    expect(capture.recording).toBeNull();
    expect(capture.active).toBe(false);
  });

  it("reports capture failures", async () => {
    const report = vi.fn();
    const capture = createHotkeyCapture({
      setCapture: vi.fn(async () => {
        throw new Error("plugin down");
      }),
      save: vi.fn(),
      report,
    });
    await capture.start("play_pause");
    expect(report).toHaveBeenCalled();
    expect(capture.recording).toBeNull();
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app mutations_are_serialized`, `npm --prefix ui test -- src/lib/hotkeyCapture.test.ts`
Expected: FAIL — `with_mutation_lock` not found; cannot resolve `./hotkeyCapture.svelte`.

- [ ] **Step 4: Implement the Rust lock and wrap both commands**

In `hotkeys.rs` (top-level, near `ACTIONS`):

```rust
/// Serializes hotkey config writes and re-registration across the Tauri
/// commands: a capture release can never interleave with a save's
/// registration, so the registration that runs last always reflects the
/// latest config (Track 10 follow-up; spec §6.3). Each command reads
/// `state.config()` inside the lock.
static MUTATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn with_mutation_lock<T>(run: impl FnOnce() -> T) -> T {
    let _guard = MUTATION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    run()
}
```

In `commands.rs`, wrap the whole bodies of `set_hotkey` (line 1977) and `set_hotkey_capture` (line 2006):

```rust
#[tauri::command(async)]
pub fn set_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    accelerator: String,
    replace: Option<String>,
) -> Result<(), String> {
    crate::hotkeys::with_mutation_lock(|| {
        // ...existing body unchanged...
    })
}

#[tauri::command(async)]
pub fn set_hotkey_capture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    active: bool,
) -> Result<(), String> {
    crate::hotkeys::with_mutation_lock(|| {
        let config = state.config();
        crate::hotkeys::set_capture(&app, &config.hotkeys, active)
    })
}
```

- [ ] **Step 5: Implement the UI controller**

Create `ui/src/lib/hotkeyCapture.svelte.ts`. All backend calls are chained on one internal queue, so capture teardown always completes before a save and a superseded engagement always disengages:

```ts
import type { HotkeyAction } from "./api";

/// Coordinates the recorder with the backend: capture teardown is awaited
/// strictly before a save invoke, so re-registration cannot race, and the
/// keydown listener is gated on `active` (which flips only after capture
/// engagement resolves) so a fast first press cannot double-dispatch.
export interface HotkeyCaptureOptions {
  setCapture(active: boolean): Promise<void>;
  save(action: HotkeyAction, accelerator: string, replace: HotkeyAction | null): Promise<void>;
  report(error: unknown): void;
}

export interface HotkeyCapture {
  readonly recording: HotkeyAction | null;
  readonly active: boolean;
  start(action: HotkeyAction): Promise<void>;
  cancel(): void;
  finish(
    action: HotkeyAction,
    accelerator: string,
    replace?: HotkeyAction | null,
  ): Promise<void>;
}

export function createHotkeyCapture(options: HotkeyCaptureOptions): HotkeyCapture {
  let recording = $state<HotkeyAction | null>(null);
  let active = $state(false);
  // Every backend mutation runs on this chain, in the order requested.
  let chain: Promise<void> = Promise.resolve();
  // Bumped by any newer operation; a queued engagement that sees a stale
  // token never claims `active` and never fights the newer operation.
  let generation = 0;
  let engaged = false;

  function enqueue(run: () => Promise<void>): Promise<void> {
    chain = chain.then(run, run);
    return chain;
  }

  async function disengage() {
    if (!engaged) return;
    engaged = false;
    try {
      await options.setCapture(false);
    } catch (error) {
      options.report(error);
    }
  }

  async function start(action: HotkeyAction) {
    recording = action;
    active = false;
    const token = ++generation;
    await enqueue(async () => {
      if (token !== generation) return;
      try {
        await options.setCapture(true);
      } catch (error) {
        if (token === generation) {
          recording = null;
          active = false;
        }
        options.report(error);
        return;
      }
      engaged = true;
      if (token === generation && recording === action) active = true;
    });
  }

  function cancel() {
    if (recording === null && !active) return;
    recording = null;
    active = false;
    generation += 1;
    void enqueue(disengage);
  }

  async function finish(
    action: HotkeyAction,
    accelerator: string,
    replace: HotkeyAction | null = null,
  ) {
    recording = null;
    active = false;
    generation += 1;
    await enqueue(disengage);
    await options.save(action, accelerator, replace);
  }

  return {
    get recording() {
      return recording;
    },
    get active() {
      return active;
    },
    start,
    cancel,
    finish,
  };
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app mutations_are_serialized`, `npm --prefix ui test -- src/lib/hotkeyCapture.test.ts`, then `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: all green.

- [ ] **Step 7: Integrate into Settings**

In `Settings.svelte`:

- Import `{ createHotkeyCapture } from "../lib/hotkeyCapture.svelte";` and delete `let recordingAction = $state<HotkeyAction | null>(null);` (line 68); replace it with:

```ts
  const capture = createHotkeyCapture({
    setCapture: (active) => api.setHotkeyCapture(active),
    save: (action, accelerator, replace) => api.setHotkey(action, accelerator, replace),
    report: apply,
  });
```

- Replace the capture effect (lines 194–208) with an `active`-gated listener (attaching only after the backend suspended global delivery):

```ts
  // While capture is active, a capture-phase listener owns the keyboard; the
  // controller attaches it only after the backend suspended global hotkeys.
  // Cleanup restores both (Escape, save, cancel, unmount).
  $effect(() => {
    if (!capture.active || capture.recording === null) return;
    const action = capture.recording;
    const handler = (event: KeyboardEvent) => onHotkeyKeydown(event);
    window.addEventListener("keydown", handler, true);
    document.getElementById(`hotkey-${action}`)?.focus();
    return () => {
      window.removeEventListener("keydown", handler, true);
    };
  });
```

- Tab effect (lines 211–213):

```ts
  $effect(() => {
    if (tab !== "system") capture.cancel();
  });
```

- Replace `onHotkeyKeydown` (lines 357–381):

```ts
  function onHotkeyKeydown(event: KeyboardEvent) {
    const action = capture.recording;
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      capture.cancel();
      return;
    }
    if (event.key === "Backspace" || event.key === "Delete") {
      void capture.finish(action, "");
      return;
    }
    const accelerator = acceleratorFromEvent(event);
    if (!accelerator) return;
    const other = config ? conflictFor(config.hotkeys, action, accelerator) : null;
    if (other) {
      capture.cancel();
      hotkeyConflict = { action, accelerator, other };
      return;
    }
    void capture.finish(action, accelerator);
  }
```

- In the recorder markup (~1036–1058), replace every `recordingAction` reference with `capture.recording` and the Record button handler with `onclick={() => void capture.start(entry.id)}`. `saveHotkey` and `confirmReassign` stay as-is (the Clear button and the conflict confirm both run without an active capture).

- [ ] **Step 8: Run the gates and commit**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: green. Manual: record a chord, save, immediately press it → it fires; record, Escape, press the previous chord → it fires; record and click another Record quickly → no stuck capture.

```bash
git add crates/oxitide-app/src/hotkeys.rs crates/oxitide-app/src/commands.rs ui/src/lib/hotkeyCapture.svelte.ts ui/src/lib/hotkeyCapture.test.ts ui/src/views/Settings.svelte
git commit -m "fix(hotkeys): serialize and order capture teardown before save"
```

---

### Task 6: Artist mixes action and per-section retries

**Files:**
- Modify: `ui/src/lib/menus.ts` (add `artistMoreItems`)
- Test: `ui/src/lib/menus.test.ts`
- Modify: `ui/src/views/Artist.svelte` (imports, state lines 48–59, `load()` lines 61–111, `openMenu` lines 139–143, shelves lines 190–278)

**Interfaces:**
- Produces: `artistMoreItems({ share, mixId, openMix })`; per-section error/retry rows on the artist page.
- Consumes: `artist.mixes.ARTIST_MIX` (`models.rs:86`), `nav.goto({name:"mix"})`, `ErrorState`.

- [ ] **Step 1: Write the failing menu test**

In `menus.test.ts`, add `artistMoreItems` to the existing `./menus` import block (line 4) and append:

```ts
  it("artistMoreItems adds the mix action only when a mix exists", () => {
    const openMix = vi.fn();
    const without = artistMoreItems({ share: vi.fn(), mixId: null, openMix });
    expect(without.map((item) => item.id)).toEqual(["share"]);

    const withMix = artistMoreItems({ share: vi.fn(), mixId: "m-1", openMix });
    expect(withMix.map((item) => item.id)).toEqual(["share", "artist-mix"]);
    withMix[1].run();
    expect(openMix).toHaveBeenCalledWith("m-1");
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — `artistMoreItems` not exported.

- [ ] **Step 3: Implement the menu builder**

In `menus.ts` (after `queueMenuItems`):

```ts
/// The artist-page overflow ("More") menu: Share always, the artist mix when
/// the parsed v1 payload exposes one (spec §6.3).
export function artistMoreItems(ctx: {
  share(): void;
  mixId?: string | null;
  openMix(id: string): void;
}): MenuItem[] {
  const items: MenuItem[] = [{ id: "share", label: "Share", run: () => ctx.share() }];
  if (ctx.mixId) {
    const mixId = ctx.mixId;
    items.push({ id: "artist-mix", label: "Artist mix", run: () => ctx.openMix(mixId) });
  }
  return items;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire the menu and the section retries**

In `Artist.svelte`:

- Import `artistMoreItems` from `../lib/menus` and `errorMessage` from `../lib/errors`.
- Add the section-error state and loaders (below `loadToken`):

```ts
  type SectionKey = "tracks" | "albums" | "eps" | "comps" | "similar";
  let sectionErrors = $state<Partial<Record<SectionKey, string>>>({});

  function setSectionError(key: SectionKey, error: unknown) {
    sectionErrors = { ...sectionErrors, [key]: errorMessage(error) };
  }

  function clearSectionError(key: SectionKey) {
    if (sectionErrors[key] === undefined) return;
    const next = { ...sectionErrors };
    delete next[key];
    sectionErrors = next;
  }

  function loadSection<T>(key: SectionKey, run: () => Promise<T>, assign: (value: T) => void) {
    const token = loadToken;
    run()
      .then((value) => {
        if (token !== loadToken) return;
        clearSectionError(key);
        assign(value);
      })
      .catch((error) => {
        if (token !== loadToken) return;
        setSectionError(key, error);
      });
  }

  function retrySection(key: SectionKey) {
    switch (key) {
      case "tracks":
        loadSection("tracks", () => api.getArtistTopTracks(id, 20), (list) => (topTracks = list));
        break;
      case "albums":
        loadSection("albums", () => api.getArtistAlbums(id, "albums", 50, 0), (list) => (albums = list));
        break;
      case "eps":
        loadSection(
          "eps",
          () => api.getArtistAlbums(id, "eps_and_singles", 50, 0),
          (list) => (eps = list),
        );
        break;
      case "comps":
        loadSection(
          "comps",
          () => api.getArtistAlbums(id, "compilations", 50, 0),
          (list) => (comps = list),
        );
        break;
      case "similar":
        loadSection("similar", () => api.getSimilarArtists(id), (list) => (similar = list));
        break;
    }
  }
```

- In `load()`, reset `sectionErrors = {};` next to the other resets, and replace the four `void api...catch(() => {})` blocks (lines 87–110) with:

```ts
    loadSection("tracks", () => api.getArtistTopTracks(id, 20), (list) => (topTracks = list));
    loadSection("albums", () => api.getArtistAlbums(id, "albums", 50, 0), (list) => (albums = list));
    loadSection("eps", () => api.getArtistAlbums(id, "eps_and_singles", 50, 0), (list) => (eps = list));
    loadSection("comps", () => api.getArtistAlbums(id, "compilations", 50, 0), (list) => (comps = list));
    loadSection("similar", () => api.getSimilarArtists(id), (list) => (similar = list));
```

- `openMenu` (lines 139–143):

```ts
  function openMenu(event: MouseEvent) {
    menu.show(
      event.clientX,
      event.clientY,
      artistMoreItems({
        share: () => void share(),
        mixId: artist?.mixes?.ARTIST_MIX ?? null,
        openMix: (mixId) => nav.goto({ name: "mix", id: mixId }),
      }),
    );
  }
```

- Each shelf gains an `{:else if}` error arm. For the four carousels and the top-tracks table, e.g.:

```svelte
  {#if topTracks.length > 0}
    <section class="shelf">
      ...existing top-tracks block...
    </section>
  {:else if sectionErrors.tracks}
    <section class="shelf">
      <div class="shelf-head"><h3>Top tracks</h3></div>
      <ErrorState message={sectionErrors.tracks} onRetry={() => retrySection("tracks")} />
    </section>
  {/if}
```

Repeat the same shape for `albums` / `Singles & EPs` (key `eps`) / `Appears on` (key `comps`) / `similar` (`Similar artists`), each wrapping its existing block with the new `{:else if sectionErrors.<key>}` arm.

- [ ] **Step 6: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: green. Manual: an offline/slow artist shelf shows a retry row instead of silently vanishing; the More menu shows "Artist mix" when the artist has one and navigates to the mix page.

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/views/Artist.svelte
git commit -m "feat(ui): artist mix action and section retries"
```

---

### Task 7: Bio links (structured segments)

**Files:**
- Modify: `crates/oxitide-tidal/src/models.rs` (`ArtistProfile` lines 99–109; add `BioSegment`; tests)
- Modify: `crates/oxitide-tidal/src/catalog.rs` (`artist_profile` lines 99–149; `bio_segments` + helpers near `clean_bio`; tests)
- Modify: `ui/src/lib/api.ts` (`ArtistProfile` line 29)
- Create: `ui/src/lib/components/BioText.svelte`
- Modify: `ui/src/lib/components/ArtistHero.svelte` (props lines 12–28, bio lines 124–131, modal lines 149–157, styles)
- Modify: `ui/src/views/Artist.svelte` (ArtistHero props line 164)

**Interfaces:**
- Produces: `BioSegment { text, artist_id? }`, `ArtistProfile.bio_segments`, `bio_segments(raw)`; `BioText { segments, onArtist? }`; `ArtistHero` prop `onOpenArtist?`.
- Consumes: Task 0's bio decision; `strip_line_breaks` / `strip_tags` (`catalog.rs:195/221`), `clean_bio` (`catalog.rs:155`).

- [ ] **Step 1: Write the failing parser tests**

In `catalog.rs` `mod tests` (next to the existing `clean_bio` tests):

```rust
    #[test]
    fn bio_segments_plain_text_is_one_run() {
        let segments = bio_segments("Just a bio.");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Just a bio.");
        assert_eq!(segments[0].artist_id, None);
    }

    #[test]
    fn bio_segments_capture_artist_links() {
        let segments =
            bio_segments("See [wimpLink artistId=\"13\"]Band[/wimpLink]. <br/>Then more.");
        let texts: Vec<&str> = segments.iter().map(|segment| segment.text.as_str()).collect();
        assert_eq!(texts, vec!["See ", "Band", ". \n\nThen more."]);
        assert_eq!(segments[1].artist_id, Some(13));
        assert_eq!(segments[0].artist_id, None);
    }

    #[test]
    fn bio_segments_without_artist_id_stay_text() {
        let segments = bio_segments("[wimpLink foo=\"bar\"]Label[/wimpLink]");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Label");
        assert_eq!(segments[0].artist_id, None);
    }

    #[test]
    fn bio_segments_concat_matches_clean_bio() {
        let raw = "One [wimpLink artistId=\"1\"]two[/wimpLink]<br/>three";
        let concatenated: String = bio_segments(raw).iter().map(|s| s.text.clone()).collect();
        let joined = concatenated.replace("\n\n", " ").replace('\n', " ");
        assert_eq!(joined.trim(), "One two three");
        assert_eq!(clean_bio(raw).as_deref(), Some("One two\n\nthree"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal bio_segments`
Expected: FAIL — `bio_segments` not found.

- [ ] **Step 3: Implement segments**

In `models.rs`, add `BioSegment` and the `ArtistProfile` field:

```rust
/// One bio text run; `artist_id` marks a `[wimpLink artistId="…"]` link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BioSegment {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<u64>,
}
```

```rust
pub struct ArtistProfile {
    #[serde(default)]
    pub number_of_followers: Option<u64>,
    #[serde(default)]
    pub external_links: Vec<ArtistLink>,
    #[serde(default)]
    pub bio: Option<String>,
    /// In-text bio runs with `[wimpLink artistId]` navigation (spec §6.3).
    #[serde(default)]
    pub bio_segments: Vec<BioSegment>,
}
```

(Add `BioSegment` to the `Default` derive usage — `ArtistProfile` already derives `Default`, so the `Vec` defaults automatically.)

In `catalog.rs`, import `BioSegment`, replace the bio extraction (lines 136–142) with:

```rust
        let biography = raw
            .included
            .iter()
            .find(|entry| entry.resource_type == "artistBiographies")
            .and_then(|entry| entry.attributes.as_ref())
            .and_then(|attributes| attributes.text.as_deref());
        let bio = biography.and_then(clean_bio);
        let bio_segments = biography.map(bio_segments).unwrap_or_default();
```

and add to the returned `ArtistProfile { ... }` literal: `bio_segments,`.

Add the parser next to `clean_bio` (~line 155):

```rust
/// In-text bio segments: plain text runs plus `[wimpLink artistId="N"]`
/// links. Fragments are cleaned like `clean_bio`; paragraphs join with
/// `\n\n`, so concatenating the texts reproduces the clean bio.
pub fn bio_segments(raw: &str) -> Vec<BioSegment> {
    let mut segments: Vec<BioSegment> = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find("[wimpLink") {
        push_bio_text(&mut segments, clean_fragment(&rest[..start]));
        let Some(open_end) = rest[start..].find(']') else {
            return segments;
        };
        let open_tag = &rest[start..start + open_end + 1];
        let after_open = &rest[start + open_end + 1..];
        let Some(close) = after_open.find("[/wimpLink]") else {
            push_bio_text(&mut segments, clean_fragment(after_open));
            return segments;
        };
        push_bio_segment(
            &mut segments,
            clean_fragment(&after_open[..close]),
            parse_wimp_artist_id(open_tag),
        );
        rest = &after_open[close + "[/wimpLink]".len()..];
    }
    push_bio_text(&mut segments, clean_fragment(rest));
    segments
}

fn clean_fragment(raw: &str) -> String {
    strip_tags(&strip_line_breaks(raw))
}

/// Append a fragment's non-empty, trimmed lines; all but the first open a new
/// paragraph. Consecutive runs for the same target merge.
fn push_bio_text(segments: &mut Vec<BioSegment>, raw_text: String) {
    for (index, paragraph) in raw_text
        .split('\n')
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .enumerate()
    {
        let text = if index == 0 {
            paragraph.to_string()
        } else {
            format!("\n\n{paragraph}")
        };
        push_bio_segment(segments, text, None);
    }
}

fn push_bio_segment(segments: &mut Vec<BioSegment>, text: String, artist_id: Option<u64>) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = segments.last_mut() {
        if last.artist_id == artist_id {
            last.text.push_str(&text);
            return;
        }
    }
    segments.push(BioSegment { text, artist_id });
}

/// `[wimpLink artistId="13" …]` -> `Some(13)`; absent or malformed ids fall
/// back to plain text.
fn parse_wimp_artist_id(open_tag: &str) -> Option<u64> {
    let start = open_tag.find("artistId=\"")? + "artistId=\"".len();
    let rest = &open_tag[start..];
    let end = rest.find('"')?;
    rest[..end].parse::<u64>().ok()
}
```

Extend the existing `artist_profile_reads_followers_links_and_bio` test with:

```rust
        assert_eq!(profile.bio_segments.len(), 2);
        assert_eq!(profile.bio_segments[0].text, "See ");
        assert_eq!(profile.bio_segments[1].text, "Band");
        assert_eq!(profile.bio_segments[1].artist_id, Some(1));
```

(The existing fixture's bio is `See [wimpLink artistId="1"]Band[/wimpLink].<br/>More.`; add the assertions after `assert_eq!(profile.bio.as_deref(), Some("See Band.\n\nMore."));`.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal bio`, `cargo test -p oxitide-tidal artist_profile`, then `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: all green.

- [ ] **Step 5: Render segments in the UI**

In `api.ts`, extend `ArtistProfile`:

```ts
export interface BioSegment { text: string; artistId?: number | null }
export interface ArtistProfile {
  numberOfFollowers?: number | null;
  externalLinks: ArtistLink[];
  bio?: string | null;
  bioSegments?: BioSegment[] | null;
}
```

Create `ui/src/lib/components/BioText.svelte`:

```svelte
<script lang="ts">
  import type { BioSegment } from "../api";

  let {
    segments,
    onArtist,
  }: { segments: BioSegment[]; onArtist?: (id: number) => void } = $props();
</script>

{#each segments as segment, i (i)}
  {#if segment.artistId !== null && segment.artistId !== undefined && onArtist}
    <button class="bio-link" onclick={() => onArtist(segment.artistId!)}>{segment.text}</button>
  {:else}
    <span>{segment.text}</span>
  {/if}
{/each}

<style>
  .bio-link {
    font: inherit;
    color: var(--accent);
    background: none;
    border: 0;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
</style>
```

In `ArtistHero.svelte`:

- Add the optional prop the same way as the others: `onOpenArtist?: (id: number) => void;`.
- Import `BioText`.
- Replace `const bio = $derived(profile?.bio ?? null);` with `const bioSegments = $derived(profile?.bioSegments ?? []);` (keep `bio` only if the modal still needs it; switch both renders to segments).
- Replace the hero bio block (lines 124–131):

```svelte
      {#if bioSegments.length > 0}
        <p class="bio" bind:this={bioEl}><BioText segments={bioSegments} onArtist={onOpenArtist} /></p>
        {#if canExpand}
          <button class="more" aria-haspopup="dialog" onclick={() => (bioModal = true)}>
            Read more
          </button>
        {/if}
      {/if}
```

- Replace the modal body (lines 149–157):

```svelte
  {#if bioModal && bioSegments.length > 0}
    <Modal title={`About ${artist.name}`} onclose={() => (bioModal = false)}>
      <div class="bio-full">
        <BioText
          segments={bioSegments}
          onArtist={(artistId) => {
            bioModal = false;
            onOpenArtist?.(artistId);
          }}
        />
      </div>
    </Modal>
  {/if}
```

In `Artist.svelte`, pass the handler at the `ArtistHero` call site (line 164):

```svelte
    onOpenArtist={(artistId) => nav.goto({ name: "artist", id: artistId })}
```

- [ ] **Step 6: Run the gates and commit**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: green. Manual: an artist bio with links renders clickable names that navigate; the modal still opens and links work there.

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/catalog.rs ui/src/lib/api.ts ui/src/lib/components/BioText.svelte ui/src/lib/components/ArtistHero.svelte ui/src/views/Artist.svelte
git commit -m "feat(ui): navigate artist bio links"
```

---

### Task 8: Playlist sort honesty and Purchases cards

**Files:**
- Modify: `ui/src/lib/collectionSort.ts` (only if Task 0 found dates; otherwise skip the logic change)
- Test: `ui/src/lib/collectionSort.test.ts`
- Modify: `ui/src/views/Playlists.svelte` (toggle title line 88; lines 85–92)
- Modify: `ui/src/views/Purchases.svelte` (imports, `routeFor` lines 32–37, markup lines 48–61, styles)

**Interfaces:**
- Produces: real added-date sorting **or** honest "Service order" copy; Purchases rendered with `MediaCard` + `routeForPageItem`.
- Consumes: Task 0's playlist-date decision; `parseTidalTime` (`playlistSort.ts:7`), `routeForPageItem` (`nav.svelte.ts:79`), `MediaCard`.

- [ ] **Step 1: Branch on the Task 0 decision**

Open the research note. **Dates present:** do Step 2 and update the `date` test. **Dates absent:** skip Step 2 and only relabel in Step 3. Record which branch ran in the note's decision line.

- [ ] **Step 2: Implement real date sorting**

In `collectionSort.ts`:

```ts
import type { CollectionEntry } from "./api";
import { parseTidalTime } from "./playlistSort";

export type CollectionSort = "date" | "name";

/// "date" sorts playlists by their most recent addition (falling back to
/// creation/update), newest first, then keeps the service order for entries
/// without dates. "name" groups folders first, then playlists, each
/// alphabetically and case-insensitively.
export function sortEntries(
  entries: readonly CollectionEntry[],
  mode: CollectionSort,
): CollectionEntry[] {
  if (mode === "name") {
    const nameOf = (entry: CollectionEntry) => (entry.kind === "folder" ? entry.name : entry.title);
    const byName = (a: CollectionEntry, b: CollectionEntry) =>
      nameOf(a).localeCompare(nameOf(b), undefined, { sensitivity: "base" });
    const folders = entries.filter((entry) => entry.kind === "folder").sort(byName);
    const playlists = entries.filter((entry) => entry.kind === "playlist").sort(byName);
    return [...folders, ...playlists];
  }
  const playlists = entries.filter(
    (entry): entry is Extract<CollectionEntry, { kind: "playlist" }> => entry.kind === "playlist",
  );
  const recency = (entry: Extract<CollectionEntry, { kind: "playlist" }>) =>
    parseTidalTime(entry.lastItemAddedAt ?? entry.created ?? entry.lastUpdated);
  playlists.sort((a, b) => recency(b) - recency(a));
  return [...playlists];
}
```

Update `collectionSort.test.ts`'s date case to build playlists with `lastItemAddedAt` values and assert newest-first.

- [ ] **Step 3: Relabel (or keep) the toggle**

In `Playlists.svelte` line 88: if dates are absent, change the title expression to `sortMode === "name" ? "Service order" : "Sort by name"` and add a comment noting the Plan 13 deviation is resolved by relabeling (spec §6.3). If dates are present, leave the copy as "Sort by date added".

- [ ] **Step 4: Purchases adopts `MediaCard`**

Replace `Purchases.svelte`'s imports and markup:

```ts
  import { api, type PageItem } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import { routeForPageItem, type Nav } from "../lib/nav.svelte";
```

Delete the local `routeFor`. Replace the list markup (lines 48–61):

```svelte
  <div class="purchases">
    {#each items as item (item.id || item.title)}
      {@const route = routeForPageItem(item)}
      <MediaCard
        item={item}
        variant="row"
        onclick={route ? () => nav.goto(route) : undefined}
      />
    {/each}
  </div>
```

Styles:

```css
  .purchases {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 8px 20px;
  }
```

- [ ] **Step 5: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: green. Manual: the playlists toggle's copy matches behavior; Purchases shows the same row cards as the rest of the app and navigates album items.

```bash
git add ui/src/lib/collectionSort.ts ui/src/lib/collectionSort.test.ts ui/src/views/Playlists.svelte ui/src/views/Purchases.svelte
git commit -m "fix(ui): honest playlist sort and purchase card language"
```

---

### Task 9: Scrobbled indicator

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/mod.rs` (sink types lines 30–31, struct lines 109–120, setters line 160, `flush_lastfm` lines 407–415, `flush_listenbrainz` lines 469–475; test)
- Modify: `crates/oxitide-app/src/main.rs` (sinks lines 252–261)
- Modify: `ui/src/lib/api.ts` (event constant line 242)
- Create: `ui/src/lib/scrobbled.svelte.ts`
- Test: `ui/src/lib/scrobbled.test.ts`
- Modify: `ui/src/App.svelte` (listener effects ~line 175)
- Modify: `ui/src/views/NowPlaying.svelte` (imports, effect near line 154, badge in `details` line 163–167)

**Interfaces:**
- Produces: `ScrobbledSink`, `ScrobblerService::set_scrobbled_sink`, event `scrobble://scrobbled` (payload `track_id: number`), `scrobbled` store, Now Playing badge.
- Consumes: `StatusSink` pattern (`mod.rs:160`), `tauri::Emitter` wiring (`main.rs:254`), `player.state.current`.

- [ ] **Step 1: Write the failing Rust test**

In `scrobble/mod.rs` `mod tests` (mirror `a_successful_flush_drains_the_queue` at line 758):

```rust
    #[tokio::test]
    async fn a_successful_flush_reports_scrobbled_track_ids() {
        use wiremock::matchers::{method as http_method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(http_method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "scrobbles": {} })),
            )
            .mount(&server)
            .await;

        let store = Arc::new(MemoryCredentialStore::default());
        store
            .save(LASTFM_SERVICE, &StoredCredential { username: "v".into(), secret: "sk".into() })
            .unwrap();
        let service = ScrobblerService::new(
            reqwest::Client::new(),
            server.uri(),
            "http://127.0.0.1:1",
            store,
            "key".into(),
            "secret".into(),
            true,
            false,
        );
        let reported: Arc<std::sync::Mutex<Vec<u64>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = reported.clone();
        service.set_scrobbled_sink(Arc::new(move |track_id| {
            sink.lock().unwrap().push(track_id);
        }));
        service
            .handle_intent(ScrobbleIntent::Scrobble { item: queue_item(), started_at: 1 })
            .await;
        service.flush_queues().await;
        assert_eq!(*reported.lock().unwrap(), vec![7]);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app a_successful_flush_reports_scrobbled_track_ids`
Expected: FAIL — `set_scrobbled_sink` not found.

- [ ] **Step 3: Implement the sink and emissions**

In `scrobble/mod.rs`:

```rust
pub type ScrobbledSink = Arc<dyn Fn(u64) + Send + Sync>;
```

Add the field to `ScrobblerService` (`on_scrobbled: Mutex<Option<ScrobbledSink>>`), initialize it in `new` (`on_scrobbled: Mutex::new(None)`), and add:

```rust
    pub fn set_scrobbled_sink(&self, sink: ScrobbledSink) {
        *self.on_scrobbled.lock().unwrap() = Some(sink);
    }

    /// One service accepted a scrobble for this play; the UI shows the badge
    /// while the track is still current.
    fn emit_scrobbled(&self, track_id: u64) {
        if let Some(sink) = self.on_scrobbled.lock().unwrap().as_ref() {
            sink(track_id);
        }
    }
```

In `flush_lastfm`'s `Ok(())` branch, after `self.emit_status();`:

```rust
                for entry in &batch {
                    self.emit_scrobbled(entry.item.track_id);
                }
```

In `flush_listenbrainz`'s `Ok(())` branch, right after `sent += 1;`:

```rust
                    self.emit_scrobbled(entry.item.track_id);
```

In `main.rs`, next to the status/auth-error sinks (clone the handle per sink, matching lines 253–259):

```rust
                let handle = app.handle().clone();
                scrobbler.set_scrobbled_sink(std::sync::Arc::new(move |track_id| {
                    let _ = tauri::Emitter::emit(&handle, "scrobble://scrobbled", track_id);
                }));
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-app a_successful_flush_reports_scrobbled_track_ids`, then `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: green.

- [ ] **Step 5: Write the failing UI store test**

Create `ui/src/lib/scrobbled.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createScrobbled } from "./scrobbled.svelte";

describe("scrobbled badge", () => {
  it("marks a track and clears", () => {
    const badge = createScrobbled();
    expect(badge.trackId).toBeNull();
    badge.mark(7);
    expect(badge.trackId).toBe(7);
    badge.clear();
    expect(badge.trackId).toBeNull();
  });
});
```

- [ ] **Step 6: Implement the store, listener, and badge**

Create `ui/src/lib/scrobbled.svelte.ts`:

```ts
export interface ScrobbledBadge {
  readonly trackId: number | null;
  mark(trackId: number): void;
  clear(): void;
}

export function createScrobbled(): ScrobbledBadge {
  let trackId = $state<number | null>(null);
  return {
    get trackId() {
      return trackId;
    },
    mark: (id) => {
      trackId = id;
    },
    clear: () => {
      trackId = null;
    },
  };
}

/// One app-wide badge: set by the scrobble event, cleared when the current
/// queue item changes (Now Playing owns the clear).
export const scrobbled = createScrobbled();
```

In `api.ts` constants (line 242):

```ts
export const SCROBBLE_SCROBBLED_EVENT = "scrobble://scrobbled";
```

In `App.svelte`, next to the scrobble auth-error effect (~line 175), import the store/constant and add:

```ts
  $effect(() => {
    const unlisten = listen<number>(SCROBBLE_SCROBBLED_EVENT, (event) => {
      scrobbled.mark(event.payload);
    });
    return () => {
      unlisten.then((off) => off());
    };
  });
```

In `NowPlaying.svelte`, import `{ scrobbled } from "../lib/scrobbled.svelte";`, add near the lyrics effect:

```ts
  // The badge only describes the current play; a track change clears it.
  $effect(() => {
    if (track?.track_id !== scrobbled.trackId) scrobbled.clear();
  });
```

and add the badge inside `details` (after the `qualityDetail` badge, line 166):

```svelte
    {#if scrobbled.trackId === t.track_id}<span class="badge">Scrobbled</span>{/if}
```

- [ ] **Step 7: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `cargo test --workspace`.
Expected: green. Manual: with Last.fm connected, play past half a track → "Scrobbled" appears; skip to the next track → it disappears.

```bash
git add crates/oxitide-app/src/scrobble/mod.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts ui/src/lib/scrobbled.svelte.ts ui/src/lib/scrobbled.test.ts ui/src/App.svelte ui/src/views/NowPlaying.svelte
git commit -m "feat(scrobble): surface a scrobbled indicator"
```

---

### Task 10: Smoke checklist, gates, and PM-D1 perf

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-06-track9-3-smoke.md`

**Interfaces:**
- Consumes: every prior task.

- [ ] **Step 1: Run the full scripted gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
Expected: all green. Record the commands and results in the note header.

- [ ] **Step 2: Re-run the affected Track 3 scenarios under unchanged budgets (PM-D1)**

Run (Vite warm, single app, foreground; the runner starts Vite if needed):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 3
node scripts/perf/check.mjs
```

Expected: `check.mjs` exits 0 with unchanged budgets. If the machine cannot be quiet, mark the run BLOCKED in the note with an unblock instruction (9.1 note pattern).

- [ ] **Step 3: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-06-track9-3-smoke.md` (date = run day) following the 9.1 note format: status/authority (spec §6.3, §5.4), a "Scripted perf results" table (S5/S6/S7 values vs budgets), the PM-D1 impact statement (fast path removes per-item resolves on Play all; one batched flags call for visible cards), and owner checklist sections:

- **Feed cards:** with `block_ai`/`block_explicit` on, blocked cards gray out and line-through; the playing card highlights; clicking a track card plays instantly (network shows no per-track `tracks/{id}` calls); Play all on a track list starts without per-item resolves; a section whose payload lacks metadata still plays (fallback).
- **Settings rollback:** toggle Oneko / SMTC / Close-to-tray with an unwritable config (or by pointing the process at a read-only config dir) → the control and toast reflect the revert; normal toggles persist across restart.
- **Hotkeys:** record and save a chord, press it immediately → fires; record + Escape → previous chord intact; Clear → the chord stops firing; capture never leaves global shortcuts suspended.
- **Artist:** the More menu shows "Artist mix" only when a mix exists and navigates; a failed shelf shows a retry row that recovers; bio links navigate (or the "not observed" decision from Task 0 is recorded).
- **Cosmetics:** the playlists toggle copy matches its behavior; Purchases uses row cards; the scrobbled badge appears after a real scrobble and clears on track change.
- **Carry-in:** every 9.0/9.1 defect folded into this plan is re-verified or explicitly deferred in the note.
- Linux GUI items follow the Track 8 exemption pattern if not run.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-06-track9-3-smoke.md
git commit -m "docs: add the track 9.3 smoke checklist and perf results"
```

---

## Plan Self-Review

- **Spec coverage:** feed enrichment §5.4/§6.3 → Tasks 1–3; settings rollback → Task 4; hotkey race + regression tests (Rust re-registration serialization + UI ordering) → Task 5; artist mixes/retries → Task 6; bio links → Task 7; playlist sort + Purchases → Task 8; scrobbled indicator → Task 9; PM-D1 S5/S6/S7 + smoke → Task 10; carry-in triage (T9-D8/D9) → Task 0 Step 1. No spec item is unassigned.
- **Placeholders:** none; every step carries code, exact paths, or exact commands. The playlist task's two branches are resolved explicitly by Task 0.
- **Type consistency:** `PageTrackDto` (Task 2) matches the UI `PageTrackInput` and `pageTrackInput` mapping (Task 3); `filter_playable_items` is used by both `play_track_ids` and `play_page_items`; `createHotkeyCapture`'s API (Task 5) matches its tests and the Settings integration; `BioSegment` (Task 7) matches the TS interface and `BioText`; `SCROBBLE_SCROBBLED_EVENT` (Task 9) matches the Rust emit string.
- **Review Focus coverage:** (1) partial payloads → `PageTrackDto::embedded` tests + fallback test; (2) late/failed flags → existing store tests + fail-open AI test + smoke; (3) mixed sections/highlight → `feedTracks` order + id comparison by track-only items; (4) hotkey ordering → controller tests including in-flight engagement; (5) playlist dates → Task 0 branch + `sortEntries` tests; (6) wrong-track badge → Now Playing clear effect + store test.
- **Risk noted:** Task 6 and Task 5 integrate into `Settings.svelte`/`Artist.svelte` with many inline references; executors should re-grep for `recordingAction` and each shelf's `.catch(() => {})` rather than assume line numbers survived earlier tasks.
