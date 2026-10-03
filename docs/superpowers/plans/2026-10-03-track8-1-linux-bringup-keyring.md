# Track 8.1 Linux Bring-up and Keyring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the WSL2 Ubuntu 22.04 build environment and make keyring persistence platform-correct: target-specific keyring backends, a startup Secret Service sentinel probe that selects a session-only fallback for both TIDAL tokens and scrobble credentials, a `get_keyring_status` command, and a minimal UI notice shown at login and in Settings when persistence is unavailable.

**Architecture:** `oxitide-tidal::token_store` gains `KeyringStatus`, a `KeyringProbe` trait with a keyring-backed `SystemKeyringProbe` (sentinel write/read/delete through the configured store), and `select_token_store`, which returns the process-wide store plus the probe verdict. `AppState` holds both; `main` wires them before the Tauri builder and picks the scrobble `CredentialStore` from the same verdict. The `get_keyring_status` command exposes the verdict through `api.ts`; a pure `keyring.ts` maps it to notice copy rendered by the new `KeyringNotice.svelte` at login and in Settings → Account. Cargo keyring features move to `[target.'cfg(windows)'...]` (`windows-native`) and `[target.'cfg(target_os = "linux")'...]` (`sync-secret-service` + `crypto-rust`).

**Tech Stack:** Rust (Tauri 2, keyring 3, serde, thiserror), Svelte 5 runes, TypeScript, vitest, WSL2 Ubuntu 22.04 (libdbus for secret-service).

**Spec:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md`

## Global Constraints

- Windows-first: all Linux additions are target-gated (`#[cfg(target_os = "linux")]` or `[target.'cfg(target_os = "linux")'.dependencies]`); Windows behavior and artifacts are unchanged.
- No CI. Every check is manual, run by the implementer.
- HTTP tests use `wiremock` only; never hit the live TIDAL API.
- Never log token or credential values. `TokenSet` and `StoredCredential` redacting `Debug` impls stay authoritative. No plaintext or weak-crypto secret fallback files are added.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case. Payload field names follow the Rust DTO serde attributes; keep `ui/src/lib/api.ts` in sync by hand.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- Unit tests never touch the real OS keyring: injected probes and `MemoryStore`/`MemoryCredentialStore` only.
- UI components consume semantic tokens only (`--surface-*`, `--text`, `--accent`, `--warning`, …), never base or `--dyn-*` values; icons are inlined Lucide paths in `lib/components/Icon.svelte` — no unicode glyphs.
- Windows gates after every Windows-runnable task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- WSL2 gate for target-gated keyring code (commands marked `# WSL2 Ubuntu 22.04`): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo build -p oxitide-app`.
- Do not edit `AGENTS.md` in this plan — a sibling plan owns AGENTS.md updates.
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **No Secret Service on Linux (the WSL2 default).** The probe must fail gracefully to a session-only store; login must still work, and no startup error may surface. Pinned by `select_token_store_falls_back_to_memory_when_the_probe_fails` (Task 3) and `sentinel_probe_rejects_a_store_that_loses_the_secret` (Task 3); smoke-covered by the restart check (Task 8).
- **Wrong keyring backend compiled in.** With no Linux keyring feature, keyring v3 silently uses its mock store: the sentinel round-trips and everything looks persistent while nothing survives a restart. This is a build-config property that is invisible at runtime by design, so it is pinned by the Task 2 WSL2 `cargo tree -e features -i keyring` check (must show `sync-secret-service` + `crypto-rust` + `dbus-secret-service`) rather than a unit test; `fallback_store_is_session_only` (Task 3) pins the fallback's session-only lifetime.
- **Windows regression.** Existing sessions must keep persisting through Credential Manager, and auth must save/clear through the one startup-selected store instead of constructing stores ad hoc. Pinned by `select_token_store_prefers_the_keyring_when_the_probe_succeeds` (Task 3), `selected_token_store_is_returned_by_later_reads` (Task 5), and the Windows gates.
- **Scrobble credentials diverging from the token fallback.** If Last.fm/ListenBrainz keep using a dead keyring while tokens fall back, connecting a service fails with a keyring error. Pinned by `fallback_selection_is_session_only` (Task 4); smoke-covered by the Last.fm connect check (Task 8).
- **Notice contract.** The notice must appear exactly when persistence is off, stating cause (no Secret Service), consequence (login will not persist), and provider links — and never show a false alarm when persistence works. Pinned by the `keyringNotice` tests (Task 6).

---

## File Structure

**Docs**

- Create `docs/linux-build.md` — WSL2 Ubuntu 22.04 environment, dependencies, gates, WSLg/keyring notes.
- Create `docs/superpowers/plans/notes/2026-10-03-track8-1-keyring-smoke.md` — human WSLg checklist.

**Rust**

- Modify `crates/oxitide-tidal/Cargo.toml:15` — target-specific keyring features.
- Modify `crates/oxitide-app/Cargo.toml:26` — target-specific keyring features.
- Modify `Cargo.lock` — regenerated by cargo for the Linux keyring tree.
- Modify `crates/oxitide-tidal/src/token_store.rs` — `KeyringStatus`, `ProbeEntry`, `probe_entry`, `KeyringProbe`, `SystemKeyringProbe`, `select_token_store` + tests.
- Modify `crates/oxitide-app/src/scrobble/credentials.rs` — always-compiled `MemoryCredentialStore`, `select_credential_store` + test.
- Modify `crates/oxitide-app/src/state.rs` — token store + keyring status fields/accessors + tests.
- Modify `crates/oxitide-app/src/commands.rs` — auth flow uses the state store; new `get_keyring_status` command.
- Modify `crates/oxitide-app/src/main.rs` — startup probe + scrobble store selection + handler registration.

**UI**

- Modify `ui/src/lib/api.ts` — `KeyringStatus` interface + `getKeyringStatus` wrapper.
- Create `ui/src/lib/keyring.ts` + `ui/src/lib/keyring.test.ts` — notice copy (pure).
- Create `ui/src/lib/components/KeyringNotice.svelte` — the notice.
- Modify `ui/src/views/Settings.svelte:15,705` — import + render.
- Modify `ui/src/views/Login.svelte:4,63` — import + render.

---

### Task 1: Linux build environment doc

**Files:**
- Create: `docs/linux-build.md`

**Interfaces:**
- Consumes: nothing.
- Produces: the WSL2 gate commands referenced by Tasks 2 and 8 (`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo build -p oxitide-app`, `npm --prefix ui run check`, `npm --prefix ui test`).

- [ ] **Step 1: Create the file**

Create `docs/linux-build.md` with exactly:

````markdown
# Linux build environment — WSL2 Ubuntu 22.04

Track 8's pinned Linux build base is **WSL2 Ubuntu 22.04 LTS** (the WebKitGTK
4.1 floor and AppImage's oldest-supported-base guidance; Track 8 spec §4.7).
Builds must run on Linux: Tauri does not cross-compile Windows → Linux.
Bare-metal audio/desktop smoke is deferred (spec §5.3), but every build and
cargo gate can be run here.

## 1. Install WSL2 + Ubuntu 22.04 (from Windows)

In an elevated Windows PowerShell:

```powershell
wsl --install -d Ubuntu-22.04
```

Reboot if prompted, create the Linux user, and confirm the distro:

```powershell
wsl -d Ubuntu-22.04 -- uname -a
```

All later commands run **inside the Ubuntu 22.04 shell**, from the repository
root.

## 2. System packages (Tauri v2 + audio + keyring)

```bash
sudo apt update
sudo apt install -y \
  build-essential curl wget file pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libasound2-plugins libdbus-1-dev \
  patchelf
```

- `build-essential`, `curl`, `wget`, `file`, `pkg-config`: general build tools.
- `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libxdo-dev`, `libssl-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`: Tauri v2 prerequisites for
  Ubuntu 22.04 (webkit2gtk 4.1 is the floor).
- `libasound2-dev`, `libasound2-plugins`: ALSA headers and the PulseAudio
  plugin that routes shared mode to WSLg's audio bridge (plan 8.2).
- `libdbus-1-dev`: keyring's `sync-secret-service` backend links libdbus.
- `patchelf`: AppImage bundling (plan 8.3).

## 3. Rust (per `rust-toolchain.toml`)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup show
```

`rust-toolchain.toml` pins `stable` with `clippy` + `rustfmt`; `rustup show`
run from the repo root installs and selects that toolchain.

## 4. Node 20

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
node --version   # v20.x
```

Install the UI dependencies **from inside WSL** (rollup/esbuild native binaries
are platform-specific; a `node_modules` populated on Windows will not work):

```bash
rm -rf ui/node_modules
npm --prefix ui install
```

If the checkout lives on the Windows filesystem (`/mnt/c/...`), prefer a
Linux-side clone or worktree for speed and to avoid file-watching issues.

## 5. Build and test gates

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build -p oxitide-app
npm --prefix ui run check
npm --prefix ui test
```

Run the app under WSLg:

```bash
npm --prefix ui run app:dev
```

## 6. WSLg notes

- WSLg supplies the Wayland/X11 display and a PulseAudio server to the distro;
  GUI windows and shared-mode audio work without extra configuration.
- Blank/black WebKitGTK window in a VM? Retry with
  `WEBKIT_DISABLE_COMPOSITING_MODE=1 npm --prefix ui run app:dev` (software
  compositing). This is a WSLg/VM rendering workaround, not a product setting.
- Exclusive/bit-perfect output and hardware mixers cannot be verified here; they
  are on the deferred bare-metal checklist (spec §5.3).

## 7. Keyring / Secret Service

WSL2 Ubuntu 22.04 has no Secret Service provider by default. OxiTide probes for
one at startup and falls back to a session-only store: login works, but nothing
persists across restarts, and the UI shows a notice. That is the expected
default here and is covered by the Track 8.1 smoke checklist.

To exercise the persistent path:

```bash
sudo apt install -y gnome-keyring
dbus-run-session -- bash -lc \
  'echo -n "" | gnome-keyring-daemon --unlock --components=secrets; npm --prefix ui run app:dev'
```

The daemon unlocks the login keyring with an empty password for the disposable
WSL environment (never do this on a real desktop). With it running, a login
survives a restart and the notice is absent.
````

- [ ] **Step 2: Verify**

Run: `Test-Path -LiteralPath docs/linux-build.md`
Expected: `True`.

- [ ] **Step 3: Commit**

```bash
git add docs/linux-build.md
git commit -m "docs: add the WSL2 Linux build environment guide"
```

---

### Task 2: Target-specific keyring features

**Files:**
- Modify: `crates/oxitide-tidal/Cargo.toml:15`
- Modify: `crates/oxitide-app/Cargo.toml:26,36-48`
- Modify: `Cargo.lock` (updated by cargo)

**Interfaces:**
- Consumes: nothing.
- Produces: the per-target keyring backend that `SystemKeyringProbe` (Task 3) relies on: Windows `windows-native`, Linux `sync-secret-service` + `crypto-rust`.

- [ ] **Step 1: Make `oxitide-tidal` keyring target-specific**

In `crates/oxitide-tidal/Cargo.toml`, delete line 15:

```toml
keyring = { version = "3", features = ["windows-native"] }
```

and append after the `[dev-dependencies]` block:

```toml
[target.'cfg(windows)'.dependencies]
keyring = { version = "3", features = ["windows-native"] }

[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3", features = ["sync-secret-service", "crypto-rust"] }

# Unsupported platforms (macOS is out of scope per spec §2) keep keyring's
# featureless mock store so they still compile.
[target.'cfg(not(any(windows, target_os = "linux")))'.dependencies]
keyring = { version = "3" }
```

- [ ] **Step 2: Make `oxitide-app` keyring target-specific**

In `crates/oxitide-app/Cargo.toml`, delete line 26:

```toml
keyring = { version = "3", features = ["windows-native"] }
```

Add `keyring` as the first entry of the existing Windows target section (line 36):

```toml
[target.'cfg(windows)'.dependencies]
keyring = { version = "3", features = ["windows-native"] }
windows = { version = "0.62", features = [
```

and append after that section:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3", features = ["sync-secret-service", "crypto-rust"] }

# Unsupported platforms (macOS is out of scope per spec §2) keep keyring's
# featureless mock store so they still compile.
[target.'cfg(not(any(windows, target_os = "linux")))'.dependencies]
keyring = { version = "3" }
```

- [ ] **Step 3: Verify on Windows**

Run: `cargo test -p oxitide-tidal memory_store_round_trips`
Expected: PASS; `Cargo.lock` is updated with the Linux keyring tree (`dbus-secret-service`, `secret-service`, …) even though it is not built here.

- [ ] **Step 4: Verify on Linux (WSL2)**

Run in WSL2 Ubuntu 22.04 from the repo root:

```bash
# WSL2 Ubuntu 22.04
cargo check -p oxitide-tidal
cargo check -p oxitide-app
cargo tree -p oxitide-tidal -e features -i keyring
```

Expected: both checks succeed (proving `libdbus-1-dev` + the secret-service backend build), and the tree shows `keyring v3.x` with `sync-secret-service` + `crypto-rust` feature edges and `dbus-secret-service v4` beneath it. If `cargo tree` shows no features, the mock store is in use — stop and fix the target config.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/Cargo.toml crates/oxitide-app/Cargo.toml Cargo.lock
git commit -m "build: select keyring backends per target"
```

---

### Task 3: Secret Service probe and session-only token fallback

**Files:**
- Modify: `crates/oxitide-tidal/src/token_store.rs` (imports at line 1; new items after the `MemoryStore` impl ending line 93; tests inside the existing `mod tests`)

**Interfaces:**
- Consumes: `KeyringStore`, `MemoryStore`, `TokenStore` (existing).
- Produces:
  - `pub struct KeyringStatus { pub persistent: bool, pub reason: Option<String> }` (Serialize; `KeyringStatus::persistent()`, `KeyringStatus::unavailable(impl Into<String>)`)
  - `pub trait KeyringProbe { fn probe(&self) -> std::result::Result<(), String>; }`
  - `pub struct SystemKeyringProbe` implementing `KeyringProbe`
  - `pub fn select_token_store(probe: &dyn KeyringProbe) -> (std::sync::Arc<dyn TokenStore>, KeyringStatus)`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-tidal/src/token_store.rs` (before the closing `}`), so it uses the existing `sample()` helper:

```rust
    #[test]
    fn sentinel_probe_rejects_a_store_that_loses_the_secret() {
        struct Forgetful;
        impl ProbeEntry for Forgetful {
            fn write(&self, _secret: &str) -> std::result::Result<(), String> {
                Ok(())
            }
            fn read(&self) -> std::result::Result<String, String> {
                Ok(String::new())
            }
            fn delete(&self) -> std::result::Result<(), String> {
                Ok(())
            }
        }
        assert!(probe_entry(&Forgetful).is_err());
    }

    #[test]
    fn sentinel_probe_writes_reads_then_deletes() {
        #[derive(Default)]
        struct Recording(std::sync::Mutex<Vec<&'static str>>);
        impl ProbeEntry for Recording {
            fn write(&self, _secret: &str) -> std::result::Result<(), String> {
                self.0.lock().unwrap().push("write");
                Ok(())
            }
            fn read(&self) -> std::result::Result<String, String> {
                self.0.lock().unwrap().push("read");
                Ok(PROBE_SENTINEL.to_string())
            }
            fn delete(&self) -> std::result::Result<(), String> {
                self.0.lock().unwrap().push("delete");
                Ok(())
            }
        }
        let recording = Recording::default();
        assert!(probe_entry(&recording).is_ok());
        assert_eq!(*recording.0.lock().unwrap(), ["write", "read", "delete"]);
    }

    struct FakeProbe(std::result::Result<(), String>);

    impl KeyringProbe for FakeProbe {
        fn probe(&self) -> std::result::Result<(), String> {
            self.0.clone()
        }
    }

    #[test]
    fn select_token_store_prefers_the_keyring_when_the_probe_succeeds() {
        let (_, status) = select_token_store(&FakeProbe(Ok(())));
        assert!(status.persistent);
        assert_eq!(status.reason, None);
    }

    #[test]
    fn select_token_store_falls_back_to_memory_when_the_probe_fails() {
        let (store, status) = select_token_store(&FakeProbe(Err("no secret service".into())));
        assert!(!status.persistent);
        assert_eq!(status.reason.as_deref(), Some("no secret service"));
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), sample());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn fallback_store_is_session_only() {
        let (store, _) = select_token_store(&FakeProbe(Err("x".into())));
        store.save(&sample()).unwrap();
        let (fresh, _) = select_token_store(&FakeProbe(Err("x".into())));
        assert!(fresh.load().unwrap().is_none());
    }

    #[test]
    fn keyring_status_serializes_for_ipc() {
        let value = serde_json::to_value(KeyringStatus::unavailable("no bus")).unwrap();
        assert_eq!(value["persistent"], serde_json::json!(false));
        assert_eq!(value["reason"], serde_json::json!("no bus"));
        let ok = serde_json::to_value(KeyringStatus::persistent()).unwrap();
        assert_eq!(ok["persistent"], serde_json::json!(true));
        assert_eq!(ok["reason"], serde_json::Value::Null);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal token_store`
Expected: compile errors (`cannot find trait ProbeEntry`, `cannot find function probe_entry`, `cannot find function select_token_store`, `cannot find type KeyringStatus`) — the failing state.

- [ ] **Step 3: Implement**

At the top of `crates/oxitide-tidal/src/token_store.rs`, change the imports to:

```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
```

Insert after the `MemoryStore` impl block (line 93) and before `#[cfg(test)]`:

```rust
/// Whether the OS credential store can persist secrets this session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyringStatus {
    pub persistent: bool,
    pub reason: Option<String>,
}

impl KeyringStatus {
    pub fn persistent() -> Self {
        Self {
            persistent: true,
            reason: None,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            persistent: false,
            reason: Some(reason.into()),
        }
    }
}

const PROBE_SERVICE: &str = "oxitide-probe";
const PROBE_USER: &str = "probe";
const PROBE_SENTINEL: &str = "oxitide-probe-sentinel";

/// The minimal entry operations the sentinel probe performs. `keyring::Entry`
/// implements it in production; tests inject fakes so no OS store is touched.
trait ProbeEntry {
    fn write(&self, secret: &str) -> std::result::Result<(), String>;
    fn read(&self) -> std::result::Result<String, String>;
    fn delete(&self) -> std::result::Result<(), String>;
}

impl ProbeEntry for keyring::Entry {
    fn write(&self, secret: &str) -> std::result::Result<(), String> {
        self.set_password(secret).map_err(|e| e.to_string())
    }

    fn read(&self) -> std::result::Result<String, String> {
        self.get_password().map_err(|e| e.to_string())
    }

    fn delete(&self) -> std::result::Result<(), String> {
        self.delete_credential().map_err(|e| e.to_string())
    }
}

/// Write, read back, and delete a sentinel through `entry`. Any error or a
/// mismatched read-back means the store cannot be trusted to persist secrets.
fn probe_entry(entry: &dyn ProbeEntry) -> std::result::Result<(), String> {
    entry
        .write(PROBE_SENTINEL)
        .map_err(|e| format!("keyring write failed: {e}"))?;
    let read = entry
        .read()
        .map_err(|e| format!("keyring read failed: {e}"));
    let cleanup = entry
        .delete()
        .map_err(|e| format!("keyring cleanup failed: {e}"));
    let value = read?;
    cleanup?;
    if value == PROBE_SENTINEL {
        Ok(())
    } else {
        Err("the credential store did not return the sentinel".to_string())
    }
}

/// Availability check of the platform credential store. Implementations must
/// not log or expose secret material; the sentinel is non-sensitive.
pub trait KeyringProbe {
    fn probe(&self) -> std::result::Result<(), String>;
}

/// Probes the configured platform store (Credential Manager on Windows,
/// Secret Service on Linux).
pub struct SystemKeyringProbe;

impl KeyringProbe for SystemKeyringProbe {
    fn probe(&self) -> std::result::Result<(), String> {
        let entry =
            keyring::Entry::new(PROBE_SERVICE, PROBE_USER).map_err(|e| e.to_string())?;
        probe_entry(&entry)
    }
}

/// Select the process-wide token store after probing the platform credential
/// store: `KeyringStore` when persistence is available, otherwise a
/// session-only `MemoryStore`.
pub fn select_token_store(probe: &dyn KeyringProbe) -> (Arc<dyn TokenStore>, KeyringStatus) {
    match probe.probe() {
        Ok(()) => (Arc::new(KeyringStore), KeyringStatus::persistent()),
        Err(reason) => (
            Arc::new(MemoryStore::default()),
            KeyringStatus::unavailable(reason),
        ),
    }
}
```

- [ ] **Step 4: Run tests and clippy to verify they pass**

Run: `cargo test -p oxitide-tidal token_store`
Expected: PASS (3 existing + 6 new tests).

Run: `cargo clippy -p oxitide-tidal -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/token_store.rs
git commit -m "feat(tidal): probe the OS keyring and fall back to a session store"
```

---

### Task 4: Session-only fallback for scrobble credentials

**Files:**
- Modify: `crates/oxitide-app/src/scrobble/credentials.rs` (imports at line 1; `MemoryCredentialStore` at lines 59-83; new function before `#[cfg(test)]`; test inside `mod tests`)

**Interfaces:**
- Consumes: `CredentialStore`, `KeyringCredentialStore` (existing).
- Produces: `pub struct MemoryCredentialStore` (now always compiled) and `pub fn select_credential_store(persistent: bool) -> std::sync::Arc<dyn CredentialStore>`.

- [ ] **Step 1: Write the failing test**

Append to the `mod tests` block in `crates/oxitide-app/src/scrobble/credentials.rs`:

```rust
    #[test]
    fn fallback_selection_is_session_only() {
        let store = select_credential_store(false);
        let cred = StoredCredential {
            username: "v".into(),
            secret: "s".into(),
        };
        store.save("lastfm", &cred).unwrap();
        assert_eq!(store.load("lastfm").unwrap(), Some(cred));
        let fresh_session = select_credential_store(false);
        assert!(fresh_session.load("lastfm").unwrap().is_none());
        store.clear("lastfm").unwrap();
        assert!(store.load("lastfm").unwrap().is_none());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app fallback_selection_is_session_only`
Expected: compile error (`cannot find function select_credential_store`) — the failing state.

- [ ] **Step 3: Implement**

At the top of `crates/oxitide-app/src/scrobble/credentials.rs`, change the imports to:

```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};
```

Remove the two `#[cfg(test)]` attributes so the in-memory store is always compiled; the declarations become:

```rust
#[derive(Default)]
pub struct MemoryCredentialStore {
    inner: std::sync::Mutex<std::collections::HashMap<String, StoredCredential>>,
}

impl CredentialStore for MemoryCredentialStore {
```

(the bodies are unchanged).

Insert before `#[cfg(test)] mod tests`:

```rust
/// Store for scrobble credentials: the OS keyring when the startup probe
/// found it usable, otherwise session-only memory (spec T8-D2).
pub fn select_credential_store(persistent: bool) -> Arc<dyn CredentialStore> {
    if persistent {
        Arc::new(KeyringCredentialStore)
    } else {
        Arc::new(MemoryCredentialStore::default())
    }
}
```

- [ ] **Step 4: Run tests and clippy to verify they pass**

Run: `cargo test -p oxitide-app credentials`
Expected: PASS (existing 2 tests + the new one).

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/scrobble/credentials.rs
git commit -m "feat(app): fall back to a session-only scrobble credential store"
```

---

### Task 5: App state, startup wiring, and `get_keyring_status`

**Files:**
- Modify: `crates/oxitide-app/src/state.rs` (imports at line 7; fields after line 41; accessors after line 135; tests at the end)
- Modify: `crates/oxitide-app/src/commands.rs` (import line 14; `finish_login` lines 165-167; `auth_logout` lines 232-235; `auth_restore` lines 287-293; new command after line 236)
- Modify: `crates/oxitide-app/src/main.rs` (probe after line 137; scrobbler store at line 166; handler after line 269)

**Interfaces:**
- Consumes: `select_token_store`, `SystemKeyringProbe`, `KeyringStatus` (Task 3); `select_credential_store` (Task 4).
- Produces: `AppState::set_token_store(Arc<dyn TokenStore>)`, `AppState::token_store() -> Option<Arc<dyn TokenStore>>`, `AppState::set_keyring_status(KeyringStatus)`, `AppState::keyring_status() -> KeyringStatus`; registered command `get_keyring_status() -> KeyringStatus` (TS: `api.getKeyringStatus()`).

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-app/src/state.rs`:

```rust
    #[test]
    fn keyring_status_round_trips() {
        let state = AppState::default();
        assert!(state.keyring_status().persistent);
        state.set_keyring_status(KeyringStatus::unavailable("no secret service"));
        let status = state.keyring_status();
        assert!(!status.persistent);
        assert_eq!(status.reason.as_deref(), Some("no secret service"));
    }

    #[test]
    fn selected_token_store_is_returned_by_later_reads() {
        use oxitide_tidal::token_store::{MemoryStore, TokenSet};
        let state = AppState::default();
        assert!(state.token_store().is_none());
        let store: Arc<dyn TokenStore> = Arc::new(MemoryStore::default());
        let tokens = TokenSet {
            access_token: "a".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_at: None,
        };
        store.save(&tokens).unwrap();
        state.set_token_store(store);
        let loaded = state.token_store().unwrap().load().unwrap().unwrap();
        assert_eq!(loaded.access_token, "a");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-app keyring_status`
Expected: compile errors (`KeyringStatus` unresolved, `no method named keyring_status`/`set_keyring_status`) — the failing state.

- [ ] **Step 3: Implement the state fields and accessors**

In `crates/oxitide-app/src/state.rs`, add after the `Pkce` import (line 7):

```rust
use oxitide_tidal::token_store::{KeyringStatus, TokenStore};
```

Add after the `scrobbler` field (line 41):

```rust
    /// The process-wide token store chosen at startup (keyring or session-only).
    pub token_store: RwLock<Option<Arc<dyn TokenStore>>>,
    /// Result of the startup keyring probe, exposed to the UI.
    pub keyring_status: Mutex<Option<KeyringStatus>>,
```

Add after `scrobbler()` (line 135):

```rust
    pub fn set_token_store(&self, store: Arc<dyn TokenStore>) {
        *self.token_store.write().unwrap() = Some(store);
    }

    pub fn token_store(&self) -> Option<Arc<dyn TokenStore>> {
        self.token_store.read().unwrap().clone()
    }

    pub fn set_keyring_status(&self, status: KeyringStatus) {
        *self.keyring_status.lock().unwrap() = Some(status);
    }

    /// The startup probe result; persistence is assumed until main sets it.
    pub fn keyring_status(&self) -> KeyringStatus {
        self.keyring_status
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(KeyringStatus::persistent)
    }
```

- [ ] **Step 4: Implement the commands**

In `crates/oxitide-app/src/commands.rs`, change the import (line 14) to:

```rust
use oxitide_tidal::token_store::{KeyringStatus, TokenSet, TokenStore};
```

In `finish_login`, replace lines 165-167:

```rust
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
    store.save(&tokens).map_err(|e| e.to_string())?;
    let client = TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
```

with:

```rust
    let store = state
        .token_store()
        .ok_or_else(|| "token store unavailable".to_string())?;
    store.save(&tokens).map_err(|e| e.to_string())?;
    let client = TidalClient::new(http, API_BASE, TOKEN_URL, session.clone(), tokens, store);
```

In `auth_logout`, replace lines 232-233:

```rust
    let store = KeyringStore;
    store.clear().map_err(|e| e.to_string())?;
```

with:

```rust
    if let Some(store) = state.token_store() {
        store.clear().map_err(|e| e.to_string())?;
    }
```

In `auth_restore`, replace line 287:

```rust
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore);
```

with:

```rust
    let store = state
        .token_store()
        .ok_or_else(|| "token store unavailable".to_string())?;
```

Add after `auth_logout` (line 236):

```rust
/// The startup credential-store probe result, for the UI notice.
#[tauri::command]
pub fn get_keyring_status(state: State<'_, AppState>) -> Result<KeyringStatus, String> {
    Ok(state.keyring_status())
}
```

- [ ] **Step 5: Implement the startup wiring**

In `crates/oxitide-app/src/main.rs`, insert after `let state = AppState::default();` (line 137):

```rust
    let (token_store, keyring_status) = oxitide_tidal::token_store::select_token_store(
        &oxitide_tidal::token_store::SystemKeyringProbe,
    );
    if !keyring_status.persistent {
        tracing::warn!(
            reason = %keyring_status.reason.as_deref().unwrap_or("unknown"),
            "OS keyring unavailable; secrets will last only for this session"
        );
    }
    state.set_token_store(token_store);
    state.set_keyring_status(keyring_status.clone());
```

Replace the scrobbler store argument (line 166):

```rust
        std::sync::Arc::new(scrobble::credentials::KeyringCredentialStore),
```

with:

```rust
        scrobble::credentials::select_credential_store(keyring_status.persistent),
```

Add after `commands::auth_logout,` (line 269) inside `generate_handler![...]`:

```rust
            commands::get_keyring_status,
```

- [ ] **Step 6: Run tests and clippy to verify they pass**

Run: `cargo test -p oxitide-app keyring_status`
Expected: PASS (1 test).

Run: `cargo test -p oxitide-app selected_token_store_is_returned_by_later_reads`
Expected: PASS (1 test).

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings (also confirms the new command compiles with the handler registration).

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-app/src/state.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose keyring status and use the startup-selected store"
```

---

### Task 6: Keyring status API and notice copy (UI logic)

**Files:**
- Modify: `ui/src/lib/api.ts` (interface after line 58; wrapper after line 218)
- Create: `ui/src/lib/keyring.ts`
- Create: `ui/src/lib/keyring.test.ts`

**Interfaces:**
- Consumes: `invoke` (existing).
- Produces: `KeyringStatus { persistent: boolean; reason?: string | null }`; `api.getKeyringStatus(): Promise<KeyringStatus>`; `keyringNotice(status: KeyringStatus | null): KeyringNoticeCopy | null` with `KeyringNoticeCopy { body: string; links: { label: string; url: string }[] }`.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/keyring.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { keyringNotice } from "./keyring";

describe("keyringNotice", () => {
  it("is hidden while loading and when persistence is available", () => {
    expect(keyringNotice(null)).toBeNull();
    expect(keyringNotice({ persistent: true, reason: null })).toBeNull();
    expect(keyringNotice({ persistent: true })).toBeNull();
  });

  it("explains the session-only consequence and links the providers", () => {
    const copy = keyringNotice({ persistent: false, reason: "no secret service" });
    expect(copy).not.toBeNull();
    expect(copy!.body).toContain("Secret Service");
    expect(copy!.body).toContain("session");
    expect(copy!.links.map((link) => link.label)).toEqual(["gnome-keyring", "KWallet"]);
    expect(copy!.links.every((link) => link.url.startsWith("https://"))).toBe(true);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/keyring.test.ts`
Expected: FAIL — cannot resolve `./keyring`.

- [ ] **Step 3: Implement**

In `ui/src/lib/api.ts`, add after `SessionInfoDto` (line 58):

```ts
export interface KeyringStatus { persistent: boolean; reason?: string | null }
```

and after `authLogout` (line 218):

```ts
  getKeyringStatus: () => invoke<KeyringStatus>("get_keyring_status"),
```

Create `ui/src/lib/keyring.ts`:

```ts
import type { KeyringStatus } from "./api";

export interface KeyringNoticeCopy {
  body: string;
  links: { label: string; url: string }[];
}

/// Copy for the session-only keyring notice, or `null` when persistence is
/// available (or the status has not loaded yet).
export function keyringNotice(status: KeyringStatus | null): KeyringNoticeCopy | null {
  if (!status || status.persistent) return null;
  return {
    body:
      "OxiTide can't reach the system keyring (Secret Service), so your TIDAL login " +
      "will last only for this session and won't be restored after a restart. " +
      "A keyring provider fixes this:",
    links: [
      { label: "gnome-keyring", url: "https://gitlab.gnome.org/GNOME/gnome-keyring" },
      { label: "KWallet", url: "https://invent.kde.org/frameworks/kwallet" },
    ],
  };
}
```

- [ ] **Step 4: Run tests and check to verify they pass**

Run: `npm --prefix ui test -- src/lib/keyring.test.ts`
Expected: PASS (2 tests).

Run: `npm --prefix ui run check`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/keyring.ts ui/src/lib/keyring.test.ts
git commit -m "feat(ui): add keyring status API and notice copy"
```

---

### Task 7: `KeyringNotice` component and rendering

**Files:**
- Create: `ui/src/lib/components/KeyringNotice.svelte`
- Modify: `ui/src/views/Settings.svelte` (import after line 15; render after line 705)
- Modify: `ui/src/views/Login.svelte` (import after line 4; render after line 63)

**Interfaces:**
- Consumes: `api.getKeyringStatus`, `keyringNotice`, `Icon` (with the existing `alert-triangle` path).
- Produces: `KeyringNotice` with no props; renders nothing unless the status says persistence is unavailable.

- [ ] **Step 1: Create the component**

Create `ui/src/lib/components/KeyringNotice.svelte`:

```svelte
<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type KeyringStatus } from "../api";
  import { keyringNotice } from "../keyring";
  import Icon from "./Icon.svelte";

  let status = $state<KeyringStatus | null>(null);
  const copy = $derived(keyringNotice(status));

  $effect(() => {
    void api
      .getKeyringStatus()
      .then((value) => (status = value))
      .catch(() => {});
  });

  function open(url: string) {
    openUrl(url).catch(() => {});
  }
</script>

{#if copy}
  <div class="keyring-notice" role="status">
    <Icon name="alert-triangle" size={16} />
    <div class="body">
      <p>{copy.body}</p>
      <p class="links">
        {#each copy.links as link (link.url)}
          <button class="link" type="button" onclick={() => open(link.url)}>{link.label}</button>
        {/each}
      </p>
    </div>
  </div>
{/if}

<style>
  .keyring-notice {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin: 8px 0 12px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--warning) 8%, transparent);
    color: var(--text-muted);
  }
  .keyring-notice :global(svg) {
    flex: 0 0 auto;
    margin-top: 2px;
    color: var(--warning);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .keyring-notice p {
    margin: 0;
    line-height: 1.5;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .link {
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
</style>
```

- [ ] **Step 2: Render it in Settings**

In `ui/src/views/Settings.svelte`, add after the `Icon` import (line 15):

```ts
  import KeyringNotice from "../lib/components/KeyringNotice.svelte";
```

and after `<h3>Account</h3>` (line 705):

```svelte
      <KeyringNotice />
```

- [ ] **Step 3: Render it at login**

In `ui/src/views/Login.svelte`, add after the `Loading` import (line 4):

```ts
  import KeyringNotice from "../lib/components/KeyringNotice.svelte";
```

and after the intro paragraph (line 63):

```svelte
    <KeyringNotice />
```

- [ ] **Step 4: Verify**

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: PASS (all suites, including `keyring.test.ts`).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/KeyringNotice.svelte ui/src/views/Settings.svelte ui/src/views/Login.svelte
git commit -m "feat(ui): show a session-only keyring notice"
```

---

### Task 8: WSL2 verification and smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-1-keyring-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: recorded WSL2 gate results and the human checklist for spec §5.3's keyring items.

- [ ] **Step 1: Create the checklist**

Create `docs/superpowers/plans/notes/2026-10-03-track8-1-keyring-smoke.md`:

```markdown
# Track 8.1 keyring smoke checklist (2026-10-03)

Human-in-the-loop checks for the session-only keyring fallback (spec T8-D2,
plan 8.1). Run in WSL2 Ubuntu 22.04 under WSLg with a real TIDAL account and
`npm --prefix ui run app:dev`.

## Automated gates (WSL2 Ubuntu 22.04)

- [ ] `cargo test --workspace` — all pass.
- [ ] `cargo clippy --workspace -- -D warnings` — clean.
- [ ] `cargo build -p oxitide-app` — builds.
- [ ] `npm --prefix ui run check` — 0 errors.
- [ ] `npm --prefix ui test` — all pass.
- [ ] `cargo tree -p oxitide-tidal -e features -i keyring` shows
      `sync-secret-service` + `crypto-rust` + `dbus-secret-service` (not the
      mock store).

## No Secret Service (the WSL2 default)

- [ ] The login screen shows the notice: cause ("can't reach the system keyring
      (Secret Service)"), consequence ("last only for this session"), and both
      provider links.
- [ ] Clicking each provider link opens the browser.
- [ ] Log in and play a track; no keyring error surfaces.
- [ ] Quit and relaunch: the app returns to the login screen (nothing
      persisted) and the notice is still present.
- [ ] After login, Settings → System → Account shows the same notice.
- [ ] Settings → Connections: connect Last.fm; it reports connected for the
      session (and disconnected after a restart).

## With a Secret Service (persistent path)

- [ ] `sudo apt install -y gnome-keyring`
- [ ] `dbus-run-session -- bash -lc 'echo -n "" | gnome-keyring-daemon --unlock --components=secrets; npm --prefix ui run app:dev'`
- [ ] Log in; relaunch; the session restores automatically and the notice is
      absent.
```

- [ ] **Step 2: Run the WSL2 automated gates**

Run in WSL2 Ubuntu 22.04 from the repo root:

```bash
# WSL2 Ubuntu 22.04
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo build -p oxitide-app
cargo tree -p oxitide-tidal -e features -i keyring
npm --prefix ui run check
npm --prefix ui test
```

Expected: all green; the tree shows the secret-service backend (this is the
only check that catches the keyring mock-store feature trap). Tick the
corresponding boxes in the checklist.

- [ ] **Step 3: Run the Windows gates**

Run on Windows from the repo root:

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

Expected: all green (Windows uses `windows-native`; final gate before commit).

- [ ] **Step 4: Run the human WSLg checks**

With a human and a real account, execute the "No Secret Service" section of the
checklist (and the persistent path if gnome-keyring is installed). Tick each
box or record the failure inline. If no human is available, leave the boxes
unchecked and note that in the commit — spec §5.3 defers bare-metal items but
requires WSLg-verifiable items before the plan closes.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-1-keyring-smoke.md
git commit -m "docs: record Track 8.1 keyring verification"
```
