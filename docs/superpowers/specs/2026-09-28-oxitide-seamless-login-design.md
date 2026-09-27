# OxiTide — Seamless TIDAL Login (Deep-Link Capture)

- **Status:** Draft for review
- **Date:** 2026-09-28
- **Owner:** V
- **Parents:** `2026-09-21-oxitide-phase1-design.md` (§8 Auth & Security) ·
  `2026-09-21-plan1-foundation-auth-browse.md`
- **Process note:** scope and decisions were brainstormed live with the owner
  (2026-09-28), including three in-browser probes of TIDAL's authorize endpoint.
  This document is the standalone source of truth.

---

## 1. Purpose & Outcome

Today login is a paste-back flow: the app opens TIDAL's authorize page in the system
browser, the user completes login, lands on a TIDAL "Oops" page, and must copy the full
URL back into a text field in OxiTide. Phase 1 §8 deferred the redirect-capture decision
to M2 and implemented the paste-back fallback.

**Outcome:** the user clicks **Log in with TIDAL**, the system browser opens, they sign in
with **email/password, Google, or Apple**, and OxiTide returns to the foreground already
logged in. No copy/paste. The paste-back UI remains available behind a disclosure as a
fallback if the browser handoff fails.

Success = all three SSO methods complete without manual URL copying, and the app is
logged in on return; and no user is ever locked out if the deep link does not arrive.

## 2. Scope

**In scope:** the authorize URL's redirect target; TIDAL's custom-scheme registration on
Windows; single-instance forwarding; deep-link URL validation and completion; the shared
login-completion path; login UI states; the retained paste fallback; scheme-ownership
hygiene around the official TIDAL app; tests.

**Out of scope:** embedded WebView login; the loopback/OAuth-callback approach (proven
unavailable); token storage/refresh (unchanged); `state`-free flow variants; macOS/Linux
deep-link registration beyond what the plugins provide for free; any account or session
management beyond login.

## 3. Findings (owner-run browser probes, 2026-09-28)

These probes are the factual basis for the design; they were run in a real browser
because TIDAL's edge (DataDome) blocks non-browser requests.

| Probe | `redirect_uri` | Client | Result |
|---|---|---|---|
| 1 | `http://127.0.0.1:8765/callback` | `6BDSRdpK9hqEBTgU` (`appMode=android`) | **Rejected** — "Something went wrong … 11102" |
| 2 | `tidal://login/auth` | `6BDSRdpK9hqEBTgU` (`appMode=android`) | **Accepted** — TIDAL login screen renders |
| 3 | `oxitide://login/auth` | `6BDSRdpK9hqEBTgU` (`appMode=android`) | **Rejected** — same error as probe 1 |

Conclusions:

- Loopback redirect is not allowlisted for this client, so an RFC 8252
  system-browser-plus-local-server flow is unavailable.
- The app cannot own a custom scheme; only TIDAL's own `tidal://login/auth` is
  allowlisted. Capture therefore requires registering the `tidal` scheme with Windows.
- Because login happens in the **system browser**, Google and Apple SSO work. This is the
  decisive advantage over the embedded-webview alternative, which cannot support Google
  (WebView2 is blocked by Google policy: `403 disallowed_useragent`).
- Still unverified: that TIDAL actually appends `?code=…` to `tidal://login/auth` after a
  successful login, and that Windows hands it to OxiTide. Phase 0 (walking skeleton)
  verifies this before UI work.

## 4. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| SL-D1 | Capture the redirect via the **`tidal` custom scheme** + Tauri deep-link, not an embedded webview. | The only mechanism that both completes automatically and keeps Google/Apple SSO working (Findings §3). |
| SL-D2 | Keep the **paste-back flow** as a fallback behind a disclosure. | The deep-link handoff depends on OS/browser/bundler behavior; a fallback guarantees no lockout and covers Phase-0 failure. |
| SL-D3 | Add the **`state`** parameter and verify it on return. | Cheap CSRF/forgery hardening; TIDAL echoes `state` on the redirect. |
| SL-D4 | Register `tidal://` **at startup** and **restore the prior handler on exit**, because the official TIDAL desktop app is installed. | OxiTide must own the scheme at login time (it is freshly registered while running); restoring on exit returns `tidal://` links to the official app instead of silently hijacking them. |
| SL-D5 | Use **`tauri-plugin-single-instance` (with the `deep-link` feature)** so a running OxiTide receives the URL instead of a second process. | Standard Tauri pairing; the second instance exits after forwarding. |
| SL-D6 | The login completion logic is **shared** between the `auth_complete` command and the deep-link handler. | One exchange/session/token path; no drift between fallback and primary. |
| SL-D7 | Buffer the **last login result** in `AppState`, exposed by a command, in addition to emitting an event. | A cold-start deep link can complete before the UI's listener is attached; the buffer removes the race. |
| SL-D8 | Clear `session_expired` on successful login. | Fixes a latent bug: after an expiry-triggered re-login the flag stayed set, suppressing a later expiry signal. |
| SL-D9 | Retain `appMode=android`, `restrict_signup=true`, and scope `r_usr w_usr w_sub`. | These are what the working flow already uses; only `redirect_uri` changes. |
| SL-D10 | Reject any incoming deep link that is not `tidal://login/auth` with a `code` and a matching pending `state`. | Guards against forged/unsolicited links and stray `tidal://` traffic intended for the official app. |
| SL-D11 | The fallback uses the **old HTTPS redirect** (`https://tidal.com/android/login/auth`) and thus a **second authorize mode**. | A browser does not leave a `tidal://…` URL in the address bar, so a user whose handoff failed cannot paste it. The pasteable "Oops" URL only exists on the HTTPS redirect. The pending login therefore records which redirect URI was used, and the token exchange sends that same URI (required for validation). |

## 5. Architecture & flow

```
[Login view] --auth_begin--> Rust: generate Pkce + state, build authorize URL
      |                       (redirect_uri = tidal://login/auth)
      |<-- { url } ----------+
      |
      +-- openUrl(url) ------> system browser -> login.tidal.com
                                   |  (email / Google / Apple)
                                   v
                        tidal://login/auth?code=…&state=…
                                   |
                     Windows routes the URL to OxiTide
                     (launch, or forward via single-instance)
                                   |
                DeepLink on_open_url / get_current  (Rust)
                                   |
                   validate scheme/host/path/code/state
                                   |
                   finish_login -> exchange_code -> fetch_session
                                -> keyring save -> build client/player
                                   |
              emit auth://success | buffer result in AppState
                                   |
                         [UI] loggedIn = true
```

### 5.1 Authorization URL

`Pkce::login_url` gains a `state` argument and keeps the existing parameters:

```
https://login.tidal.com/authorize
  ?response_type=code
  &redirect_uri=tidal%3A%2F%2Flogin%2Fauth
  &client_id=6BDSRdpK9hqEBTgU
  &lang=EN
  &appMode=android
  &client_unique_key=…
  &code_challenge=…&code_challenge_method=S256
  &restrict_signup=true
  &state=…
```

The token exchange still sends `redirect_uri = tidal://login/auth` (the constant is shared
by both call sites, so they cannot drift).

### 5.2 Deep-link registration and scheme ownership

- `tauri.conf.json` declares the static scheme:
  `plugins.deep-link.desktop.schemes: ["tidal"]`. The bundled NSIS/MSI installer registers
  it via this config.
- At startup (Windows, and debug for dev), call `app.deep_link().register_all()` so the
  dev build and installed build both point `tidal://` at the running executable.
- Because the official TIDAL desktop app is installed (SL-D4), `main.rs` captures the
  existing `HKCU\Software\Classes\tidal` key **before** registering and restores it on app
  exit. Backup/restore uses `reg export` / `reg import` (Windows-only module), tolerating
  a missing key (fresh machine / official app registered under HKLM).
- If OxiTide crashes mid-login, the scheme stays pointed at OxiTide until the next launch
  restores/refreshes; this is acceptable and documented.

### 5.3 Event delivery and cold start

- Running instance: `on_open_url` fires; the handler completes login and emits
  `auth://success` / `auth://error`.
- Cold start: the URL arrives as a process argument. `get_current()` is checked in `setup`,
  and the completion also stores its result in `AppState.last_login`. The UI reads the
  buffer at boot via `auth_take_result()` and, in the running case, also listens for the
  events. Either path triggers the existing `onLogin()`.
- `auth_begin` resets the buffer so a stale result can never surface for a new attempt.

## 6. Component changes

### 6.1 Rust — `crates/oxitide-tidal`

| File | Change |
|---|---|
| `src/pkce.rs` | Replace `PKCE_REDIRECT_URI` with two constants: `PKCE_REDIRECT_DEEP_LINK = "tidal://login/auth"` and `PKCE_REDIRECT_PASTE = "https://tidal.com/android/login/auth"`. `login_url(&self, client_id, state, redirect_uri)` uses the given redirect and includes `&state=…`. `Pkce` gains no secret fields; `state` is generated by the app layer (§6.2). |
| `src/auth.rs` | New `pub fn validate_redirect(url: &str) -> Result<RedirectOutcome, Error>` (`RedirectOutcome { code: String, state: Option<String> }`) that parses the URL and requires scheme `tidal`, host `login`, path `/auth`, and a `code` query. `extract_code` remains the internal query-parsing helper. `exchange_code` gains a `redirect_uri: &str` parameter used for the token request (must equal the URI used at authorize time); it still reads the code from `redirect_url` via `extract_code`. |

`state` handling: generate in the app layer so the tidal crate stays flow-agnostic; store
it beside the pending PKCE in `AppState`. `login_url` takes it as an argument.

### 6.2 Rust — `crates/oxitide-app`

| File | Change |
|---|---|
| `Cargo.toml` | Add `tauri-plugin-deep-link = "2"`; add `tauri-plugin-single-instance = { version = "2", features = ["deep-link"] }` on the desktop target, per the plugin docs. Windows registry helper uses the existing `std::process::Command` (no new crate) or a small `winreg` dep if the plan prefers it. |
| `tauri.conf.json` | Add `plugins.deep-link.desktop.schemes: ["tidal"]`. |
| `src/main.rs` | Register single-instance **first** (focus `main` on second launch), then deep-link. In `setup`: capture/backup the scheme key, `register_all()`, subscribe to `on_open_url`, and check `get_current()` for cold start. On exit, restore the scheme key. A `handle_deep_link(app, urls)` function validates and calls the shared completion, then emits events and buffers the result. |
| `src/commands.rs` | Extract the body of `auth_complete` into `async fn finish_login(app, state, redirect_url) -> Result<SessionInfoDto, String>`; `auth_complete` calls it. `auth_begin` gains an `AuthMode` (`DeepLink` default / `Paste`), generates `state`, stores the pending PKCE + state + the chosen redirect URI, and resets the result buffer. Add `auth_take_result()`. `SessionInfoDto` derives `Clone`. |
| `src/state.rs` | Pending becomes `Mutex<Option<PendingLogin>>` where `PendingLogin { pkce: Pkce, state: String, redirect_uri: String }`; accessors updated. Add `last_login: Mutex<Option<LoginOutcome>>` with set/take accessors, where `LoginOutcome = Success(SessionInfoDto) | Error(String)`. `session_expired` is already cleared by `replace_player` on every successful login (`commands.rs:220`), so the shared `finish_login` preserves that behavior and needs no extra clear. |

`finish_login` remains the single place that: validates the redirect, takes the pending
login, verifies `state`, exchanges the code, fetches the session, saves tokens, builds the
`TidalClient`, loads config/cache, and replaces the player.

### 6.3 Frontend — `ui/`

| File | Change |
|---|---|
| `src/lib/api.ts` | Add `authTakeResult: () => invoke<SessionInfoDto | null>("auth_take_result")`. Export the auth event names (`AUTH_SUCCESS = "auth://success"`, `AUTH_ERROR = "auth://error"`) or thin `listen` helpers. |
| `src/views/Login.svelte` | Primary "Log in with TIDAL" button: `authBegin("deep")` → `openUrl(url)` → a waiting state ("Finish signing in in your browser…") with a Cancel/reset affordance. Move the paste field/button under a "Browser didn't open the app?" disclosure; the fallback calls `authBegin("paste")` to get the pasteable HTTPS URL, then `authComplete(redirect)`. |
| `src/App.svelte` | Register a top-level `auth://success` / `auth://error` listener (mirroring the `session_expired` effect) so a deep-link login completes even outside the Login view; on success call the existing `onLogin()`. At boot, after `authRestore()`, also call `api.authTakeResult()` to catch a cold-start deep link. |

## 7. Error handling & edge cases

| Case | Behavior |
|---|---|
| Deep link URL fails validation (wrong scheme/host/path, no `code`) | Ignore silently (debug log). Do not clear pending login or change UI state. |
| `state` mismatch | Reject with `auth://error`; keep the paste fallback available. |
| No pending login when a valid-looking deep link arrives | Reject with an error event ("no login in progress"). |
| Exchange or session fetch fails | `finish_login` returns `Err`; deep-link path emits `auth://error`; paste path shows the inline error. |
| Browser does not hand off (no registered handler, user dismisses prompt) | UI stays in the waiting state; the fallback disclosure is reachable; a Cancel resets `busy`. |
| OxiTide not running when the redirect fires | Deep link launches OxiTide; single-instance not involved; cold-start path handles it (§5.3). |
| OxiTide running | Second process forwards argv and exits; running instance receives `on_open_url`. |
| Official TIDAL app holds `tidal://` when login starts | `register_all()` at OxiTide startup re-points it; Phase 0 confirms the handoff reaches OxiTide. If it does not, see §8 fallback branch. |
| App crash before exit | Scheme remains registered to OxiTide; next launch re-captures the current owner (which is now OxiTide) — restoration then restores OxiTide, a no-op net effect. Documented. |
| Session expiry, then deep-link re-login | `expired` snapshot is restored by the existing `onLogin()`; `session_expired` cleared (SL-D8). |
| Repeated/duplicate deep link | First completion consumes the pending login; later links are rejected as "no login in progress". |
| Paste-back used instead | Unchanged; `auth_complete` calls the same `finish_login`. |

## 8. Fallback / reassessment branches

- **Phase 0 fails (no `?code=` on the custom scheme, or no handoff):** abandon deep-link
  capture for the primary path and design the embedded-WebView approach (email/password
  automatic, paste for Google/Apple), keeping this spec's validation/paste work.
- **OxiTide cannot win the handler while the official app is installed:** scope scheme
  registration to the login window only (register at `auth_begin`, restore on completion,
  cancel, or timeout), rather than startup-to-exit.
- **Installer does not register the scheme:** rely on runtime `register_all()` for all
  builds, or add an explicit NSIS/MSI registry instruction.

## 9. Testing

- **Rust unit (`cargo test --workspace`):**
  - `validate_redirect` accepts `tidal://login/auth?code=abc`; rejects `https://…`,
    `oxitide://…`, `tidal://other/auth?code=abc`, and a `tidal://login/auth` without a
    `code`.
  - `login_url` uses the supplied redirect (`tidal://login/auth` for deep-link mode,
    `https://tidal.com/android/login/auth` for paste mode) and includes the given `state`;
    `exchange_code` sends the redirect URI it is given.
  - `auth_take_result` is one-shot; `auth_begin` resets it.
  - Existing wiremock exchange tests stay green (updated for the new `exchange_code`
    argument).
- **UI (`npm --prefix ui test`):**
  - Login view enters the waiting state after `authBegin`; a simulated `auth://success`
    invokes `onLogin`; `auth://error` shows the error; the fallback path still calls
    `authComplete`.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test`.
- **Manual smoke (`docs/superpowers/plans/notes/`):** real account, `app:dev`; log in via
  email/password, Google, and Apple; cold start vs already-running; browser handoff prompt;
  fallback paste; confirm `tidal://` ownership is restored to the official app after exit.

## 10. Security

- The pending PKCE verifier and `state` live only in memory (`AppState`); never persisted.
- Incoming deep links are validated against an exact scheme/host/path and the pending
  `state` (SL-D10); forged links cannot complete a login.
- PKCE remains the proof-of-possession mechanism; the custom scheme does not weaken it.
- Token storage is unchanged: Windows Credential Manager via `KeyringStore`; tokens are
  never logged (`TokenSet` redaction is retained).
- Restoring the scheme key on exit avoids leaving the OS pointed at OxiTide for traffic
  meant for the official app.

## 11. Open items for the plan

- Exact helper shape for the registry backup/restore (`reg export`/`reg import` wrapper
  vs. a `winreg`-based recursive copy), and where the backup file lives
  (`%APPDATA%\OxiTide\` vs. an in-memory copy restored via temporary `.reg` files).
- Whether `Pkce::generate` should own `state` generation or the app layer should; the spec
  assumes the app layer.
- Whether the UI listens for events at `App.svelte` only, or also inside `Login.svelte`
  (the spec assumes both: view-local for the waiting state, app-level for cold/global).
- Precise wording/placement of the fallback disclosure and the waiting-state cancel.
- Whether to also register the scheme in `bundle.windows.nsis`/MSI explicitly, or trust the
  plugin's config-driven installer registration.
