# Playlist Write Hotfix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every playlist write work against live TIDAL again: send the playlist `etag` in `If-None-Match` (retrying once on 412), and stop treating a permissions 401 as an expired session.

**Architecture:** `TidalClient` gains ETag-aware GET/mutation plumbing and a rule that a 401 surviving a *successful* token refresh maps to `Forbidden` (logout only when the refresh itself fails). `playlists.rs` fetches the playlist `etag` before each write, sends it, and retries once on 412 with a fresh ETag (refreshing it between >100-track chunks). `errors.ts` maps a persistent 412 to friendly copy.

**Tech Stack:** Rust (reqwest, wiremock), Svelte/TypeScript (vitest). No new crates or npm packages.

**Spec:** none — this is a live-API hotfix; the diagnosis below is the authority. It was found during the Plan 8 smoke run (2026-09-25).

## Diagnosis (root causes)

- **412 `subStatus 7002` "must send the correct Etag value in the If-None-Match header"** — TIDAL requires `If-None-Match: <etag>` on playlist-modifying requests. `TidalClient::get` drops response headers, so the `etag` returned by `GET /playlists/{id}` (already fetched by `add_playlist_tracks`) is discarded; `send_mutation` cannot attach a header; rename/delete/remove/move never fetch an ETag at all. P7-D8 deferred this; TIDAL now enforces it.
- **"unauthorized" + logout on a playlist you do not own** — TIDAL answers **401** (not 403) for such edits. `mutate` sees 401 → refreshes the token successfully → retries → still 401 → `Error::Unauthorized` → `AppState::fail_tidal` emits `session_expired` → the app logs out. A 401 that survives a successful refresh is a permission error, not an expired session; only a failed refresh (or missing refresh token) means the session is gone, and `try_refresh` already returns `Unauthorized` for that.

## Global Constraints

- Rust tests use `wiremock` only, never the live TIDAL API; never log token values.
- No new Rust crates and no new npm packages; version stays 0.2.0.
- Exact backend error strings stay stable (`error.rs` Display impls).
- TDD: every behavior change lands with a test watched failing first.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **ETag copied verbatim:** the value includes quotes (e.g. `"e1"`); it must be sent unchanged, and a missing header must not be invented. Task 1/2 tests pin the quoted value.
- **401 after a successful refresh never logs out:** it must map to `Forbidden`; only `try_refresh` failures yield `Unauthorized`. Task 1 tests both paths; Task 2's rename-of-unowned flow ends in `Forbidden`.
- **412 retry happens exactly once:** a second 412 is a real conflict and must surface; the retry refetches the ETag first. Task 2 tests one retry and no loop.
- **Chunked adds rotate the ETag:** a >100-track add updates the playlist after each chunk, so the next chunk must use a freshly fetched ETag. Task 2 updates the chunk test to expect two GETs and two different `If-None-Match` values.
- **Already-fetched playlist reads still behave:** rename/delete/remove/move now do one extra GET per write; a 404/401 there fails before any write, and existing single-flight refresh/429 handling still applies. Task 1 keeps every existing client test green.

---

### Task 1: ETag-aware client plumbing and the 401-after-refresh rule

**Files:**
- Modify: `crates/oxitide-tidal/src/client.rs`

**Interfaces:**
- Consumes: existing `access_token()`, `try_refresh(stale)`, `send_get`, `retry_after`, `decode`, `decode_status`.
- Produces: `get_with_etag<T>(path, params) -> Result<(T, Option<String>)>`; `post_form_with_etag(path, form, etag: Option<&str>) -> Result<()>`; `post_form_json_with_etag<T>(path, params, form, etag: Option<&str>) -> Result<T>`; `delete_with_etag(path, etag: Option<&str>) -> Result<()>`. Existing `get`/`post_form`/`delete`/`put_json`/`post_form_json` signatures unchanged (they pass `None`).

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-tidal/src/client.rs`, append these tests inside `mod tests` (after `put_json_retries_once_on_429`):

```rust
    #[tokio::test]
    async fn get_with_etag_returns_the_header_verbatim() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"e1\"")
                    .set_body_json(serde_json::json!({"value": 1})),
            )
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
        let (thing, etag) = client.get_with_etag::<Thing>("playlists/p", &[]).await.unwrap();
        assert_eq!(thing.value, 1);
        assert_eq!(etag.as_deref(), Some("\"e1\""));
    }

    #[tokio::test]
    async fn post_form_with_etag_sends_if_none_match() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/playlists/p/items"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(200))
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
            .post_form_with_etag("playlists/p/items", &[], Some("\"e1\""))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Forbidden), "expected Forbidden, got {err:?}");
    }

    #[tokio::test]
    async fn post_form_401_after_a_successful_refresh_is_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/playlists/p"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/playlists/p"))
            .and(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;
        let client = TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            session(),
            tokens(),
            Arc::new(MemoryStore::default()),
        );
        let err = client.post_form("playlists/p", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Forbidden), "expected Forbidden, got {err:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal get_with_etag_returns_the_header_verbatim post_form_with_etag_sends_if_none_match get_401_after_a_successful_refresh_is_forbidden post_form_401_after_a_successful_refresh_is_forbidden`
Expected: FAIL — `get_with_etag`/`post_form_with_etag` do not exist; the 401 tests expect `Forbidden` but get `Unauthorized`.

- [ ] **Step 3: Implement the plumbing**

In `client.rs`:

Replace `get` and add `get_with_etag`:

```rust
    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let (value, _) = self.get_with_etag(path, params).await?;
        Ok(value)
    }

    /// GET that also returns the response `etag` header (playlist writes must
    /// send it verbatim in `If-None-Match`), sharing `get`'s single-flight
    /// refresh-on-401. A 401 that survives a *successful* refresh is an
    /// authorization failure, not an expired session.
    pub async fn get_with_etag<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<(T, Option<String>)> {
        let token = self.access_token();
        let mut resp = self.send_get(path, params, &token).await?;
        let mut refreshed = false;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self.send_get(path, params, &self.access_token()).await?;
        }
        if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Forbidden);
        }
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        Ok((Self::decode(resp).await?, etag))
    }
```

Replace the four public wrappers and add ETag variants:

```rust
    pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<()> {
        self.post_form_with_etag(path, form, None).await
    }

    /// Form POST with an optional `If-None-Match` value (playlist writes).
    pub async fn post_form_with_etag(
        &self,
        path: &str,
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<()> {
        self.mutate(reqwest::Method::POST, &self.base_url, path, &[], form, etag)
            .await
            .map(|_| ())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.delete_with_etag(path, None).await
    }

    /// DELETE with an optional `If-None-Match` value (playlist writes).
    pub async fn delete_with_etag(&self, path: &str, etag: Option<&str>) -> Result<()> {
        self.mutate(reqwest::Method::DELETE, &self.base_url, path, &[], &[], etag)
            .await
            .map(|_| ())
    }
```

Update `put_json` and `post_form_json` to pass `None`, and add:

```rust
    /// Form POST returning JSON with an optional `If-None-Match` value.
    pub async fn post_form_json_with_etag<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<T> {
        let resp = self
            .mutate(reqwest::Method::POST, &self.base_url, path, params, form, etag)
            .await?;
        Ok(resp.json().await?)
    }
```

Update `mutate` and `send_mutation` to carry the ETag and the new 401 rule:

```rust
    async fn mutate(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<reqwest::Response> {
        let country_code = base == self.base_url.as_str();
        let token = self.access_token();
        let mut refreshed = false;
        let mut resp = self
            .send_mutation(&method, base, path, params, form, &token, country_code, etag)
            .await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh(&token).await?;
            refreshed = true;
            resp = self
                .send_mutation(
                    &method,
                    base,
                    path,
                    params,
                    form,
                    &self.access_token(),
                    country_code,
                    etag,
                )
                .await?;
        }
        // A 401 that survives a successful refresh is a permission failure;
        // only a failed refresh means the session is gone.
        if refreshed && resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Forbidden);
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
                    etag,
                )
                .await?;
        }
        Self::decode_status(resp).await
    }
```

In `send_mutation`, add the parameter and header:

```rust
        country_code: bool,
        etag: Option<&str>,
    ) -> Result<reqwest::Response> {
```

```rust
        if !form.is_empty() {
            request = request.form(form);
        }
        if let Some(value) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, value);
        }
        Ok(request.send().await?)
```

- [ ] **Step 4: Run the whole crate's tests**

Run: `cargo test -p oxitide-tidal`
Expected: all tests pass, including the pre-existing `get_refreshes_once_on_401`, `refresh_rejected_with_invalid_grant_is_unauthorized`, and `post_form_retries_once_on_429_with_retry_after`.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/client.rs
git commit -m "fix(tidal): send playlist etags and stop logging out on permission 401s"
```

---

### Task 2: Playlist writes fetch, send, and refresh the ETag

**Files:**
- Modify: `crates/oxitide-tidal/src/playlists.rs`

**Interfaces:**
- Consumes: `get_with_etag`, `post_form_with_etag`, `post_form_json_with_etag`, `delete_with_etag`, `Error::Api { status: 412, .. }` (Task 1).
- Produces: same public playlist method signatures; each write now sends `If-None-Match` and retries once on 412 with a fresh ETag.

- [ ] **Step 1: Update the failing tests**

In `crates/oxitide-tidal/src/playlists.rs`:

1. Add `header` to the wiremock matcher import:

```rust
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
```

2. Give the shared helper an ETag header:

```rust
    async fn playlist_page(server: &MockServer, total: u32) {
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"e1\"")
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": total
                    })),
            )
            .mount(server)
            .await;
    }
```

3. In `add_playlist_tracks_skips_duplicates_and_reports_added`, assert the header on the POST:

```rust
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .and(query_param("limit", "1"))
            .and(body_string_contains("trackIds=7"))
            .and(body_string_contains("toIndex=5"))
            .and(body_string_contains("onDupes=SKIP"))
            .and(body_string_contains("onArtifactNotFound=SKIP"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [7]
            })))
            .mount(&server).await;
```

4. In `add_playlist_tracks_dedupes_and_chunks`, replace `playlist_page(&server, 5).await;` with a responder that rotates the ETag per GET, and match each POST's header:

```rust
        let gets = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let responder = {
            let gets = gets.clone();
            move |_req: &wiremock::Request| {
                let n = gets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .insert_header("etag", if n == 0 { "\"e1\"" } else { "\"e2\"" })
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": 5
                    }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
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
            .and(header("if-none-match", "\"e1\""))
            .and(query_param("limit", "100"))
            .and(body_string_contains(&format!("trackIds={first_chunk}")))
            .and(body_string_contains("toIndex=5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": ids[..100].to_vec()
            })))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e2\""))
            .and(query_param("limit", "1"))
            .and(body_string_contains("trackIds=101"))
            .and(body_string_contains("toIndex=105"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "addedItemIds": [101]
            })))
            .mount(&server).await;
```

5. Add the new 412-retry test after `add_playlist_tracks_dedupes_and_chunks`:

```rust
    #[tokio::test]
    async fn add_playlist_tracks_retries_once_on_stale_etag() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let server = MockServer::start().await;
        let gets = Arc::new(AtomicUsize::new(0));
        let responder = {
            let gets = gets.clone();
            move |_req: &wiremock::Request| {
                let n = gets.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .insert_header("etag", if n == 0 { "\"e1\"" } else { "\"e2\"" })
                    .set_body_json(serde_json::json!({
                        "uuid": "p-uuid", "title": "P", "numberOfTracks": 0
                    }))
            }
        };
        Mock::given(method("GET"))
            .and(path("/playlists/p-uuid"))
            .respond_with(responder)
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(412).set_body_json(serde_json::json!({
                "status": 412, "subStatus": 7002,
                "userMessage": "You must send the correct Etag value in the If-None-Match header to modify a playlist"
            })))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items"))
            .and(header("if-none-match", "\"e2\""))
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
```

6. Update the four single-write tests to mount the playlist GET and match the header:

```rust
    #[tokio::test]
    async fn rename_playlist_posts_title_and_description() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid"))
            .and(header("if-none-match", "\"e1\""))
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
        playlist_page(&server, 0).await;
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
    async fn delete_playlist_sends_delete_with_etag() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid"))
            .and(header("if-none-match", "\"e1\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server).await;
        client(&server).delete_playlist("p-uuid").await.unwrap();
    }

    #[tokio::test]
    async fn remove_playlist_items_sends_indices_path() {
        let server = MockServer::start().await;
        playlist_page(&server, 0).await;
        Mock::given(method("DELETE"))
            .and(path("/playlists/p-uuid/items/2,5"))
            .and(header("if-none-match", "\"e1\""))
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
        playlist_page(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/playlists/p-uuid/items/4"))
            .and(header("if-none-match", "\"e1\""))
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

Run: `cargo test -p oxitide-tidal playlist`
Expected: FAIL — the write mocks now require `If-None-Match`, and the 412-retry test fails (the client sends no ETag and does not retry).

- [ ] **Step 3: Implement the ETag flow**

In `playlists.rs`, import the error and add helpers:

```rust
use crate::error::{Error, Result};
```

```rust
/// The playlist plus the `etag` TIDAL requires in `If-None-Match` for writes.
async fn playlist_and_etag(&self, id: &str) -> Result<(Playlist, Option<String>)> {
    self.get_with_etag(&format!("playlists/{id}"), &[]).await
}

/// Just the current playlist `etag`; used between chunks and on a 412 retry.
async fn playlist_etag(&self, id: &str) -> Result<Option<String>> {
    Ok(self.playlist_and_etag(id).await?.1)
}

/// TIDAL's stale-precondition response for playlist writes.
fn is_stale_etag(error: &Error) -> bool {
    matches!(error, Error::Api { status: 412, .. })
}
```

Replace `rename_playlist`:

```rust
    /// Rename a playlist and/or change its description.
    pub async fn rename_playlist(&self, id: &str, title: &str, description: &str) -> Result<()> {
        let form = [
            ("title", title.to_string()),
            ("description", description.to_string()),
        ];
        let path = format!("playlists/{id}");
        let mut etag = self.playlist_etag(id).await?;
        match self.post_form_with_etag(&path, &form, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.post_form_with_etag(&path, &form, etag.as_deref()).await
            }
            result => result,
        }
    }
```

Replace `delete_playlist`:

```rust
    /// Delete a playlist.
    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        let path = format!("playlists/{id}");
        let mut etag = self.playlist_etag(id).await?;
        match self.delete_with_etag(&path, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.delete_with_etag(&path, etag.as_deref()).await
            }
            result => result,
        }
    }
```

Replace `add_playlist_tracks`'s body after dedupe:

```rust
        let (playlist, mut etag) = self.playlist_and_etag(id).await?;
        let chunks: Vec<&[u64]> = requested.chunks(ADD_CHUNK).collect();
        let mut added: Vec<u64> = Vec::new();
        for (index, chunk) in chunks.iter().enumerate() {
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
            let path = format!("playlists/{id}/items");
            let response = match self
                .post_form_json_with_etag(&path, &params, &form, etag.as_deref())
                .await
            {
                Err(e) if is_stale_etag(&e) => {
                    etag = self.playlist_etag(id).await?;
                    self.post_form_json_with_etag(&path, &params, &form, etag.as_deref())
                        .await?
                }
                result => result?,
            };
            match response.added_item_ids {
                Some(values) => {
                    added.extend(values.iter().filter_map(IdOrString::as_id));
                }
                None => added.extend(chunk.iter().copied()),
            }
            // Each write rotates the playlist etag; refresh before the next chunk.
            if index + 1 < chunks.len() {
                etag = self.playlist_etag(id).await?;
            }
        }
        Ok(added)
```

Replace `remove_playlist_items` and `move_playlist_items` bodies after building `list`:

```rust
        let path = format!("playlists/{id}/items/{list}");
        let mut etag = self.playlist_etag(id).await?;
        match self.delete_with_etag(&path, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.delete_with_etag(&path, etag.as_deref()).await
            }
            result => result,
        }
```

```rust
        let path = format!("playlists/{id}/items/{list}");
        let form = [("toIndex", to_index.to_string())];
        let mut etag = self.playlist_etag(id).await?;
        match self.post_form_with_etag(&path, &form, etag.as_deref()).await {
            Err(e) if is_stale_etag(&e) => {
                etag = self.playlist_etag(id).await?;
                self.post_form_with_etag(&path, &form, etag.as_deref()).await
            }
            result => result,
        }
```

- [ ] **Step 4: Run the crate tests**

Run: `cargo test -p oxitide-tidal; cargo clippy -p oxitide-tidal -- -D warnings`
Expected: all pass, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/playlists.rs
git commit -m "fix(tidal): send and refresh playlist etags on every write"
```

---

### Task 3: Friendly copy for a persistent 412

**Files:**
- Modify: `ui/src/lib/errors.ts`
- Modify: `ui/src/lib/errors.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces: `errorMessage` maps `api error 412…` to friendly copy; other strings unchanged.

- [ ] **Step 1: Write the failing test**

Append to `ui/src/lib/errors.test.ts`:

```ts
  it("maps the stale playlist 412", () => {
    expect(
      errorMessage(
        'api error 412: {"status":412,"subStatus":7002,"userMessage":"You must send the correct Etag value in the If-None-Match header to modify a playlist"}',
      ),
    ).toBe("The playlist changed on TIDAL — refresh and try again");
  });
```

(Place it inside the existing `describe("errorMessage", ...)` block.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/errors.test.ts`
Expected: FAIL — the raw string passes through.

- [ ] **Step 3: Implement the mapping**

In `ui/src/lib/errors.ts`, add before the final `return raw;`:

```ts
  if (raw.startsWith("api error 412")) {
    return "The playlist changed on TIDAL — refresh and try again";
  }
```

- [ ] **Step 4: Run the test**

Run: `npm --prefix ui test -- src/lib/errors.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/errors.ts ui/src/lib/errors.test.ts
git commit -m "fix(ui): explain a persistent stale-playlist conflict"
```

---

### Task 4: Docs, smoke note, and full gates

**Files:**
- Modify: `docs/superpowers/plans/notes/plan7-smoke.md`
- Create: `docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md`

- [ ] **Step 1: Run every gate and record the output**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green (0 failures; clippy and svelte-check 0 warnings). Record the exact counts.

- [ ] **Step 2: Update the Plan 7 caveat**

In `docs/superpowers/plans/notes/plan7-smoke.md`, replace the caveat line:

```markdown
- No `If-None-Match`/ETag guard yet (P7-D8).
```

with:

```markdown
- ETag guard (P7-D8) added 2026-09-25: writes fetch the playlist `etag` and send
  it in `If-None-Match`, retrying once on 412 with a refreshed value.
```

- [ ] **Step 3: Write the hotfix smoke note**

Create `docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md`:

```markdown
# Playlist Write Hotfix Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Create a playlist with a track from the picker → it is created and the track lands (no 412 toast).
- [ ] Add tracks to an existing playlist (single and whole album) → no 412; order matches the official app.
- [ ] Remove and move rows in your own playlist → both succeed.
- [ ] Rename and delete your own playlist → both succeed.
- [ ] Rename or delete a playlist you do not own → the toast reads "You can only edit your own playlists" and you stay signed in.
- [ ] While a playlist is open, edit it in the official app, then remove/move a row here → the write either retries transparently or shows the stale-playlist copy, never raw JSON.
- [ ] Add more than 100 tracks (or a >100-track album) to a playlist → all land; the chunk boundary does not 412.

## Known caveats
- If TIDAL returns no `etag` on the playlist GET, writes fall back to the old header-less behavior and any 412 shows the friendly conflict copy.
- The retry is once per request by design; a second conflict surfaces to the user.
```

- [ ] **Step 4: Fill in the recorded gate counts and commit**

Replace each `<fill in>` with the Step 1 output and record the commit hash in the heading.

```bash
git add docs/superpowers/plans/notes/plan7-smoke.md docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md
git commit -m "docs: record the playlist write hotfix smoke checklist"
```

---

## Self-Review

- **Diagnosis coverage:** 412 ETag (Task 1 plumbing + Task 2 writes), 401-after-refresh logout (Task 1 rule, verified by Task 2's unowned-rename flow via `Forbidden`), raw 412 copy (Task 3), docs/verification (Task 4).
- **Plan alignment:** no new crates/packages; public signatures unchanged; wiremock-only tests; one retry by design.
- **Type consistency:** `get_with_etag`, `post_form_with_etag`, `post_form_json_with_etag`, `delete_with_etag`, `playlist_and_etag`, `playlist_etag`, `is_stale_etag` are used with the same shapes in Tasks 1–2.
- **Review Focus coverage:** quoted ETag verbatim (Task 1/2 header matchers), 401 rule (Task 1), single retry (Task 2), chunk rotation (Task 2 updated chunk test), existing client behavior (Task 1 Step 4 full-crate run).
