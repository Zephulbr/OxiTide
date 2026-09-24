# Plan 7 — Playlist Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create, rename, delete, and edit playlists (add tracks, remove rows, move up/down) from anywhere in OxiTide, with a duplicate-resolution flow for adds.

**Architecture:** `TidalClient` gains a shared mutation path (`mutate`) with a v2 base URL plus `put_json`/`post_form_json`; playlist concerns consolidate into a new `playlists.rs`. Six Tauri commands expose writes. A `createPlaylistActions` store owns the picker (list → create → duplicates) and a small `Notice` store surfaces outcomes; views call rename/delete directly and refetch after track edits (server-authoritative).

**Tech Stack:** Rust (reqwest, serde, wiremock), Tauri 2 commands, Svelte 5 runes (`$state`, `$effect`, `SvelteSet`/`SvelteMap`), vitest.

**Spec:** `docs/superpowers/specs/2026-09-24-oxitide-plan7-playlists-design.md`

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`.
- `invoke` sends camelCase args; Rust command parameters stay snake_case (`track_id` ↔ `trackId`, `to_index` ↔ `toIndex`).
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs`.
- All TIDAL HTTP goes through `TidalClient`; writes reuse `try_refresh` for single-flight refresh-on-401.
- Tests: wiremock only, never the live TIDAL API; `MemoryStore` only, never `KeyringStore`; never log token values.
- No new Rust crates and no new npm packages. Version stays 0.2.0; no packaging or config changes.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **Zero-track playlist:** adding to an empty playlist must send `toIndex=0`, and the row buttons enable as soon as the (empty) pager is done. Rust test in Task 4 (`numberOfTracks: 0`); UI smoke in Task 11.
- **Writing a playlist you do not own:** TIDAL answers 403; the client must map it to `Forbidden` and the UI must show the notice with no state change. Rust test in Task 3; store failure tests in Task 7.
- **Many duplicate collisions (30+):** the duplicates panel must scroll inside the modal instead of stretching the window. Smoke in Task 11 (no unit test; components are not unit-tested).
- **Deleting the playlist that is currently playing:** the queue is a snapshot and must keep playing; deleting navigates to Library. Smoke in Task 11.
- **Refetch failure after a track write:** the write succeeded but `pager.refresh()` failed; the view must show the pager error with Retry rather than silently claiming success. Smoke in Task 11 (pager behavior already covered by `pager.test.ts`).

---

### Task 1: Mutation plumbing in `TidalClient`

**Files:**
- Modify: `crates/oxitide-tidal/src/client.rs` (constructor, `post_form`/`delete` wrappers after `get`, new verbs and `mutate`/`send_mutation` helpers, tests at the bottom)

**Interfaces:**
- Consumes: existing `access_token()`, `try_refresh(stale)`, `retry_after(resp)`, `decode_status(resp)`, `Error`.
- Produces: `put_json<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T>`, `post_form_json<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)], form: &[(&str, String)]) -> Result<T>`; `post_form`/`delete` keep their current signatures. The v2 base is derived from the v1 base (`/v1` → `/v2`, otherwise append `/v2`). `decode_empty` is removed (its callers now use `mutate`).

- [ ] **Step 1: Write the failing tests**

Add `query_param` to the wiremock matcher import at the top of `mod tests` in `crates/oxitide-tidal/src/client.rs`:

```rust
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
```

Append these tests before the closing `}` of `mod tests` (after `post_form_gives_up_after_a_second_429`):

```rust
    #[tokio::test]
    async fn put_json_targets_v2_without_country_code() {
        let server = MockServer::start().await;
        let responder = |req: &wiremock::Request| {
            let has_country = req.url.query_pairs().any(|(key, _)| key == "countryCode");
            if has_country {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 3}))
            }
        };
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(query_param("name", "mix"))
            .respond_with(responder)
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
        let thing: Thing = client
            .put_json("things", &[("name", "mix".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 3);
    }

    #[tokio::test]
    async fn put_json_refreshes_once_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/v2/things"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 9})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            store.clone(),
        );
        let thing: Thing = client.put_json("things", &[]).await.unwrap();
        assert_eq!(thing.value, 9);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn post_form_json_retries_once_on_429() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 4}))
                }
            }
        };
        Mock::given(method("POST"))
            .and(path("/playlists/p/items"))
            .and(body_string_contains("a=b"))
            .respond_with(responder)
            .expect(2)
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
        let thing: Thing = client
            .post_form_json("playlists/p/items", &[], &[("a", "b".to_string())])
            .await
            .unwrap();
        assert_eq!(thing.value, 4);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal client::tests`
Expected: compile error — no method named `put_json` / `post_form_json`.

- [ ] **Step 3: Implement**

Replace the `new` constructor in `crates/oxitide-tidal/src/client.rs`:

```rust
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token_url: impl Into<String>,
        session: SessionInfo,
        tokens: TokenSet,
        store: Arc<dyn TokenStore>,
    ) -> Self {
        let base_url = base_url.into();
        let trimmed = base_url.trim_end_matches('/');
        let v2_base_url = match trimmed.strip_suffix("/v1") {
            Some(prefix) => format!("{prefix}/v2"),
            None => format!("{trimmed}/v2"),
        };
        Self {
            http,
            base_url,
            v2_base_url,
            token_url: token_url.into(),
            session,
            tokens: Arc::new(RwLock::new(tokens)),
            store,
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }
```

Add the field to the struct:

```rust
#[derive(Clone)]
pub struct TidalClient {
    http: reqwest::Client,
    base_url: String,
    v2_base_url: String,
    token_url: String,
    session: SessionInfo,
    tokens: Arc<RwLock<TokenSet>>,
    store: Arc<dyn TokenStore>,
    refresh_lock: Arc<Mutex<()>>,
}
```

Replace the `post_form` and `delete` methods with wrappers and add the new verbs:

```rust
    /// Send a form-encoded POST, refreshing once on 401 and honoring a
    /// `Retry-After` hint once on 429.
    pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<()> {
        self.mutate(reqwest::Method::POST, &self.base_url, path, &[], form)
            .await
            .map(|_| ())
    }

    /// Send a DELETE, refreshing once on 401 and honoring a `Retry-After` hint
    /// once on 429.
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.mutate(reqwest::Method::DELETE, &self.base_url, path, &[], &[])
            .await
            .map(|_| ())
    }

    /// Send a PUT with query params on the v2 API and decode the JSON response.
    pub async fn put_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::PUT, &self.v2_base_url, path, params, &[])
            .await?;
        Ok(resp.json().await?)
    }

    /// Send a form-encoded POST and decode the JSON response, sharing the write
    /// verbs' refresh-on-401 and 429 retry behavior.
    pub async fn post_form_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::POST, &self.base_url, path, params, form)
            .await?;
        Ok(resp.json().await?)
    }
```

Replace `send_mutation` with the base-aware version and add `mutate` next to it:

```rust
    /// Shared write path: v1 bases get the `countryCode` query, 401 refreshes
    /// once, 429 retries once after `Retry-After`, and non-success statuses map
    /// to errors.
    async fn mutate(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
    ) -> Result<reqwest::Response> {
        let country_code = base == self.base_url.as_str();
        let token = self.access_token();
        let mut resp = self
            .send_mutation(&method, base, path, params, form, &token, country_code)
            .await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            resp = self
                .send_mutation(
                    &method,
                    base,
                    path,
                    params,
                    form,
                    &self.access_token(),
                    country_code,
                )
                .await?;
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            tokio::time::sleep(Self::retry_after(&resp)).await;
            resp = self
                .send_mutation(
                    &method,
                    base,
                    path,
                    params,
                    form,
                    &self.access_token(),
                    country_code,
                )
                .await?;
        }
        Self::decode_status(resp).await
    }

    async fn send_mutation(
        &self,
        method: &reqwest::Method,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        token: &str,
        country_code: bool,
    ) -> Result<reqwest::Response> {
        let mut query: Vec<(&str, String)> = if country_code {
            vec![("countryCode", self.session.country_code.clone())]
        } else {
            Vec::new()
        };
        query.extend(params.iter().cloned());
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut request = self
            .http
            .request(method.clone(), url)
            .bearer_auth(token)
            .query(&query);
        if !form.is_empty() {
            request = request.form(form);
        }
        Ok(request.send().await?)
    }
```

Delete the now-unused `decode_empty`:

```rust
    async fn decode_empty(resp: reqwest::Response) -> Result<()> {
        Self::decode_status(resp).await.map(|_| ())
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass, including the three new ones and every pre-existing `post_form`/`delete` test unchanged.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/client.rs
git commit -m "feat(tidal): add v2 mutations with a shared write path"
```

---

### Task 2: Move playlist reads into `playlists.rs`

**Files:**
- Create: `crates/oxitide-tidal/src/playlists.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Modify: `crates/oxitide-tidal/src/catalog.rs` (remove playlist reads + their test)
- Modify: `crates/oxitide-tidal/src/library.rs` (remove playlist listing + its test)

**Interfaces:**
- Consumes: `TidalClient::get`, `get_all_pages`, `DEFAULT_PAGE_SIZE`, models `ItemPage`/`Playlist`/`Track`.
- Produces (all unchanged signatures, new home): `playlist(&self, id: &str) -> Result<Playlist>`, `playlist_tracks_page(&self, id: &str, limit: u32, offset: u32) -> Result<(Vec<Track>, u32)>`, `playlist_tracks_all(&self, id: &str) -> Result<Vec<Track>>`, `user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>>`, `all_user_playlists(&self) -> Result<Vec<Playlist>>`.

- [ ] **Step 1: Create the module with the moved code**

Create `crates/oxitide-tidal/src/playlists.rs`:

```rust
use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{ItemPage, Playlist, Track};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.get(&format!("playlists/{id}"), &[]).await
    }

    /// One page of playlist tracks plus the server-reported total.
    pub async fn playlist_tracks_page(
        &self,
        id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Track>, u32)> {
        let page: ItemPage<Track> =
            self.get(&format!("playlists/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok((page.items, page.total_number_of_items))
    }

    /// Fetch every track of a playlist, walking all pages.
    pub async fn playlist_tracks_all(&self, id: &str) -> Result<Vec<Track>> {
        self.get_all_pages(&format!("playlists/{id}/tracks"), &[], DEFAULT_PAGE_SIZE)
            .await
    }

    /// One page of the signed-in user's playlists.
    pub async fn user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>> {
        let page: ItemPage<Playlist> = self
            .get(
                &format!("users/{}/playlists", self.session().user_id),
                &page_params(limit, offset),
            )
            .await?;
        Ok(page.items)
    }

    /// Walk every page of the user's playlists.
    pub async fn all_user_playlists(&self) -> Result<Vec<Playlist>> {
        self.get_all_pages(
            &format!("users/{}/playlists", self.session().user_id),
            &[],
            DEFAULT_PAGE_SIZE,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
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
    async fn playlist_tracks_page_returns_items_and_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid/tracks"))
            .and(query_param("limit", "100")).and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 2, "title": "P1", "artists": [{"id": 9, "name": "X"}]}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let (tracks, total) = client(&server).playlist_tracks_page("p-uuid", 100, 0).await.unwrap();
        assert_eq!(tracks[0].artists[0].name, "X");
        assert_eq!(total, 1);
    }

    #[tokio::test]
    async fn user_playlists_handles_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/playlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        assert!(client(&server).user_playlists(50, 0).await.unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Remove the moved code from the old modules**

In `crates/oxitide-tidal/src/lib.rs`, add the module (keep alphabetical order):

```rust
pub mod playlists;
```

In `crates/oxitide-tidal/src/catalog.rs`:
- Change the models import to drop `Playlist`:

```rust
use crate::models::{Album, Artist, ItemPage, Track};
```

- Delete the three methods `playlist`, `playlist_tracks_page`, `playlist_tracks_all` (catalog keeps `track`, `album`, `album_tracks`, `album_tracks_all`, `artist`).
- Delete the test `playlist_tracks_page_returns_items_and_total`.

In `crates/oxitide-tidal/src/library.rs`:
- Change the models import to drop `Playlist`:

```rust
use crate::models::{Album, Artist, ItemPage, Track};
```

- Delete the methods `user_playlists` and `all_user_playlists`.
- Delete the test `user_playlists_handles_empty`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass; the two moved tests now appear under `playlists::tests`.

- [ ] **Step 4: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/playlists.rs crates/oxitide-tidal/src/lib.rs crates/oxitide-tidal/src/catalog.rs crates/oxitide-tidal/src/library.rs
git commit -m "refactor(tidal): move playlist reads into their own module"
```

---

### Task 3: Create, rename, and delete playlists

**Files:**
- Modify: `crates/oxitide-tidal/src/playlists.rs` (top imports, methods after `all_user_playlists`, tests at the bottom)

**Interfaces:**
- Consumes: `TidalClient::put_json` / `post_form` / `delete` (Task 1).
- Produces: `create_playlist(&self, name: &str, description: &str) -> Result<Playlist>`, `rename_playlist(&self, id: &str, title: &str, description: &str) -> Result<()>`, `delete_playlist(&self, id: &str) -> Result<()>`; private `V2Envelope<T> { data: T }`.

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-tidal/src/playlists.rs`, extend the test imports:

```rust
    use wiremock::matchers::{body_string_contains, method, path, query_param};
```

Append inside `mod tests`:

```rust
    #[tokio::test]
    async fn create_playlist_puts_v2_and_parses_the_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/create-playlist"))
            .and(query_param("name", "Mix"))
            .and(query_param("description", "for the road"))
            .and(query_param("folderId", "root"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "uuid": "new-uuid",
                    "title": "Mix",
                    "description": "for the road",
                    "numberOfTracks": 0,
                    "duration": 0
                }
            })))
            .mount(&server).await;
        let playlist = client(&server)
            .create_playlist("Mix", "for the road")
            .await
            .unwrap();
        assert_eq!(playlist.uuid, "new-uuid");
        assert_eq!(playlist.title, "Mix");
        assert_eq!(playlist.number_of_tracks, 0);
    }

    #[tokio::test]
    async fn create_playlist_without_uuid_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v2/my-collection/playlists/folders/create-playlist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"title": "No id"}
            })))
            .mount(&server).await;
        assert!(client(&server).create_playlist("No id", "").await.is_err());
    }

    #[tokio::test]
    async fn rename_playlist_posts_title_and_description() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .and(body_string_contains("title=New+name"))
            .and(body_string_contains("description=Notes"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        client(&server)
            .rename_playlist("p-uuid", "New name", "Notes")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn rename_playlist_maps_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server).await;
        let err = client(&server)
            .rename_playlist("p-uuid", "Nope", "")
            .await
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn delete_playlist_sends_delete() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server).delete_playlist("p-uuid").await.unwrap();
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal playlists::tests`
Expected: compile error — no `create_playlist` / `rename_playlist` / `delete_playlist`.

- [ ] **Step 3: Implement**

At the top of `crates/oxitide-tidal/src/playlists.rs`, add the serde import and the envelope type above `impl TidalClient`:

```rust
use serde::Deserialize;
```

```rust
/// JSON:API responses from the v2 API wrap the payload in `{ "data": ... }`.
#[derive(Debug, Deserialize)]
struct V2Envelope<T> {
    data: T,
}
```

Inside `impl TidalClient`, after `all_user_playlists`, add:

```rust
    /// Create a playlist in the user's root folder (v2 endpoint).
    pub async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let envelope: V2Envelope<Playlist> = self
            .put_json(
                "my-collection/playlists/folders/create-playlist",
                &[
                    ("name", name.to_string()),
                    ("description", description.to_string()),
                    ("folderId", "root".to_string()),
                ],
            )
            .await?;
        Ok(envelope.data)
    }

    /// Rename a playlist and/or change its description.
    pub async fn rename_playlist(&self, id: &str, title: &str, description: &str) -> Result<()> {
        self.post_form(
            &format!("playlists/{id}"),
            &[
                ("title", title.to_string()),
                ("description", description.to_string()),
            ],
        )
        .await
    }

    /// Delete a playlist.
    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.delete(&format!("playlists/{id}")).await
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass, including the five new ones.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/playlists.rs
git commit -m "feat(tidal): add playlist create, rename, and delete"
```

---

### Task 4: Add, remove, and move playlist items

**Files:**
- Modify: `crates/oxitide-tidal/src/playlists.rs` (constants/types above `impl`, methods after `delete_playlist`, tests at the bottom)

**Interfaces:**
- Consumes: `TidalClient::playlist` (Task 2), `post_form_json` / `post_form` / `delete` (Task 1).
- Produces: `add_playlist_tracks(&self, id: &str, track_ids: &[u64], allow_duplicates: bool) -> Result<Vec<u64>>`, `remove_playlist_items(&self, id: &str, indices: &[u32]) -> Result<()>`, `move_playlist_items(&self, id: &str, indices: &[u32], to_index: u32) -> Result<()>`; private `AddedItems`.

- [ ] **Step 1: Write the failing tests**

Extend the test imports in `crates/oxitide-tidal/src/playlists.rs`:

```rust
    use wiremock::matchers::{body_string_contains, method, path, query_param};
```

Append inside `mod tests`:

```rust
    async fn playlist_page(server: &MockServer, total: u32) {
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P", "numberOfTracks": total
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn add_playlist_tracks_skips_duplicates_and_reports_added() {
        let server = MockServer::start().await;
        playlist_page(&server, 5).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(query_param("limit", "1"))
            .and(body_string_contains("trackIds=7"))
            .and(body_string_contains("toIndex=5"))
            .and(body_string_contains("onDupes=SKIP"))
            .and(body_string_contains("onArtifactNotFound=SKIP"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_allow_duplicates_uses_add() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(body_string_contains("toIndex=0"))
            .and(body_string_contains("onDupes=ADD"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7], true)
            .await
            .unwrap();
        assert_eq!(added, vec![7]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_accepts_string_ids() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": ["7", "8"]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7, 8], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7, 8]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_missing_added_ids_assumes_all_added() {
        let server = MockServer::start().await;
        playlist_page(&server, 1).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &[7, 8], false)
            .await
            .unwrap();
        assert_eq!(added, vec![7, 8]);
    }

    #[tokio::test]
    async fn add_playlist_tracks_dedupes_and_chunks() {
        let server = MockServer::start().await;
        playlist_page(&server, 5).await;
        let ids: Vec<u64> = (1..=101).collect();
        let first_chunk = ids[..100]
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("%2C");
        // A repeated id must not turn into a duplicate inside one request.
        let requested: Vec<u64> = std::iter::once(1u64).chain(ids.iter().copied()).collect();
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(body_string_contains(&format!("trackIds={first_chunk}")))
            .and(body_string_contains("toIndex=5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": ids[..100].to_vec()
            })))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(body_string_contains("trackIds=101"))
            .and(body_string_contains("toIndex=105"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [101]
            })))
            .mount(&server).await;
        let added = client(&server)
            .add_playlist_tracks("p-uuid", &requested, false)
            .await
            .unwrap();
        assert_eq!(added.len(), 101);
        assert_eq!(added[0], 1);
        assert_eq!(added[100], 101);
    }

    #[tokio::test]
    async fn add_playlist_tracks_empty_list_makes_no_request() {
        let server = MockServer::start().await;
        let added = client(&server).add_playlist_tracks("p-uuid", &[], false).await.unwrap();
        assert!(added.is_empty());
    }

    #[tokio::test]
    async fn remove_playlist_items_sends_indices_path() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid/items/2,5"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server)
            .remove_playlist_items("p-uuid", &[2, 5])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn move_playlist_items_sends_to_index() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items/4"))
            .and(body_string_contains("toIndex=1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        client(&server)
            .move_playlist_items("p-uuid", &[4], 1)
            .await
            .unwrap();
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal playlists::tests`
Expected: compile error — no `add_playlist_tracks` / `remove_playlist_items` / `move_playlist_items`.

- [ ] **Step 3: Implement**

Above `impl TidalClient` in `crates/oxitide-tidal/src/playlists.rs`, add the chunk constant and the response type:

```rust
/// Rows per add request; the items endpoint honors 100-row additions.
const ADD_CHUNK: usize = 100;

/// `POST .../items` response; ids may arrive as numbers or numeric strings.
#[derive(Debug, Deserialize)]
struct AddedItems {
    #[serde(rename = "addedItemIds")]
    added_item_ids: Option<Vec<IdOrString>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IdOrString {
    Number(u64),
    Text(String),
}

impl IdOrString {
    fn as_id(&self) -> Option<u64> {
        match self {
            IdOrString::Number(number) => Some(*number),
            IdOrString::Text(text) => text.parse().ok(),
        }
    }
}
```

Inside `impl TidalClient`, after `delete_playlist`, add:

```rust
    /// Add tracks to a playlist in request order, returning the ids the server
    /// reports as added. Tracks already present are skipped unless
    /// `allow_duplicates` is set; when the response omits `addedItemIds`,
    /// every requested id is assumed added.
    pub async fn add_playlist_tracks(
        &self,
        id: &str,
        track_ids: &[u64],
        allow_duplicates: bool,
    ) -> Result<Vec<u64>> {
        let mut requested: Vec<u64> = Vec::new();
        for track_id in track_ids {
            if !requested.contains(track_id) {
                requested.push(*track_id);
            }
        }
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        let playlist = self.playlist(id).await?;
        let mut added: Vec<u64> = Vec::new();
        for chunk in requested.chunks(ADD_CHUNK) {
            let form = [
                ("onArtifactNotFound", "SKIP".to_string()),
                (
                    "trackIds",
                    chunk
                        .iter()
                        .map(|track_id| track_id.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                (
                    "toIndex",
                    (playlist.number_of_tracks as usize + added.len()).to_string(),
                ),
                (
                    "onDupes",
                    if allow_duplicates { "ADD" } else { "SKIP" }.to_string(),
                ),
            ];
            let params = [("limit", chunk.len().to_string())];
            let response: AddedItems = self
                .post_form_json(&format!("playlists/{id}/items"), &params, &form)
                .await?;
            match response.added_item_ids {
                Some(values) => {
                    added.extend(values.iter().filter_map(IdOrString::as_id));
                }
                None => added.extend(chunk.iter().copied()),
            }
        }
        Ok(added)
    }

    /// Remove playlist rows by zero-based index.
    pub async fn remove_playlist_items(&self, id: &str, indices: &[u32]) -> Result<()> {
        let list = indices
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.delete(&format!("playlists/{id}/items/{list}")).await
    }

    /// Move playlist rows (zero-based indices) to `to_index`.
    pub async fn move_playlist_items(&self, id: &str, indices: &[u32], to_index: u32) -> Result<()> {
        let list = indices
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.post_form(
            &format!("playlists/{id}/items/{list}"),
            &[("toIndex", to_index.to_string())],
        )
        .await
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass, including the eight new ones. If the `add_playlist_tracks_dedupes_and_chunks` mock order flakes, mount the second mock after the first (wiremock prefers the most recent matching mount).

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/playlists.rs
git commit -m "feat(tidal): add playlist item add, remove, and move"
```

---

### Task 5: Tauri commands and registration

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (commands after `get_favorite_ids`; test at the bottom of `mod tests`)
- Modify: `crates/oxitide-app/src/main.rs` (`generate_handler![...]`)

**Interfaces:**
- Consumes: the six client methods from Tasks 3–4; existing `require_client`, `state.fail_tidal`, `Playlist` import.
- Produces: commands `create_playlist`, `rename_playlist`, `delete_playlist`, `add_playlist_tracks`, `remove_playlist_items`, `move_playlist_items` reachable from `invoke`.

- [ ] **Step 1: Write the failing test**

Append at the bottom of `mod tests` in `crates/oxitide-app/src/commands.rs` (after `favorite_ids_dto_serializes_arrays`):

```rust
    #[test]
    fn playlist_dto_uses_camel_case_for_track_count() {
        let dto = oxitide_tidal::models::Playlist {
            uuid: "u".into(),
            title: "T".into(),
            description: None,
            number_of_tracks: 3,
            duration: 0,
            image: None,
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["numberOfTracks"], serde_json::json!(3));
        assert_eq!(value["uuid"], serde_json::json!("u"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app playlist_dto_uses_camel_case_for_track_count`
Expected: PASS already (the DTO exists); the test pins the IPC shape the new `create_playlist` command returns. If it fails, stop and fix the DTO before continuing.

- [ ] **Step 3: Implement the commands**

In `crates/oxitide-app/src/commands.rs`, insert the six commands after `get_favorite_ids` (before `get_user_playlists`):

```rust
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> Result<Playlist, String> {
    require_client(&state)?
        .create_playlist(&name, &description)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    id: String,
    title: String,
    description: String,
) -> Result<(), String> {
    require_client(&state)?
        .rename_playlist(&id, &title, &description)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: String) -> Result<(), String> {
    require_client(&state)?
        .delete_playlist(&id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn add_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<u64>,
    allow_duplicates: bool,
) -> Result<Vec<u64>, String> {
    require_client(&state)?
        .add_playlist_tracks(&id, &track_ids, allow_duplicates)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn remove_playlist_items(
    state: State<'_, AppState>,
    id: String,
    indices: Vec<u32>,
) -> Result<(), String> {
    require_client(&state)?
        .remove_playlist_items(&id, &indices)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn move_playlist_items(
    state: State<'_, AppState>,
    id: String,
    indices: Vec<u32>,
    to_index: u32,
) -> Result<(), String> {
    require_client(&state)?
        .move_playlist_items(&id, &indices, to_index)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

Register them in `crates/oxitide-app/src/main.rs` inside `generate_handler![...]`, after `commands::get_favorite_ids,`:

```rust
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_playlist_tracks,
            commands::remove_playlist_items,
            commands::move_playlist_items,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app`
Expected: all tests pass, including `playlist_dto_uses_camel_case_for_track_count`.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy --workspace -- -D warnings
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose playlist editing commands"
```

---

### Task 6: Notice store

**Files:**
- Create: `ui/src/lib/notice.svelte.ts`
- Test: `ui/src/lib/notice.test.ts`

**Interfaces:**
- Produces: `Notice { readonly message: string | null; show(msg: string): void; dismiss(): void }`, `createNotice(): Notice`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/notice.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createNotice } from "./notice.svelte";

describe("notice store", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("shows a message and auto-dismisses after six seconds", () => {
    const notice = createNotice();

    notice.show("hello");

    expect(notice.message).toBe("hello");
    vi.advanceTimersByTime(5999);
    expect(notice.message).toBe("hello");
    vi.advanceTimersByTime(1);
    expect(notice.message).toBeNull();
  });

  it("resets the timer when a new message arrives", () => {
    const notice = createNotice();

    notice.show("one");
    vi.advanceTimersByTime(5000);
    notice.show("two");
    vi.advanceTimersByTime(5000);
    expect(notice.message).toBe("two");
    vi.advanceTimersByTime(1000);
    expect(notice.message).toBeNull();
  });

  it("dismiss clears immediately", () => {
    const notice = createNotice();

    notice.show("x");
    notice.dismiss();

    expect(notice.message).toBeNull();
    vi.advanceTimersByTime(6000);
    expect(notice.message).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/notice.test.ts`
Expected: FAIL — cannot resolve `./notice.svelte`.

- [ ] **Step 3: Implement**

Create `ui/src/lib/notice.svelte.ts`:

```ts
export interface Notice {
  readonly message: string | null;
  show(msg: string): void;
  dismiss(): void;
}

/// Transient user-facing message shown in the app's notice strip; a new
/// message restarts the six-second auto-dismiss timer.
export function createNotice(): Notice {
  let message = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function stopTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  return {
    get message() {
      return message;
    },
    show(msg: string) {
      message = msg;
      stopTimer();
      timer = setTimeout(() => {
        message = null;
        timer = null;
      }, 6000);
    },
    dismiss() {
      stopTimer();
      message = null;
    },
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/notice.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Type-check and commit**

```bash
npm --prefix ui run check
git add ui/src/lib/notice.svelte.ts ui/src/lib/notice.test.ts
git commit -m "feat(ui): add a transient notice store"
```

---

### Task 7: API wrappers and the playlist actions store

**Files:**
- Modify: `ui/src/lib/api.ts` (wrappers after `getUserPlaylists`)
- Create: `ui/src/lib/playlists.svelte.ts`
- Test: `ui/src/lib/playlists.test.ts`

**Interfaces:**
- Consumes: `Notice` (Task 6); commands from Task 5.
- Produces: `api.createPlaylist/renamePlaylist/deletePlaylist/addPlaylistTracks/removePlaylistItems/movePlaylistItems`; `PlaylistActions` with `open`, `step`, `fromPicker`, `pendingTrackIds`, `duplicates`, `choices`, `busy`, `revision`, `openPicker`, `openCreate`, `startCreate`, `close`, `reset`, `addTo`, `createAndAdd`, `setChoice`, `setAllChoices`, `confirmDuplicates`; `createPlaylistActions(notice: Notice): PlaylistActions`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/playlists.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import { api } from "./api";
import { createNotice } from "./notice.svelte";
import { createPlaylistActions } from "./playlists.svelte";

vi.mock("./api", () => ({
  api: {
    createPlaylist: vi.fn(),
    addPlaylistTracks: vi.fn(),
    getUserPlaylists: vi.fn(),
  },
}));

function track(id: number): Track {
  return {
    id,
    title: `T${id}`,
    duration: 1,
    artists: [],
    trackNumber: 1,
    volumeNumber: 1,
    explicit: false,
  };
}

function playlist(uuid: string, title = "Mix"): Playlist {
  return { uuid, title, description: null, numberOfTracks: 0, duration: 0, image: null };
}

describe("playlist actions store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([]);
    vi.mocked(api.createPlaylist).mockResolvedValue(playlist("new"));
  });

  it("computes skips from the added ids and opens the duplicates step", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [1, 2, 3], false);
    expect(actions.step).toBe("duplicates");
    expect(actions.duplicates.map((t) => t.id)).toEqual([2, 3]);
    expect(actions.open).toBe(true);
  });

  it("closes and notices when nothing was skipped", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1, 2]);
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1), track(2)]);

    await actions.addTo(playlist("p", "Chill"));

    expect(actions.open).toBe(false);
    expect(notice.message).toContain("2 added");
    expect(notice.message).toContain("Chill");
  });

  it("keeps one by default and sends no second write", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([1]);
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1), track(2)]);
    await actions.addTo(playlist("p"));

    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenCalledTimes(1);
    expect(actions.open).toBe(false);
    expect(notice.message).toContain("kept one of");
  });

  it("sends only the add-both choices with allow_duplicates", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([3]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setChoice(3, true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [3], true);
  });

  it("apply-to-all sets every duplicate to add both", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValueOnce([1]).mockResolvedValueOnce([2, 3]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2), track(3)]);
    await actions.addTo(playlist("p"));

    actions.setAllChoices(true);
    await actions.confirmDuplicates();

    expect(api.addPlaylistTracks).toHaveBeenLastCalledWith("p", [2, 3], true);
  });

  it("dedupes pending ids before adding", async () => {
    vi.mocked(api.addPlaylistTracks).mockResolvedValue([7]);
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(7), track(7)]);

    await actions.addTo(playlist("p"));

    expect(api.addPlaylistTracks).toHaveBeenCalledWith("p", [7], false);
  });

  it("surfaces a failed add and keeps the picker open with its pending tracks", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("boom"));
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1)]);

    await actions.addTo(playlist("p"));

    expect(notice.message).toContain("boom");
    expect(actions.open).toBe(true);
    expect(actions.pendingTrackIds).toEqual([1]);
  });

  it("creates then adds and bumps the revision", async () => {
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2)]);

    const created = await actions.createAndAdd("New", "desc");

    expect(api.createPlaylist).toHaveBeenCalledWith("New", "desc");
    expect(api.addPlaylistTracks).toHaveBeenCalledWith("new", [1, 2], false);
    expect(created?.uuid).toBe("new");
    expect(actions.revision).toBe(1);
    expect(actions.open).toBe(false);
  });

  it("returns the created playlist even when the follow-up add fails", async () => {
    vi.mocked(api.addPlaylistTracks).mockRejectedValue(new Error("nope"));
    const notice = createNotice();
    const actions = createPlaylistActions(notice);
    actions.openPicker([track(1)]);

    const created = await actions.createAndAdd("New", "");

    expect(created?.uuid).toBe("new");
    expect(notice.message).toContain("nope");
  });

  it("tracks the picker origin and busy state", async () => {
    let release: ((ids: number[]) => void) | null = null;
    vi.mocked(api.addPlaylistTracks).mockImplementation(
      () =>
        new Promise<number[]>((resolve) => {
          release = resolve;
        }),
    );
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1)]);
    expect(actions.fromPicker).toBe(true);

    const pending = actions.addTo(playlist("p"));
    expect(actions.busy).toBe(true);

    release!([1]);
    await pending;
    expect(actions.busy).toBe(false);

    actions.openCreate();
    expect(actions.fromPicker).toBe(false);
    expect(actions.step).toBe("create");
  });

  it("reset closes the picker and clears the pending tracks", () => {
    const actions = createPlaylistActions(createNotice());
    actions.openPicker([track(1), track(2)]);

    actions.reset();

    expect(actions.open).toBe(false);
    expect(actions.pendingTrackIds).toEqual([]);
    expect(actions.step).toBe("list");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/playlists.test.ts`
Expected: FAIL — cannot resolve `./playlists.svelte`.

- [ ] **Step 3: Add the API wrappers**

In `ui/src/lib/api.ts`, add after `getUserPlaylists`:

```ts
  createPlaylist: (name: string, description: string) =>
    invoke<Playlist>("create_playlist", { name, description }),
  renamePlaylist: (id: string, title: string, description: string) =>
    invoke<void>("rename_playlist", { id, title, description }),
  deletePlaylist: (id: string) => invoke<void>("delete_playlist", { id }),
  addPlaylistTracks: (id: string, trackIds: number[], allowDuplicates: boolean) =>
    invoke<number[]>("add_playlist_tracks", { id, trackIds, allowDuplicates }),
  removePlaylistItems: (id: string, indices: number[]) =>
    invoke<void>("remove_playlist_items", { id, indices }),
  movePlaylistItems: (id: string, indices: number[], toIndex: number) =>
    invoke<void>("move_playlist_items", { id, indices, toIndex }),
```

- [ ] **Step 4: Implement the store**

Create `ui/src/lib/playlists.svelte.ts`:

```ts
import { SvelteMap } from "svelte/reactivity";
import { api, type Playlist, type Track } from "./api";
import type { Notice } from "./notice.svelte";

export interface PlaylistActions {
  readonly open: boolean;
  readonly step: "list" | "create" | "duplicates";
  /// True when the picker was opened from a track/album surface (create
  /// navigates to the new playlist); false when opened from Library.
  readonly fromPicker: boolean;
  readonly pendingTrackIds: readonly number[];
  /// Phase-1 skips awaiting a decision (true = "Add both").
  readonly duplicates: readonly Track[];
  readonly choices: SvelteMap<number, boolean>;
  readonly busy: boolean;
  /// Bumped after a successful create so mounted lists can refresh.
  readonly revision: number;
  openPicker(tracks: Track[]): void;
  openCreate(): void;
  startCreate(): void;
  close(): void;
  reset(): void;
  addTo(playlist: Playlist): Promise<void>;
  createAndAdd(name: string, description: string): Promise<Playlist | null>;
  setChoice(trackId: number, addBoth: boolean): void;
  setAllChoices(addBoth: boolean): void;
  confirmDuplicates(): Promise<void>;
}

type Step = "list" | "create" | "duplicates";

function dedupe(ids: number[]): number[] {
  return [...new Set(ids)];
}

/// The add-to-playlist picker and playlist creation flow. Track edits
/// (remove/move) are direct API calls in the views; creation, the two-phase
/// duplicate flow, and reset-on-logout live here.
export function createPlaylistActions(notice: Notice): PlaylistActions {
  let open = $state(false);
  let step = $state<Step>("list");
  let fromPicker = $state(false);
  let tracks = $state<Track[]>([]);
  let duplicates = $state<Track[]>([]);
  const choices = new SvelteMap<number, boolean>();
  let busy = $state(false);
  let revision = $state(0);
  let target = $state<Playlist | null>(null);

  function clearState() {
    step = "list";
    fromPicker = false;
    tracks = [];
    duplicates = [];
    choices.clear();
    target = null;
  }

  function openPicker(next: Track[]) {
    clearState();
    fromPicker = true;
    tracks = next;
    open = true;
  }

  function openCreate() {
    clearState();
    step = "create";
    open = true;
  }

  function startCreate() {
    step = "create";
  }

  function close() {
    open = false;
    clearState();
  }

  async function addTo(playlist: Playlist) {
    if (busy) return;
    busy = true;
    target = playlist;
    const requested = dedupe(tracks.map((t) => t.id));
    try {
      const added = new Set(await api.addPlaylistTracks(playlist.uuid, requested, false));
      const skipped = requested.filter((id) => !added.has(id));
      if (skipped.length === 0) {
        notice.show(`${requested.length} added to ${playlist.title}`);
        close();
      } else {
        const skippedSet = new Set(skipped);
        duplicates = tracks.filter((t) => skippedSet.has(t.id));
        for (const t of duplicates) choices.set(t.id, false);
        step = "duplicates";
      }
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function confirmDuplicates() {
    if (busy || !target) return;
    const addBoth = duplicates.filter((t) => choices.get(t.id) === true).map((t) => t.id);
    busy = true;
    try {
      if (addBoth.length > 0) await api.addPlaylistTracks(target.uuid, addBoth, true);
      notice.show(`${addBoth.length} added, ${duplicates.length - addBoth.length} kept one of`);
      close();
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function createAndAdd(name: string, description: string): Promise<Playlist | null> {
    if (busy) return null;
    busy = true;
    try {
      const created = await api.createPlaylist(name, description);
      revision += 1;
      const ids = dedupe(tracks.map((t) => t.id));
      if (ids.length > 0) {
        try {
          await api.addPlaylistTracks(created.uuid, ids, false);
          notice.show(`Created ${created.title} with ${ids.length} tracks`);
        } catch (e) {
          notice.show(String(e));
        }
      } else {
        notice.show(`Created ${created.title}`);
      }
      close();
      return created;
    } catch (e) {
      notice.show(String(e));
      return null;
    } finally {
      busy = false;
    }
  }

  return {
    get open() {
      return open;
    },
    get step() {
      return step;
    },
    get fromPicker() {
      return fromPicker;
    },
    get pendingTrackIds() {
      return tracks.map((t) => t.id);
    },
    get duplicates() {
      return duplicates;
    },
    get choices() {
      return choices;
    },
    get busy() {
      return busy;
    },
    get revision() {
      return revision;
    },
    openPicker,
    openCreate,
    startCreate,
    close,
    reset: close,
    addTo,
    createAndAdd,
    setChoice: (trackId, addBoth) => {
      choices.set(trackId, addBoth);
    },
    setAllChoices: (addBoth) => {
      for (const t of duplicates) choices.set(t.id, addBoth);
    },
    confirmDuplicates,
  };
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm --prefix ui test`
Expected: PASS — notice, pager, favorites, and the new playlist actions tests.

- [ ] **Step 6: Type-check and commit**

```bash
npm --prefix ui run check
git add ui/src/lib/api.ts ui/src/lib/playlists.svelte.ts ui/src/lib/playlists.test.ts
git commit -m "feat(ui): add playlist actions store and API wrappers"
```

---

### Task 8: Picker UI and add-to-playlist affordances

**Files:**
- Create: `ui/src/lib/components/Modal.svelte`
- Create: `ui/src/lib/components/ConfirmDialog.svelte`
- Create: `ui/src/lib/components/PlaylistPicker.svelte`
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/views/Search.svelte`, `ui/src/views/Album.svelte`
- Modify: `ui/src/App.svelte` (stores, picker, notice strip)

**Interfaces:**
- Consumes: `createNotice`, `createPlaylistActions` (Tasks 6–7); `api.getUserPlaylists`.
- Produces: `TrackTable` accepts `onAddToPlaylist?: (track: Track) => void`; Search/Album accept a `playlistActions: PlaylistActions` prop; App renders the picker and notice strip. Note: the picker's overlay does not close on backdrop click (Escape and the ✕ cover it) to keep the markup free of a11y warnings.

- [ ] **Step 1: Add the modal and confirm dialog**

Create `ui/src/lib/components/Modal.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    title,
    onclose,
    children,
  }: { title: string; onclose: () => void; children: Snippet } = $props();
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape") onclose();
  }}
/>

<div class="overlay">
  <div class="panel" role="dialog" aria-label={title}>
    <header>
      <strong>{title}</strong>
      <button onclick={onclose} aria-label="Close">✕</button>
    </header>
    {@render children()}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgb(0 0 0 / 0.5);
  }
  .panel {
    max-width: min(560px, 90vw);
    max-height: 80vh;
    overflow: auto;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
</style>
```

Create `ui/src/lib/components/ConfirmDialog.svelte`:

```svelte
<script lang="ts">
  import Modal from "./Modal.svelte";
  let {
    title,
    message,
    onconfirm,
    oncancel,
  }: { title: string; message: string; onconfirm: () => void; oncancel: () => void } = $props();
</script>

<Modal {title} onclose={oncancel}>
  <p>{message}</p>
  <p class="actions">
    <button onclick={oncancel}>Cancel</button>
    <button onclick={onconfirm}>Confirm</button>
  </p>
</Modal>

<style>
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
```

- [ ] **Step 2: Add the picker**

Create `ui/src/lib/components/PlaylistPicker.svelte`:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Playlist } from "../api";
  import type { PlaylistActions } from "../playlists.svelte";
  import Modal from "./Modal.svelte";

  let {
    actions,
    onnavigate,
  }: { actions: PlaylistActions; onnavigate: (id: string) => void } = $props();

  let playlists = $state<Playlist[]>([]);
  let loading = $state(false);
  let listError = $state("");
  let name = $state("");
  let description = $state("");
  let formError = $state("");

  $effect(() => {
    if (!actions.open || actions.step !== "list") return;
    untrack(() => void loadPlaylists());
  });

  async function loadPlaylists() {
    loading = true;
    listError = "";
    try {
      playlists = await api.getUserPlaylists();
    } catch (e) {
      listError = String(e);
    } finally {
      loading = false;
    }
  }

  async function create() {
    const trimmed = name.trim();
    if (!trimmed) {
      formError = "A name is required";
      return;
    }
    formError = "";
    const navigateAfter = actions.fromPicker && actions.pendingTrackIds.length > 0;
    const created = await actions.createAndAdd(trimmed, description.trim());
    if (created) {
      name = "";
      description = "";
      if (navigateAfter) onnavigate(created.uuid);
    }
  }

  function choice(trackId: number): boolean {
    return actions.choices.get(trackId) ?? false;
  }

  function close() {
    actions.close();
    name = "";
    description = "";
    formError = "";
  }
</script>

{#if actions.open}
  <Modal title="Add to playlist" onclose={close}>
    {#if actions.step === "list"}
      {#if loading}
        <p class="muted">Loading…</p>
      {:else if listError}
        <p class="error">{listError}</p>
        <button onclick={loadPlaylists}>Retry</button>
      {:else}
        <ul>
          {#each playlists as p (p.uuid)}
            <li>
              <button disabled={actions.busy} onclick={() => void actions.addTo(p)}>
                {p.title} <span class="muted">{p.numberOfTracks} tracks</span>
              </button>
            </li>
          {/each}
          {#if playlists.length === 0}<li class="muted">No playlists yet</li>{/if}
        </ul>
        <button disabled={actions.busy} onclick={() => actions.startCreate()}>New playlist…</button>
      {/if}
    {:else if actions.step === "create"}
      <p><label>Name <input bind:value={name} /></label></p>
      <p><label>Description <input bind:value={description} /></label></p>
      {#if formError}<p class="error">{formError}</p>{/if}
      <p class="actions">
        <button onclick={close}>Cancel</button>
        <button disabled={actions.busy} onclick={create}>Create</button>
      </p>
    {:else}
      <p class="muted">Some tracks are already in this playlist. Keep one, or add another copy.</p>
      <div class="dupes">
        {#each actions.duplicates as t (t.id)}
          <div class="dupe">
            <div class="card">
              <strong>{t.title}</strong>
              <span class="muted">{t.artists.map((a) => a.name).join(", ")}</span>
              <span class="muted">In playlist</span>
            </div>
            <div class="card">
              <strong>{t.title}</strong>
              <span class="muted">{t.artists.map((a) => a.name).join(", ")}</span>
              <span class="muted">Adding</span>
            </div>
            <div class="choice">
              <button class:on={!choice(t.id)} onclick={() => actions.setChoice(t.id, false)}>
                Keep one
              </button>
              <button class:on={choice(t.id)} onclick={() => actions.setChoice(t.id, true)}>
                Add both
              </button>
            </div>
          </div>
        {/each}
      </div>
      <p class="actions">
        <button onclick={() => actions.setAllChoices(false)}>Keep one of each</button>
        <button onclick={() => actions.setAllChoices(true)}>Add all duplicates</button>
      </p>
      <p class="actions">
        <button onclick={close}>Close (keep one of each)</button>
        <button disabled={actions.busy} onclick={() => void actions.confirmDuplicates()}>
          Confirm
        </button>
      </p>
    {/if}
  </Modal>
{/if}

<style>
  ul {
    padding-left: 18px;
  }
  .dupes {
    max-height: 45vh;
    overflow: auto;
  }
  .dupe {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .choice {
    display: flex;
    gap: 4px;
  }
  .choice .on {
    color: var(--accent);
  }
  .error {
    color: #ff6b6b;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
```

- [ ] **Step 3: Add the TrackTable affordance and Album button**

In `ui/src/lib/components/TrackTable.svelte`, add the prop to the destructure and type:

```ts
  let {
    tracks,
    player,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    onAddToPlaylist,
    maxHeight = "min(60vh, 560px)",
  }: {
    tracks: Track[];
    player: Player;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    /// Renders the "add to playlist" row action. Uses ⊕ to stay distinct
    /// from the "＋" queue action.
    onAddToPlaylist?: (track: Track) => void;
    maxHeight?: string;
  } = $props();
```

Add the button after the "Add to queue" button:

```svelte
            {#if onAddToPlaylist}
              <button
                onclick={() => onAddToPlaylist(t)}
                aria-label="Add to playlist"
                title="Add to playlist"
              >⊕</button>
            {/if}
```

In `ui/src/views/Search.svelte`, add the import, prop, and callback:

```ts
  import type { PlaylistActions } from "../lib/playlists.svelte";
  let {
    nav,
    player,
    favorites,
    playlistActions,
  }: { nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();
```

```svelte
  <TrackTable
    tracks={results.tracks.items}
    {player}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
```

In `ui/src/views/Album.svelte`, add the import and prop:

```ts
  import type { PlaylistActions } from "../lib/playlists.svelte";
  let {
    id,
    player,
    favorites,
    playlistActions,
  }: { id: number; player: Player; favorites: Favorites; playlistActions: PlaylistActions } = $props();
```

Add the header button after the album heart button and pass the table callback:

```svelte
      <button
        onclick={() => playlistActions.openPicker(tracks)}
        aria-label="Add album to playlist"
        title="Add album to playlist"
      >Add to playlist</button>
```

```svelte
  <TrackTable
    {tracks}
    {player}
    {favorites}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
```

- [ ] **Step 4: Wire App**

In `ui/src/App.svelte`, add the imports:

```ts
  import PlaylistPicker from "./lib/components/PlaylistPicker.svelte";
  import { createNotice } from "./lib/notice.svelte";
  import { createPlaylistActions } from "./lib/playlists.svelte";
```

Add the stores next to `favorites`:

```ts
  const notice = createNotice();
  const playlistActions = createPlaylistActions(notice);
```

Add `playlistActions.reset();` after `favorites.reset();` in both the `session_expired` listener and `onLogout`.

Render the notice strip directly after the existing `{#if favorites.error}` block inside `<main class="main">`:

```svelte
      {#if notice.message}
        <div class="notice">
          <span>{notice.message}</span>
          <button onclick={() => notice.dismiss()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
```

Pass the store to Search/Album (the other views get it in Tasks 9–10) and render the picker after `PlayerBar`:

```svelte
      {:else if nav.route.name === "search"}<Search {nav} {player} {favorites} {playlistActions} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} {player} {favorites} {playlistActions} />
```

```svelte
    <PlayerBar {player} {qualityDisplay} {favorites} />
    <PlaylistPicker {actions: playlistActions} onnavigate={(id) => nav.goto({ name: "playlist", id })} />
```

- [ ] **Step 5: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0 errors, 0 warnings; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/Modal.svelte ui/src/lib/components/ConfirmDialog.svelte ui/src/lib/components/PlaylistPicker.svelte ui/src/lib/components/TrackTable.svelte ui/src/views/Search.svelte ui/src/views/Album.svelte ui/src/App.svelte
git commit -m "feat(ui): add the playlist picker and add-to-playlist actions"
```

---

### Task 9: Playlist view editing

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte` (playlist row actions)
- Modify: `ui/src/views/Playlist.svelte`
- Modify: `ui/src/App.svelte` (pass `nav`, `playlistActions`, `notice` to Playlist)

**Interfaces:**
- Consumes: `PlaylistActions` (Task 7), `ConfirmDialog` (Task 8), `Notice` (Task 6), `pager.refresh()`.
- Produces: `TrackTable` accepts `playlistActions?: { onRemove(index: number): void; onMove(index: number, delta: -1 | 1): void; busy: boolean; disabled: boolean }`.

- [ ] **Step 1: Add the row actions to TrackTable**

In `ui/src/lib/components/TrackTable.svelte`, extend the destructure and type:

```ts
    onAddToPlaylist,
    playlistActions,
    maxHeight = "min(60vh, 560px)",
```

```ts
    /// Renders the "add to playlist" row action. Uses ⊕ to stay distinct
    /// from the "＋" queue action.
    onAddToPlaylist?: (track: Track) => void;
    /// Playlist-view row editing (move up/down, remove). `disabled` is set by
    /// the parent until the pager has the complete list.
    playlistActions?: {
      onRemove(index: number): void;
      onMove(index: number, delta: -1 | 1): void;
      busy: boolean;
      disabled: boolean;
    };
    maxHeight?: string;
```

Add after the favorite heart button:

```svelte
            {#if playlistActions}
              <button
                onclick={() => playlistActions.onMove(index, -1)}
                disabled={playlistActions.busy || playlistActions.disabled || index === 0}
                aria-label="Move up"
                title="Move up"
              >↑</button>
              <button
                onclick={() => playlistActions.onMove(index, 1)}
                disabled={playlistActions.busy || playlistActions.disabled || index === tracks.length - 1}
                aria-label="Move down"
                title="Move down"
              >↓</button>
              <button
                onclick={() => playlistActions.onRemove(index)}
                disabled={playlistActions.busy || playlistActions.disabled}
                aria-label="Remove from playlist"
                title="Remove from playlist"
              >✕</button>
            {/if}
```

- [ ] **Step 2: Rewrite the Playlist view**

Replace the `<script>` block of `ui/src/views/Playlist.svelte` with:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Playlist as PlaylistT } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Notice } from "../lib/notice.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    id,
    player,
    favorites,
    playlistActions,
    notice,
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    notice: Notice;
  } = $props();

  let playlist = $state<PlaylistT | null>(null);
  let loading = $state(true);
  let error = $state("");
  let renaming = $state(false);
  let renameTitle = $state("");
  let renameDescription = $state("");
  let confirmingDelete = $state(false);
  let busy = $state(false);

  // `$derived` so navigating to another playlist rebuilds the pager; the new
  // key hydrates from the session cache when that playlist was seen before.
  const pager = $derived(
    createTrackPager(
      `playlist:${id}`,
      (offset, limit) => api.getPlaylistTracksPage(id, offset, limit),
      { dedupe: false, prefetchWindow: 100 },
    ),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      playlist = await api.getPlaylist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  $effect(() => {
    const current = pager;
    untrack(() => current.loadMore());
  });

  function startRename() {
    renaming = true;
    renameTitle = playlist?.title ?? "";
    renameDescription = playlist?.description ?? "";
  }

  async function saveRename() {
    const title = renameTitle.trim();
    if (!title) return;
    try {
      await api.renamePlaylist(id, title, renameDescription.trim());
      if (playlist) {
        playlist.title = title;
        playlist.description = renameDescription.trim() || null;
      }
      renaming = false;
      notice.show("Playlist renamed");
    } catch (e) {
      notice.show(String(e));
    }
  }

  async function confirmDelete() {
    confirmingDelete = false;
    try {
      await api.deletePlaylist(id);
      notice.show("Playlist deleted");
      nav.goto({ name: "library" });
    } catch (e) {
      notice.show(String(e));
    }
  }

  async function removeAt(index: number) {
    if (busy) return;
    busy = true;
    try {
      await api.removePlaylistItems(id, [index]);
      await pager.refresh();
      notice.show("Removed from playlist");
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function moveAt(index: number, delta: -1 | 1) {
    if (busy) return;
    busy = true;
    try {
      await api.movePlaylistItems(id, [index], index + delta);
      await pager.refresh();
    } catch (e) {
      notice.show(String(e));
    } finally {
      busy = false;
    }
  }

  async function playFrom(index: number) {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await pager.ensureAll();
    if (pager.error) return;
    api.playTracks(pager.items.map(trackToQueueItem), 0);
  }
</script>
```

Replace the `{#if playlist}` header and the table with:

```svelte
  {#if playlist}
    <h2>
      {playlist.title}
      <button onclick={startRename}>Rename</button>
      <button onclick={() => (confirmingDelete = true)}>Delete</button>
    </h2>
  {/if}
  {#if renaming}
    <p>
      <input bind:value={renameTitle} aria-label="Playlist name" />
      <input bind:value={renameDescription} aria-label="Playlist description" />
      <button onclick={saveRename}>Save</button>
      <button onclick={() => (renaming = false)}>Cancel</button>
    </p>
  {/if}
```

```svelte
  <TrackTable
    tracks={pager.items}
    {player}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void pager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    playlistActions={{ onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done }}
  />
```

Add inside the final `{:else}` branch (the loaded state), after the `{#if pager.loading}` line and before that branch's closing `{/if}`:

```svelte
  {#if confirmingDelete}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${playlist?.title ?? "this playlist"}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (confirmingDelete = false)}
    />
  {/if}
```

- [ ] **Step 3: Pass the new props from App**

In `ui/src/App.svelte`, change the Playlist route line to:

```svelte
      {:else if nav.route.name === "playlist"}<Playlist {nav} id={nav.route.id} {player} {favorites} {playlistActions} {notice} />
```

- [ ] **Step 4: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0 errors, 0 warnings; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/TrackTable.svelte ui/src/views/Playlist.svelte ui/src/App.svelte
git commit -m "feat(ui): edit playlists from the playlist view"
```

---

### Task 10: Library playlist management

**Files:**
- Modify: `ui/src/views/Library.svelte`
- Modify: `ui/src/App.svelte` (pass `playlistActions` and `notice` to Library)

**Interfaces:**
- Consumes: `PlaylistActions.revision` (Task 7), `ConfirmDialog` (Task 8), `Notice` (Task 6).
- Produces: Library "New playlist" (opens the store's create step), inline rename, delete with confirm; a `$effect` refetches the playlist list when `playlistActions.revision` changes.

- [ ] **Step 1: Rewrite the Library imports and props**

In `ui/src/views/Library.svelte`, replace the imports and prop block with:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { api, trackToQueueItem, type Album, type Playlist } from "../lib/api";
  import ConfirmDialog from "../lib/components/ConfirmDialog.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { createTrackPager } from "../lib/pager.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Notice } from "../lib/notice.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  let {
    nav,
    player,
    favorites,
    playlistActions,
    notice,
  }: {
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    notice: Notice;
  } = $props();

  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);
  let error = $state("");
  let editingUuid = $state<string | null>(null);
  let editTitle = $state("");
  let deleting = $state<Playlist | null>(null);
  // Captured at mount so the revision effect only refreshes after a create
  // that happens while this view is alive.
  let revisionSeen = playlistActions.revision;

  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );

  async function load() {
    loading = true;
    error = "";
    try {
      [albums, playlists] = await Promise.all([api.getFavoriteAlbums(), api.getUserPlaylists()]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadPlaylists() {
    try {
      playlists = await api.getUserPlaylists();
    } catch (e) {
      notice.show(String(e));
    }
  }

  $effect(() => {
    load();
  });

  // The first page paints fast, then the pager prefetches the remainder in the
  // background. `untrack` keeps this a one-shot mount action.
  $effect(() => {
    untrack(() => favoritesPager.loadMore());
  });

  // An unfavorite from anywhere (this table, the player bar, another view)
  // removes the row from the loaded list without refetching — unless it has
  // since been re-favorited, in which case it stays.
  $effect(() => {
    favorites.trackRevision;
    const loaded = new Set(favoritesPager.items.map((t) => t.id));
    for (const id of favorites.pendingRemovalsToDrop(loaded)) favoritesPager.removeItem(id);
  });

  // A playlist created from the picker while this view is mounted.
  $effect(() => {
    const revision = playlistActions.revision;
    if (revision === revisionSeen) return;
    revisionSeen = revision;
    untrack(() => void loadPlaylists());
  });

  function startRename(p: Playlist) {
    editingUuid = p.uuid;
    editTitle = p.title;
  }

  async function saveRename(p: Playlist) {
    const title = editTitle.trim();
    if (!title) return;
    try {
      await api.renamePlaylist(p.uuid, title, p.description ?? "");
      playlists = playlists.map((item) => (item.uuid === p.uuid ? { ...item, title } : item));
      editingUuid = null;
      notice.show("Playlist renamed");
    } catch (e) {
      notice.show(String(e));
    }
  }

  async function confirmDelete() {
    const target = deleting;
    if (!target) return;
    try {
      await api.deletePlaylist(target.uuid);
      playlists = playlists.filter((item) => item.uuid !== target.uuid);
      notice.show(`Deleted ${target.title}`);
    } catch (e) {
      notice.show(String(e));
    } finally {
      deleting = null;
    }
  }

  async function playFrom(index: number) {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(favoritesPager.items.map(trackToQueueItem), index);
  }

  async function playAll() {
    await favoritesPager.ensureAll();
    if (favoritesPager.error) return;
    api.playTracks(favoritesPager.items.map(trackToQueueItem), 0);
  }
</script>
```

- [ ] **Step 2: Update the markup**

Replace the Playlists section with:

```svelte
  <h3>
    Playlists
    <button onclick={() => playlistActions.openCreate()}>New playlist</button>
  </h3>
  <ul>
    {#each playlists as p (p.uuid)}
      <li>
        {#if editingUuid === p.uuid}
          <input bind:value={editTitle} aria-label="Playlist name" />
          <button onclick={() => void saveRename(p)}>Save</button>
          <button onclick={() => (editingUuid = null)}>Cancel</button>
        {:else}
          <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
          <button onclick={() => startRename(p)}>Rename</button>
          <button onclick={() => (deleting = p)}>Delete</button>
        {/if}
      </li>
    {/each}
    {#if playlists.length === 0}<li class="muted">No playlists</li>{/if}
  </ul>
```

Add the table callback:

```svelte
  <TrackTable
    tracks={favoritesPager.items}
    {player}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void favoritesPager.loadMore()}
    onAddToPlaylist={(t) => playlistActions.openPicker([t])}
  />
```

Add before the final `{/if}`:

```svelte
  {#if deleting}
    <ConfirmDialog
      title="Delete playlist"
      message={`Delete "${deleting.title}"? This cannot be undone.`}
      onconfirm={confirmDelete}
      oncancel={() => (deleting = null)}
    />
  {/if}
```

- [ ] **Step 3: Pass the new props from App**

In `ui/src/App.svelte`, change the Library route line to:

```svelte
      {:else if nav.route.name === "library"}<Library {nav} {player} {favorites} {playlistActions} {notice} />
```

- [ ] **Step 4: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0 errors, 0 warnings; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/views/Library.svelte ui/src/App.svelte
git commit -m "feat(ui): manage playlists from the library"
```

---

### Task 11: Full gates, smoke checklist, and docs refresh

**Files:**
- Create: `docs/superpowers/plans/notes/plan7-smoke.md`
- Modify: `docs/superpowers/plans/notes/plan6-smoke.md`
- Commit: the Plan 7 spec and plan

- [ ] **Step 1: Run every gate and record the output**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green (0 failures; clippy and svelte-check 0 warnings). Record the exact counts in the checklist below.

- [ ] **Step 2: Write the smoke checklist**

Create `docs/superpowers/plans/notes/plan7-smoke.md`:

```markdown
# Plan 7 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Create a playlist from Library → it appears in Library and in the official TIDAL app.
- [ ] Create a playlist from the picker on an album ("New playlist…") → the album's tracks are added and the app navigates to the new playlist.
- [ ] Add a single track from Search, Album, Library, and Playlist views → the picker lists playlists with correct counts → the notice reports the number added.
- [ ] Add a whole album from the Album header → every track lands, order preserved.
- [ ] Add tracks that already exist → the duplicates step shows both cards per collision; "Keep one" leaves the playlist unchanged; "Add both" duplicates the track (verify in the official app); "Add all duplicates" applies to every row.
- [ ] Remove one of two identical rows in a playlist → only the clicked occurrence disappears (verify counts in the official app).
- [ ] Move a row up and down → order matches the official app after the refetch; buttons are disabled at the first/last row and while loading.
- [ ] Rename a playlist from Library (inline) and from the Playlist header → both surfaces and the official app agree.
- [ ] Delete a playlist from Library and from the Playlist view → gone from both; deleting the open playlist navigates to Library.
- [ ] Delete the playlist that is currently playing → playback continues unaffected.
- [ ] Unavailable-row index probe: open a playlist that contains an unavailable track, then remove/move the row after it → the intended track changes (not its neighbor).
- [ ] Large playlist (>1k tracks): open, add, remove, and move → refetch completes without a visible stall; editing buttons are disabled until the list is done.
- [ ] Network off → any write shows the notice with the error and changes nothing.
- [ ] Session expiry during a write → re-login; the picker is closed and later edits work.
- [ ] Restart the app → playlists and their contents match the official app.

## Known caveats
- Writes are index-based; a concurrent edit from the official app between load
  and write can shift indices — the refetch after the write shows the result.
- No `If-None-Match`/ETag guard yet (P7-D8).
- Only your own playlists are editable; TIDAL rejects others with 403.
- Pre-existing duplicates are never auto-deduped.
- If TIDAL omits `addedItemIds`, the client assumes every requested track was
  added and the duplicate dialog is skipped.
```

- [ ] **Step 3: Refresh the Plan 6 evidence**

In `docs/superpowers/plans/notes/plan6-smoke.md`, append under the `## Automated checks` bullet list:

```markdown
- Re-verified 2026-09-24 after the final favorites fix on `main`: cargo test 227 passed / 0 failed / 1 ignored; clippy clean (exit 0); vitest 56 passed across 6 files (favorites 12, pager 28); svelte-check 0 errors / 0 warnings. The counts above were recorded at 3973a38, one commit before the fix; the store's removal signal is now `pendingRemovalsToDrop(loaded)` (not `lastUnfavoritedTrackId`) as described in the plan/spec.
```

- [ ] **Step 4: Commit the docs**

```bash
git add docs/superpowers/specs/2026-09-24-oxitide-plan7-playlists-design.md docs/superpowers/plans/2026-09-24-plan7-playlists.md docs/superpowers/plans/notes/plan7-smoke.md docs/superpowers/plans/notes/plan6-smoke.md
git commit -m "docs: add the plan 7 spec and plan; refresh the plan 6 evidence"
```

---

## Self-Review

- **Spec coverage:** mutation plumbing + v2 base (§5.1 → Task 1); playlist module with reads moved and six writes (§5.2 → Tasks 2–4); commands (§5.3 → Task 5); actions store + duplicate flow (§5.4 → Task 7); notice store (§5.5 → Task 6); components incl. picker/create/duplicates and row actions (§5.6 → Tasks 8–10); IPC table (§6 → Task 5); error handling (§8 → Tasks 1/3/7 tests, notice paths); testing (§9 → each task + Task 11); performance (§11 → no polling, single refetch, chunked adds; no extra work added); risks (§12 → Tasks 4 probes and Task 11 caveats); prep docs (§15 → Task 11).
- **Placeholder scan:** the only `<fill in>` markers are the gate counts in the Task 11 checklist template, filled from the actual run in Step 1.
- **Type consistency:** `create_playlist`/`rename_playlist`/`delete_playlist`/`add_playlist_tracks`/`remove_playlist_items`/`move_playlist_items` keep the same names and signatures across client (Tasks 3–4), commands (Task 5), `api.ts` (Task 7), and views (Tasks 9–10); `PlaylistActions` members match the store, the picker, and the tests; `Notice.message`/`show`/`dismiss` match App and the views; `TrackTable`'s `onAddToPlaylist` and `playlistActions` props match their call sites.
- **Review Focus coverage:** empty-playlist `toIndex=0` (Task 4), 403 → `Forbidden` (Task 3) with notice paths (Task 7), 30+ duplicates panel scroll (Task 8 CSS + smoke), delete-while-playing and refetch-failure (smoke in Task 11).
