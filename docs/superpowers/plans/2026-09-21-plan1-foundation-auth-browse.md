# OxiTide Plan 1 — Foundation, Auth & Browse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the OxiTide workspace, implement PKCE login and the TIDAL API client, and build the browse UI (Home, Album, Playlist, Artist, Search, Library) backed by the real API.

**Architecture:** A Cargo workspace with `oxitide-tidal` (auth + API, no UI) and `oxitide-app` (Tauri 2 shell exposing commands/events). The front end is Svelte 5 + Vite, calling Rust via `invoke`. Rust owns all network/auth state; the UI is a pure view layer.

**Tech Stack:** Rust 2021, Tokio, reqwest, serde, thiserror, keyring, wiremock (dev); Tauri 2; Svelte 5 + Vite + TypeScript; Vitest.

**Spec:** `docs/superpowers/specs/2026-09-21-oxitide-phase1-design.md`

## Global Constraints

- Rust edition 2021; MSRV 1.77. Tauri 2.x. Svelte 5 (runes syntax). Node 20+.
- TIDAL API base: `https://api.tidal.com/v1/` (append `countryCode` where the endpoint accepts it).
- Auth endpoints: authorize `https://login.tidal.com/authorize`, token `https://auth.tidal.com/v1/oauth2/token`.
- PKCE client id: `6BDSRdpK9hqEBTgU`. Redirect URI: `https://tidal.com/android/login/auth`. Query params: `appMode=android`, `code_challenge_method=S256`, `response_type=code`, `lang=EN`, `restrict_signup=true`.
- Auth header on every API call: `Authorization: Bearer <access_token>`.
- **No DRM, no key extraction, no credential logging.** Tokens go to the OS keyring only.
- Config/app dir: `%APPDATA%\OxiTide`. Keyring service name: `oxitide`.
- Never log access/refresh tokens.
- Conventional commits (`feat:`, `test:`, `chore:`). Commit at the end of every task.
- Every HTTP-facing unit test uses `wiremock`; every parser test uses an inline JSON fixture. No live network in tests.

## Review Focus

Uncovered input classes most likely to bite a real user, each pinned to a test in its owning task:

1. **Empty/degenerate API payloads** — a list endpoint returning `items: []`, an artistless track, or a missing `cover` must not panic and must render an empty state. (Tasks 2, 6, 7, 8)
2. **Non-200 / network failure** — 401, 403, 404, 429, and connection errors must map to typed `Error` variants, never a panic. (Tasks 5, 6)
3. **Expired/absent token** — an expired access token triggers one refresh + retry; a failed refresh surfaces `Unauthorized` so the UI can re-prompt login. (Task 5)
4. **Partial JSON** — missing optional fields (`cover`, `audioQuality`, `artists`) deserialize to defaults via `#[serde(default)]`. (Task 2)
5. **Pagination** — `offset`/`limit` are passed through and large libraries are fetched in pages without truncation. (Task 8)

---

### Task 1: Workspace, toolchain, and `oxitide-tidal` skeleton

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `rust-toolchain.toml`
- Create: `crates/oxitide-tidal/Cargo.toml`
- Create: `crates/oxitide-tidal/src/lib.rs`
- Create: `crates/oxitide-tidal/src/error.rs`
- Test: `crates/oxitide-tidal/src/error.rs` (`#[cfg(test)]` module)

**Interfaces:**
- Consumes: nothing.
- Produces: `oxitide_tidal::{Error, Result}`; `Error` variants used by every later task.

- [ ] **Step 1: Create the workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/oxitide-tidal", "crates/oxitide-app"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-or-later"

[workspace.dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
url = "2"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.77.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create the `oxitide-tidal` crate manifest**

`crates/oxitide-tidal/Cargo.toml`:
```toml
[package]
name = "oxitide-tidal"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
tokio.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
url.workspace = true
keyring = { version = "3", features = ["windows-native"] }

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 3: Write the failing error-display test**

`crates/oxitide-tidal/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("rate limited")]
    RateLimited,
    #[error("api error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_has_stable_message() {
        assert_eq!(Error::RateLimited.to_string(), "rate limited");
    }

    #[test]
    fn api_error_includes_status_and_message() {
        let e = Error::Api { status: 429, message: "slow down".into() };
        assert_eq!(e.to_string(), "api error 429: slow down");
    }
}
```

- [ ] **Step 4: Create the crate root**

`crates/oxitide-tidal/src/lib.rs`:
```rust
pub mod error;

pub use error::{Error, Result};
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal`
Expected: FAIL to compile — `crates/oxitide-app` is a workspace member that does not exist yet.

Create a temporary placeholder so the workspace resolves:
`crates/oxitide-app/Cargo.toml`:
```toml
[package]
name = "oxitide-app"
version.workspace = true
edition.workspace = true
```
`crates/oxitide-app/src/main.rs`:
```rust
fn main() {}
```
Re-run: `cargo test -p oxitide-tidal`
Expected: PASS — 2 tests pass.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates
git commit -m "chore: scaffold cargo workspace and oxitide-tidal crate"
```

---

### Task 2: Domain models

**Files:**
- Create: `crates/oxitide-tidal/src/models.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline `#[cfg(test)]` in `models.rs`

**Interfaces:**
- Consumes: `serde`.
- Produces (used by Tasks 6–10 and the UI): `Track`, `Album`, `Artist`, `ArtistRef`, `AlbumRef`, `Playlist`, `MediaMetadata`, `ItemPage<T>`, `SearchResults`, `Track::is_hi_res()`.

- [ ] **Step 1: Write the failing deserialization tests**

`crates/oxitide-tidal/src/models.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumRef {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub album: Option<AlbumRef>,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
    #[serde(default, rename = "trackNumber")]
    pub track_number: u32,
    #[serde(default, rename = "volumeNumber")]
    pub volume_number: u32,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default, rename = "mediaMetadata")]
    pub media_metadata: Option<MediaMetadata>,
}

impl Track {
    pub fn is_hi_res(&self) -> bool {
        self.media_metadata
            .as_ref()
            .map(|m| m.tags.iter().any(|t| t == "HIRES_LOSSLESS"))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default, rename = "numberOfTracks")]
    pub number_of_tracks: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, rename = "audioQuality")]
    pub audio_quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub uuid: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "numberOfTracks")]
    pub number_of_tracks: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPage<T> {
    #[serde(default)]
    pub items: Vec<T>,
    #[serde(default, rename = "totalNumberOfItems")]
    pub total_number_of_items: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_track_json_parses() {
        let raw = r#"{
            "id": 77646170, "title": "The Golden Age", "duration": 276,
            "artists": [{"id": 1, "name": "Beck"}],
            "album": {"id": 77646169, "title": "Sea Change", "cover": "abc-def"},
            "audioQuality": "LOSSLESS", "trackNumber": 1, "volumeNumber": 1,
            "explicit": false, "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]}
        }"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert_eq!(t.id, 77646170);
        assert_eq!(t.artists[0].name, "Beck");
        assert_eq!(t.album.as_ref().unwrap().title, "Sea Change");
        assert!(t.is_hi_res());
    }

    #[test]
    fn partial_track_json_parses_with_defaults() {
        let raw = r#"{"id": 5, "title": "No Extras"}"#;
        let t: Track = serde_json::from_str(raw).unwrap();
        assert!(t.artists.is_empty());
        assert!(t.album.is_none());
        assert!(t.audio_quality.is_none());
        assert!(!t.is_hi_res());
    }

    #[test]
    fn empty_item_page_parses() {
        let raw = r#"{"items": [], "totalNumberOfItems": 0, "limit": 100, "offset": 0}"#;
        let p: ItemPage<Track> = serde_json::from_str(raw).unwrap();
        assert!(p.items.is_empty());
        assert_eq!(p.total_number_of_items, 0);
    }
}
```

- [ ] **Step 2: Register the module**

`crates/oxitide-tidal/src/lib.rs`:
```rust
pub mod error;
pub mod models;

pub use error::{Error, Result};
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal models`
Expected: PASS only after models exist — since the code is written above, a missing `mod models` initially causes a compile error. Confirm the crate previously built and now add `mod models`; if you added both at once, run the test to confirm PASS.

Run: `cargo test -p oxitide-tidal models`
Expected: PASS — 3 tests.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add domain models with lenient deserialization"
```

---

### Task 3: PKCE primitives

**Files:**
- Create: `crates/oxitide-tidal/src/pkce.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline in `pkce.rs`

**Interfaces:**
- Consumes: `url`, `Error`.
- Produces: `Pkce { code_verifier: String, code_challenge: String, client_unique_key: String }`, `Pkce::generate()`, `Pkce::login_url(&self, client_id: &str) -> String`.

- [ ] **Step 1: Write the failing tests (RFC 7636 vector)**

`crates/oxitide-tidal/src/pkce.rs`:
```rust
use base64::Engine;
use sha2::{Digest, Sha256};

pub const PKCE_CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";
pub const PKCE_REDIRECT_URI: &str = "https://tidal.com/android/login/auth";
pub const AUTHORIZE_URL: &str = "https://login.tidal.com/authorize";
pub const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";

#[derive(Debug, Clone)]
pub struct Pkce {
    pub code_verifier: String,
    pub code_challenge: String,
    pub client_unique_key: String,
}

impl Pkce {
    pub fn from_parts(code_verifier: String, client_unique_key: String) -> Self {
        let code_challenge = code_challenge(&code_verifier);
        Self { code_verifier, code_challenge, client_unique_key }
    }

    pub fn generate() -> Self {
        let verifier = random_verifier();
        let key = random_unique_key();
        Self::from_parts(verifier, key)
    }

    pub fn login_url(&self, client_id: &str) -> String {
        format!(
            "{base}?response_type=code&redirect_uri={redirect}&client_id={client}&lang=EN\
&appMode=android&client_unique_key={key}&code_challenge={challenge}\
&code_challenge_method=S256&restrict_signup=true",
            base = AUTHORIZE_URL,
            redirect = urlencode(PKCE_REDIRECT_URI),
            client = client_id,
            key = self.client_unique_key,
            challenge = self.code_challenge,
        )
    }
}

/// S256 code challenge: base64url(sha256(verifier)) without padding.
pub fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn random_verifier() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random_unique_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("{:016x}", u64::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_matches_rfc7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn login_url_contains_expected_params() {
        let pkce = Pkce::from_parts("verifier-abc".into(), "0011223344556677".into());
        let url = pkce.login_url(PKCE_CLIENT_ID);
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("appMode=android"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Ftidal.com%2Fandroid%2Flogin%2Fauth"));
        assert!(url.contains(&format!("code_challenge={}", pkce.code_challenge)));
    }

    #[test]
    fn generate_is_nonthrowing() {
        let p = Pkce::generate();
        assert!(p.code_verifier.len() >= 43);
        assert!(!p.code_challenge.is_empty());
    }
}
```

- [ ] **Step 2: Add `base64` and `sha2` dependencies**

`crates/oxitide-tidal/Cargo.toml` — add under `[dependencies]`:
```toml
base64 = "0.22"
sha2 = "0.10"
rand = "0.8"
```

- [ ] **Step 3: Register the module and run tests**

`crates/oxitide-tidal/src/lib.rs`:
```rust
pub mod error;
pub mod models;
pub mod pkce;

pub use error::{Error, Result};
```

Run: `cargo test -p oxitide-tidal pkce`
Expected: PASS — 3 tests.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings (remove unused `Error`/`Result` imports if flagged).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/pkce.rs crates/oxitide-tidal/src/lib.rs crates/oxitide-tidal/Cargo.toml
git commit -m "feat(tidal): add PKCE primitives"
```

---

### Task 4: Token store

**Files:**
- Create: `crates/oxitide-tidal/src/token_store.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline in `token_store.rs`

**Interfaces:**
- Consumes: `keyring`, `serde`.
- Produces: `TokenSet { access_token, refresh_token, token_type, expires_at }`; `trait TokenStore { save, load, clear }`; `KeyringStore`; `MemoryStore` (public, for tests and for the app's test wiring).

- [ ] **Step 1: Write the failing tests**

`crates/oxitide-tidal/src/token_store.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const KEYRING_SERVICE: &str = "oxitide";
pub const KEYRING_USER: &str = "tidal";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

pub trait TokenStore: Send + Sync {
    fn save(&self, tokens: &TokenSet) -> Result<()>;
    fn load(&self) -> Result<Option<TokenSet>>;
    fn clear(&self) -> Result<()>;
}

pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| Error::Auth(format!("keyring unavailable: {e}")))
    }
}

impl TokenStore for KeyringStore {
    fn save(&self, tokens: &TokenSet) -> Result<()> {
        let json = serde_json::to_string(tokens)?;
        Self::entry()?
            .set_password(&json)
            .map_err(|e| Error::Auth(format!("keyring write failed: {e}")))
    }

    fn load(&self) -> Result<Option<TokenSet>> {
        match Self::entry()?.get_password() {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Auth(format!("keyring read failed: {e}"))),
        }
    }

    fn clear(&self) -> Result<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Auth(format!("keyring delete failed: {e}"))),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    inner: std::sync::Mutex<Option<TokenSet>>,
}

impl TokenStore for MemoryStore {
    fn save(&self, tokens: &TokenSet) -> Result<()> {
        *self.inner.lock().unwrap() = Some(tokens.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<TokenSet>> {
        Ok(self.inner.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TokenSet {
        TokenSet {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            token_type: "Bearer".into(),
            expires_at: Some(1_790_000_000),
        }
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        assert!(store.load().unwrap().is_none());
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), sample());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn token_set_defaults_token_type_to_bearer() {
        let t: TokenSet = serde_json::from_str(r#"{"access_token":"x"}"#).unwrap();
        assert_eq!(t.token_type, "Bearer");
        assert!(t.refresh_token.is_none());
    }
}
```

- [ ] **Step 2: Register the module and run tests**

`crates/oxitide-tidal/src/lib.rs`:
```rust
pub mod error;
pub mod models;
pub mod pkce;
pub mod token_store;

pub use error::{Error, Result};
```

Run: `cargo test -p oxitide-tidal token_store`
Expected: PASS — 2 tests. (`KeyringStore` is compiled but not exercised — keyring access is a manual/CI-excluded path.)

- [ ] **Step 3: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-tidal/src/token_store.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add token store with keyring and in-memory impls"
```

---

### Task 5: Auth flows and the authenticated HTTP client

**Files:**
- Create: `crates/oxitide-tidal/src/auth.rs`
- Create: `crates/oxitide-tidal/src/client.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline `#[cfg(test)]` in `auth.rs` and `client.rs`

**Interfaces:**
- Consumes: `TokenSet`, `TokenStore`, `Pkce`, `Error`, `reqwest`.
- Produces: `SessionInfo { session_id, country_code, user_id }`; `exchange_code(pkce, redirect_url) -> Result<TokenSet>`; `fetch_session(access_token) -> Result<SessionInfo>`; `refresh(refresh_token) -> Result<TokenSet>`; `TidalClient { new(tokens, session, store), get<T>(path, params) }`.

- [ ] **Step 1: Write the failing auth tests (wiremock)**

`crates/oxitide-tidal/src/auth.rs`:
```rust
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::pkce::{Pkce, PKCE_CLIENT_ID, PKCE_REDIRECT_URI};
use crate::token_store::TokenSet;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionInfo {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "userId")]
    pub user_id: u64,
}

#[derive(Deserialize)]
struct RawToken {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default = "bearer")]
    token_type: String,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn bearer() -> String {
    "Bearer".into()
}

fn raw_to_tokens(raw: RawToken) -> TokenSet {
    let expires_at = raw.expires_in.map(|s| now_unix() + s);
    TokenSet {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        token_type: raw.token_type,
        expires_at,
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    pkce: &Pkce,
    redirect_url: &str,
) -> Result<TokenSet> {
    let code = extract_code(redirect_url)?;
    let params = [
        ("code", code.as_str()),
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("redirect_uri", PKCE_REDIRECT_URI),
        ("scope", "r_usr w_usr w_sub"),
        ("code_verifier", pkce.code_verifier.as_str()),
        ("client_unique_key", pkce.client_unique_key.as_str()),
    ];
    let resp = http.post(token_url).form(&params).send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(Error::Unauthorized);
    }
    if !status.is_success() {
        let message = resp.text().await.unwrap_or_default();
        return Err(Error::Api { status: status.as_u16(), message });
    }
    let raw: RawToken = resp.json().await?;
    if raw.access_token.is_empty() {
        return Err(Error::Auth("empty access token".into()));
    }
    Ok(raw_to_tokens(raw))
}

pub async fn refresh(
    http: &reqwest::Client,
    token_url: &str,
    refresh_token: &str,
) -> Result<TokenSet> {
    let params = [
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", "r_usr w_usr w_sub"),
    ];
    let resp = http.post(token_url).form(&params).send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(Error::Unauthorized);
    }
    if !status.is_success() {
        let message = resp.text().await.unwrap_or_default();
        return Err(Error::Api { status: status.as_u16(), message });
    }
    let raw: RawToken = resp.json().await?;
    Ok(raw_to_tokens(raw))
}

pub async fn fetch_session(http: &reqwest::Client, base_url: &str, access_token: &str) -> Result<SessionInfo> {
    let url = format!("{base_url}sessions");
    let resp = http.get(url).bearer_auth(access_token).send().await?;
    match resp.status() {
        s if s.is_success() => Ok(resp.json().await?),
        reqwest::StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
        reqwest::StatusCode::FORBIDDEN => Err(Error::Forbidden),
        s => Err(Error::Api { status: s.as_u16(), message: resp.text().await.unwrap_or_default() }),
    }
}

pub fn extract_code(redirect_url: &str) -> Result<String> {
    let parsed = url::Url::parse(redirect_url)
        .map_err(|e| Error::Auth(format!("invalid redirect url: {e}")))?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| Error::Auth("redirect url has no code".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn extract_code_reads_query_param() {
        let url = "https://tidal.com/android/login/auth?code=ABC123&state=na";
        assert_eq!(extract_code(url).unwrap(), "ABC123");
    }

    #[test]
    fn extract_code_errors_without_code() {
        let e = extract_code("https://tidal.com/android/login/auth?state=na").unwrap_err();
        assert!(matches!(e, Error::Auth(_)));
    }

    #[tokio::test]
    async fn exchange_code_parses_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT", "refresh_token": "RT", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let tokens = exchange_code(&http, &server.uri().to_string() + "/token", &pkce,
            "https://tidal.com/android/login/auth?code=XYZ").await.unwrap();
        assert_eq!(tokens.access_token, "AT");
        assert_eq!(tokens.refresh_token.as_deref(), Some("RT"));
        assert!(tokens.expires_at.is_some());
    }

    #[tokio::test]
    async fn exchange_code_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        let http = reqwest::Client::new();
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let err = exchange_code(&http, &server.uri().to_string() + "/token", &pkce,
            "https://tidal.com/android/login/auth?code=XYZ").await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    #[tokio::test]
    async fn fetch_session_parses_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/sessions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sessionId": "sid", "countryCode": "GR", "userId": 191812568
            })))
            .mount(&server).await;
        let http = reqwest::Client::new();
        let info = fetch_session(&http, &(server.uri() + "/"), "AT").await.unwrap();
        assert_eq!(info.session_id, "sid");
        assert_eq!(info.country_code, "GR");
        assert_eq!(info.user_id, 191812568);
    }
}
```

- [ ] **Step 2: Run the auth tests to verify they fail (initially) then pass**

Run: `cargo test -p oxitide-tidal auth`
Expected: PASS — 5 tests once the module is registered (Step 4).

- [ ] **Step 3: Write the failing client tests (refresh-on-401)**

`crates/oxitide-tidal/src/client.rs`:
```rust
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;

use crate::auth::{self, SessionInfo};
use crate::error::{Error, Result};
use crate::token_store::{TokenSet, TokenStore};

#[derive(Clone)]
pub struct TidalClient {
    http: reqwest::Client,
    base_url: String,
    token_url: String,
    session: SessionInfo,
    tokens: Arc<RwLock<TokenSet>>,
    store: Arc<dyn TokenStore>,
}

impl TidalClient {
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token_url: impl Into<String>,
        session: SessionInfo,
        tokens: TokenSet,
        store: Arc<dyn TokenStore>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into(),
            token_url: token_url.into(),
            session,
            tokens: Arc::new(RwLock::new(tokens)),
            store,
        }
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    fn access_token(&self) -> String {
        self.tokens.read().unwrap().access_token.clone()
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let resp = self.send_get(path, params, &self.access_token()).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.try_refresh().await?;
            let retry = self.send_get(path, params, &self.access_token()).await?;
            return Self::decode(retry).await;
        }
        Self::decode(resp).await
    }

    async fn send_get(
        &self,
        path: &str,
        params: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response> {
        let mut query: Vec<(&str, String)> = vec![("countryCode", self.session.country_code.clone())];
        query.extend(params.iter().cloned());
        let url = format!("{}{}", self.base_url, path.trim_start_matches('/'));
        Ok(self.http.get(url).bearer_auth(token).query(&query).send().await?)
    }

    async fn try_refresh(&self) -> Result<()> {
        let refresh_token = self
            .tokens
            .read()
            .unwrap()
            .refresh_token
            .clone()
            .ok_or(Error::Unauthorized)?;
        let new_tokens = auth::refresh(&self.http, &self.token_url, &refresh_token).await?;
        *self.tokens.write().unwrap() = new_tokens.clone();
        self.store.save(&new_tokens)?;
        Ok(())
    }

    async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        match resp.status() {
            s if s.is_success() => Ok(resp.json().await?),
            reqwest::StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
            reqwest::StatusCode::FORBIDDEN => Err(Error::Forbidden),
            reqwest::StatusCode::NOT_FOUND => Err(Error::NotFound(resp.url().path().to_string())),
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(Error::RateLimited),
            s => Err(Error::Api { status: s.as_u16(), message: resp.text().await.unwrap_or_default() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_store::MemoryStore;
    use serde::Deserialize;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Deserialize, PartialEq, Debug)]
    struct Thing {
        value: u32,
    }

    fn session() -> SessionInfo {
        SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 }
    }

    fn tokens() -> TokenSet {
        TokenSet { access_token: "old".into(), refresh_token: Some("rt".into()),
            token_type: "Bearer".into(), expires_at: None }
    }

    #[tokio::test]
    async fn get_parses_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 7})))
            .mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store);
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 7);
    }

    #[tokio::test]
    async fn get_refreshes_once_on_401() {
        let server = MockServer::start().await;
        // First call with old token -> 401
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        // Retry with refreshed token -> 200
        Mock::given(method("GET")).and(path("/tracks/1"))
            .and(wiremock::matchers::header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 9})))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new", "refresh_token": "rt2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server).await;

        let store = Arc::new(MemoryStore::default());
        store.save(&tokens()).unwrap();
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), store.clone());
        let t: Thing = client.get("tracks/1", &[]).await.unwrap();
        assert_eq!(t.value, 9);
        assert_eq!(store.load().unwrap().unwrap().access_token, "new");
    }

    #[tokio::test]
    async fn get_without_refresh_token_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(401)).mount(&server).await;
        let store = Arc::new(MemoryStore::default());
        let no_refresh = TokenSet { access_token: "old".into(), refresh_token: None,
            token_type: "Bearer".into(), expires_at: None };
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), no_refresh, store);
        let err = client.get::<Thing>("tracks/1", &[]).await.unwrap_err();
        assert!(matches!(err, Error::Unauthorized));
    }

    #[tokio::test]
    async fn get_maps_429_and_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(429)).mount(&server).await;
        Mock::given(method("GET")).and(path("/b"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        let client = TidalClient::new(reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            session(), tokens(), Arc::new(MemoryStore::default()));
        assert!(matches!(client.get::<Thing>("a", &[]).await.unwrap_err(), Error::RateLimited));
        assert!(matches!(client.get::<Thing>("b", &[]).await.unwrap_err(), Error::NotFound(_)));
    }
}
```

- [ ] **Step 4: Register modules and run tests**

`crates/oxitide-tidal/src/lib.rs`:
```rust
pub mod auth;
pub mod client;
pub mod error;
pub mod models;
pub mod pkce;
pub mod token_store;

pub use client::TidalClient;
pub use error::{Error, Result};
```

Run: `cargo test -p oxitide-tidal`
Expected: PASS — all tests from Tasks 1–5.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-tidal/src/auth.rs crates/oxitide-tidal/src/client.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add auth flows and refresh-on-401 http client"
```

---

### Task 6: Catalog endpoints

**Files:**
- Create: `crates/oxitide-tidal/src/catalog.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline in `catalog.rs`

**Interfaces:**
- Consumes: `TidalClient`, `models::{Track, Album, Artist, Playlist, ItemPage}`, `Error`.
- Produces: `impl TidalClient { album(id), album_tracks(id, limit, offset), track(id), artist(id), playlist(id), playlist_tracks(id, limit, offset) }`.

- [ ] **Step 1: Write the failing tests**

`crates/oxitide-tidal/src/catalog.rs`:
```rust
use crate::client::TidalClient;
use crate::error::{Error, Result};
use crate::models::{Album, Artist, ItemPage, Playlist, Track};

fn page_params(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    pub async fn track(&self, id: u64) -> Result<Track> {
        self.get(&format!("tracks/{id}"), &[]).await
    }

    pub async fn album(&self, id: u64) -> Result<Album> {
        self.get(&format!("albums/{id}"), &[]).await
    }

    pub async fn album_tracks(&self, id: u64, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let page: ItemPage<Track> =
            self.get(&format!("albums/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok(page.items)
    }

    pub async fn artist(&self, id: u64) -> Result<Artist> {
        self.get(&format!("artists/{id}"), &[]).await
    }

    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.get(&format!("playlists/{id}"), &[]).await
    }

    pub async fn playlist_tracks(&self, id: &str, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let page: ItemPage<Track> =
            self.get(&format!("playlists/{id}/tracks"), &page_params(limit, offset)).await?;
        Ok(page.items)
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
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn album_tracks_sends_limit_and_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/albums/5/tracks"))
            .and(query_param("limit", "50")).and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 1, "title": "T1", "artists": []}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 100
            })))
            .mount(&server).await;
        let tracks = client(&server).album_tracks(5, 50, 100).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "T1");
    }

    #[tokio::test]
    async fn album_tracks_handles_empty_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/albums/5/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "totalNumberOfItems": 0, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        assert!(client(&server).album_tracks(5, 50, 0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn track_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/999"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(matches!(client(&server).track(999).await.unwrap_err(), Error::NotFound(_)));
    }

    #[tokio::test]
    async fn playlist_tracks_parses_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid/tracks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 2, "title": "P1", "artists": [{"id": 9, "name": "X"}]}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server).await;
        let tracks = client(&server).playlist_tracks("p-uuid", 100, 0).await.unwrap();
        assert_eq!(tracks[0].artists[0].name, "X");
    }
}
```

- [ ] **Step 2: Register module and run tests**

`crates/oxitide-tidal/src/lib.rs` — add `pub mod catalog;` after `pub mod client;`.

Run: `cargo test -p oxitide-tidal catalog`
Expected: PASS — 4 tests.

- [ ] **Step 3: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: `Deserialize` import may be unused — remove it if flagged.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-tidal/src/catalog.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add catalog endpoints (track/album/artist/playlist)"
```

---

### Task 7: Search

**Files:**
- Create: `crates/oxitide-tidal/src/search.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline in `search.rs`

**Interfaces:**
- Consumes: `TidalClient`, `SearchResults`.
- Produces: `impl TidalClient { search(&self, query, types: &[&str], limit, offset) -> Result<SearchResults> }`.

- [ ] **Step 1: Write the failing tests**

`crates/oxitide-tidal/src/search.rs`:
```rust
use crate::client::TidalClient;
use crate::error::Result;
use crate::models::SearchResults;

impl TidalClient {
    pub async fn search(
        &self,
        query: &str,
        types: &[&str],
        limit: u32,
        offset: u32,
    ) -> Result<SearchResults> {
        let params = vec![
            ("query", query.to_string()),
            ("types", types.join(",")),
            ("limit", limit.to_string()),
            ("offset", offset.to_string()),
        ];
        self.get("search", &params).await
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
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn search_parses_grouped_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search"))
            .and(query_param("query", "beck"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": [{"id": 1, "title": "Loser", "artists": []}]},
                "albums": {"items": [{"id": 2, "title": "Sea Change"}]},
                "artists": {"items": [{"id": 3, "name": "Beck"}]},
                "playlists": {"items": [{"uuid": "u", "title": "Mix"}]}
            })))
            .mount(&server).await;
        let r = client(&server).search("beck", &["tracks", "albums", "artists", "playlists"], 20, 0)
            .await.unwrap();
        assert_eq!(r.track_items()[0].title, "Loser");
        assert_eq!(r.album_items()[0].title, "Sea Change");
        assert_eq!(r.artist_items()[0].name, "Beck");
        assert_eq!(r.playlist_items()[0].uuid, "u");
    }

    #[tokio::test]
    async fn search_with_no_results_is_empty_not_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []},
                "artists": {"items": []}, "playlists": {"items": []}
            })))
            .mount(&server).await;
        let r = client(&server).search("zzzz", &["tracks"], 20, 0).await.unwrap();
        assert!(r.track_items().is_empty() && r.album_items().is_empty());
    }
}
```

Note: TIDAL's `/search` returns each type as a wrapper object with an `items` array. The `SearchResults` model in Task 2 expects `tracks: Vec<Track>`, so add a wire-shape shim in this module.

- [ ] **Step 2: Adjust `SearchResults` deserialization for TIDAL's grouped shape**

Modify `crates/oxitide-tidal/src/models.rs` — replace the `SearchResults` definition with a shape that unwraps `items`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchGroup<T> {
    #[serde(default)]
    pub items: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub tracks: SearchGroup<Track>,
    #[serde(default)]
    pub albums: SearchGroup<Album>,
    #[serde(default)]
    pub artists: SearchGroup<Artist>,
    #[serde(default)]
    pub playlists: SearchGroup<Playlist>,
}

impl SearchResults {
    pub fn track_items(&self) -> &[Track] { &self.tracks.items }
    pub fn album_items(&self) -> &[Album] { &self.albums.items }
    pub fn artist_items(&self) -> &[Artist] { &self.artists.items }
    pub fn playlist_items(&self) -> &[Playlist] { &self.playlists.items }
}
```

- [ ] **Step 3: Register module and run tests**

`crates/oxitide-tidal/src/lib.rs` — add `pub mod search;` after `pub mod pkce;`.

Run: `cargo test -p oxitide-tidal`
Expected: PASS — all tests (the Task 2 `SearchResults` usage changed; confirm the empty-search test still passes).

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/search.rs crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add search endpoint and grouped result shape"
```

---

### Task 8: Favorites, user playlists, and home page

**Files:**
- Create: `crates/oxitide-tidal/src/library.rs`
- Create: `crates/oxitide-tidal/src/home.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`
- Test: inline in each

**Interfaces:**
- Consumes: `TidalClient`, `models::{Track, Album, Artist, Playlist, ItemPage}`, `TidalClient::session()`.
- Produces: `impl TidalClient { favorite_tracks(limit, offset), favorite_albums(limit, offset), favorite_artists(limit, offset), user_playlists(limit, offset), home_page() }`; `home::HomeSection { title, items: Vec<HomeItem> }`, `home::HomeItem { id, title, subtitle: Option<String>, item_type: String }`.

- [ ] **Step 1: Write the failing library tests**

`crates/oxitide-tidal/src/library.rs`:
```rust
use crate::client::TidalClient;
use crate::error::Result;
use crate::models::{Album, Artist, ItemPage, Playlist, Track};

fn page(limit: u32, offset: u32) -> Vec<(&'static str, String)> {
    vec![("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl TidalClient {
    fn user_base(&self) -> String {
        format!("users/{}/favorites", self.session().user_id)
    }

    pub async fn favorite_tracks(&self, limit: u32, offset: u32) -> Result<Vec<Track>> {
        let p: ItemPage<Track> = self.get(&format!("{}/tracks", self.user_base()), &page(limit, offset)).await?;
        Ok(p.items)
    }

    pub async fn favorite_albums(&self, limit: u32, offset: u32) -> Result<Vec<Album>> {
        let p: ItemPage<Album> = self.get(&format!("{}/albums", self.user_base()), &page(limit, offset)).await?;
        Ok(p.items)
    }

    pub async fn favorite_artists(&self, limit: u32, offset: u32) -> Result<Vec<Artist>> {
        let p: ItemPage<Artist> = self.get(&format!("{}/artists", self.user_base()), &page(limit, offset)).await?;
        Ok(p.items)
    }

    pub async fn user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>> {
        let p: ItemPage<Playlist> =
            self.get(&format!("users/{}/playlists", self.session().user_id), &page(limit, offset)).await?;
        Ok(p.items)
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
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 42 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn favorite_tracks_pages_by_offset() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/favorites/tracks"))
            .and(query_param("offset", "200")).and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 7, "title": "Fav"}], "totalNumberOfItems": 201, "limit": 100, "offset": 200
            })))
            .mount(&server).await;
        let tracks = client(&server).favorite_tracks(100, 200).await.unwrap();
        assert_eq!(tracks[0].id, 7);
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

- [ ] **Step 2: Write the failing home tests**

`crates/oxitide-tidal/src/home.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default = "default_kind")]
    pub item_type: String,
}

fn default_kind() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<HomeItem>,
}

#[derive(Debug, Deserialize)]
struct RawRow {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub modules: Vec<RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(rename = "type", default)]
    pub item_type: String,
}

#[derive(Debug, Deserialize)]
struct RawPage {
    #[serde(default)]
    pub rows: Vec<RawRow>,
}

impl TidalClient {
    pub async fn home_page(&self) -> Result<Vec<HomeSection>> {
        let raw: RawPage = self.get("pages/home", &[]).await?;
        let sections = raw
            .rows
            .into_iter()
            .map(|row| {
                let items: Vec<HomeItem> = row
                    .modules
                    .into_iter()
                    .flat_map(|m| m.items.into_iter())
                    .map(|i| HomeItem {
                        id: value_to_id(&i.id),
                        title: i.title,
                        subtitle: i.subtitle,
                        item_type: i.item_type,
                    })
                    .collect();
                HomeSection { title: row.title, items }
            })
            .filter(|s| !s.items.is_empty())
            .collect();
        Ok(sections)
    }
}

fn value_to_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn home_page_flattens_rows_and_skips_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"title": "For You", "modules": [
                        {"title": "Mixes", "items": [
                            {"id": "m1", "title": "Daily Mix", "subtitle": "Made for you", "type": "MIX"}
                        ]}
                    ]},
                    {"title": "Empty", "modules": [{"title": "x", "items": []}]}
                ]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "For You");
        assert_eq!(sections[0].items[0].id, "m1");
        assert_eq!(sections[0].items[0].item_type, "MIX");
    }

    #[tokio::test]
    async fn home_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })))
            .mount(&server).await;
        assert!(client(&server).home_page().await.unwrap().is_empty());
    }
}
```

- [ ] **Step 3: Register modules and run tests**

`crates/oxitide-tidal/src/lib.rs` — add `pub mod home;` and `pub mod library;`.

Run: `cargo test -p oxitide-tidal`
Expected: PASS — all tests.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/library.rs crates/oxitide-tidal/src/home.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add favorites, user playlists, and home feed"
```

---

### Task 9: Tauri app scaffold with auth commands

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml`
- Create: `crates/oxitide-app/build.rs`
- Create: `crates/oxitide-app/tauri.conf.json`
- Create: `crates/oxitide-app/capabilities/default.json`
- Create: `crates/oxitide-app/src/main.rs`
- Create: `crates/oxitide-app/src/state.rs`
- Create: `crates/oxitide-app/src/commands.rs`
- Test: inline `#[cfg(test)]` in `state.rs`

**Interfaces:**
- Consumes: `oxitide_tidal::{TidalClient, auth, pkce, token_store}`.
- Produces: Tauri commands `auth_begin`, `auth_complete`, `auth_status`, `auth_logout`; managed state `AppState { pending_pkce: Mutex<Option<Pkce>>, client: RwLock<Option<TidalClient>> }`.

- [ ] **Step 1: Write the app manifest**

`crates/oxitide-app/Cargo.toml`:
```toml
[package]
name = "oxitide-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
oxitide-tidal = { path = "../oxitide-tidal" }
tauri = { version = "2", features = [] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

`crates/oxitide-app/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`crates/oxitide-app/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "OxiTide",
  "version": "0.1.0",
  "identifier": "org.codeberg.zephulbr.oxitide",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm --prefix ../../ui run dev",
    "beforeBuildCommand": "npm --prefix ../../ui run build"
  },
  "app": {
    "windows": [{ "title": "OxiTide", "width": 1200, "height": 800 }],
    "security": { "csp": null }
  },
  "bundle": { "active": true, "targets": ["msi", "nsis"] }
}
```

`crates/oxitide-app/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "default capability",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 2: Write the failing state test**

`crates/oxitide-app/src/state.rs`:
```rust
use std::sync::{Mutex, RwLock};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::Pkce;

#[derive(Default)]
pub struct AppState {
    pub pending_pkce: Mutex<Option<Pkce>>,
    pub client: RwLock<Option<TidalClient>>,
}

impl AppState {
    pub fn set_pending(&self, pkce: Pkce) {
        *self.pending_pkce.lock().unwrap() = Some(pkce);
    }

    pub fn take_pending(&self) -> Option<Pkce> {
        self.pending_pkce.lock().unwrap().take()
    }

    pub fn is_logged_in(&self) -> bool {
        self.client.read().unwrap().is_some()
    }

    pub fn set_client(&self, client: TidalClient) {
        *self.client.write().unwrap() = Some(client);
    }

    pub fn logout(&self) {
        *self.client.write().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_pkce_is_taken_once() {
        let state = AppState::default();
        state.set_pending(Pkce::from_parts("v".into(), "k".into()));
        assert!(state.take_pending().is_some());
        assert!(state.take_pending().is_none());
    }

    #[test]
    fn logged_in_reflects_client_presence() {
        let state = AppState::default();
        assert!(!state.is_logged_in());
        state.logout();
        assert!(!state.is_logged_in());
    }
}
```

- [ ] **Step 3: Write the commands**

`crates/oxitide-app/src/commands.rs`:
```rust
use std::sync::Arc;

use oxitide_tidal::auth;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::{Pkce, PKCE_CLIENT_ID, TOKEN_URL};
use oxitide_tidal::token_store::{KeyringStore, TokenStore};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

pub const API_BASE: &str = "https://api.tidal.com/v1/";

#[derive(Serialize)]
pub struct LoginPrompt {
    pub url: String,
}

#[derive(Serialize)]
pub struct SessionInfoDto {
    pub country_code: String,
    pub user_id: u64,
}

#[tauri::command]
pub fn auth_begin(state: State<'_, AppState>) -> Result<LoginPrompt, String> {
    let pkce = Pkce::generate();
    let url = pkce.login_url(PKCE_CLIENT_ID);
    state.set_pending(pkce);
    Ok(LoginPrompt { url })
}

#[tauri::command]
pub async fn auth_complete(
    state: State<'_, AppState>,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    let pkce = state.take_pending().ok_or_else(|| "no login in progress".to_string())?;
    let http = reqwest::Client::new();
    let tokens = auth::exchange_code(&http, TOKEN_URL, &pkce, &redirect_url)
        .await
        .map_err(|e| e.to_string())?;
    let session = auth::fetch_session(&http, API_BASE, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    store.save(&tokens).map_err(|e| e.to_string())?;
    let client = TidalClient::new(
        http,
        API_BASE,
        TOKEN_URL,
        session.clone(),
        tokens,
        store,
    );
    state.set_client(client);
    Ok(SessionInfoDto { country_code: session.country_code, user_id: session.user_id })
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> bool {
    state.is_logged_in()
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    let store = KeyringStore;
    store.clear().map_err(|e| e.to_string())?;
    state.logout();
    Ok(())
}
```

- [ ] **Step 4: Write `main.rs` and run the state tests**

`crates/oxitide-app/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::auth_begin,
            commands::auth_complete,
            commands::auth_status,
            commands::auth_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OxiTide");
}
```

Run: `cargo test -p oxitide-app`
Expected: PASS — 2 state tests. (The binary requires the `ui/dist` directory to exist for `generate_context!`; run `npm --prefix ui run build` first, or `cargo test` compiles tests without building the binary context. If `generate_context!` complains, create `ui/dist/index.html` as a temporary placeholder and remove it in Task 11.)

- [ ] **Step 5: Build check**

Run: `cargo build -p oxitide-app`
Expected: builds (Tauri build script runs). If it fails on the missing frontend dist, note it and continue — Task 11 produces it.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-app
git commit -m "feat(app): scaffold tauri app with auth commands"
```

---

### Task 10: Browse commands (catalog, search, library, home)

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Test: inline `#[cfg(test)]` in `commands.rs` for the response DTO mapping

**Interfaces:**
- Consumes: `oxitide_tidal::models::*`, `AppState.client`.
- Produces commands: `get_home`, `get_album(id)`, `get_album_tracks(id)`, `get_playlist(id)`, `get_playlist_tracks(id)`, `get_artist(id)`, `search(query)`, `get_favorite_tracks`, `get_favorite_albums`, `get_favorite_artists`, `get_user_playlists`. Each returns serialized `oxitide_tidal::models` directly (they already derive `Serialize`).

- [ ] **Step 1: Add an async-safe client accessor**

`TidalClient` derives `Clone` and shares its token via `Arc<RwLock<TokenSet>>` (Task 5), so we
clone it out of the state guard and drop the guard before awaiting.

Append to `crates/oxitide-app/src/commands.rs`:
```rust
fn require_client(state: &State<'_, AppState>) -> Result<TidalClient, String> {
    state
        .client
        .read()
        .map_err(|_| "state poisoned".to_string())?
        .clone()
        .ok_or_else(|| "not logged in".to_string())
}
```

- [ ] **Step 2: Add the browse commands**

Append to `crates/oxitide-app/src/commands.rs`:
```rust
use oxitide_tidal::home::HomeSection;
use oxitide_tidal::models::{Album, Artist, Playlist, SearchResults, Track};

#[tauri::command]
pub async fn get_home(state: State<'_, AppState>) -> Result<Vec<HomeSection>, String> {
    require_client(&state)?.home_page().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_album(state: State<'_, AppState>, id: u64) -> Result<Album, String> {
    require_client(&state)?.album(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_album_tracks(state: State<'_, AppState>, id: u64) -> Result<Vec<Track>, String> {
    require_client(&state)?.album_tracks(id, 100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist(state: State<'_, AppState>, id: String) -> Result<Playlist, String> {
    require_client(&state)?.playlist(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist_tracks(state: State<'_, AppState>, id: String) -> Result<Vec<Track>, String> {
    require_client(&state)?.playlist_tracks(&id, 100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_artist(state: State<'_, AppState>, id: u64) -> Result<Artist, String> {
    require_client(&state)?.artist(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<SearchResults, String> {
    require_client(&state)?
        .search(&query, &["tracks", "albums", "artists", "playlists"], 20, 0)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    require_client(&state)?.favorite_tracks(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    require_client(&state)?.favorite_albums(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favorite_artists(state: State<'_, AppState>) -> Result<Vec<Artist>, String> {
    require_client(&state)?.favorite_artists(100, 0).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    require_client(&state)?.user_playlists(100, 0).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register the commands**

`crates/oxitide-app/src/main.rs` — extend `generate_handler!` with all new commands:
```rust
.invoke_handler(tauri::generate_handler![
    commands::auth_begin,
    commands::auth_complete,
    commands::auth_status,
    commands::auth_logout,
    commands::get_home,
    commands::get_album,
    commands::get_album_tracks,
    commands::get_playlist,
    commands::get_playlist_tracks,
    commands::get_artist,
    commands::search,
    commands::get_favorite_tracks,
    commands::get_favorite_albums,
    commands::get_favorite_artists,
    commands::get_user_playlists,
])
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p oxitide-app`
Expected: builds.

Run: `cargo test -p oxitide-tidal -p oxitide-app`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app crates/oxitide-tidal/src/client.rs
git commit -m "feat(app): add browse, search, and library commands"
```

---

### Task 11: Svelte UI scaffold, shell layout, and IPC bridge

**Files:**
- Create: `ui/package.json`, `ui/vite.config.ts`, `ui/svelte.config.js`, `ui/tsconfig.json`, `ui/index.html`
- Create: `ui/src/main.ts`, `ui/src/App.svelte`
- Create: `ui/src/lib/api.ts`, `ui/src/lib/format.ts`, `ui/src/lib/nav.svelte.ts`
- Create: `ui/src/lib/components/Sidebar.svelte`, `PlayerBar.svelte`, `QueueRail.svelte`
- Create: `ui/src/lib/components/TrackTable.svelte`
- Create: `ui/src/views/Login.svelte`, `Home.svelte`, `Album.svelte`, `Playlist.svelte`, `Artist.svelte`, `Search.svelte`, `Library.svelte`
- Test: `ui/src/lib/format.test.ts`

**Interfaces:**
- Consumes: Tauri `invoke` from `@tauri-apps/api/core`.
- Produces: `api.{authBegin, authComplete, authStatus, authLogout, getHome, getAlbum, getAlbumTracks, getPlaylist, getPlaylistTracks, getArtist, search, getFavoriteTracks, getFavoriteAlbums, getFavoriteArtists, getUserPlaylists}`; stores `session`, `route`, `queueOpen`; components with documented props.

- [ ] **Step 1: Write the front-end package**

`ui/package.json`:
```json
{
  "name": "oxitide-ui",
  "private": true,
  "type": "module",
  "version": "0.1.0",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run"
  },
  "dependencies": {
    "@tauri-apps/api": "^2"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^4",
    "@tsconfig/svelte": "^5",
    "svelte": "^5",
    "svelte-check": "^4",
    "typescript": "^5",
    "vite": "^6",
    "vitest": "^2"
  }
}
```

`ui/vite.config.ts`:
```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
```

`ui/svelte.config.js`:
```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default { preprocess: vitePreprocess() };
```

`ui/tsconfig.json`:
```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": { "verbatimModuleSyntax": true, "isolatedModules": true },
  "include": ["src/**/*"]
}
```

`ui/index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OxiTide</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: Write the IPC bridge**

`ui/src/lib/api.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";

export interface ArtistRef { id: number; name: string }
export interface AlbumRef { id: number; title: string; cover?: string | null }
export interface Track {
  id: number; title: string; duration: number;
  artists: ArtistRef[]; album?: AlbumRef | null;
  audio_quality?: string | null; track_number: number; volume_number: number;
  explicit: boolean;
}
export interface Album {
  id: number; title: string; artists: ArtistRef[]; cover?: string | null;
  number_of_tracks: number; duration: number; release_date?: string | null;
  audio_quality?: string | null;
}
export interface Artist { id: number; name: string; picture?: string | null }
export interface Playlist {
  uuid: string; title: string; description?: string | null;
  number_of_tracks: number; duration: number; image?: string | null;
}
export interface HomeItem { id: string; title: string; subtitle?: string | null; item_type: string }
export interface HomeSection { title: string; items: HomeItem[] }
export interface SearchGroup<T> { items: T[] }
export interface SearchResults {
  tracks: SearchGroup<Track>; albums: SearchGroup<Album>;
  artists: SearchGroup<Artist>; playlists: SearchGroup<Playlist>;
}
export interface SessionInfoDto { country_code: string; user_id: number }

export const api = {
  authBegin: () => invoke<{ url: string }>("auth_begin"),
  authComplete: (redirectUrl: string) => invoke<SessionInfoDto>("auth_complete", { redirectUrl }),
  authStatus: () => invoke<boolean>("auth_status"),
  authLogout: () => invoke<void>("auth_logout"),
  getHome: () => invoke<HomeSection[]>("get_home"),
  getAlbum: (id: number) => invoke<Album>("get_album", { id }),
  getAlbumTracks: (id: number) => invoke<Track[]>("get_album_tracks", { id }),
  getPlaylist: (id: string) => invoke<Playlist>("get_playlist", { id }),
  getPlaylistTracks: (id: string) => invoke<Track[]>("get_playlist_tracks", { id }),
  getArtist: (id: number) => invoke<Artist>("get_artist", { id }),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  getFavoriteTracks: () => invoke<Track[]>("get_favorite_tracks"),
  getFavoriteAlbums: () => invoke<Album[]>("get_favorite_albums"),
  getFavoriteArtists: () => invoke<Artist[]>("get_favorite_artists"),
  getUserPlaylists: () => invoke<Playlist[]>("get_user_playlists"),
};
```

- [ ] **Step 3: Write formatting + navigation modules and a failing test**

`ui/src/lib/format.ts`:
```ts
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}
```

`ui/src/lib/nav.svelte.ts`:
```ts
export type Route =
  | { name: "home" }
  | { name: "search" }
  | { name: "library" }
  | { name: "album"; id: number }
  | { name: "playlist"; id: string }
  | { name: "artist"; id: number };

export interface Nav {
  readonly route: Route;
  readonly queueOpen: boolean;
  goto(next: Route): void;
  toggleQueue(): void;
}

export function createNav(): Nav {
  let route = $state<Route>({ name: "home" });
  let queueOpen = $state(false);
  return {
    get route() { return route; },
    get queueOpen() { return queueOpen; },
    goto(next: Route) { route = next; },
    toggleQueue() { queueOpen = !queueOpen; },
  };
}
```

`ui/src/lib/format.test.ts`:
```ts
import { describe, it, expect } from "vitest";
import { formatDuration } from "./format";

describe("formatDuration", () => {
  it("formats minutes and seconds with padding", () => {
    expect(formatDuration(276)).toBe("4:36");
    expect(formatDuration(65)).toBe("1:05");
  });
  it("never returns negative", () => {
    expect(formatDuration(-10)).toBe("0:00");
  });
});
```
(Svelte 5 runes require the `.svelte.ts` extension, which is why navigation lives in `nav.svelte.ts`.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui install && npm --prefix ui test`
Expected: PASS — 2 tests.

- [ ] **Step 5: Write the app shell and placeholder views**

`ui/src/main.ts`:
```ts
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

export default mount(App, { target: document.getElementById("app")! });
```

`ui/src/app.css`:
```css
:root { --bg:#121212; --panel:#181818; --panel-2:#1c1c1c; --line:#2a2a2a; --text:#e8e8e8; --muted:#9a9a9a; --accent:#00d3b8; }
* { box-sizing: border-box; }
html, body, #app { height: 100%; margin: 0; }
body { background: var(--bg); color: var(--text); font: 14px/1.4 system-ui, sans-serif; }
.app { display: grid; grid-template-columns: 220px 1fr auto; grid-template-rows: 1fr 64px; height: 100%; }
.sidebar { background: var(--panel); border-right: 1px solid var(--line); padding: 12px; }
.main { padding: 16px; overflow: auto; }
.queue { background: var(--panel); border-left: 1px solid var(--line); width: 300px; }
.player { grid-column: 1 / -1; background: var(--panel-2); border-top: 1px solid var(--line); display: flex; align-items: center; padding: 0 12px; }
.muted { color: var(--muted); }
```

`ui/src/App.svelte`:
```svelte
<script lang="ts">
  import Sidebar from "./lib/components/Sidebar.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import QueueRail from "./lib/components/QueueRail.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Album from "./views/Album.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Artist from "./views/Artist.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import { createNav } from "./lib/nav.svelte";
  import { api } from "./lib/api";

  const nav = createNav();
  let loggedIn = $state(false);

  $effect(() => {
    api.authStatus().then((v) => (loggedIn = v));
  });

  function onLogin() { loggedIn = true; }
</script>

{#if !loggedIn}
  <Login onLogin={onLogin} />
{:else}
  <div class="app">
    <Sidebar {nav} />
    <main class="main">
      {#if nav.route.name === "home"}<Home {nav} />
      {:else if nav.route.name === "search"}<Search {nav} />
      {:else if nav.route.name === "library"}<Library {nav} />
      {:else if nav.route.name === "album"}<Album id={nav.route.id} />
      {:else if nav.route.name === "playlist"}<Playlist id={nav.route.id} />
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} />{/if}
    </main>
    {#if nav.queueOpen}<aside class="queue"><QueueRail /></aside>{/if}
    <PlayerBar />
  </div>
{/if}
```

`ui/src/lib/components/Sidebar.svelte`:
```svelte
<script lang="ts">
  import type { Nav } from "../nav.svelte";
  let { nav }: { nav: Nav } = $props();
</script>

<nav class="sidebar">
  <h1 style="font-size:16px">OxiTide</h1>
  <button onclick={() => nav.goto({ name: "home" })}>Home</button>
  <button onclick={() => nav.goto({ name: "search" })}>Search</button>
  <button onclick={() => nav.goto({ name: "library" })}>Library</button>
  <button onclick={() => nav.toggleQueue()}>Queue</button>
</nav>
```

`ui/src/lib/components/PlayerBar.svelte`:
```svelte
<div class="player"><span class="muted">Playback engine arrives in Plan 2</span></div>
```

`ui/src/lib/components/QueueRail.svelte`:
```svelte
<div style="padding:12px">
  <strong>Queue</strong>
  <p class="muted">Wired up in Plan 2.</p>
</div>
```

`ui/src/lib/components/TrackTable.svelte`:
```svelte
<script lang="ts">
  import type { Track } from "../api";
  import { formatDuration } from "../format";
  let { tracks }: { tracks: Track[] } = $props();
</script>

<table style="width:100%; border-collapse:collapse">
  <thead><tr><th style="text-align:left">#</th><th style="text-align:left">Title</th><th style="text-align:right">Time</th></tr></thead>
  <tbody>
    {#each tracks as t, i (t.id)}
      <tr style="border-bottom:1px solid var(--line)">
        <td class="muted">{i + 1}</td>
        <td>{t.title}<span class="muted"> · {t.artists.map((a) => a.name).join(", ")}</span></td>
        <td style="text-align:right" class="muted">{formatDuration(t.duration)}</td>
      </tr>
    {/each}
    {#if tracks.length === 0}
      <tr><td colspan="3" class="muted">No tracks</td></tr>
    {/if}
  </tbody>
</table>
```

- [ ] **Step 6: Write the views**

`ui/src/views/Login.svelte`:
```svelte
<script lang="ts">
  import { api } from "../lib/api";
  let { onLogin }: { onLogin: () => void } = $props();
  let url = $state("");
  let redirect = $state("");
  let error = $state("");

  async function begin() {
    const prompt = await api.authBegin();
    url = prompt.url;
    window.open(url, "_blank");
  }
  async function complete() {
    error = "";
    try { await api.authComplete(redirect); onLogin(); }
    catch (e) { error = String(e); }
  }
</script>

<div style="max-width:640px;margin:10vh auto">
  <h1>Sign in to TIDAL</h1>
  <button onclick={begin}>Open login page</button>
  {#if url}<p class="muted">After logging in you'll land on an "Oops" page. Paste its full URL below.</p>{/if}
  <input placeholder="Paste the redirect URL" bind:value={redirect} style="width:100%" />
  <button onclick={complete} disabled={!redirect}>Complete login</button>
  {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
</div>
```

`ui/src/views/Home.svelte`:
```svelte
<script lang="ts">
  import { api, type HomeSection } from "../lib/api";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let sections = $state<HomeSection[]>([]);
  let error = $state("");

  $effect(() => {
    api.getHome().then((s) => (sections = s)).catch((e) => (error = String(e)));
  });
</script>

<h2>Home</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
{#each sections as section (section.title)}
  <h3>{section.title}</h3>
  <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px">
    {#each section.items as item (item.id)}
      <button style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
        <div>{item.title}</div>
        {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
      </button>
    {/each}
  </div>
{:else}
  <p class="muted">Loading…</p>
{/each}
```

`ui/src/views/Album.svelte`:
```svelte
<script lang="ts">
  import { api, type Album as AlbumT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: number } = $props();
  let album = $state<AlbumT | null>(null);
  let tracks = $state<Track[]>([]);

  $effect(() => {
    api.getAlbum(id).then((a) => (album = a));
    api.getAlbumTracks(id).then((t) => (tracks = t));
  });
</script>

{#if album}
  <h2>{album.title}</h2>
  <p class="muted">{album.artists.map((a) => a.name).join(", ")} · {album.number_of_tracks} tracks</p>
{/if}
<TrackTable {tracks} />
```

`ui/src/views/Playlist.svelte`:
```svelte
<script lang="ts">
  import { api, type Playlist as PlaylistT, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  let { id }: { id: string } = $props();
  let playlist = $state<PlaylistT | null>(null);
  let tracks = $state<Track[]>([]);

  $effect(() => {
    api.getPlaylist(id).then((p) => (playlist = p));
    api.getPlaylistTracks(id).then((t) => (tracks = t));
  });
</script>

{#if playlist}
  <h2>{playlist.title}</h2>
  <p class="muted">{playlist.number_of_tracks} tracks</p>
{/if}
<TrackTable {tracks} />
```

`ui/src/views/Artist.svelte`:
```svelte
<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  let { id }: { id: number } = $props();
  let artist = $state<ArtistT | null>(null);
  $effect(() => { api.getArtist(id).then((a) => (artist = a)); });
</script>

{#if artist}
  <h2>{artist.name}</h2>
  <p class="muted">Artist</p>
{/if}
```

`ui/src/views/Search.svelte`:
```svelte
<script lang="ts">
  import { api, type SearchResults } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let query = $state("");
  let results = $state<SearchResults | null>(null);

  async function run() {
    if (query.trim().length === 0) return;
    results = await api.search(query);
  }
</script>

<h2>Search</h2>
<input bind:value={query} onkeydown={(e) => e.key === "Enter" && run()} placeholder="Search TIDAL" />
{#if results}
  <h3>Tracks</h3>
  <TrackTable tracks={results.tracks.items} />
  <h3>Albums</h3>
  <ul>
    {#each results.albums.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
    {/each}
  </ul>
  <h3>Artists</h3>
  <ul>
    {#each results.artists.items as a (a.id)}
      <li><button onclick={() => nav.goto({ name: "artist", id: a.id })}>{a.name}</button></li>
    {/each}
  </ul>
  <h3>Playlists</h3>
  <ul>
    {#each results.playlists.items as p (p.uuid)}
      <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
    {/each}
  </ul>
{/if}
```

`ui/src/views/Library.svelte`:
```svelte
<script lang="ts">
  import { api, type Album, type Playlist, type Track } from "../lib/api";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import type { Nav } from "../lib/nav.svelte";
  let { nav }: { nav: Nav } = $props();
  let tracks = $state<Track[]>([]);
  let albums = $state<Album[]>([]);
  let playlists = $state<Playlist[]>([]);

  $effect(() => {
    api.getFavoriteTracks().then((t) => (tracks = t));
    api.getFavoriteAlbums().then((a) => (albums = a));
    api.getUserPlaylists().then((p) => (playlists = p));
  });
</script>

<h2>Library</h2>
<h3>Playlists</h3>
<ul>
  {#each playlists as p (p.uuid)}
    <li><button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button></li>
  {/each}
  {#if playlists.length === 0}<li class="muted">No playlists</li>{/if}
</ul>
<h3>Albums</h3>
<ul>
  {#each albums as a (a.id)}
    <li><button onclick={() => nav.goto({ name: "album", id: a.id })}>{a.title}</button></li>
  {/each}
  {#if albums.length === 0}<li class="muted">No albums</li>{/if}
</ul>
<h3>Favorite tracks</h3>
<TrackTable {tracks} />
```

- [ ] **Step 7: Type-check and build**

Run: `npm --prefix ui install`
Run: `npm --prefix ui run check`
Expected: no errors.
Run: `npm --prefix ui run build`
Expected: `ui/dist` produced.

- [ ] **Step 8: Commit**

```bash
git add ui
git commit -m "feat(ui): add svelte shell, ipc bridge, and browse views"
```

---

### Task 12: End-to-end smoke verification

**Files:**
- Create: `docs/superpowers/plans/notes/plan1-smoke.md` (manual checklist, committed)

**Interfaces:**
- Consumes: everything from Tasks 1–11.
- Produces: a verified running app.

- [ ] **Step 1: Build everything**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 2: Write the manual smoke checklist**

`docs/superpowers/plans/notes/plan1-smoke.md`:
```markdown
# Plan 1 Smoke Checklist

- [ ] `npm --prefix ui run build` then `cargo run -p oxitide-app` launches a window.
- [ ] Login screen: "Open login page" opens the TIDAL PKCE URL in the browser.
- [ ] Complete login; app shows Home.
- [ ] Home renders at least one section (or shows an empty state without crashing).
- [ ] Search "Beck" returns tracks and albums; clicking an album opens the album view with a populated track table.
- [ ] Library lists your playlists; clicking one opens the playlist view.
- [ ] Clicking Queue toggles the right rail.
- [ ] Logout clears the status; restarting the app requires login again (token cleared).
```

- [ ] **Step 3: Run the smoke test**

Run: `npm --prefix ui run build; cargo run -p oxitide-app`
Manually execute each checklist item. Record results inline in the file (check the boxes).

- [ ] **Step 4: Full test suite**

Run: `cargo test --workspace`
Run: `cargo clippy --workspace -- -D warnings`
Run: `npm --prefix ui test`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/notes/plan1-smoke.md
git commit -m "test: add plan 1 smoke checklist and verify"
```

---

## Self-Review (completed by author)

- **Spec coverage:** PKCE auth (Tasks 3–5, 9), token storage (4), API client + refresh (5), catalog (6), search (7), library + home (8), Tauri shell + commands (9–10), Svelte shell + views with sidebar/queue rail/player placeholder (11), verification (12). Deferred spec items (playback, queue function, cache, settings) are intentionally not in Plan 1.
- **Placeholder scan:** no TBD/TODO; every code step contains runnable code or an exact command.
- **Type consistency:** `TokenSet`, `SessionInfo`, `ItemPage<T>`, `SearchResults`/`SearchGroup<T>`, `HomeSection`/`HomeItem`, and all `impl TidalClient` method signatures are defined once and reused unchanged across tasks.
- **Review Focus tests:** empty payloads (Tasks 2, 6, 7, 8), non-200 mapping (Tasks 5, 6), refresh-on-401 (Task 5), partial JSON (Task 2), pagination offset (Tasks 6, 8).
