# Seamless TIDAL Login Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the paste-back TIDAL login with a `tidal://login/auth` deep-link capture so the system browser returns the user to OxiTide already signed in, while keeping a functional paste fallback.

**Architecture:** `auth_begin` builds an authorize URL whose redirect is `tidal://login/auth` and opens it in the system browser. TIDAL redirects the browser to that custom scheme; Windows routes the URL to OxiTide (launching it or forwarding via single-instance). Tauri's deep-link plugin fires `on_open_url`; Rust validates the URL, verifies the pending `state`, runs the existing exchange/session/token path (shared with the fallback `auth_complete`), and emits `auth://success` / `auth://error`. A Windows scheme-ownership guard snapshots the prior `tidal` handler before registering and restores it on exit, because the official TIDAL desktop app also owns the scheme.

**Tech Stack:** Rust (Tauri 2, `tauri-plugin-deep-link`, `tauri-plugin-single-instance`), `reg export`/`reg import` for scheme ownership, Svelte 5 runes, TypeScript, vitest, wiremock.

**Spec:** `docs/superpowers/specs/2026-09-28-oxitide-seamless-login-design.md`

## Global Constraints

- Windows-first; all new OS behavior is gated so the workspace still builds cross-platform.
- Any new `#[tauri::command]` must be added to `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails.
- IPC: `invoke` sends camelCase args, Rust params stay snake_case; Rust DTOs are the source of truth — keep `ui/src/lib/api.ts` in sync by hand.
- Svelte 5 runes only (`$state`, `$props`, `$effect`); no Svelte 4 stores, no `export let`; no new npm packages.
- Only two new Rust crates: `tauri-plugin-deep-link = "2"`, `tauri-plugin-single-instance = { version = "2", features = ["deep-link"] }`.
- Never log token values (`TokenSet` stays redacting).
- UI consumes semantic tokens only.
- Version stays 0.2.0.
- Gates for every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.

## Review Focus

- **All three SSO methods must complete:** email/password, Google, and Apple all happen in the system browser and must end with OxiTide logged in. Task 3 gates on a real browser login; Task 5 smokes all three.
- **Scheme collision with the official TIDAL app:** OxiTide must win `tidal://` while running (login start) and hand it back on exit. Task 2 verifies handoff; Task 5 verifies restoration.
- **Cold start:** if OxiTide is not running when the redirect fires, the deep link launches it and the login must still complete (no missed event). Task 3 buffers the result; Task 5 smokes cold start.
- **Forged/stray `tidal://` traffic:** a link without a matching pending `state` (or with no pending login) must not complete a login and must not disturb the UI. Task 1 unit-tests validation; Task 3 unit-tests the state check.
- **Browser handoff fails or the user dismisses the prompt:** the UI must not hang, and the paste fallback (HTTPS redirect mode) must still sign the user in. Task 4 implements the waiting/cancel/fallback states; Task 5 smokes it.

---

### Task 1: TIDAL client — redirect modes, `state`, and redirect validation

**Files:**
- Modify: `crates/oxitide-tidal/src/pkce.rs`
- Modify: `crates/oxitide-tidal/src/auth.rs`
- Modify: `crates/oxitide-app/src/commands.rs` (call-site shim only, to keep the workspace compiling)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pkce::PKCE_REDIRECT_DEEP_LINK: &str = "tidal://login/auth"`
  - `pkce::PKCE_REDIRECT_PASTE: &str = "https://tidal.com/android/login/auth"`
  - `pkce::random_state() -> String` (32-char hex)
  - `Pkce::login_url(&self, client_id: &str, state: &str, redirect_uri: &str) -> String`
  - `auth::RedirectOutcome { pub code: String, pub state: Option<String> }`
  - `auth::validate_redirect(url: &str) -> Result<RedirectOutcome>`
  - `auth::exchange_code(http, token_url, pkce, redirect_uri: &str, redirect_url: &str)`
  - `PKCE_REDIRECT_URI` no longer exists (replaced by the two constants above).

- [ ] **Step 0: Commit the approved spec (only the spec file)**

```powershell
git add docs/superpowers/specs/2026-09-28-oxitide-seamless-login-design.md
git commit -m "docs: specify seamless TIDAL login via deep-link capture"
```

- [ ] **Step 1: Write the failing `pkce.rs` tests**

Replace `login_url_contains_expected_params` in `crates/oxitide-tidal/src/pkce.rs` and add the paste test and the state test:

```rust
    #[test]
    fn login_url_contains_expected_params() {
        let pkce = Pkce::from_parts("verifier-abc".into(), "0011223344556677".into());
        let url = pkce.login_url(PKCE_CLIENT_ID, "state-123", PKCE_REDIRECT_DEEP_LINK);
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("appMode=android"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=tidal%3A%2F%2Flogin%2Fauth"));
        assert!(url.contains("state=state-123"));
        assert!(url.contains(&format!("code_challenge={}", pkce.code_challenge)));
    }

    #[test]
    fn login_url_paste_mode_uses_the_https_redirect() {
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let url = pkce.login_url(PKCE_CLIENT_ID, "s", PKCE_REDIRECT_PASTE);
        assert!(url.contains("redirect_uri=https%3A%2F%2Ftidal.com%2Fandroid%2Flogin%2Fauth"));
    }

    #[test]
    fn random_state_is_hex_and_unique() {
        let a = random_state();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, random_state());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p oxitide-tidal pkce`
Expected: FAIL — `PKCE_REDIRECT_DEEP_LINK`, `PKCE_REDIRECT_PASTE`, `random_state`, and the three-argument `login_url` are missing.

- [ ] **Step 3: Implement the `pkce.rs` changes**

Replace the `PKCE_REDIRECT_URI` constant:

```rust
pub const PKCE_CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";
/// Redirect captured by the OS and handed to OxiTide (primary flow).
pub const PKCE_REDIRECT_DEEP_LINK: &str = "tidal://login/auth";
/// Redirect that leaves a pasteable URL in the browser (fallback flow).
pub const PKCE_REDIRECT_PASTE: &str = "https://tidal.com/android/login/auth";
pub const AUTHORIZE_URL: &str = "https://login.tidal.com/authorize";
pub const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";
```

Replace `login_url`:

```rust
    pub fn login_url(&self, client_id: &str, state: &str, redirect_uri: &str) -> String {
        format!(
            "{base}?response_type=code&redirect_uri={redirect}&client_id={client}&lang=EN\
&appMode=android&client_unique_key={key}&code_challenge={challenge}\
&code_challenge_method=S256&restrict_signup=true&state={state}",
            base = AUTHORIZE_URL,
            redirect = urlencode(redirect_uri),
            client = client_id,
            key = self.client_unique_key,
            challenge = self.code_challenge,
            state = urlencode(state),
        )
    }
```

Add `random_state` next to `random_unique_key`:

```rust
/// A URL-safe anti-forgery `state` value (16 random bytes, hex).
pub fn random_state() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
```

- [ ] **Step 4: Write the failing `auth.rs` tests**

In `crates/oxitide-tidal/src/auth.rs`, add `use crate::pkce::PKCE_REDIRECT_PASTE;` to the test module imports (or reference `crate::pkce::PKCE_REDIRECT_PASTE` inline), update every `exchange_code(...)` call to pass the redirect URI as the fourth argument, and add the validation tests:

```rust
    #[test]
    fn validate_redirect_accepts_tidal_login() {
        let out = validate_redirect("tidal://login/auth?code=ABC&state=s1").unwrap();
        assert_eq!(out.code, "ABC");
        assert_eq!(out.state.as_deref(), Some("s1"));
    }

    #[test]
    fn validate_redirect_rejects_other_targets() {
        for url in [
            "https://tidal.com/android/login/auth?code=ABC",
            "oxitide://login/auth?code=ABC",
            "tidal://other/auth?code=ABC",
            "tidal://login/wrong?code=ABC",
            "tidal://login/auth?state=only",
        ] {
            assert!(validate_redirect(url).is_err(), "should reject {url}");
        }
    }
```

Update the three exchange tests, for example:

```rust
        let tokens = exchange_code(&http, &format!("{}/token", server.uri()), &pkce,
            PKCE_REDIRECT_PASTE, "https://tidal.com/android/login/auth?code=XYZ").await.unwrap();
```

- [ ] **Step 5: Run to verify failure**

Run: `cargo test -p oxitide-tidal`
Expected: FAIL — `validate_redirect`/`RedirectOutcome` missing and `exchange_code` arity mismatch.

- [ ] **Step 6: Implement the `auth.rs` changes**

Change the import line to `use crate::pkce::{Pkce, PKCE_CLIENT_ID};` (drop `PKCE_REDIRECT_URI`), add the outcome type and validator, and thread `redirect_uri` through `exchange_code`:

```rust
#[derive(Debug, PartialEq)]
pub struct RedirectOutcome {
    pub code: String,
    pub state: Option<String>,
}

/// Validate that a URL is the TIDAL login redirect for OxiTide and read its
/// `code`/`state`. Rejects anything that is not `tidal://login/auth`.
pub fn validate_redirect(url_str: &str) -> Result<RedirectOutcome> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| Error::Auth(format!("invalid redirect url: {e}")))?;
    if parsed.scheme() != "tidal"
        || parsed.host_str() != Some("login")
        || parsed.path() != "/auth"
    {
        return Err(Error::Auth("unexpected redirect target".into()));
    }
    let mut code = None;
    let mut state = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    let code = code.ok_or_else(|| Error::Auth("redirect url has no code".into()))?;
    Ok(RedirectOutcome { code, state })
}
```

Replace the `exchange_code` signature and the `redirect_uri` form value:

```rust
pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    pkce: &Pkce,
    redirect_uri: &str,
    redirect_url: &str,
) -> Result<TokenSet> {
    let code = extract_code(redirect_url)?;
    let params = [
        ("code", code.as_str()),
        ("client_id", PKCE_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
        ("scope", "r_usr w_usr w_sub"),
        ("code_verifier", pkce.code_verifier.as_str()),
        ("client_unique_key", pkce.client_unique_key.as_str()),
    ];
    // ... unchanged from here
```

- [ ] **Step 7: Update the app call sites so the workspace compiles (behavior still paste/HTTPS)**

In `crates/oxitide-app/src/commands.rs`:

```rust
use oxitide_tidal::pkce::{Pkce, PKCE_CLIENT_ID, PKCE_REDIRECT_PASTE, TOKEN_URL};
```

`auth_begin` (temporary state value; replaced in Task 3):

```rust
#[tauri::command]
pub fn auth_begin(state: State<'_, AppState>) -> Result<LoginPrompt, String> {
    let pkce = Pkce::generate();
    let url = pkce.login_url(PKCE_CLIENT_ID, "", PKCE_REDIRECT_PASTE);
    state.set_pending(pkce);
    Ok(LoginPrompt { url })
}
```

`auth_complete`'s exchange call:

```rust
    let tokens = auth::exchange_code(&http, TOKEN_URL, &pkce, PKCE_REDIRECT_PASTE, &redirect_url)
        .await
        .map_err(|e| e.to_string())?;
```

- [ ] **Step 8: Run to verify pass + commit**

Run: `cargo test --workspace; cargo clippy --workspace -- -D warnings`
Expected: green.
```powershell
git add crates/oxitide-tidal/src/pkce.rs crates/oxitide-tidal/src/auth.rs crates/oxitide-app/src/commands.rs
git commit -m "feat(tidal): add deep-link/paste redirect modes, state, and redirect validation"
```

---

### Task 2: App — deep-link registration and scheme-ownership guard (Phase 0)

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml`
- Modify: `crates/oxitide-app/tauri.conf.json`
- Create: `crates/oxitide-app/src/scheme.rs`
- Modify: `crates/oxitide-app/src/main.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `scheme::points_to_self(reg_output: &str, exe_fragment: &str) -> bool` (pure)
  - `scheme::snapshot() -> Option<PathBuf>`
  - `scheme::remember(backup: Option<PathBuf>)`
  - `scheme::restore_remembered()`
  - `tauri.conf.json` scheme config and the runtime `register_all()` call that Task 3's handler relies on.

- [ ] **Step 1: Add the dependencies**

In `crates/oxitide-app/Cargo.toml` `[dependencies]`, after `tauri-plugin-opener = "2"`:

```toml
tauri-plugin-deep-link = "2"
tauri-plugin-single-instance = { version = "2", features = ["deep-link"] }
```

Run `cargo check -p oxitide-app` to fetch and compile them.

- [ ] **Step 2: Declare the scheme in `tauri.conf.json`**

Add a top-level `plugins` key (sibling of `app`/`bundle`):

```json
  "plugins": {
    "deep-link": {
      "desktop": { "schemes": ["tidal"] }
    }
  },
```

- [ ] **Step 3: Write the failing `scheme.rs` test**

Create `crates/oxitide-app/src/scheme.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_self_registration_case_insensitively() {
        let text = r#"HKEY_CURRENT_USER\Software\Classes\tidal\shell\open\command
    (Default)    REG_SZ    "C:\Program Files\OxiTide\OxiTide.exe" "%1""#;
        assert!(points_to_self(text, "oxitide.exe"));
        assert!(!points_to_self(text, "tidal.exe"));
    }
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test -p oxitide-app scheme`
Expected: FAIL — module/functions missing.

- [ ] **Step 5: Implement `scheme.rs`**

```rust
//! Windows custom-scheme ownership hygiene (spec §5.2). OxiTide registers
//! `tidal://` at startup so TIDAL's login redirect reaches it; because the
//! official TIDAL desktop app may own the same scheme, we snapshot the existing
//! HKCU registration before registering and restore it on exit.

use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(windows)]
const KEY: &str = r"HKCU\Software\Classes\tidal";

/// True when a `reg query` output points the scheme at OxiTide itself
/// (case-insensitive), meaning there is nothing of the official app to preserve.
pub fn points_to_self(reg_output: &str, exe_fragment: &str) -> bool {
    reg_output.to_ascii_lowercase().contains(&exe_fragment.to_ascii_lowercase())
}

static BACKUP: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);

/// Remember the pre-registration snapshot (or `None` when there was none).
pub fn remember(backup: Option<PathBuf>) {
    *BACKUP.lock().unwrap() = Some(backup);
}

/// Restore the remembered handler on exit.
pub fn restore_remembered() {
    let guard = BACKUP.lock().unwrap();
    if let Some(backup) = guard.as_ref() {
        restore(backup);
    }
}

#[cfg(windows)]
fn backup_path() -> Option<PathBuf> {
    crate::config::config_path()?.parent().map(|dir| dir.join("tidal-scheme.reg"))
}

#[cfg(windows)]
fn scheme_points_to_self(exe_name: &str) -> bool {
    let command_key = format!("{KEY}\\shell\\open\\command");
    let mut text = String::new();
    for args in [["query", KEY, "/ve"], ["query", command_key.as_str(), "/ve"]] {
        if let Ok(out) = std::process::Command::new("reg").args(args).output() {
            text.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }
    points_to_self(&text, exe_name)
}

/// Snapshot the current HKCU `tidal` registration before registering. Returns
/// `None` when the key is absent (exit then deletes OxiTide's key and falls back
/// to an HKLM registration) or already points at OxiTide.
#[cfg(windows)]
pub fn snapshot() -> Option<PathBuf> {
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))?;
    if scheme_points_to_self(&exe_name) {
        return None;
    }
    let path = backup_path()?;
    let file = path.to_string_lossy().to_string();
    let ok = std::process::Command::new("reg")
        .args(["export", KEY, &file, "/y"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(path)
}

#[cfg(not(windows))]
pub fn snapshot() -> Option<PathBuf> {
    None
}

/// Import the snapshot, or delete OxiTide's key when there was none.
#[cfg(windows)]
fn restore(backup: &Option<PathBuf>) {
    match backup {
        Some(path) if path.exists() => {
            let file = path.to_string_lossy().to_string();
            let _ = std::process::Command::new("reg")
                .args(["import", &file])
                .status();
            let _ = std::fs::remove_file(path);
        }
        _ => {
            let _ = std::process::Command::new("reg")
                .args(["delete", KEY, "/f"])
                .status();
        }
    }
}

#[cfg(not(windows))]
fn restore(_backup: &Option<PathBuf>) {}
```

Add `mod scheme;` to `main.rs` with the other module declarations.

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p oxitide-app scheme`
Expected: PASS.

- [ ] **Step 7: Wire the plugins, registration, and a log-only handler in `main.rs`**

Add imports:

```rust
use tauri::Emitter;
use tauri_plugin_deep_link::DeepLinkExt;
```

Replace the single builder plugin line so single-instance is first:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
```

Replace the whole `.setup(...)` closure with:

```rust
        .setup(|app| {
            scheme::remember(scheme::snapshot());
            #[cfg(any(windows, target_os = "linux"))]
            if let Err(e) = app.deep_link().register_all() {
                tracing::warn!("deep link registration failed: {e}");
            }
            app.deep_link().on_open_url(|event| {
                for url in event.urls() {
                    tracing::info!("deep link received: {url}");
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    tracing::info!("deep link (startup): {url}");
                }
            }
            app.state::<AppState>().set_handle(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                let config = app.state::<AppState>().config();
                if let Err(e) = window::apply(&window, &config) {
                    tracing::warn!("failed to apply window geometry: {e}");
                }
                if let Err(e) = window.show() {
                    tracing::warn!("failed to show the main window: {e}");
                }
            }
            Ok(())
        })
```

Replace the `RunEvent::Exit` arm of `.run(...)` with:

```rust
            tauri::RunEvent::Exit => {
                scheme::restore_remembered();
                let state = app_handle.state::<AppState>();
                let config = state.config();
                if config.cache.clear_on_exit {
                    if let Some(cache) = state.cache() {
                        cache.clear();
                    }
                }
            }
```

- [ ] **Step 8: Verify build + run the Phase 0 handoff gate (manual, Windows)**

Run: `cargo clippy -p oxitide-app -- -D warnings; npm --prefix ui run check`
Expected: green.

Then:
1. Start the app: `npm --prefix ui run app:dev`
2. In a second PowerShell: `Start-Process "tidal://login/auth?code=probe"`
3. Expect the app's console/logs (`%APPDATA%\OxiTide\logs\`) to show `deep link received: tidal://login/auth?code=probe`.

**Stop and report if the handoff does not arrive** — the design's fallback branch applies (see spec §8).

- [ ] **Step 9: Commit**

```powershell
git add crates/oxitide-app/Cargo.toml crates/oxitide-app/tauri.conf.json crates/oxitide-app/src/scheme.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): register the tidal deep link and guard scheme ownership"
```

---

### Task 3: App — login completion, modes, and result buffering

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/state.rs`
- Modify: `crates/oxitide-app/src/main.rs`

**Interfaces:**
- Consumes: `auth::validate_redirect`, `auth::RedirectOutcome`, `pkce::{PKCE_REDIRECT_DEEP_LINK, PKCE_REDIRECT_PASTE, random_state}` (Task 1); the deep-link plugin wiring (Task 2).
- Produces:
  - `commands::AuthMode { Deep, Paste }` (serde lowercase)
  - `commands::SessionInfoDto` derives `Clone`
  - `commands::LoginOutcome { Success(SessionInfoDto), Error(String) }`
  - `state::PendingLogin { pkce, state, redirect_uri }`
  - `commands::finish_login(&AppHandle, &AppState, String) -> Result<SessionInfoDto, String>`
  - `commands::verify_login_state(&str, &RedirectOutcome) -> Result<(), String>` (pure)
  - `auth_begin(state, mode: Option<AuthMode>)`, `auth_take_result(state)` commands
  - events `auth://success` (SessionInfoDto) and `auth://error` (String)

- [ ] **Step 1: Write the failing `state.rs` tests**

Replace `pending_pkce_is_taken_once` and add the result-buffer test in `crates/oxitide-app/src/state.rs`:

```rust
    use crate::commands::LoginOutcome;

    #[test]
    fn pending_login_is_taken_once() {
        let state = AppState::default();
        state.set_pending(PendingLogin {
            pkce: Pkce::from_parts("v".into(), "k".into()),
            state: "s".into(),
            redirect_uri: "tidal://login/auth".into(),
        });
        assert!(state.take_pending().is_some());
        assert!(state.take_pending().is_none());
    }

    #[test]
    fn last_login_is_taken_once() {
        let state = AppState::default();
        state.set_last_login(LoginOutcome::Error("x".into()));
        assert!(state.take_last_login().is_some());
        assert!(state.take_last_login().is_none());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p oxitide-app state`
Expected: FAIL — `PendingLogin`, `set_last_login`, `take_last_login`, `LoginOutcome` missing.

- [ ] **Step 3: Implement the `state.rs` changes**

Add the pending/outcome types and fields, and replace the old `pending_pkce` accessors:

```rust
use crate::commands::LoginOutcome;

pub struct PendingLogin {
    pub pkce: Pkce,
    pub state: String,
    pub redirect_uri: String,
}
```

In `AppState`, replace `pub pending_pkce: Mutex<Option<Pkce>>` with:

```rust
    pub pending_login: Mutex<Option<PendingLogin>>,
    pub last_login: Mutex<Option<LoginOutcome>>,
```

Replace the accessors:

```rust
    pub fn set_pending(&self, pending: PendingLogin) {
        *self.pending_login.lock().unwrap() = Some(pending);
    }

    pub fn take_pending(&self) -> Option<PendingLogin> {
        self.pending_login.lock().unwrap().take()
    }

    pub fn set_last_login(&self, outcome: LoginOutcome) {
        *self.last_login.lock().unwrap() = Some(outcome);
    }

    pub fn clear_last_login(&self) {
        *self.last_login.lock().unwrap() = None;
    }

    pub fn take_last_login(&self) -> Option<LoginOutcome> {
        self.last_login.lock().unwrap().take()
    }
```

- [ ] **Step 4: Implement the `commands.rs` types, `AuthMode`, and `finish_login`**

Update the imports at the top of `crates/oxitide-app/src/commands.rs`:

```rust
use oxitide_tidal::pkce::{
    random_state, Pkce, PKCE_CLIENT_ID, PKCE_REDIRECT_DEEP_LINK, PKCE_REDIRECT_PASTE, TOKEN_URL,
};
use serde::{Deserialize, Serialize};
use crate::state::{AppState, PendingLogin, Player};
```

Then replace the DTOs:

```rust
#[derive(Serialize, Clone)]
pub struct SessionInfoDto {
    pub country_code: String,
    pub user_id: u64,
}

#[derive(Clone)]
pub enum LoginOutcome {
    Success(SessionInfoDto),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Deep,
    Paste,
}
```

Add the pure state check (unit-tested in Step 5):

```rust
/// A redirect may omit `state`; when present it must match the pending value.
pub fn verify_login_state(expected: &str, redirect: &auth::RedirectOutcome) -> Result<(), String> {
    match redirect.state.as_deref() {
        None => Ok(()),
        Some(got) if got == expected => Ok(()),
        Some(_) => Err("login state mismatch".to_string()),
    }
}
```

Replace `auth_begin`:

```rust
#[tauri::command]
pub fn auth_begin(
    state: State<'_, AppState>,
    mode: Option<AuthMode>,
) -> Result<LoginPrompt, String> {
    let redirect_uri = match mode.unwrap_or(AuthMode::Deep) {
        AuthMode::Deep => PKCE_REDIRECT_DEEP_LINK,
        AuthMode::Paste => PKCE_REDIRECT_PASTE,
    };
    let pkce = Pkce::generate();
    let login_state = random_state();
    let url = pkce.login_url(PKCE_CLIENT_ID, &login_state, redirect_uri);
    state.set_pending(PendingLogin {
        pkce,
        state: login_state,
        redirect_uri: redirect_uri.to_string(),
    });
    state.clear_last_login();
    Ok(LoginPrompt { url })
}
```

Add `finish_login` (the shared completion path) and reduce `auth_complete` to a call:

```rust
pub async fn finish_login(
    app: &tauri::AppHandle,
    state: &AppState,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    let login = state.take_pending().ok_or_else(|| "no login in progress".to_string())?;
    let redirect = auth::validate_redirect(&redirect_url).map_err(|e| e.to_string())?;
    verify_login_state(&login.state, &redirect)?;
    let http = reqwest::Client::new();
    let tokens =
        auth::exchange_code(&http, TOKEN_URL, &login.pkce, &login.redirect_uri, &redirect_url)
            .await
            .map_err(|e| e.to_string())?;
    let session = auth::fetch_session(&http, API_BASE, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    store.save(&tokens).map_err(|e| e.to_string())?;
    let client = TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
    state.set_client(client.clone());
    let config = Config::load();
    state.set_config(config.clone());
    let cache = state.cache().unwrap_or_else(|| crate::player::build_cache(&config));
    state.set_player(replace_player(app, state, client, &config, cache)?);
    Ok(SessionInfoDto { country_code: session.country_code, user_id: session.user_id })
}

#[tauri::command]
pub async fn auth_complete(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    redirect_url: String,
) -> Result<SessionInfoDto, String> {
    finish_login(&app, &state, redirect_url).await
}

#[tauri::command]
pub fn auth_take_result(state: State<'_, AppState>) -> Result<Option<SessionInfoDto>, String> {
    match state.take_last_login() {
        None => Ok(None),
        Some(LoginOutcome::Success(session)) => Ok(Some(session)),
        Some(LoginOutcome::Error(error)) => Err(error),
    }
}
```

Add `use crate::state::{AppState, PendingLogin, Player};` (extend the existing `use`), and import `random_state` and the two redirect constants in the `pkce` import.

- [ ] **Step 5: Add the `verify_login_state` unit tests**

In the `#[cfg(test)] mod tests` of `crates/oxitide-app/src/commands.rs` (create the module if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(state: Option<&str>) -> auth::RedirectOutcome {
        auth::RedirectOutcome { code: "c".into(), state: state.map(str::to_string) }
    }

    #[test]
    fn verify_login_state_accepts_match_or_absent_state() {
        assert!(verify_login_state("s", &outcome(Some("s"))).is_ok());
        assert!(verify_login_state("s", &outcome(None)).is_ok());
    }

    #[test]
    fn verify_login_state_rejects_mismatch() {
        assert!(verify_login_state("s", &outcome(Some("other"))).is_err());
    }
}
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p oxitide-app state; cargo test -p oxitide-app commands`
Expected: PASS.

- [ ] **Step 7: Replace the log-only handler with completion in `main.rs`**

Add `use oxitide_tidal::auth;` and add the handler function above `main`:

```rust
fn handle_deep_link(app: &tauri::AppHandle, raw: &str) {
    if auth::validate_redirect(raw).is_err() {
        tracing::debug!("ignoring non-login deep link: {raw}");
        return;
    }
    let app = app.clone();
    let url = raw.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match commands::finish_login(&app, &state, url).await {
            Ok(session) => {
                state.set_last_login(commands::LoginOutcome::Success(session.clone()));
                let _ = app.emit("auth://success", session);
            }
            Err(error) => {
                tracing::warn!("deep-link login failed: {error}");
                state.set_last_login(commands::LoginOutcome::Error(error.clone()));
                let _ = app.emit("auth://error", error);
            }
        }
    });
}
```

Replace the log-only `on_open_url` / `get_current` block in `setup` (from Task 2) with an owned-handle version that calls the handler:

```rust
            let handle = app.handle().clone();
            app.deep_link().on_open_url({
                let handle = handle.clone();
                move |event| {
                    for url in event.urls() {
                        handle_deep_link(&handle, url.as_str());
                    }
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_deep_link(&handle, url.as_str());
                }
            }
```

Register the new command in `generate_handler![...]`:

```rust
            commands::auth_take_result,
```

- [ ] **Step 8: Full gates + the decisive browser gate**

Run: `cargo test --workspace; cargo clippy --workspace -- -D warnings`
Expected: green.

Then start `npm --prefix ui run app:dev` and, from the existing login screen (still the old UI), click **Open login page**. It now uses the deep-link redirect (Task 3 made `Deep` the default). Complete a real login in the browser. Expect:
- The browser hands off `tidal://login/auth?code=…` to OxiTide.
- OxiTide logs in (the log shows no error), and the file `auth://success` event is emitted (the old UI won't visibly log in yet — this lands in Task 4).

If the redirect does not arrive, stop and report (spec §8 fallback branch).

- [ ] **Step 9: Commit**

```powershell
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/state.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): complete login from deep links with modes and result buffering"
```

---

### Task 4: UI — waiting state, fallback mode, and event handling

**Files:**
- Modify: `ui/src/lib/api.ts`
- Create: `ui/src/lib/authGate.ts`
- Create: `ui/src/lib/authGate.test.ts`
- Modify: `ui/src/views/Login.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: commands `auth_begin(mode)` / `auth_complete` / `auth_take_result`, events `auth://success` / `auth://error` (Task 3).
- Produces:
  - `api.authBegin(mode?: "deep" | "paste")`, `api.authTakeResult()`
  - `AUTH_SUCCESS_EVENT`, `AUTH_ERROR_EVENT` constants
  - `createAuthGate(deps) -> { dispose(): void }`

- [ ] **Step 1: Update `ui/src/lib/api.ts`**

Add the event constants near the top (after the interfaces):

```ts
export const AUTH_SUCCESS_EVENT = "auth://success";
export const AUTH_ERROR_EVENT = "auth://error";
```

Change `authBegin` and add `authTakeResult`:

```ts
  authBegin: (mode: "deep" | "paste" = "deep") =>
    invoke<{ url: string }>("auth_begin", { mode }),
  authTakeResult: () => invoke<SessionInfoDto | null>("auth_take_result"),
```

- [ ] **Step 2: Write the failing `authGate.test.ts`**

Create `ui/src/lib/authGate.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createAuthGate } from "./authGate";
import { AUTH_ERROR_EVENT, AUTH_SUCCESS_EVENT } from "./api";

type Handler = (event: { payload: unknown }) => void;

function harness(takeResult: () => Promise<unknown> = () => Promise.resolve(null)) {
  const handlers = new Map<string, Handler>();
  const listen = vi.fn((event: string, cb: Handler) => {
    handlers.set(event, cb);
    return Promise.resolve(() => handlers.delete(event));
  });
  const onLogin = vi.fn();
  const onError = vi.fn();
  const gate = createAuthGate({ listen, takeResult, onLogin, onError });
  return { handlers, onLogin, onError, gate };
}

describe("createAuthGate", () => {
  it("logs in on auth://success", () => {
    const { handlers, onLogin } = harness();
    handlers.get(AUTH_SUCCESS_EVENT)!({ payload: { country_code: "US", user_id: 1 } });
    expect(onLogin).toHaveBeenCalledTimes(1);
  });

  it("surfaces auth://error", () => {
    const { handlers, onError } = harness();
    handlers.get(AUTH_ERROR_EVENT)!({ payload: "boom" });
    expect(onError).toHaveBeenCalledWith("boom");
  });

  it("consumes a cold-start result", async () => {
    const { onLogin } = harness(() => Promise.resolve({ country_code: "US", user_id: 1 }));
    await vi.waitFor(() => expect(onLogin).toHaveBeenCalledTimes(1));
  });

  it("surfaces a cold-start error", async () => {
    const { onError } = harness(() => Promise.reject("no login in progress"));
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith("no login in progress"));
  });
});
```

- [ ] **Step 3: Run to verify failure**

Run: `npm --prefix ui test -- src/lib/authGate.test.ts`
Expected: FAIL — cannot resolve `./authGate`.

- [ ] **Step 4: Implement `ui/src/lib/authGate.ts`**

```ts
import {
  AUTH_ERROR_EVENT,
  AUTH_SUCCESS_EVENT,
  type SessionInfoDto,
} from "./api";

export interface AuthGateDeps {
  listen: (
    event: string,
    handler: (event: { payload: unknown }) => void,
  ) => Promise<() => void>;
  takeResult: () => Promise<SessionInfoDto | null>;
  onLogin: () => void;
  onError: (message: string) => void;
}

/// Bridges the deep-link login events and the cold-start buffered result to the
/// app's logged-in state. One instance lives for the app's lifetime.
export function createAuthGate(deps: AuthGateDeps): { dispose: () => void } {
  const offs: Array<() => void> = [];
  let disposed = false;
  const keep = (off: () => void) => {
    if (disposed) off();
    else offs.push(off);
  };

  void deps
    .listen(AUTH_SUCCESS_EVENT, () => deps.onLogin())
    .then(keep)
    .catch(() => {});
  void deps
    .listen(AUTH_ERROR_EVENT, (event) => deps.onError(String(event.payload ?? "")))
    .then(keep)
    .catch(() => {});

  void deps
    .takeResult()
    .then((session) => {
      if (!disposed && session) deps.onLogin();
    })
    .catch((e) => {
      if (!disposed) deps.onError(String(e));
    });

  return {
    dispose() {
      disposed = true;
      for (const off of offs) off();
      offs.length = 0;
    },
  };
}
```

- [ ] **Step 5: Run to verify pass**

Run: `npm --prefix ui test -- src/lib/authGate.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 6: Rework `ui/src/views/Login.svelte`**

Replace the `<script>` and markup:

```svelte
<script lang="ts">
  import { api } from "../lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";
  let {
    onLogin,
    message = "",
    authError = "",
  }: { onLogin: () => void; message?: string; authError?: string } = $props();
  let url = $state("");
  let redirect = $state("");
  let error = $state("");
  let busy = $state(false);
  let waiting = $state(false);
  let showFallback = $state(false);

  async function begin(mode: "deep" | "paste") {
    error = "";
    busy = true;
    try {
      const prompt = await api.authBegin(mode);
      url = prompt.url;
      waiting = mode === "deep";
      await openUrl(url);
    } catch (e) {
      error = String(e);
      waiting = false;
    } finally {
      busy = false;
    }
  }

  function reset() {
    waiting = false;
    error = "";
  }

  async function complete() {
    error = "";
    busy = true;
    try {
      await api.authComplete(redirect);
      onLogin();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div style="max-width:640px;margin:10vh auto">
  <h1>{message || "Sign in to TIDAL"}</h1>
  <button onclick={() => begin("deep")} disabled={busy}>Log in with TIDAL</button>
  {#if waiting}
    <p class="muted">Finish signing in in your browser. OxiTide will unlock automatically.</p>
    <button onclick={reset} disabled={busy}>Cancel</button>
  {/if}
  <p>
    <button class="link" onclick={() => (showFallback = !showFallback)}>
      Browser didn't open the app?
    </button>
  </p>
  {#if showFallback}
    <button onclick={() => begin("paste")} disabled={busy}>Open browser (paste-back)</button>
    {#if url && !waiting}
      <p class="muted">After logging in you'll land on an "Oops" page. Paste its full URL below.</p>
    {/if}
    <input placeholder="Paste the redirect URL" bind:value={redirect} style="width:100%" />
    <button onclick={complete} disabled={busy || !redirect}>Complete login</button>
  {/if}
  {#if error || authError}<p style="color:var(--danger)">{error || authError}</p>{/if}
</div>
```

- [ ] **Step 7: Wire `ui/src/App.svelte`**

Add imports:

```ts
  import { createAuthGate } from "./lib/authGate";
```

Add state next to `error`:

```ts
  let authError = $state("");
```

Add a one-shot gate effect (near the existing `session_expired` effect):

```ts
  $effect(() => {
    const gate = createAuthGate({
      listen,
      takeResult: api.authTakeResult,
      onLogin,
      onError: (message) => (authError = message),
    });
    return () => gate.dispose();
  });
```

Clear the error on a successful login in `onLogin`:

```ts
  async function onLogin() {
    authError = "";
    clearPagerCache();
    // ... existing body unchanged
```

Pass it to the view:

```svelte
  <Login
    onLogin={onLogin}
    message={expired ? "Session expired — sign in to continue" : ""}
    {authError}
  />
```

- [ ] **Step 8: Run to verify pass + commit**

Run: `npm --prefix ui test; npm --prefix ui run check`
Expected: green.
```powershell
git add ui/src/lib/api.ts ui/src/lib/authGate.ts ui/src/lib/authGate.test.ts ui/src/views/Login.svelte ui/src/App.svelte
git commit -m "feat(ui): log in from deep links with a paste-back fallback"
```

---

### Task 5: Smoke checklist and final gates

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-28-seamless-login-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the recorded manual evidence required by the repo's testing conventions.

- [ ] **Step 1: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-28-seamless-login-smoke.md`:

```markdown
# Seamless Login — Smoke Checklist

Spec: `docs/superpowers/specs/2026-09-28-oxitide-seamless-login-design.md`
Run: `npm --prefix ui run app:dev` (real account, browser login)

- [ ] Email/password: Log in → browser → sign in → OxiTide returns logged in.
- [ ] Google SSO: same, via Continue with Google.
- [ ] Apple SSO: same, via Continue with Apple.
- [ ] Already running: app open, login completes in place (single-instance forwarding).
- [ ] Cold start: quit OxiTide, click Log in, complete in browser; the deep link launches and logs in OxiTide.
- [ ] Browser handoff prompt appears once and Open launches OxiTide.
- [ ] Fallback: open "Browser didn't open the app?", use "Open browser (paste-back)", paste the Oops URL, Complete login succeeds.
- [ ] Cancel: while waiting, Cancel returns to the idle login screen with no error.
- [ ] Scheme restored: after quitting OxiTide, `Start-Process "tidal://login/auth"` reaches the official TIDAL app (or `reg query HKCU\Software\Classes\tidal` no longer points at OxiTide).
- [ ] No token values appear in `%APPDATA%\OxiTide\logs\`.

Notes:
```

- [ ] **Step 2: Run the full gate suite**

Run: `cargo test --workspace; cargo clippy --workspace -- -D warnings; npm --prefix ui run check; npm --prefix ui test`
Expected: all green.

- [ ] **Step 3: Execute the checklist**

Work through every box in the smoke file, recording outcomes in the Notes section. Report any failure; if the deep-link path is broken, do not paper over it — invoke the spec §8 fallback branch.

- [ ] **Step 4: Commit**

```powershell
git add docs/superpowers/plans/notes/2026-09-28-seamless-login-smoke.md
git commit -m "test: record the seamless login smoke checklist"
```
