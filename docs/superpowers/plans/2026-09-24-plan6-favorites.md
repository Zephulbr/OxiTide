# Plan 6 — Favorites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Favorite/unfavorite tracks, albums, and artists from anywhere in OxiTide, with hearts that reflect the account's real state and a Library list that stays consistent.

**Architecture:** The TIDAL client gains its first write verbs (form POST / DELETE with the existing single-flight refresh-on-401 and a bounded 429 `Retry-After` retry). Three toggle commands plus one `get_favorite_ids` hydration command are exposed to the UI. A `createFavorites()` store (matching the existing `createNav`/`createPlayer` factory pattern) owns three reactive `SvelteSet<number>` id sets, hydrates once after login, and toggles optimistically. Hearts render in `TrackTable` rows, Album/Artist headers, and the PlayerBar.

**Tech Stack:** Rust (reqwest, serde, wiremock for tests), Tauri 2 commands, Svelte 5 runes (`$state`, `$effect`, `SvelteSet` from `svelte/reactivity`), vitest.

**Spec:** `docs/superpowers/specs/2026-09-24-oxitide-plan6-favorites-design.md`

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`.
- `invoke` sends camelCase args; Rust command parameters stay snake_case (`track_id` ↔ `trackId`).
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs`.
- All TIDAL HTTP goes through `TidalClient`; writes reuse `try_refresh` for single-flight refresh-on-401.
- Tests: wiremock only, never the live TIDAL API; `MemoryStore` only, never `KeyringStore`; never log token values.
- No new Rust crates and no new npm packages.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
- Version stays 0.2.0; no packaging or config changes.

## Review Focus

- **Empty favorites:** an account with zero favorite tracks/albums/artists must hydrate to empty sets with no error and render hollow hearts. Test in Task 5.
- **Reset during in-flight hydration:** logging out (or a session expiry) while `get_favorite_ids` is in flight must not repopulate the cleared sets. Test in Task 5.
- **Failed toggle:** a network error or rate limit must revert the heart and surface a message, never leave the wrong state. Rust tests in Task 1, store test in Task 5.
- **Repeat clicks while pending:** clicking the same heart twice before the first request resolves must send one request. Test in Task 5.
- **Unavailable favorited track:** TIDAL's favorites list omits unavailable tracks, so their hearts read hollow; this cannot be unit-tested and is recorded as a caveat in the Task 10 smoke checklist.

---

### Task 1: Write verbs in `TidalClient`

**Files:**
- Modify: `crates/oxitide-tidal/src/client.rs` (public methods after `get`, helpers around `decode`, tests at the bottom)
- Test: `crates/oxitide-tidal/src/client.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `access_token()`, `try_refresh(stale)`, `Error` enum.
- Produces: `post_form(&self, path: &str, form: &[(&str, String)]) -> Result<()>`, `delete(&self, path: &str) -> Result<()>`, private `send_mutation`, `retry_after`, `decode_status`, `decode_empty`.

- [ ] **Step 1: Write the failing tests**

Extend the test imports at the top of the tests module in `crates/oxitide-tidal/src/client.rs`:

```rust
    use wiremock::matchers::{body_string_contains, header, method, path};
```

(replacing the current `use wiremock::matchers::{method, path};`), then append these tests inside the `mod tests` block:

```rust
    #[tokio::test]
    async fn post_form_sends_form_and_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .and(body_string_contains("trackId=7"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_maps_no_content_to_ok() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE")).and(path("/users/1/favorites/tracks/7"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client.delete("users/1/favorites/tracks/7").await.unwrap();
    }

    #[tokio::test]
    async fn post_form_refreshes_once_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store.clone());
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn post_form_retries_once_on_429_with_retry_after() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));
        let responder = {
            let attempts = attempts.clone();
            move |_req: &wiremock::Request| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(429).insert_header("retry-after", "0")
                } else {
                    ResponseTemplate::new(200)
                }
            }
        };
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .respond_with(responder)
            .expect(2)
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn post_form_gives_up_after_a_second_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/1/favorites/tracks"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .expect(2)
            .mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        let err = client
            .post_form("users/1/favorites/tracks", &[("trackId", "7".to_string())])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::RateLimited), "expected RateLimited, got {err:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal client::tests`
Expected: compile error — no method named `post_form` / `delete`.

- [ ] **Step 3: Implement the write verbs**

In `crates/oxitide-tidal/src/client.rs`, insert these two public methods directly after the `get` method (before `get_all_pages`):

```rust
    /// Send a form-encoded POST, refreshing once on 401 and honoring a
    /// `Retry-After` hint once on 429.
    pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<()> {
        let token = self.access_token();
        let mut resp = self.send_mutation(reqwest::Method::POST, path, form, &token).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            resp = self
                .send_mutation(reqwest::Method::POST, path, form, &self.access_token())
                .await?;
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            tokio::time::sleep(Self::retry_after(&resp)).await;
            resp = self
                .send_mutation(reqwest::Method::POST, path, form, &self.access_token())
                .await?;
        }
        Self::decode_empty(resp).await
    }

    /// Send a DELETE, refreshing once on 401 and honoring a `Retry-After` hint
    /// once on 429.
    pub async fn delete(&self, path: &str) -> Result<()> {
        let token = self.access_token();
        let mut resp = self.send_mutation(reqwest::Method::DELETE, path, &[], &token).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            resp = self
                .send_mutation(reqwest::Method::DELETE, path, &[], &self.access_token())
                .await?;
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            tokio::time::sleep(Self::retry_after(&resp)).await;
            resp = self
                .send_mutation(reqwest::Method::DELETE, path, &[], &self.access_token())
                .await?;
        }
        Self::decode_empty(resp).await
    }
```

Insert the private helpers after `send_get`:

```rust
    async fn send_mutation(
        &self,
        method: reqwest::Method,
        path: &str,
        form: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response> {
        let query: Vec<(&str, String)> = vec![("countryCode", self.session.country_code.clone())];
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut request = self.http.request(method, url).bearer_auth(token).query(&query);
        if !form.is_empty() {
            request = request.form(form);
        }
        Ok(request.send().await?)
    }

    /// `Retry-After` in whole seconds, capped at 5s; a missing or HTTP-date
    /// value falls back to 1s.
    fn retry_after(resp: &reqwest::Response) -> std::time::Duration {
        resp.headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|secs| std::time::Duration::from_secs(secs.min(5)))
            .unwrap_or(std::time::Duration::from_secs(1))
    }
```

Replace the existing `decode` method with the shared status mapper plus the two decoders:

```rust
    async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        Ok(Self::decode_status(resp).await?.json().await?)
    }

    /// Map a non-success status to an error, returning the response otherwise.
    async fn decode_status(resp: reqwest::Response) -> Result<reqwest::Response> {
        match resp.status() {
            s if s.is_success() => Ok(resp),
            reqwest::StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
            reqwest::StatusCode::FORBIDDEN => Err(Error::Forbidden),
            reqwest::StatusCode::NOT_FOUND => Err(Error::NotFound(resp.url().path().to_string())),
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(Error::RateLimited),
            s => Err(Error::Api { status: s.as_u16(), message: resp.text().await.unwrap_or_default() }),
        }
    }

    async fn decode_empty(resp: reqwest::Response) -> Result<()> {
        Self::decode_status(resp).await.map(|_| ())
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass, including the five new ones.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/client.rs
git commit -m "feat(tidal): add write verbs with a bounded 429 retry"
```

---

### Task 2: Favorites add/remove and id hydration

**Files:**
- Modify: `crates/oxitide-tidal/src/library.rs`
- Test: `crates/oxitide-tidal/src/library.rs` (inline tests)

**Interfaces:**
- Consumes: `TidalClient::post_form` / `delete` (Task 1), existing `user_base()`, `Favorited<T>`, `get_all_pages`.
- Produces: `FavoriteIds { tracks: Vec<u64>, albums: Vec<u64>, artists: Vec<u64> }` (public, `Serialize`); `set_favorite_track/album/artist(&self, id: u64, favorite: bool) -> Result<()>`; `favorite_ids(&self) -> Result<FavoriteIds>`.

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-tidal/src/library.rs`, extend the test imports:

```rust
    use wiremock::matchers::{body_string_contains, method, path, query_param};
```

(replacing the current `use wiremock::matchers::{method, path, query_param};`), then append:

```rust
    #[tokio::test]
    async fn set_favorite_track_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/tracks"))
            .and(body_string_contains("trackId=7"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/tracks/7"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_track(7, true).await.unwrap();
        client.set_favorite_track(7, false).await.unwrap();
    }

    #[tokio::test]
    async fn set_favorite_album_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/albums"))
            .and(body_string_contains("albumId=9"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/albums/9"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_album(9, true).await.unwrap();
        client.set_favorite_album(9, false).await.unwrap();
    }

    #[tokio::test]
    async fn set_favorite_artist_adds_and_removes() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/users/42/favorites/artists"))
            .and(body_string_contains("artistId=11"))
            .respond_with(ResponseTemplate::new(200)).mount(&server).await;
        Mock::given(method("DELETE")).and(path("/users/42/favorites/artists/11"))
            .respond_with(ResponseTemplate::new(204)).mount(&server).await;
        let client = client(&server);
        client.set_favorite_artist(11, true).await.unwrap();
        client.set_favorite_artist(11, false).await.unwrap();
    }

    #[tokio::test]
    async fn favorite_ids_walks_all_three_lists() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("limit", "1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 1, "title": "A"}}, {"item": {"id": 2, "title": "B"}}],
                "totalNumberOfItems": 2, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/albums"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 3, "title": "C"}}],
                "totalNumberOfItems": 1, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users/42/favorites/artists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"item": {"id": 4, "name": "D"}}],
                "totalNumberOfItems": 1, "limit": 1000, "offset": 0
            })))
            .mount(&server).await;
        let ids = client(&server).favorite_ids().await.unwrap();
        assert_eq!(ids.tracks, vec![1, 2]);
        assert_eq!(ids.albums, vec![3]);
        assert_eq!(ids.artists, vec![4]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal library::tests`
Expected: compile error — no `set_favorite_track`, `set_favorite_album`, `set_favorite_artist`, `favorite_ids`.

- [ ] **Step 3: Implement**

At the top of `crates/oxitide-tidal/src/library.rs`, change the serde import and add the page-size constant:

```rust
use serde::{Deserialize, Serialize};
```

```rust
/// Window used when hydrating favorite ids; the favorites endpoint is known to
/// honor 1000-row requests.
const FAVORITE_ID_PAGE_SIZE: u32 = 1000;
```

Add the public DTO above `impl TidalClient`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct FavoriteIds {
    pub tracks: Vec<u64>,
    pub albums: Vec<u64>,
    pub artists: Vec<u64>,
}
```

Inside `impl TidalClient`, after `all_user_playlists`, add:

```rust
    async fn set_favorite(&self, kind: &str, id_key: &str, id: u64, favorite: bool) -> Result<()> {
        if favorite {
            self.post_form(&format!("{}/{kind}", self.user_base()), &[(id_key, id.to_string())])
                .await
        } else {
            self.delete(&format!("{}/{kind}/{id}", self.user_base())).await
        }
    }

    pub async fn set_favorite_track(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("tracks", "trackId", id, favorite).await
    }

    pub async fn set_favorite_album(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("albums", "albumId", id, favorite).await
    }

    pub async fn set_favorite_artist(&self, id: u64, favorite: bool) -> Result<()> {
        self.set_favorite("artists", "artistId", id, favorite).await
    }

    /// Walk the three favorites lists in parallel and return only the ids.
    pub async fn favorite_ids(&self) -> Result<FavoriteIds> {
        let base = self.user_base();
        let (tracks, albums, artists) = tokio::join!(
            self.get_all_pages::<Favorited<Track>>(
                &format!("{base}/tracks"),
                &[],
                FAVORITE_ID_PAGE_SIZE
            ),
            self.get_all_pages::<Favorited<Album>>(
                &format!("{base}/albums"),
                &[],
                FAVORITE_ID_PAGE_SIZE
            ),
            self.get_all_pages::<Favorited<Artist>>(
                &format!("{base}/artists"),
                &[],
                FAVORITE_ID_PAGE_SIZE
            ),
        );
        Ok(FavoriteIds {
            tracks: tracks?.into_iter().map(|f| f.item.id).collect(),
            albums: albums?.into_iter().map(|f| f.item.id).collect(),
            artists: artists?.into_iter().map(|f| f.item.id).collect(),
        })
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p oxitide-tidal -- -D warnings
git add crates/oxitide-tidal/src/library.rs
git commit -m "feat(tidal): add favorite toggles and id hydration"
```

---

### Task 3: Tauri commands and registration

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs` (`generate_handler![...]`)
- Test: `crates/oxitide-app/src/commands.rs` (inline tests)

**Interfaces:**
- Consumes: `TidalClient::set_favorite_track/album/artist`, `TidalClient::favorite_ids`, `FavoriteIds` (Task 2); existing `require_client`, `state.fail_tidal`.
- Produces: commands `set_favorite_track`, `set_favorite_album`, `set_favorite_artist`, `get_favorite_ids` reachable from `invoke`.

- [ ] **Step 1: Write the failing test**

In the `mod tests` block of `crates/oxitide-app/src/commands.rs`, append:

```rust
    #[test]
    fn favorite_ids_dto_serializes_arrays() {
        let dto = oxitide_tidal::library::FavoriteIds {
            tracks: vec![1, 2],
            albums: vec![3],
            artists: vec![],
        };
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["tracks"], serde_json::json!([1, 2]));
        assert_eq!(value["albums"], serde_json::json!([3]));
        assert_eq!(value["artists"], serde_json::json!([]));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app favorite_ids_dto_serializes_arrays`
Expected: compile error — `FavoriteIds` is not public / does not exist yet if Task 2 was skipped; with Task 2 done it compiles, so the test is really pinning the IPC shape (it passes once the type exists). If it already passes, continue.

- [ ] **Step 3: Implement the commands**

Add the import near the other `oxitide_tidal` imports in `crates/oxitide-app/src/commands.rs`:

```rust
use oxitide_tidal::library::FavoriteIds;
```

Add the commands after `get_favorite_artists` (before `get_user_playlists`):

```rust
#[tauri::command]
pub async fn set_favorite_track(
    state: State<'_, AppState>,
    track_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_track(track_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_favorite_album(
    state: State<'_, AppState>,
    album_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_album(album_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn set_favorite_artist(
    state: State<'_, AppState>,
    artist_id: u64,
    favorite: bool,
) -> Result<(), String> {
    require_client(&state)?
        .set_favorite_artist(artist_id, favorite)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_favorite_ids(state: State<'_, AppState>) -> Result<FavoriteIds, String> {
    require_client(&state)?.favorite_ids().await.map_err(|e| state.fail_tidal(e))
}
```

Register them in `crates/oxitide-app/src/main.rs` inside `generate_handler![...]`, after `commands::get_playlist_tracks_page,`:

```rust
            commands::set_favorite_track,
            commands::set_favorite_album,
            commands::set_favorite_artist,
            commands::get_favorite_ids,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app`
Expected: all tests pass, including `favorite_ids_dto_serializes_arrays`.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy --workspace -- -D warnings
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose favorite commands"
```

---

### Task 4: Pager row removal and cache invalidation

**Files:**
- Modify: `ui/src/lib/pager.svelte.ts`
- Test: `ui/src/lib/pager.test.ts`

**Interfaces:**
- Consumes: existing `cache` map, `publish()`, `inflight`, `items`/`total`/`nextOffset`.
- Produces: `Pager.removeItem(id: number): void`; exported `removePagerItem(key: string, id: number): void`; exported `invalidatePager(key: string): void`.

- [ ] **Step 1: Write the failing tests**

In `ui/src/lib/pager.test.ts`, extend the import:

```ts
import {
  clearPagerCache,
  createTrackPager,
  invalidatePager,
  removePagerItem,
} from "./pager.svelte";
```

and append these tests inside `describe("track pager", ...)`:

```ts
  it("removeItem drops the row, decrements total and nextOffset, and updates the cache", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2), track(3)], offset, limit, 3),
    );
    const pager = createTrackPager("remove-item", load, { pageSize: 3, prefetch: false });
    await pager.loadMore();
    expect(pager.done).toBe(true);

    pager.removeItem(2);

    expect(ids(pager.items)).toEqual([1, 3]);
    expect(pager.total).toBe(2);
    expect(pager.unavailable).toBe(0);

    const fresh = createTrackPager("remove-item", load, { pageSize: 3, prefetch: false });
    expect(ids(fresh.items)).toEqual([1, 3]);
    expect(fresh.total).toBe(2);
  });

  it("removeItem ignores ids that are not loaded", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const pager = createTrackPager("remove-missing", load, { pageSize: 1, prefetch: false });
    await pager.loadMore();

    pager.removeItem(99);

    expect(ids(pager.items)).toEqual([1]);
    expect(pager.total).toBe(1);
  });

  it("removePagerItem corrects a cache entry with no mounted pager", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1), track(2)], offset, limit, 2),
    );
    const a = createTrackPager("remove-cached", load, { pageSize: 2, prefetch: false });
    await a.loadMore();

    removePagerItem("remove-cached", 1);

    const b = createTrackPager("remove-cached", load, { pageSize: 2, prefetch: false });
    expect(ids(b.items)).toEqual([2]);
    expect(b.total).toBe(1);
  });

  it("invalidatePager forces the next mount to refetch", async () => {
    const load = vi.fn(async (offset: number, limit: number) =>
      page([track(1)], offset, limit, 1),
    );
    const a = createTrackPager("invalidate", load, { pageSize: 1, prefetch: false });
    await a.loadMore();

    invalidatePager("invalidate");

    const b = createTrackPager("invalidate", load, { pageSize: 1, prefetch: false });
    expect(b.items).toEqual([]);
  });

  it("defers a removal until an in-flight page settles", async () => {
    let resolveSecond: (page: TrackPage) => void = () => {};
    const second = new Promise<TrackPage>((resolve) => {
      resolveSecond = resolve;
    });
    let calls = 0;
    const load = vi.fn(async (offset: number, limit: number) => {
      calls += 1;
      if (calls === 1) return page([track(1)], offset, limit, 3);
      return second;
    });
    const pager = createTrackPager("remove-inflight", load, { pageSize: 1, prefetch: false });
    await pager.loadMore();
    expect(pager.done).toBe(false);

    const pending = pager.loadMore();
    pager.removeItem(1);
    expect(ids(pager.items)).toEqual([1]);

    resolveSecond(page([track(2)], 1, 1, 3));
    await pending;

    expect(ids(pager.items)).toEqual([2]);
    expect(pager.total).toBe(2);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: FAIL — `pager.removeItem is not a function` and missing exports.

- [ ] **Step 3: Implement**

In `ui/src/lib/pager.svelte.ts`, add `removeItem` to the `Pager` interface after `refresh()`:

```ts
  refresh(): Promise<void>;
  /// Drop one row locally (an unfavorited track); no-op when absent.
  removeItem(id: number): void;
```

After `clearPagerCache`, add the two module-level helpers:

```ts
/// Drop one row from a cached list when no pager instance is mounted.
/// `nextOffset` is the first unscanned row, so removing a scanned row shifts
/// it down by one.
export function removePagerItem(key: string, id: number) {
  const entry = cache.get(key);
  if (!entry) return;
  if (!entry.items.some((t) => t.id === id)) return;
  entry.items = entry.items.filter((t) => t.id !== id);
  if (entry.total > 0) entry.total -= 1;
  entry.nextOffset = Math.max(0, entry.nextOffset - 1);
}

/// Forget a cached list so the next mount refetches it.
export function invalidatePager(key: string) {
  cache.delete(key);
}
```

Inside `createTrackPager`, after the `inflight` declaration, add:

```ts
  /// Removals requested while a page fetch is in flight are applied after it
  /// settles, so a window fetched before the removal cannot re-add the row.
  const removals = new Set<number>();
```

After `applyWindow`, add:

```ts
  function applyRemoval(id: number) {
    if (!items.some((t) => t.id === id)) return;
    items = items.filter((t) => t.id !== id);
    if (total > 0) total -= 1;
    nextOffset = Math.max(0, nextOffset - 1);
    publish();
  }

  function removeItem(id: number) {
    if (inflight) {
      removals.add(id);
      return;
    }
    applyRemoval(id);
  }
```

Extend `loadMore`'s `finally` block:

```ts
      } finally {
        loading = false;
        inflight = null;
        if (removals.size > 0) {
          for (const id of removals) applyRemoval(id);
          removals.clear();
        }
      }
```

Add `removeItem` to the returned object, after `refresh`:

```ts
    refresh,
    removeItem,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/pager.test.ts`
Expected: PASS (all pager tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/pager.svelte.ts ui/src/lib/pager.test.ts
git commit -m "feat(ui): support removing a row from a paged list"
```

---

### Task 5: API wrappers and the favorites store

**Files:**
- Modify: `ui/src/lib/api.ts`
- Create: `ui/src/lib/favorites.svelte.ts`
- Test: `ui/src/lib/favorites.test.ts`

**Interfaces:**
- Consumes: `removePagerItem`, `invalidatePager` (Task 4); commands from Task 3.
- Produces: `FavoriteIds { tracks: number[]; albums: number[]; artists: number[] }`; `api.getFavoriteIds/setFavoriteTrack/setFavoriteAlbum/setFavoriteArtist`; `createFavorites(): Favorites` with `hydrated`, `error`, `trackRevision`, `lastUnfavoritedTrackId`, `hydrate()`, `hasTrack/hasAlbum/hasArtist`, `toggleTrack/toggleAlbum/toggleArtist`, `dismissError()`, `reset()`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/favorites.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FavoriteIds, Track, TrackPage } from "./api";
import { api } from "./api";
import { createFavorites } from "./favorites.svelte";
import { clearPagerCache, createTrackPager } from "./pager.svelte";

vi.mock("./api", () => ({
  api: {
    getFavoriteIds: vi.fn(),
    setFavoriteTrack: vi.fn(),
    setFavoriteAlbum: vi.fn(),
    setFavoriteArtist: vi.fn(),
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

describe("favorites store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearPagerCache();
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [], albums: [], artists: [] });
    vi.mocked(api.setFavoriteTrack).mockResolvedValue(undefined);
    vi.mocked(api.setFavoriteAlbum).mockResolvedValue(undefined);
    vi.mocked(api.setFavoriteArtist).mockResolvedValue(undefined);
  });

  it("hydrates the three sets from one call", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [3], artists: [4] });
    const favorites = createFavorites();

    await favorites.hydrate();

    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(favorites.hasTrack(9)).toBe(false);
    expect(favorites.hasAlbum(3)).toBe(true);
    expect(favorites.hasArtist(4)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("dedupes concurrent hydrates", async () => {
    const favorites = createFavorites();

    await Promise.all([favorites.hydrate(), favorites.hydrate()]);

    expect(api.getFavoriteIds).toHaveBeenCalledTimes(1);
  });

  it("leaves the store hollow after a failed hydrate and retries on the next call", async () => {
    vi.mocked(api.getFavoriteIds).mockRejectedValueOnce(new Error("offline"));
    const favorites = createFavorites();

    await favorites.hydrate();
    expect(favorites.hydrated).toBe(false);
    expect(favorites.error).toContain("offline");

    vi.mocked(api.getFavoriteIds).mockResolvedValueOnce({ tracks: [1], albums: [], artists: [] });
    await favorites.hydrate();
    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(2);
  });

  it("toggles optimistically and calls the matching command", async () => {
    const favorites = createFavorites();

    expect(await favorites.toggleTrack(5)).toBe(true);
    expect(api.setFavoriteTrack).toHaveBeenCalledWith(5, true);
    expect(favorites.hasTrack(5)).toBe(true);

    expect(await favorites.toggleTrack(5)).toBe(false);
    expect(api.setFavoriteTrack).toHaveBeenLastCalledWith(5, false);
    expect(favorites.hasTrack(5)).toBe(false);

    expect(await favorites.toggleAlbum(6)).toBe(true);
    expect(api.setFavoriteAlbum).toHaveBeenCalledWith(6, true);
    expect(await favorites.toggleArtist(7)).toBe(true);
    expect(api.setFavoriteArtist).toHaveBeenCalledWith(7, true);
  });

  it("reverts a failed toggle and surfaces the error", async () => {
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("rate limited"));
    const favorites = createFavorites();

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(false);
    expect(favorites.error).toContain("rate limited");

    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [5], albums: [], artists: [] });
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(true);
  });

  it("ignores a repeat click while a toggle is pending", async () => {
    let release: (() => void) | null = null;
    vi.mocked(api.setFavoriteTrack).mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );
    const favorites = createFavorites();

    const first = favorites.toggleTrack(5);
    expect(await favorites.toggleTrack(5)).toBe(true);
    expect(api.setFavoriteTrack).toHaveBeenCalledTimes(1);

    release!();
    await first;
    expect(favorites.hasTrack(5)).toBe(true);
  });

  it("drops an unfavorited track from the cached favorites list and bumps the revision", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1, 2], albums: [], artists: [] });
    const load = vi.fn(
      async (offset: number, limit: number): Promise<TrackPage> => ({
        items: [track(1), track(2)],
        offset,
        limit,
        total: 2,
      }),
    );
    const pager = createTrackPager("favorites", load, { pageSize: 2, prefetch: false });
    await pager.loadMore();

    const favorites = createFavorites();
    await favorites.hydrate();
    expect(await favorites.toggleTrack(1)).toBe(false);

    expect(favorites.trackRevision).toBe(1);
    expect(favorites.lastUnfavoritedTrackId).toBe(1);
    const fresh = createTrackPager("favorites", load, { pageSize: 2, prefetch: false });
    expect(fresh.items.map((t) => t.id)).toEqual([2]);
    expect(fresh.total).toBe(1);
  });

  it("does not repopulate after a reset during an in-flight hydrate", async () => {
    let resolveIds: ((ids: FavoriteIds) => void) | null = null;
    vi.mocked(api.getFavoriteIds).mockImplementation(
      () =>
        new Promise<FavoriteIds>((resolve) => {
          resolveIds = resolve;
        }),
    );
    const favorites = createFavorites();

    const hydrating = favorites.hydrate();
    favorites.reset();
    resolveIds!({ tracks: [1], albums: [], artists: [] });
    await hydrating;

    expect(favorites.hydrated).toBe(false);
    expect(favorites.hasTrack(1)).toBe(false);
  });

  it("clears everything on reset", async () => {
    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [1], albums: [2], artists: [3] });
    const favorites = createFavorites();
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));
    await favorites.toggleTrack(9);

    favorites.reset();

    expect(favorites.hydrated).toBe(false);
    expect(favorites.error).toBeNull();
    expect(favorites.hasTrack(1)).toBe(false);
    expect(favorites.hasAlbum(2)).toBe(false);
    expect(favorites.hasArtist(3)).toBe(false);
    expect(favorites.trackRevision).toBe(0);
    expect(favorites.lastUnfavoritedTrackId).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/favorites.test.ts`
Expected: FAIL — cannot resolve `./favorites.svelte`.

- [ ] **Step 3: Add the API wrappers**

In `ui/src/lib/api.ts`, add the type after `SessionInfoDto`:

```ts
export interface FavoriteIds { tracks: number[]; albums: number[]; artists: number[] }
```

Add the wrappers after `getPlaylistTracksPage`:

```ts
  getFavoriteIds: () => invoke<FavoriteIds>("get_favorite_ids"),
  setFavoriteTrack: (trackId: number, favorite: boolean) =>
    invoke<void>("set_favorite_track", { trackId, favorite }),
  setFavoriteAlbum: (albumId: number, favorite: boolean) =>
    invoke<void>("set_favorite_album", { albumId, favorite }),
  setFavoriteArtist: (artistId: number, favorite: boolean) =>
    invoke<void>("set_favorite_artist", { artistId, favorite }),
```

- [ ] **Step 4: Implement the store**

Create `ui/src/lib/favorites.svelte.ts`:

```ts
import { SvelteSet } from "svelte/reactivity";
import { api } from "./api";
import { invalidatePager, removePagerItem } from "./pager.svelte";

export interface Favorites {
  readonly hydrated: boolean;
  readonly error: string | null;
  readonly trackRevision: number;
  readonly lastUnfavoritedTrackId: number | null;
  hydrate(): Promise<void>;
  hasTrack(id: number): boolean;
  hasAlbum(id: number): boolean;
  hasArtist(id: number): boolean;
  toggleTrack(id: number): Promise<boolean | null>;
  toggleAlbum(id: number): Promise<boolean | null>;
  toggleArtist(id: number): Promise<boolean | null>;
  dismissError(): void;
  reset(): void;
}

type Kind = "track" | "album" | "artist";

/// Favorite state for the signed-in account. Hydrated once per session from
/// `get_favorite_ids`; toggles are optimistic and revert on failure.
export function createFavorites(): Favorites {
  const tracks = new SvelteSet<number>();
  const albums = new SvelteSet<number>();
  const artists = new SvelteSet<number>();
  const pending = new Set<string>();
  let hydrated = $state(false);
  let error = $state<string | null>(null);
  let trackRevision = $state(0);
  let lastUnfavoritedTrackId = $state<number | null>(null);
  let hydratePromise: Promise<void> | null = null;
  let generation = 0;

  function setOf(kind: Kind): SvelteSet<number> {
    if (kind === "album") return albums;
    if (kind === "artist") return artists;
    return tracks;
  }

  function callToggle(kind: Kind, id: number, favorite: boolean): Promise<void> {
    if (kind === "album") return api.setFavoriteAlbum(id, favorite);
    if (kind === "artist") return api.setFavoriteArtist(id, favorite);
    return api.setFavoriteTrack(id, favorite);
  }

  async function hydrate(): Promise<void> {
    if (hydrated) return;
    if (hydratePromise) return hydratePromise;
    const gen = generation;
    hydratePromise = (async () => {
      try {
        const ids = await api.getFavoriteIds();
        if (gen !== generation) return;
        for (const id of ids.tracks) tracks.add(id);
        for (const id of ids.albums) albums.add(id);
        for (const id of ids.artists) artists.add(id);
        hydrated = true;
      } catch (e) {
        if (gen !== generation) return;
        error = String(e);
      } finally {
        if (gen === generation) hydratePromise = null;
      }
    })();
    return hydratePromise;
  }

  async function toggle(kind: Kind, id: number): Promise<boolean | null> {
    const set = setOf(kind);
    const key = `${kind}:${id}`;
    if (pending.has(key)) return set.has(id);
    const was = set.has(id);
    const next = !was;
    if (next) set.add(id);
    else set.delete(id);
    pending.add(key);
    try {
      await callToggle(kind, id, next);
      error = null;
      if (kind === "track") {
        if (next) {
          invalidatePager("favorites");
        } else {
          removePagerItem("favorites", id);
          lastUnfavoritedTrackId = id;
          trackRevision += 1;
        }
      }
      return next;
    } catch (e) {
      if (was) set.add(id);
      else set.delete(id);
      error = String(e);
      return null;
    } finally {
      pending.delete(key);
    }
  }

  return {
    get hydrated() { return hydrated; },
    get error() { return error; },
    get trackRevision() { return trackRevision; },
    get lastUnfavoritedTrackId() { return lastUnfavoritedTrackId; },
    hydrate,
    hasTrack: (id) => tracks.has(id),
    hasAlbum: (id) => albums.has(id),
    hasArtist: (id) => artists.has(id),
    toggleTrack: (id) => toggle("track", id),
    toggleAlbum: (id) => toggle("album", id),
    toggleArtist: (id) => toggle("artist", id),
    dismissError: () => {
      error = null;
    },
    reset: () => {
      generation += 1;
      hydratePromise = null;
      tracks.clear();
      albums.clear();
      artists.clear();
      pending.clear();
      hydrated = false;
      error = null;
      trackRevision = 0;
      lastUnfavoritedTrackId = null;
    },
  };
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm --prefix ui test`
Expected: PASS (pager + favorites + existing tests).

- [ ] **Step 6: Type-check and commit**

```bash
npm --prefix ui run check
git add ui/src/lib/api.ts ui/src/lib/favorites.svelte.ts ui/src/lib/favorites.test.ts
git commit -m "feat(ui): add the favorites store and API wrappers"
```

---

### Task 6: App wiring — hydration, reset, notice strip

**Files:**
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `createFavorites()` (Task 5).
- Produces: a `favorites` instance passed to views/PlayerBar in Tasks 7–8; hydration after login; reset on logout/expiry; a transient error strip.

- [ ] **Step 1: Wire the store**

In `ui/src/App.svelte`, add `untrack` to the Svelte import:

```ts
  import { untrack } from "svelte";
```

Add the store import and instance next to `player`:

```ts
  import { createFavorites } from "./lib/favorites.svelte";
```

```ts
  const nav = createNav();
  const player = createPlayer();
  const favorites = createFavorites();
```

Add the hydration and notice-timer effects after the existing `playerReady` effect:

```ts
  $effect(() => {
    if (loggedIn) untrack(() => favorites.hydrate());
  });

  $effect(() => {
    if (!favorites.error) return;
    const timer = setTimeout(() => favorites.dismissError(), 6000);
    return () => clearTimeout(timer);
  });
```

Add `favorites.reset()` in the `session_expired` listener (after `loggedIn = false;`):

```ts
      loggedIn = false;
      favorites.reset();
```

and in `onLogout` (after `clearPagerCache();`):

```ts
    clearPagerCache();
    favorites.reset();
    loggedIn = false;
```

Render the strip at the top of `<main class="main">`:

```svelte
    <main class="main">
      {#if favorites.error}
        <div class="notice">
          <span>{favorites.error}</span>
          <button onclick={() => favorites.dismissError()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
      {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
```

Add a scoped style block at the end of the file:

```svelte
<style>
  .notice {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 0 0 12px;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    color: #ff6b6b;
  }
  .notice button {
    margin-left: auto;
  }
</style>
```

- [ ] **Step 2: Verify**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/App.svelte
git commit -m "feat(ui): hydrate favorites after login and surface errors"
```

---

### Task 7: Track-row hearts

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/views/Search.svelte`, `ui/src/views/Album.svelte`, `ui/src/views/Playlist.svelte`, `ui/src/views/Library.svelte`
- Modify: `ui/src/App.svelte` (pass `favorites` to Search/Library/Album/Playlist)

**Interfaces:**
- Consumes: `Favorites` (Task 5).
- Produces: `TrackTable` requires a `favorites: Favorites` prop; every track list renders a heart button.

- [ ] **Step 1: Add the heart to `TrackTable`**

In `ui/src/lib/components/TrackTable.svelte`, add the type import:

```ts
  import type { Favorites } from "../favorites.svelte";
```

Add `favorites` to the destructured props and its type:

```ts
  let {
    tracks,
    player,
    favorites,
    onPlay,
    onPlayAll,
    onEndReached,
    maxHeight = "min(60vh, 560px)",
  }: {
    tracks: Track[];
    player: Player;
    favorites: Favorites;
    onPlay?: (index: number) => void | Promise<void>;
    onPlayAll?: () => void | Promise<void>;
    /// Called when the list is scrolled near its end; used for lazy paging.
    onEndReached?: () => void;
    maxHeight?: string;
  } = $props();
```

Add the heart button in the row action cell after the "Add to queue" button:

```svelte
            <button
              class:on={favorites.hasTrack(t.id)}
              onclick={() => favorites.toggleTrack(t.id)}
              aria-label={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
              title={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
            >{favorites.hasTrack(t.id) ? "♥" : "♡"}</button>
```

Add to the style block:

```css
  .on {
    color: var(--accent);
  }
```

- [ ] **Step 2: Thread the prop through the views**

`ui/src/views/Search.svelte`:

```ts
  import type { Favorites } from "../lib/favorites.svelte";
  let { nav, player, favorites }: { nav: Nav; player: Player; favorites: Favorites } = $props();
```

```svelte
  <TrackTable tracks={results.tracks.items} {player} {favorites} />
```

`ui/src/views/Album.svelte`:

```ts
  import type { Favorites } from "../lib/favorites.svelte";
  let { id, player, favorites }: { id: number; player: Player; favorites: Favorites } = $props();
```

```svelte
  <TrackTable {tracks} {player} {favorites} />
```

`ui/src/views/Playlist.svelte`:

```ts
  import type { Favorites } from "../lib/favorites.svelte";
  let { id, player, favorites }: { id: string; player: Player; favorites: Favorites } = $props();
```

```svelte
  <TrackTable
    tracks={pager.items}
    {player}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void pager.loadMore()}
  />
```

`ui/src/views/Library.svelte` — rename the local pager (it currently shadows the store name) and add the prop:

```ts
  import type { Favorites } from "../lib/favorites.svelte";
  let { nav, player, favorites }: { nav: Nav; player: Player; favorites: Favorites } = $props();
```

Rename every reference to the pager in this file from `favorites` to `favoritesPager`:

```ts
  const favoritesPager = createTrackPager("favorites", (offset, limit) =>
    api.getFavoriteTracksPage(offset, limit),
  );
```

```ts
  $effect(() => {
    untrack(() => favoritesPager.loadMore());
  });

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
```

and in the markup:

```svelte
  <p>
    <button onclick={() => favoritesPager.refresh()}>Refresh</button>
    <span class="muted">
      {#if favoritesPager.unavailable > 0}
        {favoritesPager.items.length} tracks ({favoritesPager.unavailable} unavailable)
      {:else}
        {favoritesPager.items.length} tracks
      {/if}
    </span>
  </p>
  <TrackTable
    tracks={favoritesPager.items}
    {player}
    {favorites}
    onPlay={playFrom}
    onPlayAll={playAll}
    onEndReached={() => void favoritesPager.loadMore()}
  />
  {#if favoritesPager.error}
    <ErrorState message={favoritesPager.error} onRetry={() => favoritesPager.loadMore()} />
  {/if}
  {#if favoritesPager.loading}<p class="muted">Loading…</p>{/if}
```

- [ ] **Step 3: Pass the store from App**

In `ui/src/App.svelte`:

```svelte
      {:else if nav.route.name === "search"}<Search {nav} {player} {favorites} />
      {:else if nav.route.name === "library"}<Library {nav} {player} {favorites} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} {player} {favorites} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} {player} {favorites} />
```

- [ ] **Step 4: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0/0; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/TrackTable.svelte ui/src/views/Search.svelte ui/src/views/Album.svelte ui/src/views/Playlist.svelte ui/src/views/Library.svelte ui/src/App.svelte
git commit -m "feat(ui): add favorite hearts to track rows"
```

---

### Task 8: Album, Artist, and PlayerBar hearts

**Files:**
- Modify: `ui/src/views/Album.svelte`, `ui/src/views/Artist.svelte`, `ui/src/lib/components/PlayerBar.svelte`
- Modify: `ui/src/App.svelte` (pass `favorites` to Artist and PlayerBar)

**Interfaces:**
- Consumes: `Favorites` (Task 5).
- Produces: header hearts for albums/artists and a now-playing heart.

- [ ] **Step 1: Album header heart**

In `ui/src/views/Album.svelte`, inside the `{#if album}` block, replace the header with:

```svelte
    <h2>
      {album.title}
      <button
        class:on={favorites.hasAlbum(album.id)}
        onclick={() => favorites.toggleAlbum(album.id)}
        aria-label={favorites.hasAlbum(album.id) ? "Unfavorite album" : "Favorite album"}
        title={favorites.hasAlbum(album.id) ? "Unfavorite album" : "Favorite album"}
      >{favorites.hasAlbum(album.id) ? "♥" : "♡"}</button>
    </h2>
```

- [ ] **Step 2: Artist header heart**

In `ui/src/views/Artist.svelte`, add the import and prop:

```ts
  import type { Favorites } from "../lib/favorites.svelte";
  let { id, favorites }: { id: number; favorites: Favorites } = $props();
```

Replace the header block:

```svelte
{:else if artist}
  <h2>
    {artist.name}
    <button
      class:on={favorites.hasArtist(artist.id)}
      onclick={() => favorites.toggleArtist(artist.id)}
      aria-label={favorites.hasArtist(artist.id) ? "Unfavorite artist" : "Favorite artist"}
      title={favorites.hasArtist(artist.id) ? "Unfavorite artist" : "Favorite artist"}
    >{favorites.hasArtist(artist.id) ? "♥" : "♡"}</button>
  </h2>
  <p class="muted">Artist</p>
{/if}
```

- [ ] **Step 3: PlayerBar heart**

In `ui/src/lib/components/PlayerBar.svelte`, add the import and prop:

```ts
  import type { Favorites } from "../favorites.svelte";
  let { player, qualityDisplay, favorites }: { player: Player; qualityDisplay: QualityDisplay; favorites: Favorites } = $props();
```

Add the heart after the `.now` div, still inside the `{#if player.state.current}` block:

```svelte
  {#if player.state.current}
    {@const trackId = player.state.current.track_id}
    <div class="now">
      <strong>{player.state.current.title}</strong>
      <span class="muted">{player.state.current.artist}</span>
      <span class="badge" title={player.state.stream ? "Actual stream quality" : "Preferred quality"}>{quality}</span>
      {#if player.state.bit_perfect}
        <span class="badge" title="Exclusive output at the source's native format">Bit-perfect</span>
      {/if}
    </div>
    <button
      class:on={favorites.hasTrack(trackId)}
      onclick={() => favorites.toggleTrack(trackId)}
      aria-label={favorites.hasTrack(trackId) ? "Unfavorite" : "Favorite"}
      title={favorites.hasTrack(trackId) ? "Unfavorite" : "Favorite"}
    >{favorites.hasTrack(trackId) ? "♥" : "♡"}</button>
```

Add to the existing style block:

```css
  .on { color: var(--accent); }
```

- [ ] **Step 4: Pass the store from App**

In `ui/src/App.svelte`:

```svelte
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {favorites} />
```

```svelte
    <PlayerBar {player} {qualityDisplay} {favorites} />
```

- [ ] **Step 5: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0/0; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add ui/src/views/Album.svelte ui/src/views/Artist.svelte ui/src/lib/components/PlayerBar.svelte ui/src/App.svelte
git commit -m "feat(ui): add album, artist, and now-playing hearts"
```

---

### Task 9: Keep the Library list in sync with unfavorites

**Files:**
- Modify: `ui/src/views/Library.svelte`

**Interfaces:**
- Consumes: `favorites.trackRevision`, `favorites.lastUnfavoritedTrackId` (Task 5); `favoritesPager.removeItem` (Task 4).
- Produces: an unfavorite from any view removes the row from a mounted Library list.

- [ ] **Step 1: Add the sync effect**

In `ui/src/views/Library.svelte`, after the pager mount effect, add:

```ts
  // An unfavorite from anywhere (this table, the player bar, another view)
  // removes the row from the loaded list without refetching.
  $effect(() => {
    favorites.trackRevision;
    const id = favorites.lastUnfavoritedTrackId;
    if (id === null) return;
    if (favoritesPager.items.some((t) => t.id === id)) favoritesPager.removeItem(id);
  });
```

- [ ] **Step 2: Verify**

Run: `npm --prefix ui run check; npm --prefix ui test`
Expected: check 0/0; all tests pass.

- [ ] **Step 3: Commit**

```bash
git add ui/src/views/Library.svelte
git commit -m "feat(ui): drop unfavorited rows from the library list"
```

---

### Task 10: Full gates and smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/plan6-smoke.md`

- [ ] **Step 1: Run every gate and record the output**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green (0 failures; clippy and svelte-check 0 warnings). Record the exact counts in the checklist file below.

- [ ] **Step 2: Write the smoke checklist**

Create `docs/superpowers/plans/notes/plan6-smoke.md`:

```markdown
# Plan 6 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and a browser login. Run with
> `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Hearts are filled for a track, an album, and an artist already in the library (check Album view, Artist view, Search results, PlayerBar).
- [ ] Favorite a track from Album view → open Library → the track is in the list and the count matches.
- [ ] Unfavorite a track from the Library list → the row disappears immediately and the count drops by one.
- [ ] Unfavorite from Album view → open Library → the row is gone and the count is right.
- [ ] Favorite/unfavorite an album and an artist from their pages → Library reflects it on the next visit.
- [ ] Network off → click a heart → it reverts and the notice strip shows the error; dismiss works.
- [ ] Session expiry (delete the keyring entry, then click Library) → re-login → hearts hydrate again.
- [ ] Restart the app → hearts still reflect the library.

## Known caveats
- A favorited track that TIDAL marks unavailable is omitted from the favorites list, so its heart can read hollow.
- Favoriting a track while the Library is already mounted does not insert the row until Refresh or a remount.
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/plan6-smoke.md
git commit -m "docs: add the plan 6 smoke checklist"
```

---

## Self-Review

- **Spec coverage:** write verbs + 429 (§5.1 → Task 1); favorites API + `favorite_ids` (§5.2 → Task 2); commands (§5.3 → Task 3); store + hydration (§5.4 → Task 5); App wiring + notice (§5.5 → Task 6); hearts (§5.6 → Tasks 7–8); pager removal/invalidation (§5.7 → Task 4); Library sync (§5.6 → Task 9); tests + smoke (§9 → Tasks 1–5, 10); performance notes require no code.
- **Placeholder scan:** the only `<fill in>` markers are in the Task 10 checklist template, filled from the actual gate output in Step 1.
- **Type consistency:** `FavoriteIds` (Rust `library.rs` ↔ TS `api.ts`), `post_form`/`delete`, `removeItem`/`removePagerItem`/`invalidatePager`, and `Favorites` member names match across tasks.
- **Review Focus coverage:** empty favorites, reset-during-hydrate, failed toggle, repeat clicks (Task 5 tests); unavailable track (Task 10 caveat).
