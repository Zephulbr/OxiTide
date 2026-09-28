# Sidebar, Explore/Feed, and My Collection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the sidebar and pages to mirror the official TIDAL client: Home, Explore (absorbing search), Feed, an expandable My Collection section with dedicated Playlists/Albums/Tracks/Mixes/Videos/Profiles/Purchases pages, and a folder-aware quick-access playlist tree.

**Architecture:** Generalize the v1 `pages/*` parser in `oxitide-tidal` and add v2 collection-folder endpoints plus permissive video/mix/purchases fetchers, all behind new Tauri commands. On the UI side, extend the hand-rolled `Route` union, add a reactive collection-tree store and shared `PageSections` renderer, then rebuild the sidebar and add the new views.

**Tech Stack:** Rust (Tauri 2, tokio, reqwest, wiremock tests), Svelte 5 runes, TypeScript, vitest, svelte-check.

**Spec:** `docs/superpowers/specs/2026-09-28-oxitide-sidebar-collection-navigation-design.md`

## Global Constraints

- Windows-first; no new Cargo or npm dependencies.
- All HTTP flows through `TidalClient` (`get`, `get_all_pages`, new `get_v2`/`put_v2`); tests are `wiremock` only — never hit the live TIDAL API.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails.
- `invoke` sends camelCase args; Rust params stay snake_case (`redirect_url` ↔ `redirectUrl`).
- UI uses Svelte 5 runes only; no Svelte 4 stores or `export let`.
- Styles consume semantic tokens only (`--surface-*`, `--text`, `--accent`, …); icons are inlined Lucide paths in `Icon.svelte` — no unicode glyphs.
- Never log token values.
- Gates for every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test` (UI gates only where the task touches `ui/`).
- Video playback stays out of scope: video surfaces are inert with a visible note.
- Commit after each task; keep unrelated working-tree changes out of task commits.

## Review Focus

- A folder-list API response whose `cursor` repeats or never ends must stop the walk after 20 pages and keep what loaded, never loop forever (Task 5 test).
- Feed must degrade to an empty section list — not an error banner — when both candidate endpoints 404 (Task 2 test).
- A mix whose items are all videos must resolve to zero tracks and surface an informational toast, not an empty queue (Task 3 test, Task 19 toast).
- A root collection larger than one 50-item page must load completely via cursor paging (Task 5 multi-page test).
- A failed playlist/folder mutation (rename, remove, move) must leave the tree and page lists untouched and only toast the error (Task 13 tests).

---

## Phase A — Rust client

### Task 1: Generalize the page parser and rename the DTOs

**Files:**
- Rename: `crates/oxitide-tidal/src/home.rs` → `crates/oxitide-tidal/src/pages.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs:5`
- Modify: `crates/oxitide-app/src/commands.rs:363` (import)

**Interfaces:**
- Consumes: `TidalClient::get(path, params)` (`crates/oxitide-tidal/src/client.rs:87`).
- Produces: `pub struct PageItem { id: String, title: String, subtitle: Option<String>, item_type: String, artwork: Option<String> }`, `pub struct PageSection { title: String, items: Vec<PageItem> }`, `TidalClient::home_page()`, `TidalClient::explore_page()`, `TidalClient::mixes_page()` — all `-> Result<Vec<PageSection>>`. Serde field names unchanged (`item_type` stays snake_case on the wire).

- [ ] **Step 1: Rename the module**

```powershell
git mv crates/oxitide-tidal/src/home.rs crates/oxitide-tidal/src/pages.rs
```

- [ ] **Step 2: Update the module list**

In `crates/oxitide-tidal/src/lib.rs` replace `pub mod home;` with `pub mod pages;`.

- [ ] **Step 3: Rewrite `pages.rs`**

Replace the file body above the tests with (keep the existing `#[cfg(test)] mod tests` block, renamed types only):

```rust
use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default = "default_kind")]
    pub item_type: String,
    #[serde(default)]
    pub artwork: Option<String>,
}

fn default_kind() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<PageItem>,
}

#[derive(Debug, Deserialize)]
struct RawRow {
    #[serde(default)]
    pub modules: Vec<RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type", default)]
    pub module_type: String,
    #[serde(default)]
    pub items: Vec<RawItem>,
    #[serde(default, rename = "pagedList")]
    pub paged_list: Option<RawPagedList>,
}

#[derive(Debug, Deserialize)]
struct RawPagedList {
    #[serde(default)]
    pub items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub item_type: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default, rename = "squareImage")]
    pub square_image: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPage {
    #[serde(default)]
    pub rows: Vec<RawRow>,
}

fn page_params() -> Vec<(&'static str, String)> {
    vec![("deviceType", "BROWSER".into()), ("platform", "WEB".into())]
}

impl TidalClient {
    /// Parse any v1 page endpoint that uses the rows/modules format.
    async fn page_sections_at(
        &self,
        path: &str,
        extra: &[(&str, String)],
    ) -> Result<Vec<PageSection>> {
        let mut params = page_params();
        params.extend(extra.iter().cloned());
        let raw: RawPage = self.get(path, &params).await?;
        Ok(sections_from_page(raw))
    }

    /// Fetch the home feed. TIDAL requires `deviceType`/`platform` on this endpoint.
    pub async fn home_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/home", &[]).await
    }

    /// Fetch the explore page (same rows/modules shape as home).
    pub async fn explore_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/explore", &[]).await
    }

    /// The signed-in user's mixes & radio, as a page.
    pub async fn mixes_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/my_collection_my_mixes", &[]).await
    }
}

fn sections_from_page(raw: RawPage) -> Vec<PageSection> {
    let mut sections = Vec::new();
    for row in raw.rows {
        for module in row.modules {
            let mut raw_items = module.items;
            if let Some(paged) = module.paged_list {
                raw_items.extend(paged.items);
            }
            if raw_items.is_empty() {
                continue;
            }
            let module_type = module.module_type.clone();
            let items: Vec<PageItem> = raw_items
                .into_iter()
                .map(|i| PageItem {
                    id: item_id(&i),
                    title: i.title,
                    subtitle: i.subtitle.or(i.description),
                    item_type: if i.item_type.is_empty() {
                        module_type.clone()
                    } else {
                        i.item_type
                    },
                    artwork: i.cover.or(i.square_image).or(i.image).or(i.picture),
                })
                .collect();
            sections.push(PageSection { title: module.title, items });
        }
    }
    sections
}

/// TIDAL uses a numeric `id` for tracks/albums and a `uuid` for playlists.
fn item_id(item: &RawItem) -> String {
    let from_id = value_to_id(&item.id);
    if !from_id.is_empty() {
        return from_id;
    }
    item.uuid.clone().unwrap_or_default()
}

fn value_to_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
```

In the test module: replace `HomeItem`/`HomeSection` type annotations with `PageItem`/`PageSection` (the assertions stay identical), and add:

```rust
#[tokio::test]
async fn explore_page_reads_the_explore_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/explore"))
        .and(query_param("deviceType", "BROWSER"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rows": [{"modules": [{"type": "PLAYLIST_LIST", "title": "For You", "items": [
                {"uuid": "p-1", "title": "Mix"}
            ]}]}]
        })))
        .mount(&server).await;
    let sections = client(&server).explore_page().await.unwrap();
    assert_eq!(sections[0].title, "For You");
    assert_eq!(sections[0].items[0].id, "p-1");
}
```

- [ ] **Step 4: Update the command import**

In `crates/oxitide-app/src/commands.rs:363` change `use oxitide_tidal::home::HomeSection;` to `use oxitide_tidal::pages::PageSection;` and `get_home`'s return type to `Result<Vec<PageSection>, String>`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p oxitide-tidal pages && cargo test -p oxitide-app`
Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/oxitide-tidal/src/pages.rs crates/oxitide-tidal/src/lib.rs crates/oxitide-tidal/src/home.rs crates/oxitide-app/src/commands.rs
git commit -m "refactor(tidal): generalize the pages parser and rename home DTOs"
```

---

### Task 2: Feed endpoint probe chain

**Files:**
- Modify: `crates/oxitide-tidal/src/pages.rs`

**Interfaces:**
- Consumes: `page_sections_at` (Task 1), `Error::NotFound` (`error.rs`).
- Produces: `TidalClient::feed_page() -> Result<Vec<PageSection>>` (empty vec when both candidates 404; non-404 errors propagate).

- [ ] **Step 1: Write the failing tests**

Append to the test module in `pages.rs`:

```rust
#[tokio::test]
async fn feed_page_falls_back_when_the_first_candidate_is_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/feed"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/pages/my_collection_feed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rows": [{"modules": [{"type": "ALBUM_LIST", "title": "New Releases", "items": [
                {"id": 9, "title": "Album", "cover": "c-9"}
            ]}]}]
        })))
        .mount(&server).await;
    let sections = client(&server).feed_page().await.unwrap();
    assert_eq!(sections[0].title, "New Releases");
    assert_eq!(sections[0].items[0].item_type, "ALBUM_LIST");
}

#[tokio::test]
async fn feed_page_is_empty_when_every_candidate_is_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/feed"))
        .respond_with(ResponseTemplate::new(404)).mount(&server).await;
    Mock::given(method("GET")).and(path("/pages/my_collection_feed"))
        .respond_with(ResponseTemplate::new(404)).mount(&server).await;
    assert!(client(&server).feed_page().await.unwrap().is_empty());
}

#[tokio::test]
async fn feed_page_propagates_non_404_failures() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/feed"))
        .respond_with(ResponseTemplate::new(500)).mount(&server).await;
    assert!(client(&server).feed_page().await.is_err());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal feed_page`
Expected: FAIL — `feed_page` not found.

- [ ] **Step 3: Implement**

Add to the `impl TidalClient` block in `pages.rs`:

```rust
/// The user's feed. TIDAL's endpoint path is unconfirmed; candidates are
/// probed in order and a missing feed degrades to an empty list.
pub async fn feed_page(&self) -> Result<Vec<PageSection>> {
    for path in ["pages/feed", "pages/my_collection_feed"] {
        match self.page_sections_at(path, &[]).await {
            Ok(sections) => return Ok(sections),
            Err(crate::error::Error::NotFound(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(Vec::new())
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p oxitide-tidal feed_page`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-tidal/src/pages.rs
git commit -m "feat(tidal): probe feed endpoints with an empty fallback"
```

---

### Task 3: Mix detail parser

**Files:**
- Modify: `crates/oxitide-tidal/src/pages.rs`

**Interfaces:**
- Consumes: `TidalClient::get`, `models::Track`.
- Produces: `pub struct MixDetail { id: String, title: String, tracks: Vec<Track> }`, `TidalClient::mix_page(mix_id: &str) -> Result<MixDetail>`.

- [ ] **Step 1: Write the failing tests**

Append to the test module:

```rust
#[tokio::test]
async fn mix_page_keeps_tracks_and_drops_videos() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/mix"))
        .and(query_param("mixId", "mix-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rows": [
                {"modules": [{"type": "MIX_HEADER", "mix": {"id": "mix-7", "title": "Daily Mix"}}]},
                {"modules": [
                    {"type": "TRACK_LIST", "title": "Tracks", "pagedList": {"items": [
                        {"id": 1, "title": "Song", "duration": 200, "artists": [{"id": 5, "name": "A"}]}
                    ]}},
                    {"type": "VIDEO_LIST", "title": "Videos", "items": [
                        {"id": 99, "title": "Clip", "type": "VIDEO"}
                    ]}
                ]}
            ]
        })))
        .mount(&server).await;
    let detail = client(&server).mix_page("mix-7").await.unwrap();
    assert_eq!(detail.title, "Daily Mix");
    assert_eq!(detail.tracks.len(), 1);
    assert_eq!(detail.tracks[0].id, 1);
    assert_eq!(detail.tracks[0].duration, 200);
}

#[tokio::test]
async fn mix_page_with_only_videos_has_no_tracks() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/pages/mix"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rows": [
                {"modules": [{"type": "MIX_HEADER", "mix": {"id": "m", "title": "Video Mix"}}]},
                {"modules": [{"type": "VIDEO_LIST", "items": [{"id": 4, "title": "Clip"}]}]}
            ]
        })))
        .mount(&server).await;
    let detail = client(&server).mix_page("m").await.unwrap();
    assert!(detail.tracks.is_empty());
    assert_eq!(detail.title, "Video Mix");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal mix_page`
Expected: FAIL — `mix_page` not found.

- [ ] **Step 3: Implement**

Add to `pages.rs` (top-level types near `PageSection`, methods inside `impl TidalClient`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixDetail {
    pub id: String,
    pub title: String,
    pub tracks: Vec<crate::models::Track>,
}
```

```rust
/// Resolve a mix id into its playable track list. Videos are dropped because
/// the engine is audio-only; `pages/mix` returns tracks and videos together.
pub async fn mix_page(&self, mix_id: &str) -> Result<MixDetail> {
    let params = vec![
        ("mixId", mix_id.to_string()),
        ("deviceType", "BROWSER".to_string()),
        ("platform", "WEB".to_string()),
    ];
    let raw: serde_json::Value = self.get("pages/mix", &params).await?;
    Ok(mix_detail_from_json(mix_id, &raw))
}
```

```rust
fn mix_detail_from_json(mix_id: &str, raw: &serde_json::Value) -> MixDetail {
    let title = raw
        .pointer("/rows/0/modules/0/mix/title")
        .or_else(|| raw.pointer("/rows/0/modules/0/title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut tracks = Vec::new();
    if let Some(rows) = raw.get("rows").and_then(serde_json::Value::as_array) {
        for row in rows {
            let Some(modules) = row.get("modules").and_then(serde_json::Value::as_array) else {
                continue;
            };
            for module in modules {
                let module_type = module
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let module_is_track = module_type.to_uppercase().contains("TRACK");
                let mut items: Vec<&serde_json::Value> = module
                    .get("items")
                    .and_then(serde_json::Value::as_array)
                    .map(|values| values.iter().collect())
                    .unwrap_or_default();
                if let Some(paged) = module
                    .pointer("/pagedList/items")
                    .and_then(serde_json::Value::as_array)
                {
                    items.extend(paged.iter());
                }
                for item in items {
                    let item_type = item
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let is_track = if item_type.is_empty() {
                        module_is_track
                    } else {
                        item_type.to_uppercase().contains("TRACK")
                    };
                    if !is_track {
                        continue;
                    }
                    if let Ok(track) =
                        serde_json::from_value::<crate::models::Track>(item.clone())
                    {
                        tracks.push(track);
                    }
                }
            }
        }
    }
    MixDetail { id: mix_id.to_string(), title, tracks }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p oxitide-tidal mix_page`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-tidal/src/pages.rs
git commit -m "feat(tidal): resolve mix pages into playable tracks"
```

---

### Task 4: v2 GET helper, Video/Mix DTOs, favorite videos and mixes

**Files:**
- Modify: `crates/oxitide-tidal/src/client.rs`
- Modify: `crates/oxitide-tidal/src/models.rs`
- Modify: `crates/oxitide-tidal/src/library.rs`

**Interfaces:**
- Consumes: `TidalClient::send_get` internals, `Favorited<T>` envelope.
- Produces:
  - `TidalClient::get_v2<T: DeserializeOwned>(path, params) -> Result<T>` (no `countryCode` query; refresh-on-401 identical to `get`).
  - `models::Video { id: u64, title: String, duration: u32, artists: Vec<ArtistRef>, image: Option<String> }`.
  - `models::Mix { id: String, title: String, sub_title: Option<String>, mix_type: Option<String>, image: Option<String> }` (serializes as `subTitle`, `mixType`).
  - `TidalClient::all_favorite_videos() -> Result<Vec<Video>>`.
  - `TidalClient::favorite_mixes(limit, offset) -> Result<(Vec<Mix>, u32)>`.

- [ ] **Step 1: Write the failing client test**

Append to `client.rs`'s test module:

```rust
#[tokio::test]
async fn get_v2_targets_v2_without_country_code() {
    let server = MockServer::start().await;
    let responder = |req: &wiremock::Request| {
        let has_country = req.url.query_pairs().any(|(key, _)| key == "countryCode");
        if has_country {
            ResponseTemplate::new(500)
        } else {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 8}))
        }
    };
    Mock::given(method("GET")).and(path("/v2/things"))
        .and(query_param("limit", "50"))
        .respond_with(responder)
        .mount(&server).await;
    let client = TidalClient::new(
        reqwest::Client::new(),
        server.uri(),
        format!("{}/token", server.uri()),
        session(),
        tokens(),
        Arc::new(MemoryStore::default()),
    );
    let thing: Thing = client
        .get_v2("things", &[("limit", "50".to_string())])
        .await
        .unwrap();
    assert_eq!(thing.value, 8);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p oxitide-tidal get_v2_targets`
Expected: FAIL — `get_v2` not found.

- [ ] **Step 3: Implement `get_v2`**

In `client.rs`, refactor `send_get` to take a base URL and add `get_v2`:

```rust
async fn send_get(
    &self,
    path: &str,
    params: &[(&str, String)],
    token: &str,
) -> Result<reqwest::Response> {
    self.send_get_at(&self.base_url, path, params, token).await
}

async fn send_get_at(
    &self,
    base: &str,
    path: &str,
    params: &[(&str, String)],
    token: &str,
) -> Result<reqwest::Response> {
    let mut query: Vec<(&str, String)> = Vec::new();
    if base == self.base_url.as_str() {
        query.push(("countryCode", self.session.country_code.clone()));
    }
    query.extend(params.iter().cloned());
    let url = format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'));
    Ok(self.http.get(url).bearer_auth(token).query(&query).send().await?)
}

/// GET on the v2 API base (no `countryCode`), sharing `get`'s refresh-on-401.
pub async fn get_v2<T: DeserializeOwned>(
    &self,
    path: &str,
    params: &[(&str, String)],
) -> Result<T> {
    let token = self.access_token();
    let mut resp = self.send_get_at(&self.v2_base_url, path, params, &token).await?;
    let mut refreshed = false;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        self.try_refresh(&token).await?;
        refreshed = true;
        resp = self
            .send_get_at(&self.v2_base_url, path, params, &self.access_token())
            .await?;
    }
    if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Forbidden);
    }
    Self::decode(resp).await
}
```

- [ ] **Step 4: Add the DTOs**

Append to `models.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "subTitle")]
    pub sub_title: Option<String>,
    #[serde(default, rename = "mixType")]
    pub mix_type: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}
```

Add a unit test:

```rust
#[test]
fn mix_parses_v2_fields() {
    let raw = r#"{"id": "m1", "title": "Daily", "subTitle": "For you", "mixType": "DAILY_MIX"}"#;
    let mix: Mix = serde_json::from_str(raw).unwrap();
    assert_eq!(mix.id, "m1");
    assert_eq!(mix.sub_title.as_deref(), Some("For you"));
    assert_eq!(mix.mix_type.as_deref(), Some("DAILY_MIX"));
}
```

- [ ] **Step 5: Write the failing library tests**

Append to `library.rs`'s test module:

```rust
#[tokio::test]
async fn all_favorite_videos_unwraps_the_item_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/users/42/favorites/videos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [{"item": {"id": 3, "title": "Clip", "duration": 120,
                "artists": [{"id": 1, "name": "A"}], "image": "img-3"}}],
            "totalNumberOfItems": 1, "limit": 100, "offset": 0
        })))
        .mount(&server).await;
    let videos = client(&server).all_favorite_videos().await.unwrap();
    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].id, 3);
    assert_eq!(videos[0].image.as_deref(), Some("img-3"));
}

#[tokio::test]
async fn favorite_mixes_accepts_the_v2_data_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v2/favorites/mixes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "m1", "title": "Daily Mix", "subTitle": "For you",
                 "images": {"SMALL": {"url": "https://img/small.jpg"}}},
                {"mix": {"id": "m2", "title": "Artist Radio",
                 "mixImages": [{"url": "https://img/a.jpg"}]}}
            ],
            "totalNumberOfItems": 2
        })))
        .mount(&server).await;
    let (mixes, total) = client(&server).favorite_mixes(50, 0).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(mixes[0].id, "m1");
    assert_eq!(mixes[0].image.as_deref(), Some("https://img/small.jpg"));
    assert_eq!(mixes[1].id, "m2");
    assert_eq!(mixes[1].image.as_deref(), Some("https://img/a.jpg"));
}

#[tokio::test]
async fn favorite_mixes_accepts_the_v1_items_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v2/favorites/mixes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [{"id": "m3", "title": "Mood", "type": "MIX"}]
        })))
        .mount(&server).await;
    let (mixes, total) = client(&server).favorite_mixes(50, 0).await.unwrap();
    assert_eq!(mixes.len(), 1);
    assert_eq!(mixes[0].id, "m3");
    assert_eq!(total, 1);
}
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal favorite_videos favorite_mixes`
Expected: FAIL — methods not found.

- [ ] **Step 7: Implement the library methods**

In `library.rs`, add `Mix, Video` to the `use crate::models::{...}` list and add:

```rust
/// Walk every page of the user's favorite videos.
pub async fn all_favorite_videos(&self) -> Result<Vec<Video>> {
    let favs: Vec<Favorited<Video>> = self
        .get_all_pages(&format!("{}/videos", self.user_base()), &[], DEFAULT_PAGE_SIZE)
        .await?;
    Ok(favs.into_iter().map(|f| f.item).collect())
}

/// Favorite mixes & radio (v2). The envelope varies by account/endpoint, so
/// both `{data:[…]}` and `{items:[…]}` are accepted, with entries bare or
/// wrapped in `{mix}`/`{data}`.
pub async fn favorite_mixes(&self, limit: u32, offset: u32) -> Result<(Vec<Mix>, u32)> {
    let raw: serde_json::Value = self.get_v2("favorites/mixes", &page(limit, offset)).await?;
    Ok(mixes_from_json(&raw))
}

fn mixes_from_json(raw: &serde_json::Value) -> (Vec<Mix>, u32) {
    let entries = raw
        .get("data")
        .or_else(|| raw.get("items"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total = raw
        .get("totalNumberOfItems")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(entries.len() as u64) as u32;
    let mixes = entries.iter().filter_map(mix_from_value).collect();
    (mixes, total)
}

fn mix_from_value(value: &serde_json::Value) -> Option<Mix> {
    let mix = value
        .get("mix")
        .or_else(|| value.get("data"))
        .unwrap_or(value);
    let id = value_to_string(mix.get("id")?);
    let title = mix
        .get("title")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/titleTextInfo/text").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_string();
    let sub_title = mix
        .get("subTitle")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/subtitleTextInfo/text").and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let mix_type = mix.get("mixType").and_then(serde_json::Value::as_str).map(str::to_string);
    let image = mix
        .pointer("/images/SMALL/url")
        .and_then(serde_json::Value::as_str)
        .or_else(|| mix.pointer("/mixImages/0/url").and_then(serde_json::Value::as_str))
        .map(str::to_string);
    Some(Mix { id, title, sub_title, mix_type, image })
}

fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p oxitide-tidal`
Expected: PASS (existing playlist/library tests plus the 4 new ones).

- [ ] **Step 9: Commit**

```powershell
git add crates/oxitide-tidal/src/client.rs crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/library.rs
git commit -m "feat(tidal): add v2 GET, video DTO, and favorite videos/mixes"
```

---

### Task 5: Folder tree read (v2, cursor-paged)

**Files:**
- Create: `crates/oxitide-tidal/src/folders.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`

**Interfaces:**
- Consumes: `TidalClient::get_v2` (Task 4), `models::Playlist` (`mark_owned`).
- Produces:
  - `pub enum CollectionEntry { Folder(CollectionFolder), Playlist(Playlist) }`, serialized internally tagged: `{"kind":"folder","id":…,"name":…,"totalNumberOfItems":…}` / `{"kind":"playlist", …Playlist}`.
  - `pub struct CollectionFolder { id: String, name: String, total_number_of_items: u32 }` (serializes `totalNumberOfItems`).
  - `TidalClient::collection_items(folder_id: &str) -> Result<Vec<CollectionEntry>>` (walks up to 20 cursor pages, folders first in API order).

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-tidal/src/folders.rs` with only the test skeleton first:

```rust
#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 42 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn collection_items_walks_cursor_pages_and_parses_both_kinds() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let has_cursor = req.url.query_pairs().any(|(key, _)| key == "cursor");
            if has_cursor {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "PLAYLIST", "data": {"uuid": "p-2", "title": "Second",
                            "numberOfTracks": 3, "creator": {"id": 42}}}
                    ],
                    "totalNumberOfItems": 2
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "FOLDER", "name": "MISC",
                            "data": {"id": "f-1", "totalNumberOfItems": 10}},
                        {"type": "PLAYLIST", "data": {"uuid": "p-1", "title": "Road trip",
                            "numberOfTracks": 12, "squareImage": "sq-1", "creator": {"id": 42}}}
                    ],
                    "cursor": "c2",
                    "totalNumberOfItems": 2
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;

        let entries = client(&server).collection_items("root").await.unwrap();
        assert_eq!(entries.len(), 3);
        match &entries[0] {
            crate::folders::CollectionEntry::Folder(folder) => {
                assert_eq!(folder.id, "f-1");
                assert_eq!(folder.name, "MISC");
                assert_eq!(folder.total_number_of_items, 10);
            }
            other => panic!("expected folder first, got {other:?}"),
        }
        match &entries[1] {
            crate::folders::CollectionEntry::Playlist(playlist) => {
                assert_eq!(playlist.uuid, "p-1");
                assert!(playlist.owned, "assume_owned applies to collection playlists");
            }
            other => panic!("expected playlist, got {other:?}"),
        }
        match &entries[2] {
            crate::folders::CollectionEntry::Playlist(playlist) => {
                assert_eq!(playlist.uuid, "p-2");
            }
            other => panic!("expected playlist, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn collection_items_stops_at_the_page_cap() {
        let server = MockServer::start().await;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let responder = {
            let calls = calls.clone();
            move |_req: &wiremock::Request| {
                let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [
                        {"type": "PLAYLIST", "data": {"uuid": "p-1", "title": "One"}}
                    ],
                    "cursor": format!("cursor-{n}")
                }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(responder)
            .expect(20)
            .mount(&server)
            .await;

        let entries = client(&server).collection_items("root").await.unwrap();
        assert_eq!(entries.len(), 20);
    }

    #[tokio::test]
    async fn collection_items_empty_page_returns_nothing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/my-collection/playlists/folders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0
            })))
            .mount(&server)
            .await;
        assert!(client(&server).collection_items("root").await.unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Register the module and run the tests to verify they fail**

Add `pub mod folders;` to `lib.rs`, then run:

Run: `cargo test -p oxitide-tidal collection_items`
Expected: FAIL — `collection_items` not found.

- [ ] **Step 3: Implement**

Insert above the test module in `folders.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;
use crate::models::Playlist;

/// 50 is the v2 folders endpoint's maximum; the cap below bounds a broken
/// cursor so a server bug can never loop forever.
const FOLDER_PAGE_LIMIT: u32 = 50;
const MAX_FOLDER_PAGES: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CollectionEntry {
    Folder(CollectionFolder),
    Playlist(Playlist),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionFolder {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "totalNumberOfItems")]
    pub total_number_of_items: u32,
}

#[derive(Debug, Deserialize)]
struct RawCollectionPage {
    #[serde(default)]
    items: Vec<RawCollectionItem>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCollectionItem {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    data: serde_json::Value,
}

impl TidalClient {
    /// Every folder and playlist inside `folder_id` (`"root"` for the top
    /// level), following the v2 cursor to the end.
    pub async fn collection_items(&self, folder_id: &str) -> Result<Vec<CollectionEntry>> {
        let mut entries = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_FOLDER_PAGES {
            let mut params = vec![
                ("folderId", folder_id.to_string()),
                ("limit", FOLDER_PAGE_LIMIT.to_string()),
                ("order", "NAME".to_string()),
            ];
            if let Some(value) = &cursor {
                params.push(("cursor", value.clone()));
            }
            let page: RawCollectionPage = self
                .get_v2("my-collection/playlists/folders", &params)
                .await?;
            let next = page.cursor.clone();
            let empty = page.items.is_empty();
            for item in page.items {
                if let Some(entry) = self.collection_entry(item) {
                    entries.push(entry);
                }
            }
            match next {
                Some(value) if !empty && Some(&value) != cursor.as_ref() => cursor = Some(value),
                _ => break,
            }
        }
        Ok(entries)
    }

    fn collection_entry(&self, item: RawCollectionItem) -> Option<CollectionEntry> {
        match item.kind.to_uppercase().as_str() {
            "FOLDER" => {
                let id = value_to_string(item.data.get("id")?)?;
                let total = item
                    .data
                    .get("totalNumberOfItems")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let name = item
                    .name
                    .or_else(|| {
                        item.data
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                Some(CollectionEntry::Folder(CollectionFolder {
                    id,
                    name,
                    total_number_of_items: total,
                }))
            }
            "PLAYLIST" => {
                let mut playlist: Playlist = serde_json::from_value(item.data).ok()?;
                playlist.mark_owned(self.session().user_id, true);
                Some(CollectionEntry::Playlist(playlist))
            }
            _ => None,
        }
    }
}

fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p oxitide-tidal folders`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-tidal/src/folders.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): list collection folders and playlists with cursor paging"
```

---

### Task 6: Folder writes (create, rename, remove, move)

**Files:**
- Modify: `crates/oxitide-tidal/src/client.rs`
- Modify: `crates/oxitide-tidal/src/folders.rs`

**Interfaces:**
- Consumes: `TidalClient::put_json` (`client.rs:152`), `mutate`.
- Produces:
  - `TidalClient::put_v2(path, params) -> Result<()>` (PUT on the v2 base, body discarded).
  - `TidalClient::create_folder(name, parent_id) -> Result<CollectionFolder>`.
  - `TidalClient::rename_folder(id, name) -> Result<()>`.
  - `TidalClient::remove_collection_entry(kind: &str, id: &str) -> Result<()>` (kind `"folder"`/`"playlist"`).
  - `TidalClient::move_playlists(ids: &[String], folder_id: &str) -> Result<()>` (empty list is a no-op).

- [ ] **Step 1: Write the failing client test for `put_v2`**

Append to `client.rs`'s test module:

```rust
#[tokio::test]
async fn put_v2_targets_v2_and_tolerates_an_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/folders/rename"))
        .and(query_param("name", "New"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let client = TidalClient::new(
        reqwest::Client::new(),
        server.uri(),
        format!("{}/token", server.uri()),
        session(),
        tokens(),
        Arc::new(MemoryStore::default()),
    );
    client
        .put_v2("folders/rename", &[("name", "New".to_string())])
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p oxitide-tidal put_v2_targets`
Expected: FAIL — `put_v2` not found.

- [ ] **Step 3: Implement `put_v2`**

Add to `client.rs` next to `put_json`:

```rust
/// PUT with query params on the v2 API, discarding the response body (several
/// v2 collection writes answer 204).
pub async fn put_v2(&self, path: &str, params: &[(&str, String)]) -> Result<()> {
    self.mutate(reqwest::Method::PUT, &self.v2_base_url, path, params, &[], None)
        .await
        .map(|_| ())
}
```

- [ ] **Step 4: Write the failing folder-write tests**

Append to `folders.rs`'s test module:

```rust
#[tokio::test]
async fn create_folder_puts_to_v2_and_parses_the_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/create-folder"))
        .and(wiremock::matchers::query_param("name", "Road"))
        .and(wiremock::matchers::query_param("folderId", "root"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"id": "f-9", "totalNumberOfItems": 0}
        })))
        .mount(&server)
        .await;
    let folder = client(&server).create_folder("Road", "root").await.unwrap();
    assert_eq!(folder.id, "f-9");
    assert_eq!(folder.name, "Road");
}

#[tokio::test]
async fn rename_folder_sends_the_folder_trn() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/rename"))
        .and(wiremock::matchers::query_param("trn", "trn:folder:f-1"))
        .and(wiremock::matchers::query_param("name", "Renamed"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    client(&server).rename_folder("f-1", "Renamed").await.unwrap();
}

#[tokio::test]
async fn remove_collection_entry_prefixes_the_trn_by_kind() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/remove"))
        .and(wiremock::matchers::query_param("trns", "trn:playlist:p-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/remove"))
        .and(wiremock::matchers::query_param("trns", "trn:folder:f-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    client(&server).remove_collection_entry("playlist", "p-1").await.unwrap();
    client(&server).remove_collection_entry("folder", "f-1").await.unwrap();
}

#[tokio::test]
async fn move_playlists_joins_trns_and_targets_the_folder() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/move"))
        .and(wiremock::matchers::query_param("folderId", "f-2"))
        .and(wiremock::matchers::query_param(
            "trns",
            "trn:playlist:p-1,trn:playlist:p-2",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    client(&server)
        .move_playlists(&["p-1".to_string(), "p-2".to_string()], "f-2")
        .await
        .unwrap();
}

#[tokio::test]
async fn move_playlists_with_no_ids_makes_no_request() {
    let server = MockServer::start().await;
    client(&server).move_playlists(&[], "f-2").await.unwrap();
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal folder`
Expected: FAIL — methods not found.

- [ ] **Step 6: Implement the writes**

Add to the `impl TidalClient` block in `folders.rs`:

```rust
/// Create a folder inside `parent_id` (`"root"` for the top level).
pub async fn create_folder(&self, name: &str, parent_id: &str) -> Result<CollectionFolder> {
    #[derive(Deserialize)]
    struct RawFolder {
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default, rename = "totalNumberOfItems")]
        total_number_of_items: u32,
    }
    #[derive(Deserialize)]
    struct Envelope {
        data: RawFolder,
    }
    let envelope: Envelope = self
        .put_json(
            "my-collection/playlists/folders/create-folder",
            &[("name", name.to_string()), ("folderId", parent_id.to_string())],
        )
        .await?;
    Ok(CollectionFolder {
        id: envelope.data.id,
        name: envelope.data.name.unwrap_or_else(|| name.to_string()),
        total_number_of_items: envelope.data.total_number_of_items,
    })
}

pub async fn rename_folder(&self, id: &str, name: &str) -> Result<()> {
    self.put_v2(
        "my-collection/playlists/folders/rename",
        &[
            ("trn", format!("trn:folder:{id}")),
            ("name", name.to_string()),
        ],
    )
    .await
}

pub async fn remove_collection_entry(&self, kind: &str, id: &str) -> Result<()> {
    let trn = if kind == "playlist" {
        format!("trn:playlist:{id}")
    } else {
        format!("trn:folder:{id}")
    };
    self.put_v2(
        "my-collection/playlists/folders/remove",
        &[("trns", trn)],
    )
    .await
}

pub async fn move_playlists(&self, ids: &[String], folder_id: &str) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let trns = ids
        .iter()
        .map(|id| format!("trn:playlist:{id}"))
        .collect::<Vec<_>>()
        .join(",");
    self.put_v2(
        "my-collection/playlists/folders/move",
        &[("folderId", folder_id.to_string()), ("trns", trns)],
    )
    .await
}
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p oxitide-tidal`
Expected: PASS.

- [ ] **Step 8: Commit**

```powershell
git add crates/oxitide-tidal/src/client.rs crates/oxitide-tidal/src/folders.rs
git commit -m "feat(tidal): manage collection folders and playlist moves"
```

---

### Task 7: Purchases fetcher

**Files:**
- Create: `crates/oxitide-tidal/src/purchases.rs`
- Modify: `crates/oxitide-tidal/src/pages.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`

**Interfaces:**
- Consumes: `PageItem`, `ItemPage`, `TidalClient::get`.
- Produces:
  - `pub fn page_item_from_value(value: &serde_json::Value) -> Option<PageItem>` in `pages.rs` (accepts bare items or `{item: …}` wrappers; picks `id`/`uuid`, `title`/`name`, `type`, and `cover`/`squareImage`/`image`/`picture`).
  - `TidalClient::purchases(limit, offset) -> Result<(Vec<PageItem>, u32)>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-tidal/src/purchases.rs` with the client helper and tests:

```rust
#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 42 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn purchases_accepts_wrapped_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/purchases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {"item": {"id": 7, "title": "Bought Album", "type": "ALBUM", "cover": "c-7"}}
                ],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let (items, total) = client(&server).purchases(100, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].id, "7");
        assert_eq!(items[0].item_type, "ALBUM");
        assert_eq!(items[0].artwork.as_deref(), Some("c-7"));
    }

    #[tokio::test]
    async fn purchases_accepts_bare_items_with_uuids() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/purchases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"uuid": "p-1", "title": "Bought Playlist", "type": "PLAYLIST"}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let (items, _) = client(&server).purchases(100, 0).await.unwrap();
        assert_eq!(items[0].id, "p-1");
    }
}
```

- [ ] **Step 2: Register the module and run the tests to verify they fail**

Add `pub mod purchases;` to `lib.rs`, then run:

Run: `cargo test -p oxitide-tidal purchases`
Expected: FAIL — `purchases` not found.

- [ ] **Step 3: Implement**

`pages.rs` — add near `item_id`:

```rust
/// Best-effort mapping of a foreign item shape into a `PageItem`; accepts
/// `{item: …}` wrappers (favorites/purchases) and bare objects, ids or uuids.
pub fn page_item_from_value(value: &serde_json::Value) -> Option<PageItem> {
    let item = value.get("item").unwrap_or(value);
    let id = value_to_id(item.get("id")?)
        .or_else(|| item.get("uuid").and_then(|u| u.as_str()).map(str::to_string))
        .filter(|id| !id.is_empty())?;
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)
        .or_else(|| item.get("name").and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_string();
    let item_type = item
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let artwork = ["cover", "squareImage", "image", "picture"]
        .iter()
        .find_map(|key| item.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    Some(PageItem { id, title, subtitle: None, item_type, artwork })
}
```

`purchases.rs` — above the tests:

```rust
use crate::client::TidalClient;
use crate::error::Result;
use crate::models::ItemPage;
use crate::pages::{page_item_from_value, PageItem};

impl TidalClient {
    /// The user's purchase history. The response shape is unconfirmed; the
    /// permissive mapping accepts wrapped or bare items and degrades to empty
    /// when nothing maps.
    pub async fn purchases(&self, limit: u32, offset: u32) -> Result<(Vec<PageItem>, u32)> {
        let page: ItemPage<serde_json::Value> = self
            .get(
                &format!("users/{}/purchases", self.session().user_id),
                &[("limit", limit.to_string()), ("offset", offset.to_string())],
            )
            .await?;
        let total = page.total_number_of_items;
        let items = page.items.iter().filter_map(page_item_from_value).collect();
        Ok((items, total))
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p oxitide-tidal purchases`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-tidal/src/purchases.rs crates/oxitide-tidal/src/pages.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): fetch purchase history permissively"
```

---

### Task 8: Tauri commands and handler registration

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs:161-228`

**Interfaces:**
- Consumes: all client methods from Tasks 1–7.
- Produces (all registered in `generate_handler!`):
  - `get_explore() -> Vec<PageSection>`
  - `get_feed() -> Vec<PageSection>`
  - `get_favorite_mixes() -> Vec<Mix>`
  - `get_mix(id: String) -> MixDetail`
  - `get_favorite_videos() -> Vec<Video>`
  - `get_collection_items(folder_id: Option<String>) -> Vec<CollectionEntry>`
  - `create_folder(name: String, parent_id: Option<String>) -> CollectionFolder`
  - `rename_folder(id: String, name: String) -> ()`
  - `remove_collection_entry(kind: String, id: String) -> ()`
  - `move_playlists_to_folder(ids: Vec<String>, folder_id: String) -> ()`
  - `get_purchases() -> Vec<PageItem>`

- [ ] **Step 1: Add the command functions**

Append to `commands.rs` after `get_home` — extend the existing `use oxitide_tidal::…` lines with `folders::{CollectionEntry, CollectionFolder}`, `models::{Mix, Video}`, and `pages::{MixDetail, PageItem, PageSection}` first:

```rust
#[tauri::command]
pub async fn get_explore(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?.explore_page().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_feed(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?.feed_page().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_mixes(state: State<'_, AppState>) -> Result<Vec<Mix>, String> {
    let (mixes, _total) = require_client(&state)?
        .favorite_mixes(100, 0)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(mixes)
}

#[tauri::command]
pub async fn get_mix(state: State<'_, AppState>, id: String) -> Result<MixDetail, String> {
    require_client(&state)?.mix_page(&id).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_videos(state: State<'_, AppState>) -> Result<Vec<Video>, String> {
    require_client(&state)?.all_favorite_videos().await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_collection_items(
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Vec<CollectionEntry>, String> {
    let folder = folder_id.unwrap_or_else(|| "root".to_string());
    require_client(&state)?.collection_items(&folder).await.map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<CollectionFolder, String> {
    let parent = parent_id.unwrap_or_else(|| "root".to_string());
    require_client(&state)?
        .create_folder(&name, &parent)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn rename_folder(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    require_client(&state)?
        .rename_folder(&id, &name)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn remove_collection_entry(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<(), String> {
    require_client(&state)?
        .remove_collection_entry(&kind, &id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn move_playlists_to_folder(
    state: State<'_, AppState>,
    ids: Vec<String>,
    folder_id: String,
) -> Result<(), String> {
    require_client(&state)?
        .move_playlists(&ids, &folder_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_purchases(state: State<'_, AppState>) -> Result<Vec<PageItem>, String> {
    let (items, _total) = require_client(&state)?
        .purchases(100, 0)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    Ok(items)
}
```

- [ ] **Step 2: Register the commands**

In `main.rs`, add to `generate_handler!` after `commands::search`:

```rust
commands::get_explore,
commands::get_feed,
commands::get_favorite_mixes,
commands::get_mix,
commands::get_favorite_videos,
commands::get_collection_items,
commands::create_folder,
commands::rename_folder,
commands::remove_collection_entry,
commands::move_playlists_to_folder,
commands::get_purchases,
```

- [ ] **Step 3: Run the Rust gates**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose explore, feed, collection, and mix commands"
```

---

## Phase B — UI foundations

### Task 9: API types and invoke wrappers

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/views/Home.svelte:2,10` (type rename only)

**Interfaces:**
- Consumes: command names from Task 8.
- Produces: `PageItem`, `PageSection`, `Video`, `Mix`, `CollectionFolder`, `CollectionEntry`, `MixDetail` interfaces and `api.getExplore`, `api.getFeed`, `api.getFavoriteMixes`, `api.getMix`, `api.getFavoriteVideos`, `api.getCollectionItems`, `api.createFolder`, `api.renameFolder`, `api.removeCollectionEntry`, `api.movePlaylistsToFolder`, `api.getPurchases`.

- [ ] **Step 1: Replace the home types and add the new DTOs**

In `ui/src/lib/api.ts`, replace lines 24–25:

```ts
export interface PageItem { id: string; title: string; subtitle?: string | null; item_type: string; artwork?: string | null }
export interface PageSection { title: string; items: PageItem[] }
```

and add after the `FavoriteIds` interface:

```ts
export interface Video {
  id: number; title: string; duration: number;
  artists: ArtistRef[]; image?: string | null;
}
export interface Mix {
  id: string; title: string; subTitle?: string | null;
  mixType?: string | null; image?: string | null;
}
export interface CollectionFolder { id: string; name: string; totalNumberOfItems: number }
export type CollectionEntry =
  | ({ kind: "folder" } & CollectionFolder)
  | ({ kind: "playlist" } & Playlist);
export interface MixDetail { id: string; title: string; tracks: Track[] }
```

- [ ] **Step 2: Add the invoke wrappers**

Change the existing `getHome` line to return the new type:

```ts
  getHome: () => invoke<PageSection[]>("get_home"),
```

In the `api` object, after `getHome`, add (keep alphabetical grouping with the other getters):

```ts
  getExplore: () => invoke<PageSection[]>("get_explore"),
  getFeed: () => invoke<PageSection[]>("get_feed"),
  getFavoriteMixes: () => invoke<Mix[]>("get_favorite_mixes"),
  getMix: (id: string) => invoke<MixDetail>("get_mix", { id }),
  getFavoriteVideos: () => invoke<Video[]>("get_favorite_videos"),
  getCollectionItems: (folderId = "root") =>
    invoke<CollectionEntry[]>("get_collection_items", { folderId }),
  createFolder: (name: string, parentId = "root") =>
    invoke<CollectionFolder>("create_folder", { name, parentId }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  removeCollectionEntry: (kind: "folder" | "playlist", id: string) =>
    invoke<void>("remove_collection_entry", { kind, id }),
  movePlaylistsToFolder: (ids: string[], folderId: string) =>
    invoke<void>("move_playlists_to_folder", { ids, folderId }),
  getPurchases: () => invoke<PageItem[]>("get_purchases"),
```

- [ ] **Step 3: Update the Home view's type names**

In `ui/src/views/Home.svelte`, change the import to `type PageSection` and the state annotation to `$state<PageSection[]>([])`. This is the only change in this task.

- [ ] **Step 4: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/api.ts ui/src/views/Home.svelte
git commit -m "feat(ui): add page, mix, video, and collection API types"
```

---

### Task 10: Navigation routes and page-item routing

**Files:**
- Modify: `ui/src/lib/nav.svelte.ts`
- Modify: `ui/src/lib/nav.test.ts`
- Modify: `ui/src/views/Home.svelte:7-8,43`

**Interfaces:**
- Consumes: `PageItem` (Task 9).
- Produces: extended `Route` union (`explore`, `feed`, `playlists`, `folder; id`, `albums`, `tracks`, `mixes`, `videos`, `profiles`, `purchases` added; `search` and `library` retained until Tasks 15/17 remove them) and renamed `routeForPageItem(item: PageItem): Route | null`.

- [ ] **Step 1: Update the tests first**

In `ui/src/lib/nav.test.ts`:
- Change the import to `routeForPageItem`.
- Replace every `{ name: "search" }` with `{ name: "explore" }` and every `{ name: "library" }` with `{ name: "feed" }` in the history tests.
- Rename `function homeItem` to `pageItem` and the `describe("routeForHomeItem")` block to `describe("routeForPageItem")`, updating calls.
- Change the mixes expectation to inert and add the new inert routes:

```ts
  it("leaves tracks, mixes, videos, and empty ids inert", () => {
    expect(routeForPageItem(pageItem("TRACK_LIST", "55"))).toBeNull();
    expect(routeForPageItem(pageItem("MIX", "mix-1"))).toBeNull();
    expect(routeForPageItem(pageItem("VIDEO", "77"))).toBeNull();
    expect(routeForPageItem(pageItem("", "1"))).toBeNull();
    expect(routeForPageItem(pageItem("ALBUM", ""))).toBeNull();
  });
```

Also add a history test proving a folder route with different ids is distinct:

```ts
  it("keeps distinct folder ids in history", () => {
    const nav = createNav();
    nav.goto({ name: "folder", id: "a" });
    nav.goto({ name: "folder", id: "b" });

    nav.back();

    expect(nav.route).toEqual({ name: "folder", id: "a" });
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: FAIL — `routeForPageItem` not exported.

- [ ] **Step 3: Implement**

In `nav.svelte.ts`, replace the `Route` union with:

```ts
export type Route =
  | { name: "home" }
  | { name: "explore" }
  | { name: "feed" }
  | { name: "search" }
  | { name: "library" }
  | { name: "playlists" }
  | { name: "folder"; id: string }
  | { name: "albums" }
  | { name: "tracks" }
  | { name: "mixes" }
  | { name: "videos" }
  | { name: "profiles" }
  | { name: "purchases" }
  | { name: "album"; id: number }
  | { name: "playlist"; id: string }
  | { name: "artist"; id: number }
  | { name: "settings" };
```

Keep the comment about `search`/`library` as temporary: replace it with a short note that they are removed in the final sidebar task. Rename the import and function:

```ts
import type { PageItem } from "./api";
```

```ts
/// Map a page-feed item to a route only when the type and id shape agree.
/// Mixes are handled by the shared renderer before this is consulted; tracks,
/// videos, and unknown types stay inert.
export function routeForPageItem(item: PageItem): Route | null {
  if (item.id.length === 0) return null;
  const type = item.item_type.toUpperCase();
  if ((type.includes("PLAYLIST") || type.includes("EDITORIAL")) && !NUMERIC_ID.test(item.id)) {
    return { name: "playlist", id: item.id };
  }
  if (type.includes("ALBUM") && NUMERIC_ID.test(item.id)) {
    return { name: "album", id: Number(item.id) };
  }
  return null;
}
```

- [ ] **Step 4: Update Home's import**

In `ui/src/views/Home.svelte`, change `routeForHomeItem` to `routeForPageItem` in the import and the `{@const route = …}` line. No behavior change for Home yet.

- [ ] **Step 5: Run the tests**

Run: `npm --prefix ui test -- src/lib/nav.test.ts && npm --prefix ui run check`
Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts ui/src/views/Home.svelte
git commit -m "feat(ui): add collection and feed routes, generalize page-item routing"
```

---

### Task 11: Collection tree store

**Files:**
- Create: `ui/src/lib/collection.svelte.ts`
- Create: `ui/src/lib/collection.test.ts`

**Interfaces:**
- Consumes: `api.getCollectionItems` (Task 9), `errorMessage`.
- Produces: `CollectionTree` interface (below) and `createCollectionTree(toasts: Toasts): CollectionTree`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/collection.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CollectionEntry } from "./api";
import { api } from "./api";
import { createCollectionTree } from "./collection.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({ api: { getCollectionItems: vi.fn() } }));

function playlist(uuid: string): CollectionEntry {
  return {
    kind: "playlist",
    uuid,
    title: uuid,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
  };
}

function folder(id: string): CollectionEntry {
  return { kind: "folder", id, name: id, totalNumberOfItems: 1 };
}

function fakeToasts(): Toasts {
  return {
    get toasts() {
      return [];
    },
    show(_message: string, _options?: { severity?: ToastSeverity }) {
      return 1;
    },
    dismiss() {},
  };
}

describe("collection tree store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads the root once", async () => {
    vi.mocked(api.getCollectionItems).mockResolvedValue([playlist("p1")]);
    const tree = createCollectionTree(fakeToasts());

    await tree.ensureRoot();
    await tree.ensureRoot();

    expect(api.getCollectionItems).toHaveBeenCalledTimes(1);
    expect(api.getCollectionItems).toHaveBeenCalledWith("root");
    expect(tree.root).toHaveLength(1);
  });

  it("loads folder children lazily and collapses without refetching", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) =>
      folderId === "root" ? [folder("f1")] : [playlist("p2")],
    );
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();

    await tree.toggleFolder("f1");
    expect(tree.isExpanded("f1")).toBe(true);
    expect(tree.children("f1")).toHaveLength(1);

    await tree.toggleFolder("f1");
    await tree.toggleFolder("f1");

    expect(api.getCollectionItems).toHaveBeenCalledTimes(2);
  });

  it("refresh reloads the root and expanded folders", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) =>
      folderId === "root" ? [folder("f1")] : [playlist("p2")],
    );
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();
    await tree.toggleFolder("f1");

    await tree.refresh();

    expect(api.getCollectionItems).toHaveBeenCalledTimes(4);
    expect(tree.children("f1")).toHaveLength(1);
  });

  it("surfaces load failures without clearing the tree", async () => {
    vi.mocked(api.getCollectionItems).mockRejectedValue("offline");
    const tree = createCollectionTree(fakeToasts());

    await tree.ensureRoot();

    expect(tree.error).toBe("offline");
    expect(tree.loading).toBe(false);
  });

  it("reset empties the tree and expansion", async () => {
    vi.mocked(api.getCollectionItems).mockResolvedValue([folder("f1")]);
    const tree = createCollectionTree(fakeToasts());
    await tree.ensureRoot();
    await tree.toggleFolder("f1");

    tree.reset();

    expect(tree.root).toHaveLength(0);
    expect(tree.isExpanded("f1")).toBe(false);
    expect(tree.children("f1")).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/collection.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

Create `ui/src/lib/collection.svelte.ts`:

```ts
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { api, type CollectionEntry } from "./api";
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";

export interface CollectionTree {
  readonly loading: boolean;
  readonly error: string;
  readonly root: readonly CollectionEntry[];
  children(folderId: string): readonly CollectionEntry[] | null;
  isExpanded(folderId: string): boolean;
  isLoading(folderId: string): boolean;
  ensureRoot(): Promise<void>;
  toggleFolder(folderId: string): Promise<void>;
  refresh(): Promise<void>;
  reset(): void;
}

/// The sidebar's root folders/playlists plus lazily fetched folder children.
/// Views call `refresh()` after a successful mutation; the root and any
/// expanded folders are then reloaded.
export function createCollectionTree(toasts: Toasts): CollectionTree {
  let root = $state<CollectionEntry[]>([]);
  let loading = $state(false);
  let error = $state("");
  const childrenMap = new SvelteMap<string, CollectionEntry[]>();
  const expanded = new SvelteSet<string>();
  const loadingFolders = new SvelteSet<string>();

  async function loadRoot(): Promise<void> {
    loading = true;
    error = "";
    try {
      root = await api.getCollectionItems("root");
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  async function loadChildren(folderId: string): Promise<void> {
    if (loadingFolders.has(folderId)) return;
    loadingFolders.add(folderId);
    try {
      childrenMap.set(folderId, await api.getCollectionItems(folderId));
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      loadingFolders.delete(folderId);
    }
  }

  async function ensureRoot(): Promise<void> {
    if (root.length > 0 || loading) return;
    await loadRoot();
  }

  async function toggleFolder(folderId: string): Promise<void> {
    if (expanded.has(folderId)) {
      expanded.delete(folderId);
      return;
    }
    expanded.add(folderId);
    if (!childrenMap.has(folderId)) await loadChildren(folderId);
  }

  async function refresh(): Promise<void> {
    childrenMap.clear();
    await loadRoot();
    for (const folderId of [...expanded]) await loadChildren(folderId);
  }

  function reset(): void {
    root = [];
    error = "";
    loading = false;
    childrenMap.clear();
    expanded.clear();
    loadingFolders.clear();
  }

  return {
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get root() {
      return root;
    },
    children: (folderId) => childrenMap.get(folderId) ?? null,
    isExpanded: (folderId) => expanded.has(folderId),
    isLoading: (folderId) => loadingFolders.has(folderId),
    ensureRoot,
    toggleFolder,
    refresh,
    reset,
  };
}
```

- [ ] **Step 4: Run the tests**

Run: `npm --prefix ui test -- src/lib/collection.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/collection.svelte.ts ui/src/lib/collection.test.ts
git commit -m "feat(ui): add the reactive collection tree store"
```

---

### Task 12: Shared page renderer and mix playback

**Files:**
- Create: `ui/src/lib/components/PageSections.svelte`
- Create: `ui/src/lib/mixPlay.ts`
- Create: `ui/src/lib/mixPlay.test.ts`
- Modify: `ui/src/views/Home.svelte`

**Interfaces:**
- Consumes: `routeForPageItem` (Task 10), `PageSection` (Task 9), `trackToQueueItem`, `Nav`, `Toasts`.
- Produces: `PageSections.svelte` props `{ sections: PageSection[]; nav: Nav; onPlayMix?: (id: string) => void }`; `playMix(id: string, toasts: Toasts): Promise<void>`.

- [ ] **Step 1: Write the failing mixPlay test**

Create `ui/src/lib/mixPlay.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { playMix } from "./mixPlay";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: { getMix: vi.fn(), playTracks: vi.fn() },
  trackToQueueItem: (t: { id: number }) => ({ track_id: t.id }),
}));

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

describe("playMix", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("queues the mapped tracks", async () => {
    vi.mocked(api.getMix).mockResolvedValue({
      id: "m1",
      title: "Mix",
      tracks: [{ id: 1 } as never, { id: 2 } as never],
    });
    const { toasts, calls } = fakeToasts();

    await playMix("m1", toasts);

    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 1 }, { track_id: 2 }], 0);
    expect(calls).toHaveLength(0);
  });

  it("informs instead of playing when the mix has no tracks", async () => {
    vi.mocked(api.getMix).mockResolvedValue({ id: "m1", title: "Mix", tracks: [] });
    const { toasts, calls } = fakeToasts();

    await playMix("m1", toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls[0]?.message).toContain("no playable tracks");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/mixPlay.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the helper**

Create `ui/src/lib/mixPlay.ts`:

```ts
import { api, trackToQueueItem } from "./api";
import type { Toasts } from "./toasts.svelte";

export async function playMix(id: string, toasts: Toasts): Promise<void> {
  const detail = await api.getMix(id);
  if (detail.tracks.length === 0) {
    toasts.show("This mix has no playable tracks.", { severity: "info" });
    return;
  }
  await api.playTracks(detail.tracks.map(trackToQueueItem), 0);
}
```

- [ ] **Step 4: Run the mixPlay tests**

Run: `npm --prefix ui test -- src/lib/mixPlay.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Create the shared renderer**

Create `ui/src/lib/components/PageSections.svelte`:

```svelte
<script lang="ts">
  import type { PageSection } from "../api";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import Cover from "./Cover.svelte";

  let {
    sections,
    nav,
    onPlayMix,
  }: {
    sections: PageSection[];
    nav: Nav;
    onPlayMix?: (id: string) => void;
  } = $props();
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  <h3>{section.title}</h3>
  <div class="card-grid">
    {#each section.items as item, i (item.id || `${item.title}-${i}`)}
      {#if item.item_type.toUpperCase().includes("MIX") && onPlayMix}
        <button class="card" onclick={() => onPlayMix(item.id)}>
          <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
          <span class="card-title">{item.title}</span>
          {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
        </button>
      {:else}
        {@const route = routeForPageItem(item)}
        {#if route}
          <button class="card" onclick={() => nav.goto(route)}>
            <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
            <span class="card-title">{item.title}</span>
            {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
          </button>
        {:else}
          <div class="card inert">
            <span class="card-art"><Cover uuid={item.artwork} size={320} radius={12} alt="" /></span>
            <span class="card-title">{item.title}</span>
            {#if item.subtitle}<span class="muted card-sub">{item.subtitle}</span>{/if}
          </div>
        {/if}
      {/if}
    {/each}
  </div>
{/each}

<style>
  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .card-art {
    display: block;
    overflow: hidden;
    border-radius: var(--radius-card);
  }
  .card-art :global(img) {
    transition: transform var(--dur-state) var(--ease-in);
  }
  .card:hover .card-art :global(img) {
    transform: scale(1.03);
  }
  .card-title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card.inert {
    opacity: 0.9;
  }
</style>
```

- [ ] **Step 6: Rewrite Home to use it**

Replace the middle of `ui/src/views/Home.svelte` (from `<h2>Home</h2>` through the closing `{/if}`) and the script imports:

```svelte
<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import { playMix } from "../lib/mixPlay";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let sections = $state<PageSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = await api.getHome();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<h2>Home</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to show yet." />
{:else}
  <PageSections {sections} {nav} onPlayMix={(id) => void playMix(id, toasts)} />
{/if}
```

Remove the old `<style>` block entirely (styles now live in `PageSections.svelte`).

- [ ] **Step 7: Wire the new `toasts` prop**

In `App.svelte`, change the home branch to `<Home {nav} {toasts} />`.

- [ ] **Step 8: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add ui/src/lib/components/PageSections.svelte ui/src/lib/mixPlay.ts ui/src/lib/mixPlay.test.ts ui/src/views/Home.svelte ui/src/App.svelte
git commit -m "feat(ui): share page-section rendering and mix playback"
```

---

### Task 13: Collection mutation actions and dialogs

**Files:**
- Create: `ui/src/lib/collectionActions.svelte.ts`
- Create: `ui/src/lib/collectionActions.test.ts`
- Create: `ui/src/lib/components/CollectionDialogs.svelte`

**Interfaces:**
- Consumes: `CollectionTree` (Task 11), `api` collection writes (Task 9), `Modal`, `ConfirmDialog`, `Loading`.
- Produces: `createCollectionActions(collection: CollectionTree, toasts: Toasts): CollectionActions` with `renameTarget`, `removeTarget`, `moveTarget`, `busy`, `folderOptions`, `foldersLoading`, `revision`, and `openRenamePlaylist`, `openRenameFolder`, `openRemove`, `openMove`, `close`, `rename(value)`, `confirmRemove()`, `move(folderId)`, `reset()`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/collectionActions.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CollectionEntry, CollectionFolder, Playlist } from "./api";
import { api } from "./api";
import { createCollectionActions } from "./collectionActions.svelte";
import type { CollectionTree } from "./collection.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    getCollectionItems: vi.fn(),
    renamePlaylist: vi.fn(),
    renameFolder: vi.fn(),
    deletePlaylist: vi.fn(),
    removeCollectionEntry: vi.fn(),
    movePlaylistsToFolder: vi.fn(),
  },
}));

function playlist(uuid: string, owned = true): Playlist {
  return {
    uuid,
    title: `P${uuid}`,
    description: null,
    numberOfTracks: 1,
    duration: 0,
    image: null,
    owned,
  };
}

function entry(uuid: string, owned = true): CollectionEntry {
  return { kind: "playlist", ...playlist(uuid, owned) };
}

function folder(id: string): CollectionFolder {
  return { id, name: `F${id}`, totalNumberOfItems: 1 };
}

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

function fakeTree() {
  const refresh = vi.fn().mockResolvedValue(undefined);
  return { tree: { refresh } as unknown as CollectionTree, refresh };
}

describe("collection actions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renames a playlist, bumps revision, refreshes, and closes", async () => {
    vi.mocked(api.renamePlaylist).mockResolvedValue(undefined);
    const { tree, refresh } = fakeTree();
    const { toasts } = fakeToasts();
    const actions = createCollectionActions(tree, toasts);

    actions.openRenamePlaylist(playlist("p1"));
    await actions.rename("  New name  ");

    expect(api.renamePlaylist).toHaveBeenCalledWith("p1", "New name", "");
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(actions.revision).toBe(1);
    expect(actions.renameTarget).toBeNull();
  });

  it("keeps the rename dialog open and does not refresh when the write fails", async () => {
    vi.mocked(api.renameFolder).mockRejectedValue("forbidden");
    const { tree, refresh } = fakeTree();
    const { toasts, calls } = fakeToasts();
    const actions = createCollectionActions(tree, toasts);

    actions.openRenameFolder(folder("f1"));
    await actions.rename("Nope");

    expect(refresh).not.toHaveBeenCalled();
    expect(actions.revision).toBe(0);
    expect(actions.renameTarget).not.toBeNull();
    expect(calls[0]?.message).toBe("You can only edit your own playlists");
  });

  it("deletes owned playlists but removes liked ones from the collection", async () => {
    vi.mocked(api.deletePlaylist).mockResolvedValue(undefined);
    vi.mocked(api.removeCollectionEntry).mockResolvedValue(undefined);
    const { toasts } = fakeToasts();
    const actions = createCollectionActions(fakeTree().tree, toasts);

    actions.openRemove(entry("owned", true));
    await actions.confirmRemove();
    actions.openRemove(entry("liked", false));
    await actions.confirmRemove();
    actions.openRemove({ kind: "folder", ...folder("f1") });
    await actions.confirmRemove();

    expect(api.deletePlaylist).toHaveBeenCalledWith("owned");
    expect(api.removeCollectionEntry).toHaveBeenCalledWith("playlist", "liked");
    expect(api.removeCollectionEntry).toHaveBeenCalledWith("folder", "f1");
  });

  it("moves a playlist into a folder", async () => {
    vi.mocked(api.movePlaylistsToFolder).mockResolvedValue(undefined);
    const { tree, refresh } = fakeTree();
    const actions = createCollectionActions(tree, fakeToasts().toasts);

    actions.openMove(playlist("p1"));
    await actions.move("f2");

    expect(api.movePlaylistsToFolder).toHaveBeenCalledWith(["p1"], "f2");
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(actions.moveTarget).toBeNull();
  });

  it("lists nested folders with depth for the move picker", async () => {
    vi.mocked(api.getCollectionItems).mockImplementation(async (folderId) => {
      if (folderId === "root") return [{ kind: "folder", ...folder("a") }];
      if (folderId === "a") return [{ kind: "folder", ...folder("b") }];
      return [];
    });
    const actions = createCollectionActions(fakeTree().tree, fakeToasts().toasts);

    actions.openMove(playlist("p1"));
    await vi.waitFor(() => expect(actions.foldersLoading).toBe(false));

    expect(actions.folderOptions).toEqual([
      { folder: folder("a"), depth: 0 },
      { folder: folder("b"), depth: 1 },
    ]);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/collectionActions.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the store**

Create `ui/src/lib/collectionActions.svelte.ts`:

```ts
import { api, type CollectionEntry, type CollectionFolder, type Playlist } from "./api";
import { errorMessage } from "./errors";
import type { CollectionTree } from "./collection.svelte";
import type { Toasts } from "./toasts.svelte";

export type RenameTarget =
  | { kind: "playlist"; playlist: Playlist }
  | { kind: "folder"; folder: CollectionFolder };

export interface FolderOption {
  folder: CollectionFolder;
  depth: number;
}

export interface CollectionActions {
  readonly renameTarget: RenameTarget | null;
  readonly removeTarget: CollectionEntry | null;
  readonly moveTarget: Playlist | null;
  readonly busy: boolean;
  readonly revision: number;
  readonly folderOptions: readonly FolderOption[];
  readonly foldersLoading: boolean;
  openRenamePlaylist(playlist: Playlist): void;
  openRenameFolder(folder: CollectionFolder): void;
  openRemove(entry: CollectionEntry): void;
  openMove(playlist: Playlist): void;
  close(): void;
  rename(value: string): Promise<void>;
  confirmRemove(): Promise<void>;
  move(folderId: string): Promise<void>;
  reset(): void;
}

const MAX_FOLDER_DEPTH = 5;

/// Playlist/folder mutations shared by the sidebar tree and the Playlists
/// page. Successful writes refresh the `CollectionTree` and bump `revision`
/// so the page can reload; failures only toast and keep dialogs open.
export function createCollectionActions(
  collection: CollectionTree,
  toasts: Toasts,
): CollectionActions {
  let renameTarget = $state<RenameTarget | null>(null);
  let removeTarget = $state<CollectionEntry | null>(null);
  let moveTarget = $state<Playlist | null>(null);
  let busy = $state(false);
  let revision = $state(0);
  let folderOptions = $state<FolderOption[]>([]);
  let foldersLoading = $state(false);

  function close() {
    renameTarget = null;
    removeTarget = null;
    moveTarget = null;
  }

  function reset() {
    close();
    busy = false;
    folderOptions = [];
    foldersLoading = false;
  }

  async function collectFolders(parentId: string, depth: number): Promise<FolderOption[]> {
    if (depth > MAX_FOLDER_DEPTH) return [];
    const entries = await api.getCollectionItems(parentId);
    const folders = entries.filter(
      (item): item is { kind: "folder" } & CollectionFolder => item.kind === "folder",
    );
    const nested = await Promise.all(
      folders.map((item) => collectFolders(item.id, depth + 1)),
    );
    return [...folders.map((folder) => ({ folder, depth })), ...nested.flat()];
  }

  async function loadFolders() {
    foldersLoading = true;
    try {
      folderOptions = await collectFolders("root", 0);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      foldersLoading = false;
    }
  }

  function openRenamePlaylist(playlist: Playlist) {
    renameTarget = { kind: "playlist", playlist };
  }

  function openRenameFolder(folder: CollectionFolder) {
    renameTarget = { kind: "folder", folder };
  }

  function openRemove(entry: CollectionEntry) {
    removeTarget = entry;
  }

  function openMove(playlist: Playlist) {
    moveTarget = playlist;
    folderOptions = [];
    void loadFolders();
  }

  async function rename(value: string) {
    const target = renameTarget;
    const name = value.trim();
    if (!target || name.length === 0 || busy) return;
    busy = true;
    try {
      if (target.kind === "playlist") {
        await api.renamePlaylist(target.playlist.uuid, name, target.playlist.description ?? "");
        toasts.show("Playlist renamed", { severity: "success" });
      } else {
        await api.renameFolder(target.folder.id, name);
        toasts.show("Folder renamed", { severity: "success" });
      }
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function confirmRemove() {
    const target = removeTarget;
    if (!target || busy) return;
    busy = true;
    try {
      if (target.kind === "folder") {
        await api.removeCollectionEntry("folder", target.id);
        toasts.show(`Removed ${target.name}`, { severity: "success" });
      } else if (target.owned) {
        await api.deletePlaylist(target.uuid);
        toasts.show(`Deleted ${target.title}`, { severity: "success" });
      } else {
        await api.removeCollectionEntry("playlist", target.uuid);
        toasts.show(`Removed ${target.title}`, { severity: "success" });
      }
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  async function move(folderId: string) {
    const target = moveTarget;
    if (!target || busy) return;
    busy = true;
    try {
      await api.movePlaylistsToFolder([target.uuid], folderId);
      toasts.show(`Moved ${target.title}`, { severity: "success" });
      revision += 1;
      close();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }

  return {
    get renameTarget() {
      return renameTarget;
    },
    get removeTarget() {
      return removeTarget;
    },
    get moveTarget() {
      return moveTarget;
    },
    get busy() {
      return busy;
    },
    get revision() {
      return revision;
    },
    get folderOptions() {
      return folderOptions;
    },
    get foldersLoading() {
      return foldersLoading;
    },
    openRenamePlaylist,
    openRenameFolder,
    openRemove,
    openMove,
    close,
    rename,
    confirmRemove,
    move,
    reset,
  };
}
```

- [ ] **Step 4: Run the tests**

Run: `npm --prefix ui test -- src/lib/collectionActions.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Create the dialog host**

Create `ui/src/lib/components/CollectionDialogs.svelte`:

```svelte
<script lang="ts">
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Loading from "./Loading.svelte";
  import Modal from "./Modal.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";

  let { actions }: { actions: CollectionActions } = $props();

  let name = $state("");

  $effect(() => {
    const target = actions.renameTarget;
    name = target
      ? target.kind === "playlist"
        ? target.playlist.title
        : target.folder.name
      : "";
  });

  function removeMessage(): string {
    const target = actions.removeTarget;
    if (!target) return "";
    if (target.kind === "folder") return `Remove "${target.name}"? Its playlists move back to All playlists.`;
    if (target.owned) return `Delete "${target.title}"? This cannot be undone.`;
    return `Remove "${target.title}" from your collection?`;
  }
</script>

{#if actions.renameTarget}
  <Modal
    title={actions.renameTarget.kind === "playlist" ? "Rename playlist" : "Rename folder"}
    onclose={() => actions.close()}
  >
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void actions.rename(name);
      }}
    >
      <label>
        Name
        <input bind:value={name} aria-label="Name" />
      </label>
      <p class="actions">
        <button type="button" onclick={() => actions.close()}>Cancel</button>
        <button type="submit" disabled={actions.busy || name.trim() === ""}>Save</button>
      </p>
    </form>
  </Modal>
{/if}

{#if actions.removeTarget}
  <ConfirmDialog
    title={actions.removeTarget.kind === "folder"
      ? "Remove folder"
      : actions.removeTarget.owned
        ? "Delete playlist"
        : "Remove from collection"}
    message={removeMessage()}
    onconfirm={() => void actions.confirmRemove()}
    oncancel={() => actions.close()}
  />
{/if}

{#if actions.moveTarget}
  <Modal title={`Move ${actions.moveTarget.title}`} onclose={() => actions.close()}>
    {#if actions.foldersLoading && actions.folderOptions.length === 0}
      <Loading label="Loading folders…" />
    {/if}
    <ul class="folders">
      <li>
        <button onclick={() => void actions.move("root")}>Root</button>
      </li>
      {#each actions.folderOptions as option (option.folder.id)}
        <li style="padding-left:{option.depth * 14}px">
          <button onclick={() => void actions.move(option.folder.id)}>{option.folder.name}</button>
        </li>
      {/each}
    </ul>
    {#if actions.folderOptions.length === 0 && !actions.foldersLoading}
      <p class="muted">No folders yet.</p>
    {/if}
  </Modal>
{/if}

<style>
  label {
    display: grid;
    gap: 6px;
  }
  input {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 40vh;
    overflow: auto;
  }
  .folders button {
    width: 100%;
    text-align: left;
    padding: 7px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .folders button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
</style>
```

- [ ] **Step 6: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add ui/src/lib/collectionActions.svelte.ts ui/src/lib/collectionActions.test.ts ui/src/lib/components/CollectionDialogs.svelte
git commit -m "feat(ui): add shared collection mutation actions and dialogs"
```

---

### Task 14: Collection menu items, new icons, and the recursive playlist tree

**Files:**
- Modify: `ui/src/lib/menus.ts`
- Modify: `ui/src/lib/components/Icon.svelte`
- Create: `ui/src/lib/components/PlaylistTree.svelte`

**Interfaces:**
- Consumes: `CollectionTree` (Task 11), `CollectionActions` (Task 13), `Menu` helpers, `PlaylistCover`.
- Produces: `collectionFolderMenuItems(ctx)` and `collectionPlaylistMenuItems(ctx)`; `PlaylistTree.svelte` props `{ entries: readonly CollectionEntry[]; collection: CollectionTree; nav: Nav; actions: CollectionActions; rail?: boolean; depth?: number }`; icons `folder`, `globe`, `bell`, `arrow-up-down`, `folder-plus`.

- [ ] **Step 1: Add the menu helpers**

Append to `ui/src/lib/menus.ts` (add `CollectionFolder` to the api import):

```ts
export function collectionFolderMenuItems(ctx: {
  folder: CollectionFolder;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "rename", label: "Rename", run: () => ctx.rename() },
    { id: "remove", label: "Remove", danger: true, run: () => ctx.remove() },
  ];
}

export function collectionPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
  move(): void;
}): MenuItem[] {
  const items: MenuItem[] = [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "move", label: "Move to folder…", run: () => ctx.move() },
  ];
  if (ctx.playlist.owned) {
    items.push(
      { id: "rename", label: "Rename", run: () => ctx.rename() },
      { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
    );
  } else {
    items.push({
      id: "remove",
      label: "Remove from collection",
      danger: true,
      run: () => ctx.remove(),
    });
  }
  return items;
}
```

- [ ] **Step 2: Add the icons**

In `ui/src/lib/components/Icon.svelte`, add to `ICONS` before the closing `} as const`:

```ts
    "folder": '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />',
    "folder-plus": '<path d="M12 10v6" /> <path d="M9 13h6" /> <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />',
    "globe": '<circle cx="12" cy="12" r="10" /> <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" /> <path d="M2 12h20" />',
    "bell": '<path d="M10.268 21a2 2 0 0 0 3.464 0" /> <path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326" />',
    "arrow-up-down": '<path d="m21 16-4 4-4-4" /> <path d="M17 20V4" /> <path d="m3 8 4-4 4 4" /> <path d="M7 4v16" />',
```

- [ ] **Step 3: Create the recursive tree**

Create `ui/src/lib/components/PlaylistTree.svelte`:

```svelte
<script lang="ts">
  import type { CollectionEntry } from "../api";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import { menuFor } from "../menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../menus";
  import type { Nav } from "../nav.svelte";
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    entries,
    collection,
    nav,
    actions,
    rail = false,
  }: {
    entries: readonly CollectionEntry[];
    collection: CollectionTree;
    nav: Nav;
    actions: CollectionActions;
    rail?: boolean;
  } = $props();

  function openFolder(id: string, label: string) {
    nav.goto({ name: "folder", id, label });
  }

  function openPlaylist(id: string) {
    nav.goto({ name: "playlist", id });
  }
</script>

{#each entries as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
  {#if entry.kind === "folder"}
    <button
      class="row"
      class:active={nav.route.name === "folder" && nav.route.id === entry.id}
      onclick={() => (rail ? nav.goto({ name: "playlists" }) : void collection.toggleFolder(entry.id))}
      use:menuFor={() =>
        collectionFolderMenuItems({
          folder: entry,
          open: () => openFolder(entry.id, entry.name),
          rename: () => actions.openRenameFolder(entry),
          remove: () => actions.openRemove(entry),
        })}
      title={entry.name}
    >
      <span class="thumb folder"><Icon name="folder" size={rail ? 20 : 16} /></span>
      {#if !rail}
        <span class="label">{entry.name}</span>
        <Icon name={collection.isExpanded(entry.id) ? "chevron-down" : "chevron-right"} size={14} />
      {/if}
    </button>
    {#if !rail && collection.isExpanded(entry.id)}
      {#if collection.isLoading(entry.id)}
        <div class="tree-note"><Loading /></div>
      {:else if collection.children(entry.id)?.length === 0}
        <p class="muted tree-note">No playlists</p>
      {:else if collection.children(entry.id) !== null}
        <div class="nested">
          <PlaylistTree
            entries={collection.children(entry.id) ?? []}
            {collection}
            {nav}
            {actions}
            {rail}
          />
        </div>
      {/if}
    {/if}
  {:else}
    <button
      class="row"
      class:active={nav.route.name === "playlist" && nav.route.id === entry.uuid}
      onclick={() => openPlaylist(entry.uuid)}
      use:menuFor={() =>
        collectionPlaylistMenuItems({
          playlist: entry,
          open: () => openPlaylist(entry.uuid),
          rename: () => actions.openRenamePlaylist(entry),
          remove: () => actions.openRemove(entry),
          move: () => actions.openMove(entry),
        })}
      title={entry.title}
    >
      <span class="thumb">
        <PlaylistCover
          owned={entry.owned}
          playlist={entry.uuid}
          image={entry.squareImage}
          size={80}
          radius={6}
          alt=""
        />
      </span>
      {#if !rail}<span class="label">{entry.title}</span>{/if}
    </button>
  {/if}
{/each}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 5px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .row:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .row.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }
  .thumb {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    overflow: hidden;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nested {
    margin-left: 12px;
  }
  .tree-note {
    padding-left: 44px;
  }
</style>
```

- [ ] **Step 4: Run the UI gate**

Run: `npm --prefix ui run check`
Expected: PASS. (The component is not imported yet; this only checks syntax/types.)

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/menus.ts ui/src/lib/components/Icon.svelte ui/src/lib/components/PlaylistTree.svelte
git commit -m "feat(ui): add collection menu items, icons, and the playlist tree"
```

---

### Task 15: Explore view

**Files:**
- Create: `ui/src/views/Explore.svelte`
- Modify: `ui/src/App.svelte` (import, route branch, `focusSearch`)

**Interfaces:**
- Consumes: `api.search`, `api.getExplore`, `createSearch`, `createTrackView`, `PageSections`, `playMix`, `TrackTable`.
- Produces: `Explore.svelte` props `{ nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts }`, owning `#search-input`.

- [ ] **Step 1: Create the view**

Create `ui/src/views/Explore.svelte` with the search markup moved from `Search.svelte` plus explore sections:

```svelte
<script lang="ts">
  import { api, trackToQueueItem, type PageSection } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { playMix } from "../lib/mixPlay";
  import { createSearch } from "../lib/search.svelte";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  const search = createSearch(api.search);
  const resultsView = createTrackView(() => search.results?.tracks.items ?? []);

  let sections = $state<PageSection[]>([]);
  let exploreLoading = $state(true);
  let exploreError = $state("");

  async function loadExplore() {
    exploreLoading = true;
    exploreError = "";
    try {
      sections = await api.getExplore();
    } catch (e) {
      exploreError = String(e);
    } finally {
      exploreLoading = false;
    }
  }

  $effect(() => {
    loadExplore();
  });

  const searching = $derived(search.query.trim().length > 0);

  const noResults = $derived(
    search.results !== null &&
      search.results.tracks.items.length === 0 &&
      search.results.albums.items.length === 0 &&
      search.results.artists.items.length === 0 &&
      search.results.playlists.items.length === 0,
  );

  function playAll() {
    if (resultsView.items.length === 0) return;
    void api.playTracks(resultsView.items.map(trackToQueueItem), 0);
  }

  function shuffleAll() {
    if (resultsView.items.length === 0) return;
    void api.playTracks(resultsView.items.map(trackToQueueItem), 0, true);
  }
</script>

<h2>Explore</h2>
<div class="search-bar">
  <input
    id="search-input"
    value={search.query}
    oninput={(event) => search.setQuery(event.currentTarget.value)}
    onkeydown={(event) => event.key === "Enter" && search.runNow()}
    placeholder="Search TIDAL"
  />
  {#if search.loading && search.results}
    <Loading label="Searching…" />
  {/if}
</div>

{#if searching}
  {#if search.loading && !search.results && !search.error}
    <Loading label="Searching…" />
  {:else if search.error}
    <ErrorState message={search.error} onRetry={() => search.retry()} />
  {:else if noResults}
    <EmptyState message={`No results for “${search.settledQuery}”`} />
  {:else if search.results}
    <div class="list-head">
      <h3>Tracks</h3>
      <button
        class="list-action"
        onclick={shuffleAll}
        disabled={resultsView.items.length === 0}
        aria-label="Shuffle"
        title="Shuffle"
      ><Icon name="shuffle" size={16} /></button>
      <button
        class="list-action"
        onclick={playAll}
        disabled={resultsView.items.length === 0}
        aria-label="Play all"
        title="Play all"
      ><Icon name="play" size={16} /></button>
    </div>
    <TrackTable
      view={resultsView}
      {favorites}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
      playingTrackId={player.state.current?.track_id ?? null}
    />
    <h3>Albums</h3>
    <ul class="results">
      {#each search.results.albums.items as a (a.id)}
        <li>
          <span class="res-thumb"><Cover uuid={a.cover} size={80} radius={6} alt="" /></span>
          <button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button>
        </li>
      {/each}
    </ul>
    <h3>Artists</h3>
    <ul class="results">
      {#each search.results.artists.items as a (a.id)}
        <li>
          <span class="res-thumb round"><Cover uuid={a.picture} size={80} radius={50} alt="" /></span>
          <button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button>
        </li>
      {/each}
    </ul>
    <h3>Playlists</h3>
    <ul class="results">
      {#each search.results.playlists.items as p (p.uuid)}
        <li>
          <span class="res-thumb">
            <PlaylistCover
              owned={p.owned}
              playlist={p.uuid}
              image={p.squareImage}
              size={80}
              radius={6}
              alt=""
            />
          </span>
          <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
        </li>
      {/each}
    </ul>
  {/if}
{:else if exploreLoading}
  <Loading />
{:else if exploreError}
  <ErrorState message={exploreError} onRetry={loadExplore} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to explore yet." />
{:else}
  <PageSections {sections} {nav} onPlayMix={(id) => void playMix(id, toasts)} />
{/if}

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .list-head h3 {
    flex: 1;
    min-width: 0;
  }
  .list-action {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .list-action:hover {
    color: var(--text);
  }
  .results {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
  }
  .results li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 0;
  }
  .res-thumb {
    flex: 0 0 auto;
    width: var(--thumb-size);
  }
</style>
```

- [ ] **Step 2: Wire it into App**

In `ui/src/App.svelte`:
- Add `import Explore from "./views/Explore.svelte";` next to the Search import.
- Add the branch before the search branch:

```svelte
      {:else if nav.route.name === "explore"}<Explore {nav} {player} {favorites} {playlistActions} {toasts} />
```

- Change `focusSearch` to:

```ts
  async function focusSearch() {
    if (nav.route.name !== "explore") nav.goto({ name: "explore" });
    await tick();
    document.getElementById("search-input")?.focus();
  }
```

- [ ] **Step 3: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add ui/src/views/Explore.svelte ui/src/App.svelte
git commit -m "feat(ui): add the Explore page and route search shortcut to it"
```

---

### Task 16: Feed view

**Files:**
- Create: `ui/src/views/Feed.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `api.getFeed`, `PageSections`, `playMix`.
- Produces: `Feed.svelte` props `{ nav: Nav; toasts: Toasts }`.

- [ ] **Step 1: Create the view**

Create `ui/src/views/Feed.svelte`:

```svelte
<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import { playMix } from "../lib/mixPlay";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let sections = $state<PageSection[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      sections = await api.getFeed();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<h2>Feed</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Feed isn't available for this account." />
{:else}
  <PageSections {sections} {nav} onPlayMix={(id) => void playMix(id, toasts)} />
{/if}
```

- [ ] **Step 2: Wire it into App**

Add `import Feed from "./views/Feed.svelte";` and the branch:

```svelte
      {:else if nav.route.name === "feed"}<Feed {nav} {toasts} />
```

- [ ] **Step 3: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add ui/src/views/Feed.svelte ui/src/App.svelte
git commit -m "feat(ui): add the Feed page"
```

---

### Task 17: Playlists and folder pages

**Files:**
- Create: `ui/src/lib/collectionSort.ts`
- Create: `ui/src/lib/collectionSort.test.ts`
- Create: `ui/src/views/Playlists.svelte`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/views/Playlist.svelte:113`

**Interfaces:**
- Consumes: `api.getCollectionItems`, `api.createFolder`, `CollectionTree`, `CollectionActions`, menu helpers, `PlaylistCover`.
- Produces: `sortEntries(entries, mode)`; `Playlists.svelte` props `{ nav, playlistActions, toasts, collection, actions, folderId?: string | null, folderLabel?: string }`.

- [ ] **Step 1: Add the `label` field to the folder route**

In `ui/src/lib/nav.svelte.ts`, change the folder route to `| { name: "folder"; id: string; label?: string }` (route identity still compares only `id` via `sameRoute`).

- [ ] **Step 2: Write the failing sort test**

Create `ui/src/lib/collectionSort.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { CollectionEntry } from "./api";
import { sortEntries } from "./collectionSort";

function folder(id: string, name: string): CollectionEntry {
  return { kind: "folder", id, name, totalNumberOfItems: 1 };
}

function playlist(uuid: string, title: string): CollectionEntry {
  return {
    kind: "playlist",
    uuid,
    title,
    description: null,
    numberOfTracks: 0,
    duration: 0,
    image: null,
    owned: true,
  };
}

describe("sortEntries", () => {
  it("keeps API order for date mode", () => {
    const entries = [playlist("b", "Beta"), folder("f", "Zeta")];
    expect(sortEntries(entries, "date")).toEqual(entries);
  });

  it("sorts folders first then playlists by name", () => {
    const entries = [
      playlist("z", "zebra"),
      folder("a", "alpha"),
      playlist("a", "Apple"),
      folder("z", "Zulu"),
    ];
    expect(sortEntries(entries, "name").map((e) => (e.kind === "folder" ? e.name : e.title))).toEqual([
      "alpha",
      "Zulu",
      "Apple",
      "zebra",
    ]);
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/collectionSort.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 4: Implement**

Create `ui/src/lib/collectionSort.ts`:

```ts
import type { CollectionEntry } from "./api";

export type CollectionSort = "date" | "name";

/// "date" keeps the API's newest-first order; "name" groups folders first,
/// then playlists, each alphabetically and case-insensitively.
export function sortEntries(
  entries: readonly CollectionEntry[],
  mode: CollectionSort,
): CollectionEntry[] {
  if (mode === "date") return [...entries];
  const nameOf = (entry: CollectionEntry) => (entry.kind === "folder" ? entry.name : entry.title);
  const byName = (a: CollectionEntry, b: CollectionEntry) =>
    nameOf(a).localeCompare(nameOf(b), undefined, { sensitivity: "base" });
  const folders = entries.filter((entry) => entry.kind === "folder").sort(byName);
  const playlists = entries.filter((entry) => entry.kind === "playlist").sort(byName);
  return [...folders, ...playlists];
}
```

- [ ] **Step 5: Create the view**

Create `ui/src/views/Playlists.svelte`:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, type CollectionEntry } from "../lib/api";
  import { sortEntries, type CollectionSort } from "../lib/collectionSort";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import Modal from "../lib/components/Modal.svelte";
  import PlaylistCover from "../lib/components/PlaylistCover.svelte";
  import type { CollectionActions } from "../lib/collectionActions.svelte";
  import type { CollectionTree } from "../lib/collection.svelte";
  import { errorMessage } from "../lib/errors";
  import { menuFor } from "../lib/menu.svelte";
  import { collectionFolderMenuItems, collectionPlaylistMenuItems } from "../lib/menus";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    nav,
    playlistActions,
    toasts,
    collection,
    actions,
    folderId = null,
    folderLabel = "",
  }: {
    nav: Nav;
    playlistActions: PlaylistActions;
    toasts: Toasts;
    collection: CollectionTree;
    actions: CollectionActions;
    folderId?: string | null;
    folderLabel?: string;
  } = $props();

  let entries = $state<CollectionEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let sortMode = $state<CollectionSort>("date");
  let creatingFolder = $state(false);
  let folderName = $state("");
  let savingFolder = $state(false);
  let actionsSeen = untrack(() => actions.revision);
  let createSeen = untrack(() => playlistActions.revision);

  const rows = $derived(sortEntries(entries, sortMode));

  async function load() {
    loading = true;
    error = "";
    try {
      entries = await api.getCollectionItems(folderId ?? "root");
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    folderId;
    void load();
  });

  $effect(() => {
    const revision = actions.revision;
    if (revision === actionsSeen) return;
    actionsSeen = revision;
    untrack(() => void load());
  });

  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === createSeen) return;
    createSeen = revision;
    untrack(() => void load());
  });

  function closeCreateFolder() {
    creatingFolder = false;
    folderName = "";
  }

  async function submitFolder() {
    const name = folderName.trim();
    if (name.length === 0 || savingFolder) return;
    savingFolder = true;
    try {
      await api.createFolder(name, folderId ?? "root");
      toasts.show(`Created ${name}`, { severity: "success" });
      closeCreateFolder();
      await load();
      await collection.refresh();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      savingFolder = false;
    }
  }
</script>

<div class="page-head">
  <h2>{folderId ? folderLabel || "Folder" : "All playlists"}</h2>
  <button class="round" onclick={() => playlistActions.openCreate()} title="New playlist" aria-label="New playlist">
    <Icon name="plus" size={16} />
  </button>
  <button class="round" onclick={() => (creatingFolder = true)} title="New folder" aria-label="New folder">
    <Icon name="folder-plus" size={16} />
  </button>
  <button
    class="round"
    onclick={() => (sortMode = sortMode === "name" ? "date" : "name")}
    title={sortMode === "name" ? "Sort by date added" : "Sort by name"}
    aria-label="Toggle sort"
  >
    <Icon name="arrow-up-down" size={16} />
  </button>
</div>
{#if folderId}
  <p class="crumbs"><button onclick={() => nav.goto({ name: "playlists" })}>All playlists</button></p>
{/if}

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if rows.length === 0}
  <EmptyState message={folderId ? "This folder is empty." : "No playlists yet."} />
{:else}
  <div role="list">
    {#each rows as entry (entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`)}
      <div
        class="row"
        role="listitem"
        use:menuFor={() =>
          entry.kind === "folder"
            ? collectionFolderMenuItems({
                folder: entry,
                open: () => nav.goto({ name: "folder", id: entry.id, label: entry.name }),
                rename: () => actions.openRenameFolder(entry),
                remove: () => actions.openRemove(entry),
              })
            : collectionPlaylistMenuItems({
                playlist: entry,
                open: () => nav.goto({ name: "playlist", id: entry.uuid }),
                rename: () => actions.openRenamePlaylist(entry),
                remove: () => actions.openRemove(entry),
                move: () => actions.openMove(entry),
              })}
      >
        {#if entry.kind === "folder"}
          <button
            class="row-main"
            onclick={() => nav.goto({ name: "folder", id: entry.id, label: entry.name })}
          >
            <span class="tile folder"><Icon name="folder" size={20} /></span>
            <span class="text">
              <span class="title">{entry.name}</span>
              <span class="muted sub">
                {entry.totalNumberOfItems}
                {entry.totalNumberOfItems === 1 ? "playlist" : "playlists"}
              </span>
            </span>
          </button>
        {:else}
          <button class="row-main" onclick={() => nav.goto({ name: "playlist", id: entry.uuid })}>
            <span class="tile">
              <PlaylistCover
                owned={entry.owned}
                playlist={entry.uuid}
                image={entry.squareImage}
                size={80}
                radius={6}
                alt=""
              />
            </span>
            <span class="text">
              <span class="title">{entry.title}</span>
              <span class="muted sub">{entry.numberOfTracks} items</span>
            </span>
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

{#if creatingFolder}
  <Modal title="New folder" onclose={closeCreateFolder}>
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void submitFolder();
      }}
    >
      <label>
        Name
        <input bind:value={folderName} aria-label="Folder name" />
      </label>
      <p class="dialog-actions">
        <button type="button" onclick={closeCreateFolder}>Cancel</button>
        <button type="submit" disabled={savingFolder || folderName.trim() === ""}>Create</button>
      </p>
    </form>
  </Modal>
{/if}

<style>
  .page-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .page-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .round {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .round:hover {
    color: var(--text);
  }
  .crumbs {
    margin: 4px 0 12px;
  }
  .crumbs button {
    color: var(--text-muted);
  }
  .crumbs button:hover {
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
  }
  .row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 6px 4px;
    border-radius: var(--radius-control);
    text-align: left;
  }
  .row-main:hover {
    background: var(--surface-2);
  }
  .tile {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
  }
  label {
    display: grid;
    gap: 6px;
  }
  input {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>
```

- [ ] **Step 6: Wire it into App**

Add imports `import Playlists from "./views/Playlists.svelte";` and the branches:

```svelte
      {:else if nav.route.name === "playlists"}<Playlists {nav} {playlistActions} {toasts} {collection} actions={collectionActions} />
      {:else if nav.route.name === "folder"}<Playlists {nav} {playlistActions} {toasts} {collection} actions={collectionActions} folderId={nav.route.id} folderLabel={nav.route.label ?? ""} />
```

The `collection` and `actions` instances are created in Task 20; for this task create them locally in App next to `toasts`:

```ts
  const collection = createCollectionTree(toasts);
  const collectionActions = createCollectionActions(collection, toasts);
```

with imports `import { createCollectionTree } from "./lib/collection.svelte";` and `import { createCollectionActions } from "./lib/collectionActions.svelte";`. In the branches use `actions={collectionActions}`.

- [ ] **Step 7: Update the post-delete navigation**

In `ui/src/views/Playlist.svelte:113` change `nav.goto({ name: "library" })` to `nav.goto({ name: "playlists" })`.

- [ ] **Step 8: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add ui/src/lib/collectionSort.ts ui/src/lib/collectionSort.test.ts ui/src/views/Playlists.svelte ui/src/App.svelte ui/src/lib/nav.svelte.ts ui/src/views/Playlist.svelte
git commit -m "feat(ui): add the playlists and folder pages with sorting"
```

---

### Task 18: Albums, Tracks, and Profiles pages

**Files:**
- Create: `ui/src/views/CollectionAlbums.svelte`
- Create: `ui/src/views/CollectionTracks.svelte`
- Create: `ui/src/views/Profiles.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `api.getFavoriteAlbums`, `api.getFavoriteArtists`, `createTrackPager`, `createTrackView`, `TrackTable`, `Cover`.
- Produces: `CollectionAlbums.svelte` `{ nav }`; `Profiles.svelte` `{ nav }`; `CollectionTracks.svelte` `{ player, favorites, playlistActions }`.

- [ ] **Step 1: Create the Albums page**

Create `ui/src/views/CollectionAlbums.svelte`:

```svelte
<script lang="ts">
  import { api, type Album } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let albums = $state<Album[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      albums = await api.getFavoriteAlbums();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Albums</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if albums.length === 0}
  <EmptyState message="No albums yet." />
{:else}
  <div class="grid">
    {#each albums as album (album.id)}
      <button class="card" onclick={() => nav.goto({ name: "album", id: album.id })}>
        <span class="art"><Cover uuid={album.cover} size={320} radius={12} alt="" /></span>
        <span class="title">{album.title}</span>
        <span class="muted sub">{album.artists.map((a) => a.name).join(", ")}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    display: block;
    overflow: hidden;
    border-radius: var(--radius-card);
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

- [ ] **Step 2: Create the Profiles page**

Create `ui/src/views/Profiles.svelte`:

```svelte
<script lang="ts">
  import { api, type Artist } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let artists = $state<Artist[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      artists = await api.getFavoriteArtists();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Profiles</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artists.length === 0}
  <EmptyState message="No artists yet." />
{:else}
  <div class="grid">
    {#each artists as artist (artist.id)}
      <button class="card" onclick={() => nav.goto({ name: "artist", id: artist.id })}>
        <span class="art"><Cover uuid={artist.picture} size={320} radius={160} alt="" /></span>
        <span class="title">{artist.name}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 16px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    display: block;
    width: 100%;
    overflow: hidden;
    border-radius: 50%;
  }
  .title {
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

- [ ] **Step 3: Create the Tracks page**

Create `ui/src/views/CollectionTracks.svelte` by moving the favorite-tracks machinery out of `Library.svelte` verbatim (lines 36–56 for state and pager, 78–103 for the mount/prefetch and unfavorite-drop effects, 137–156 for play/shuffle) into:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Track } from "../lib/api";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import { createTrackView, visibleIndexOf } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";

  let {
    player,
    favorites,
    playlistActions,
  }: { player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  async function ensureAllFavorites() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) throw favoritesPager.error;
  }

  const view = createTrackView(() => favoritesPager.items, ensureAllFavorites);

  $effect(() => {
    untrack(() => favoritesPager.loadMore());
  });

  $effect(() => {
    favorites.trackRevision;
    const loaded = new Set(favoritesPager.items.map((t) => t.id));
    for (const id of favorites.pendingRemovalsToDrop(loaded)) favoritesPager.removeItem(id);
  });

  async function playFrom(index: number) {
    const clicked = view.items[index];
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    const start = visibleIndexOf(view.items, clicked, index);
    if (start < 0) return;
    api.playTracks(view.items.map(trackToQueueItem), start);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0);
  }

  async function shuffleAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(view.items.map(trackToQueueItem), 0, true);
  }
</script>

<div class="view-fill">
  <div class="list-head">
    <h2>Tracks</h2>
    <button
      class="list-action"
      onclick={shuffleAll}
      disabled={view.items.length === 0}
      aria-label="Shuffle"
      title="Shuffle"
    ><Icon name="shuffle" size={16} /></button>
    <button
      class="list-action"
      onclick={playAll}
      disabled={view.items.length === 0}
      aria-label="Play all"
      title="Play all"
    ><Icon name="play" size={16} /></button>
  </div>
  <p>
    <button onclick={() => favoritesPager.refresh()}>Refresh</button>
    <span class="muted">
      {#if favoritesPager.unavailable > 0}
        {favoritesPager.items.length} tracks ({favoritesPager.unavailable} unavailable)
      {:else}
        {favoritesPager.items.length} tracks
      {/if}
    </span>
    {#if favoritesPager.loading}<Loading />{/if}
  </p>
  <TrackTable
    {view}
    {favorites}
    fill
    onPlay={playFrom}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playingTrackId={player.state.current?.track_id ?? null}
  />
  {#if favoritesPager.error}
    <ErrorState message={favoritesPager.error} onRetry={() => favoritesPager.loadMore()} />
  {/if}
</div>

<style>
  .list-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .list-head h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .list-action {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .list-action:hover {
    color: var(--text);
  }
</style>
```

Remove the now-unused `Track` import if svelte-check flags it.

- [ ] **Step 4: Wire the pages into App**

Add the imports:

```ts
  import CollectionAlbums from "./views/CollectionAlbums.svelte";
  import CollectionTracks from "./views/CollectionTracks.svelte";
  import Profiles from "./views/Profiles.svelte";
```

and the branches:

```svelte
      {:else if nav.route.name === "albums"}<CollectionAlbums {nav} />
      {:else if nav.route.name === "tracks"}<CollectionTracks {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "profiles"}<Profiles {nav} />
```

- [ ] **Step 5: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add ui/src/views/CollectionAlbums.svelte ui/src/views/CollectionTracks.svelte ui/src/views/Profiles.svelte ui/src/App.svelte
git commit -m "feat(ui): add albums, tracks, and profiles collection pages"
```

---

### Task 19: Mixes, Videos, and Purchases pages

**Files:**
- Create: `ui/src/views/Mixes.svelte`
- Create: `ui/src/views/Videos.svelte`
- Create: `ui/src/views/Purchases.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `api.getFavoriteMixes`, `api.getFavoriteVideos`, `api.getPurchases`, `playMix`, `formatDuration`.
- Produces: `Mixes.svelte` `{ toasts }`; `Videos.svelte` `{}`; `Purchases.svelte` `{ nav }`.

- [ ] **Step 1: Create the Mixes page**

Create `ui/src/views/Mixes.svelte`:

```svelte
<script lang="ts">
  import { api, type Mix } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { playMix } from "../lib/mixPlay";
  import type { Toasts } from "../lib/toasts.svelte";

  let { toasts }: { toasts: Toasts } = $props();

  let mixes = $state<Mix[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      mixes = await api.getFavoriteMixes();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Mixes &amp; Radio</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if mixes.length === 0}
  <EmptyState message="No mixes yet." />
{:else}
  <div class="grid">
    {#each mixes as mix (mix.id)}
      <button class="card" onclick={() => void playMix(mix.id, toasts)} title={`Play ${mix.title}`}>
        <span class="art">
          {#if mix.image}
            <img src={mix.image} alt="" loading="lazy" />
          {:else}
            <span class="placeholder"><Icon name="disc-3" size={28} /></span>
          {/if}
        </span>
        <span class="title">{mix.title}</span>
        {#if mix.subTitle}<span class="muted sub">{mix.subTitle}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
  }
  .card:hover {
    background: var(--surface-2);
  }
  .art {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

- [ ] **Step 2: Create the Videos page**

Create `ui/src/views/Videos.svelte`:

```svelte
<script lang="ts">
  import { api, type Video } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import { formatDuration } from "../lib/format";

  let videos = $state<Video[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      videos = await api.getFavoriteVideos();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });
</script>

<h2>Videos</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if videos.length === 0}
  <EmptyState message="No videos yet." />
{:else}
  <ul class="videos">
    {#each videos as video (video.id)}
      <li>
        <span class="thumb"><Cover uuid={video.image} size={160} radius={6} alt="" /></span>
        <span class="text">
          <span class="title">{video.title}</span>
          <span class="muted sub">
            {video.artists.map((a) => a.name).join(", ")} · {formatDuration(video.duration)}
          </span>
        </span>
      </li>
    {/each}
  </ul>
  <p class="muted note">Video playback isn't supported yet — these are your saved videos.</p>
{/if}

<style>
  .videos {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .videos li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .thumb {
    flex: 0 0 auto;
    width: 72px;
    overflow: hidden;
    border-radius: 6px;
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
  }
  .note {
    margin-top: 16px;
  }
</style>
```

- [ ] **Step 3: Create the Purchases page**

Create `ui/src/views/Purchases.svelte`:

```svelte
<script lang="ts">
  import { api, type PageItem } from "../lib/api";
  import Cover from "../lib/components/Cover.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Route } from "../lib/nav.svelte";
  import type { Nav } from "../lib/nav.svelte";

  let { nav }: { nav: Nav } = $props();

  let items = $state<PageItem[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      items = await api.getPurchases();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load();
  });

  function routeFor(item: PageItem): Route | null {
    if (item.item_type.toUpperCase().includes("ALBUM") && /^\d+$/.test(item.id)) {
      return { name: "album", id: Number(item.id) };
    }
    return null;
  }
</script>

<h2>Purchases</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if items.length === 0}
  <EmptyState message="Nothing purchased on this account." />
{:else}
  <ul class="purchases">
    {#each items as item (item.id || item.title)}
      {@const route = routeFor(item)}
      <li>
        <span class="thumb"><Cover uuid={item.artwork} size={80} radius={6} alt="" /></span>
        {#if route}
          <button onclick={() => nav.goto(route)}>{item.title}</button>
        {:else}
          <span>{item.title}</span>
        {/if}
        <span class="muted sub">{item.item_type}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .purchases {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .purchases li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .thumb {
    flex: 0 0 auto;
    width: 40px;
  }
  .sub {
    margin-left: auto;
    font-size: 11.5px;
  }
</style>
```

- [ ] **Step 4: Wire the pages into App**

Add the imports:

```ts
  import Mixes from "./views/Mixes.svelte";
  import Videos from "./views/Videos.svelte";
  import Purchases from "./views/Purchases.svelte";
```

and the branches:

```svelte
      {:else if nav.route.name === "mixes"}<Mixes {toasts} />
      {:else if nav.route.name === "videos"}<Videos />
      {:else if nav.route.name === "purchases"}<Purchases {nav} />
```

- [ ] **Step 5: Run the UI gate**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add ui/src/views/Mixes.svelte ui/src/views/Videos.svelte ui/src/views/Purchases.svelte ui/src/App.svelte
git commit -m "feat(ui): add mixes, videos, and purchases pages"
```

---

### Task 20: Sidebar rewrite, final wiring, and smoke checklist

**Files:**
- Rewrite: `ui/src/lib/components/Sidebar.svelte`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/lib/nav.svelte.ts` (drop `search`/`library`)
- Delete: `ui/src/views/Search.svelte`, `ui/src/views/Library.svelte`
- Create: `docs/superpowers/plans/notes/2026-09-28-sidebar-collection-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: sidebar props `{ nav, rail, onToggleRail, onLogout, collection, playlistActions, actions }`; final `Route` union without `search`/`library`.

- [ ] **Step 1: Rewrite the sidebar**

Replace `ui/src/lib/components/Sidebar.svelte` with:

```svelte
<script lang="ts">
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import type { CollectionActions } from "../collectionActions.svelte";
  import type { CollectionTree } from "../collection.svelte";
  import type { PlaylistActions } from "../playlists.svelte";
  import Icon from "./Icon.svelte";
  import PlaylistTree from "./PlaylistTree.svelte";

  let {
    nav,
    rail,
    onToggleRail,
    onLogout,
    collection,
    playlistActions,
    actions,
  }: {
    nav: Nav;
    rail: boolean;
    onToggleRail: () => void;
    onLogout: () => void;
    collection: CollectionTree;
    playlistActions: PlaylistActions;
    actions: CollectionActions;
  } = $props();

  let collectionOpen = $state(true);
  let showMore = $state(false);

  const collectionItems: { route: Route; label: string; icon: "list-music" | "disc-3" | "music" }[] = [
    { route: { name: "playlists" }, label: "Playlists", icon: "list-music" },
    { route: { name: "albums" }, label: "Albums", icon: "disc-3" },
    { route: { name: "tracks" }, label: "Tracks", icon: "music" },
  ];

  const moreItems: {
    route: Route;
    label: string;
    icon: "disc-3" | "play" | "heart" | "copy";
  }[] = [
    { route: { name: "mixes" }, label: "Mixes & Radio", icon: "disc-3" },
    { route: { name: "videos" }, label: "Videos", icon: "play" },
    { route: { name: "profiles" }, label: "Profiles", icon: "heart" },
    { route: { name: "purchases" }, label: "Purchases", icon: "copy" },
  ];

  function isActive(name: string): boolean {
    if (name === "playlists") {
      return nav.route.name === "playlists" || nav.route.name === "folder";
    }
    return nav.route.name === name;
  }
</script>

<nav class="sidebar" class:rail>
  <div class="top">
    <h1 class="wordmark">{rail ? "O" : "OxiTide"}</h1>
    <button
      class="collapse"
      onclick={onToggleRail}
      aria-label={rail ? "Expand sidebar" : "Collapse sidebar"}
      title={rail ? "Expand sidebar" : "Collapse sidebar"}
    ><Icon name={rail ? "panel-left-open" : "panel-left-close"} size={16} /></button>
  </div>

  {#if !rail}
    <div class="history">
      <button disabled={!nav.canBack} onclick={() => nav.back()} title="Back (Alt+←)" aria-label="Back"><Icon name="chevron-left" /></button>
      <button disabled={!nav.canForward} onclick={() => nav.forward()} title="Forward (Alt+→)" aria-label="Forward"><Icon name="chevron-right" /></button>
    </div>
  {/if}

  <button class="nav-item" class:active={nav.route.name === "home"} onclick={() => nav.goto({ name: "home" })} title="Home" aria-label="Home">
    <Icon name="home" />{#if !rail}<span>Home</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "explore"} onclick={() => nav.goto({ name: "explore" })} title="Explore" aria-label="Explore">
    <Icon name="globe" />{#if !rail}<span>Explore</span>{/if}
  </button>
  <button class="nav-item" class:active={nav.route.name === "feed"} onclick={() => nav.goto({ name: "feed" })} title="Feed" aria-label="Feed">
    <Icon name="bell" />{#if !rail}<span>Feed</span>{/if}
  </button>

  <div class="divider" role="separator"></div>

  {#if rail}
    <button class="nav-item" class:active={isActive("playlists")} onclick={() => nav.goto({ name: "playlists" })} title="My Collection" aria-label="My Collection">
      <Icon name="library" />
    </button>
  {:else}
    <button class="section-head" onclick={() => (collectionOpen = !collectionOpen)} aria-expanded={collectionOpen}>
      <span>My Collection</span>
      <Icon name={collectionOpen ? "chevron-down" : "chevron-right"} size={14} />
    </button>
  {/if}

  {#if collectionOpen || rail}
    {#each collectionItems as item (item.label)}
      <button class="nav-item" class:active={isActive(item.route.name)} onclick={() => nav.goto(item.route)} title={item.label} aria-label={item.label}>
        <Icon name={item.icon} />{#if !rail}<span>{item.label}</span>{/if}
      </button>
    {/each}
    {#if rail}
      <button class="nav-item" onclick={() => (showMore = !showMore)} aria-expanded={showMore} title="Show more" aria-label="Show more">
        <Icon name={showMore ? "chevron-up" : "chevron-down"} />
      </button>
    {:else}
      <button class="section-head sub" onclick={() => (showMore = !showMore)} aria-expanded={showMore}>
        <span>{showMore ? "Show less" : "Show more"}</span>
        <Icon name={showMore ? "chevron-up" : "chevron-down"} size={14} />
      </button>
    {/if}
    {#if showMore}
      {#each moreItems as item (item.label)}
        <button class="nav-item" class:active={isActive(item.route.name)} onclick={() => nav.goto(item.route)} title={item.label} aria-label={item.label}>
          <Icon name={item.icon} />{#if !rail}<span>{item.label}</span>{/if}
        </button>
      {/each}
    {/if}
  {/if}

  <div class="divider" role="separator"></div>

  <div class="playlist-head">
    {#if !rail}<span class="playlist-title">Playlists</span>{/if}
    <button class="mini" onclick={() => playlistActions.openCreate()} title="New playlist" aria-label="New playlist">
      <Icon name="plus" size={14} />
    </button>
  </div>
  <div class="tree">
    {#if collection.error}
      <p class="muted tree-error">{collection.error}</p>
    {:else}
      <PlaylistTree entries={collection.root} {collection} {nav} {actions} {rail} />
    {/if}
  </div>

  <div class="spacer"></div>

  <button class="nav-item" class:active={nav.route.name === "settings"} onclick={() => nav.goto({ name: "settings" })} title="Settings" aria-label="Settings">
    <Icon name="settings" />{#if !rail}<span>Settings</span>{/if}
  </button>
  <button class="nav-item" onclick={onLogout} title="Log out" aria-label="Log out">
    <Icon name="log-out" />{#if !rail}<span>Log out</span>{/if}
  </button>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .wordmark {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 640;
    margin: 0;
    letter-spacing: -0.02em;
    transition: opacity var(--dur-micro) var(--ease-in);
  }
  .history {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 8px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
    text-align: left;
  }
  .nav-item:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav-item.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }
  .divider {
    height: 1px;
    background: var(--line);
    margin: 8px 4px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 7px 8px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .section-head:hover {
    color: var(--text);
  }
  .section-head.sub {
    font-weight: 500;
    text-transform: none;
    letter-spacing: normal;
    padding-left: 32px;
  }
  .playlist-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 8px 3px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .mini:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .tree {
    min-height: 0;
  }
  .tree-error {
    padding: 0 8px;
    font-size: 11.5px;
  }
  .spacer {
    flex: 1;
    min-height: 12px;
  }
  .rail .nav-item {
    justify-content: center;
    padding: 9px 0;
  }
  .rail .top {
    position: relative;
    justify-content: center;
  }
  .rail .collapse {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    color: var(--text-muted);
  }
  .rail .collapse:hover,
  .rail .collapse:focus-visible {
    opacity: 1;
    color: var(--text);
  }
  .rail .top:has(.collapse:hover, .collapse:focus-visible) .wordmark {
    opacity: 0;
  }
  .rail .playlist-head {
    justify-content: center;
    padding: 4px 0;
  }
  .rail .tree :global(.row) {
    justify-content: center;
  }
</style>
```

- [ ] **Step 2: Finalize App wiring**

In `ui/src/App.svelte`:
- Delete the `Search` and `Library` imports; delete their route branches.
- Pass the collection instances to the sidebar:

```svelte
    <Sidebar
      {nav}
      rail={sidebarRail}
      onToggleRail={() => { railTouched = true; sidebarRail = !sidebarRail; }}
      onLogout={onLogout}
      {collection}
      {playlistActions}
      actions={collectionActions}
    />
```

- Add the load effect next to the favorites hydrate effect:

```ts
  $effect(() => {
    if (loggedIn) untrack(() => collection.ensureRoot());
  });
```

- In `onLogout`, after `playlistActions.reset()`, add:

```ts
    collection.reset();
    collectionActions.reset();
```

- In the `session_expired` listener, after `playlistActions.reset()`, add the same two lines.
- Render the dialog host next to `PlaylistPicker`:

```svelte
    <CollectionDialogs actions={collectionActions} />
```

with `import CollectionDialogs from "./lib/components/CollectionDialogs.svelte";`.

- [ ] **Step 3: Drop the retired routes**

In `ui/src/lib/nav.svelte.ts`, remove `| { name: "search" }` and `| { name: "library" }` from the `Route` union. Delete the two view files:

```powershell
Remove-Item ui/src/views/Search.svelte, ui/src/views/Library.svelte
```

- [ ] **Step 4: Run all gates**

Run: `npm --prefix ui run check && npm --prefix ui test && cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-28-sidebar-collection-smoke.md`:

```markdown
# Sidebar / My Collection smoke checklist (2026-09-28)

Requires a real account, `npm --prefix ui run app:dev`, and a browser login.

## Endpoint probes

- [ ] Feed renders content, or shows "Feed isn't available for this account."
      Record which candidate endpoint answered (watch `%APPDATA%\OxiTide\logs\oxitide*.log`
      for requests to `pages/feed` vs `pages/my_collection_feed`) and paste the observed
      JSON shape here:
- [ ] Purchases renders content or the empty state. Record the observed path and JSON
      shape (probe `users/{id}/purchases`; if the shape differs, note the fields):
- [ ] Mixes & Radio lists mixes and clicking one starts playback of its tracks.

## Navigation

- [ ] Sidebar order: Home, Explore, Feed, divider, My Collection (expanded), Playlists,
      Albums, Tracks, Show more, then the playlist tree, then Settings/Log out.
- [ ] Show more reveals Mixes & Radio, Videos, Profiles, Purchases.
- [ ] Every sidebar entry navigates to its page; active states highlight correctly.
- [ ] Ctrl+F jumps to Explore and focuses the search box; typing shows results; clearing
      restores the explore cards.
- [ ] Narrow the window below 1080px: rail mode shows page icons, folder icons, and
      playlist covers; no labels or headers. Toggle collapse manually.

## Collection pages

- [ ] Playlists page lists folders first (with "N playlists") and playlists (mosaic cover,
      "N items"); sort toggle switches name/newest-first.
- [ ] New playlist from the page header and sidebar `+` both create and refresh the tree.
- [ ] New folder creates a folder at the current level and refreshes both page and tree.
- [ ] Folder row opens the folder page with a breadcrumb back to All playlists.
- [ ] Rename playlist and folder from context menus; failures (e.g. offline) toast and
      leave the list unchanged.
- [ ] Delete an owned playlist asks for confirmation and removes it; a liked playlist
      offers "Remove from collection" instead.
- [ ] Move to folder moves a playlist and refreshes tree and page; nested folders appear
      indented in the picker.
- [ ] Tracks page matches the old Library favorites table (play, shuffle, refresh,
      unfavorite drops the row).
- [ ] Albums and Profiles open album/artist pages.
- [ ] Videos lists saved videos with the unsupported-playback note and no play action.
```

- [ ] **Step 6: Commit**

```powershell
git add ui/src/lib/components/Sidebar.svelte ui/src/App.svelte ui/src/lib/nav.svelte.ts ui/src/views/Search.svelte ui/src/views/Library.svelte docs/superpowers/plans/notes/2026-09-28-sidebar-collection-smoke.md
git commit -m "feat(ui): rebuild the sidebar around Home, Explore, Feed, and My Collection"
```

---

## Self-Review Notes

- **Spec coverage:** SC-D1 (Explore absorbs search, Task 15; Library retired, Task 20),
  SC-D2/SC-D3 (sidebar sections, Task 20), SC-D4 (folder tree, Tasks 5/11/14/17),
  SC-D5 (rail covers, Tasks 14/20), SC-D6 (shared renderer, Tasks 1/12), SC-D7 (feed
  probe, Tasks 2/16), SC-D8 (profiles = artists, Task 18), SC-D9 (inert videos, Task 19),
  SC-D10 (mix playback, Tasks 3/12/19), SC-D11 (purchases, Tasks 7/19), SC-D12 (folder
  management + sort, Tasks 6/13/14/17), SC-D13 (four search groups, Task 15 keeps the
  existing groups), SC-D14 (tracks page moves intact, Task 18), SC-D15 (Home mix cards,
  Task 12).
- **Review Focus pinning:** repeated cursor → Task 5; missing feed → Task 2; video-only
  mix → Tasks 3/19; multi-page collection → Task 5; failed mutation leaves state alone →
  Task 13.
- **Known deviation:** the Playlists page "Date" sort uses the API's newest-first order
  rather than a stored date field (the folder endpoint exposes no date in the mapping);
  documented in the spec's open items.

