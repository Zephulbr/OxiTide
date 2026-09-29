# Track 4b Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Track 4b discovery — For You mixes in Mixes & Radio, immediate track/artist radio playback, and a Similar Artists row on the artist page — on the existing legacy endpoints and token.

**Architecture:** Four thin additions to `oxitide-tidal` (one page method, three list methods with lenient parsing), four Tauri commands, a small `radio.ts` playback helper, and three UI touch-ups (Mixes, Artist, track context menu). No new routes, no auth changes, no new dependencies.

**Tech Stack:** Rust (tokio/reqwest/serde, wiremock tests), Tauri 2 commands, Svelte 5 runes, vitest, svelte-check.

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track4b-discovery-design.md`

**Plan/process note (deviation from the spec's "Task 0" wording):** the spec wanted a live capture before the remaining tasks. There is no token-safe way to call these endpoints before the commands exist (the keyring token is only readable inside the app), and all three parsers are deliberately lenient about the unverified envelope. Live verification therefore happens at the end (Task 8) through the shipped UI, with an explicit debug-and-fix path. Everything else in the spec is implemented unchanged.

## Global Constraints

- Windows-first; **no new dependencies** (Rust or npm) — `tracing` already exists in `oxitide-tidal`.
- All discovery calls are GETs through `TidalClient::get` (single-flight refresh-on-401 lives there); no auth or scope changes.
- Fixed limits: `RADIO_LIMIT = 50`, `SIMILAR_ARTISTS_LIMIT = 20`; the UI never chooses limits.
- Video entries are never playable (audio-only engine) — radio drops `"type": "VIDEO"` items, same rule as `mix_page`.
- Every `#[tauri::command]` must be registered in `generate_handler![]` or `invoke` fails at runtime.
- IPC: UI sends camelCase args (`trackId`), Rust params stay snake_case (`track_id`).
- UI styles use semantic tokens only (`--line`, `--surface-2`, `--radius-card`); icons are inlined Lucide paths, no unicode glyphs.
- Tests never hit the live TIDAL API; HTTP tests use wiremock.
- No changes to Feed, Home, routing, playback internals, or the release-activity feed.

## Review Focus

- **Region-gap / permission failure** on any endpoint: for_you and similar hide their section and the view still renders; radio shows an error toast; nothing hangs in `loading`.
- **Unverified envelopes**: radio/similar may return `{"items": […]}` or a bare array; for_you may be missing entirely. Empty results must degrade gracefully, never throw.
- **Junk radio items**: VIDEO-typed and malformed entries are dropped; one bad item must not sink the list.
- **Empty radio list** (region gap or fully filtered): info toast, current queue and playback untouched.
- **`toasts` prop ripple**: all four `TrackTable` call sites compile and every track list (Explore, Album, Playlist, Tracks) offers "Track Radio" with the official-client label.

---

### Task 1: `for_you_page` client method

**Files:**
- Modify: `crates/oxitide-tidal/src/pages.rs` (method after `explore_page`, tests in the existing `mod tests`)

**Interfaces:**
- Consumes: `TidalClient::page_sections_at(path, extra)` (pages.rs:134), `page_params()` (pages.rs:128).
- Produces: `pub async fn for_you_page(&self) -> Result<Vec<PageSection>>` used by Task 3.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/oxitide-tidal/src/pages.rs` (after `explore_page_reads_the_explore_path`, pages.rs:500):

```rust
    #[tokio::test]
    async fn for_you_page_reads_the_for_you_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/for_you"))
            .and(query_param("deviceType", "BROWSER"))
            .and(query_param("platform", "WEB"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_LIST", "title": "Made for you",
                    "pagedList": {"items": [
                        {"id": "mix-1", "title": "Daily Mix 1", "subTitle": "Daily Mix"}
                    ]}}]}]
            })))
            .mount(&server)
            .await;
        let sections = client(&server).for_you_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Made for you");
        assert_eq!(sections[0].items[0].id, "mix-1");
        assert_eq!(sections[0].items[0].subtitle.as_deref(), Some("Daily Mix"));
    }

    #[tokio::test]
    async fn for_you_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pages/for_you"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })),
            )
            .mount(&server)
            .await;
        assert!(client(&server).for_you_page().await.unwrap().is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal for_you_page`

Expected: FAIL to compile — `no method named 'for_you_page'`.

- [ ] **Step 3: Implement the method**

In `crates/oxitide-tidal/src/pages.rs`, after `explore_page` (ends pages.rs:153):

```rust
    /// The discovery mix feed; same rows/modules shape as home and explore.
    pub async fn for_you_page(&self) -> Result<Vec<PageSection>> {
        self.page_sections_at("pages/for_you", &[]).await
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal for_you_page`

Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/pages.rs
git commit -m "feat(tidal): add the for-you discovery page"
```

---

### Task 2: `discovery.rs` — radio and similar artists

**Files:**
- Create: `crates/oxitide-tidal/src/discovery.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs` (module list, lib.rs:1-16)

**Interfaces:**
- Consumes: `TidalClient::get<T: DeserializeOwned>(path, &[(&str, String)])` (client.rs:87), `models::{Artist, Track}`, `Error::NotFound` behavior from `decode`.
- Produces: `RADIO_LIMIT`, `SIMILAR_ARTISTS_LIMIT`, `TidalClient::track_radio(u64)`, `artist_radio(u64)`, `similar_artists(u64)` — all `Result<Vec<…>>`; used by Task 3.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-tidal/src/discovery.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::error::Error;
    use crate::token_store::{MemoryStore, TokenSet};
    use crate::TidalClient;
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
    async fn track_radio_sends_the_limit_and_drops_videos() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/5/radio"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {"id": 1, "title": "Song", "duration": 200, "artists": [], "type": "TRACK"},
                    {"id": 2, "title": "Clip", "duration": 60, "artists": [], "type": "VIDEO"}
                ]
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).track_radio(5).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 1);
    }

    #[tokio::test]
    async fn track_radio_accepts_a_bare_array_and_skips_junk() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/5/radio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"type": "TRACK"},
                {"id": 3, "title": "Good", "artists": []}
            ])))
            .mount(&server)
            .await;
        let tracks = client(&server).track_radio(5).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 3);
    }

    #[tokio::test]
    async fn artist_radio_reads_the_artist_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/radio"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 4, "title": "A", "artists": []}]
            })))
            .mount(&server)
            .await;
        assert_eq!(client(&server).artist_radio(9).await.unwrap()[0].id, 4);
    }

    #[tokio::test]
    async fn similar_artists_maps_id_name_and_picture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/similar"))
            .and(query_param("limit", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 11, "name": "B", "picture": "p-11"}]
            })))
            .mount(&server)
            .await;
        let artists = client(&server).similar_artists(9).await.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].id, 11);
        assert_eq!(artists[0].name, "B");
        assert_eq!(artists[0].picture.as_deref(), Some("p-11"));
    }

    #[tokio::test]
    async fn radio_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/999/radio"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).track_radio(999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }
}
```

- [ ] **Step 2: Register the module and run the tests to verify they fail**

Add `pub mod discovery;` to `crates/oxitide-tidal/src/lib.rs` between `pub mod client;` and `pub mod error;` (alphabetical).

Run: `cargo test -p oxitide-tidal discovery`

Expected: FAIL to compile — `no method named 'track_radio'`, `'artist_radio'`, `'similar_artists'`.

- [ ] **Step 3: Implement the module**

Prepend to `crates/oxitide-tidal/src/discovery.rs` (above the test module):

```rust
use serde_json::Value;

use crate::client::TidalClient;
use crate::error::Result;
use crate::models::{Artist, Track};

/// Radio responses are fixed-size; 50 matches the official clients' first page.
pub const RADIO_LIMIT: u32 = 50;
/// The artist page renders a single row of similar artists.
pub const SIMILAR_ARTISTS_LIMIT: u32 = 20;

impl TidalClient {
    /// Audio-only radio seeded on a track (`tracks/{id}/radio`).
    pub async fn track_radio(&self, id: u64) -> Result<Vec<Track>> {
        let params = [("limit", RADIO_LIMIT.to_string())];
        let raw: Value = self.get(&format!("tracks/{id}/radio"), &params).await?;
        Ok(radio_tracks_from_value(&raw))
    }

    /// Audio-only radio seeded on an artist (`artists/{id}/radio`).
    pub async fn artist_radio(&self, id: u64) -> Result<Vec<Track>> {
        let params = [("limit", RADIO_LIMIT.to_string())];
        let raw: Value = self.get(&format!("artists/{id}/radio"), &params).await?;
        Ok(radio_tracks_from_value(&raw))
    }

    /// Artists similar to `id` (`artists/{id}/similar`).
    pub async fn similar_artists(&self, id: u64) -> Result<Vec<Artist>> {
        let params = [("limit", SIMILAR_ARTISTS_LIMIT.to_string())];
        let raw: Value = self.get(&format!("artists/{id}/similar"), &params).await?;
        Ok(similar_artists_from_value(&raw))
    }
}

/// v1 collection endpoints return `{"items": […]}`; a bare array is accepted
/// too because the live envelope was never captured.
fn items_from_value(raw: &Value) -> &[Value] {
    raw.as_array()
        .or_else(|| raw.get("items").and_then(Value::as_array))
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Drops videos (the engine is audio-only) and entries that do not parse as
/// tracks, mirroring `mix_detail_from_json`'s best-effort item mapping.
fn radio_tracks_from_value(raw: &Value) -> Vec<Track> {
    items_from_value(raw)
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) != Some("VIDEO"))
        .filter_map(|item| serde_json::from_value::<Track>(item.clone()).ok())
        .collect()
}

fn similar_artists_from_value(raw: &Value) -> Vec<Artist> {
    items_from_value(raw)
        .iter()
        .filter_map(|item| serde_json::from_value::<Artist>(item.clone()).ok())
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal discovery`

Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/discovery.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add radio and similar-artists discovery"
```

---

### Task 3: Tauri commands and registration

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (after `get_mix`, commands.rs:564)
- Modify: `crates/oxitide-app/src/main.rs` (after `commands::get_mix,`, main.rs:253)

**Interfaces:**
- Consumes: Task 1's `for_you_page`, Task 2's `track_radio`/`artist_radio`/`similar_artists`, `require_client`/`state.fail_tidal` (commands.rs:428-434), imported types `Track`/`Artist`/`PageSection` (commands.rs:425-426).
- Produces: IPC commands `get_for_you`, `get_track_radio(track_id)`, `get_artist_radio(artist_id)`, `get_similar_artists(artist_id)` used by Task 4.

- [ ] **Step 1: Add the four command wrappers**

In `crates/oxitide-app/src/commands.rs`, directly after `get_mix` (ends commands.rs:564):

```rust
#[tauri::command]
pub async fn get_for_you(state: State<'_, AppState>) -> Result<Vec<PageSection>, String> {
    require_client(&state)?
        .for_you_page()
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_track_radio(
    state: State<'_, AppState>,
    track_id: u64,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .track_radio(track_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_radio(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .artist_radio(artist_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_similar_artists(
    state: State<'_, AppState>,
    artist_id: u64,
) -> Result<Vec<Artist>, String> {
    require_client(&state)?
        .similar_artists(artist_id)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

- [ ] **Step 2: Register them**

In `crates/oxitide-app/src/main.rs`, after `commands::get_mix,` (main.rs:253) add:

```rust
            commands::get_for_you,
            commands::get_track_radio,
            commands::get_artist_radio,
            commands::get_similar_artists,
```

- [ ] **Step 3: Verify registration and compile**

Run: `cargo build -p oxitide-app`

Expected: compiles clean. Then verify all four names appear in `generate_handler!`:

Run: `rg -n "get_for_you|get_track_radio|get_artist_radio|get_similar_artists" crates/oxitide-app/src/main.rs`

Expected: four matches.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): expose the discovery commands"
```

---

### Task 4: API surface and the radio playback helper

**Files:**
- Modify: `ui/src/lib/api.ts` (near `getMix`/`getArtist`, api.ts:196/223)
- Create: `ui/src/lib/radio.ts`
- Create: `ui/src/lib/radio.test.ts`

**Interfaces:**
- Consumes: IPC names from Task 3; `trackToQueueItem` (api.ts:173), `errorMessage` (`lib/errors`), `playSource` (`lib/playSource.svelte`).
- Produces: `api.getForYou()`, `api.getTrackRadio(trackId)`, `api.getArtistRadio(artistId)`, `api.getSimilarArtists(artistId)`; `playTrackRadio(trackId, toasts)`, `playArtistRadio(artistId, toasts)` — used by Tasks 5-7.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/radio.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { playArtistRadio, playTrackRadio } from "./radio";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: { getTrackRadio: vi.fn(), getArtistRadio: vi.fn(), playTracks: vi.fn() },
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

describe("playTrackRadio", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("queues the mapped tracks", async () => {
    vi.mocked(api.getTrackRadio).mockResolvedValue([{ id: 1 } as never, { id: 2 } as never]);
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.getTrackRadio).toHaveBeenCalledWith(7);
    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 1 }, { track_id: 2 }], 0);
    expect(calls).toHaveLength(0);
  });

  it("informs instead of playing when the radio is empty", async () => {
    vi.mocked(api.getTrackRadio).mockResolvedValue([]);
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls[0]?.message).toContain("No radio tracks");
  });

  it("surfaces a fetch failure as an error toast instead of rejecting", async () => {
    vi.mocked(api.getTrackRadio).mockRejectedValue(new Error("boom"));
    const { toasts, calls } = fakeToasts();

    await playTrackRadio(7, toasts);

    expect(api.playTracks).not.toHaveBeenCalled();
    expect(calls).toHaveLength(1);
    expect(calls[0]?.severity).toBe("error");
    expect(calls[0]?.message).toContain("boom");
  });
});

describe("playArtistRadio", () => {
  it("fetches by artist id and plays", async () => {
    vi.mocked(api.getArtistRadio).mockResolvedValue([{ id: 3 } as never]);
    const { toasts } = fakeToasts();

    await playArtistRadio(9, toasts);

    expect(api.getArtistRadio).toHaveBeenCalledWith(9);
    expect(api.playTracks).toHaveBeenCalledWith([{ track_id: 3 }], 0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/radio.test.ts`

Expected: FAIL — cannot resolve `./radio`.

- [ ] **Step 3: Add the API functions and the helper**

In `ui/src/lib/api.ts`, after `getMix` (api.ts:196) add:

```ts
  getForYou: () => invoke<PageSection[]>("get_for_you"),
  getTrackRadio: (trackId: number) => invoke<Track[]>("get_track_radio", { trackId }),
  getArtistRadio: (artistId: number) => invoke<Track[]>("get_artist_radio", { artistId }),
  getSimilarArtists: (artistId: number) => invoke<Artist[]>("get_similar_artists", { artistId }),
```

Create `ui/src/lib/radio.ts`:

```ts
import { api, trackToQueueItem, type Track } from "./api";
import { errorMessage } from "./errors";
import { playSource } from "./playSource.svelte";
import type { Toasts } from "./toasts.svelte";

async function playRadio(fetchTracks: () => Promise<Track[]>, toasts: Toasts): Promise<void> {
  try {
    const tracks = await fetchTracks();
    if (tracks.length === 0) {
      toasts.show("No radio tracks available.", { severity: "info" });
      return;
    }
    playSource.set(null);
    await api.playTracks(tracks.map(trackToQueueItem), 0);
  } catch (e) {
    toasts.show(errorMessage(e), { severity: "error" });
  }
}

export function playTrackRadio(trackId: number, toasts: Toasts): Promise<void> {
  return playRadio(() => api.getTrackRadio(trackId), toasts);
}

export function playArtistRadio(artistId: number, toasts: Toasts): Promise<void> {
  return playRadio(() => api.getArtistRadio(artistId), toasts);
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/radio.test.ts`

Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/radio.ts ui/src/lib/radio.test.ts
git commit -m "feat(ui): add the discovery API surface and radio playback"
```

---

### Task 5: Mixes & Radio shows For You mixes

**Files:**
- Modify: `ui/src/views/Mixes.svelte` (full rewrite below)
- Modify: `ui/src/App.svelte:391` (`<Mixes {toasts} />` → `<Mixes {nav} {toasts} />`)

**Interfaces:**
- Consumes: Task 4's `api.getForYou`; existing `PageSections` props `{ sections, nav, onPlayMix }` (PageSections.svelte:6-14); `playMix` (mixPlay.ts:6).
- Produces: nothing downstream.

- [ ] **Step 1: Rewrite `Mixes.svelte`**

Full new file content (styles unchanged from the current file):

```svelte
<script lang="ts">
  import { api, type Mix, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import { playMix } from "../lib/mixPlay";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let forYou = $state<PageSection[]>([]);
  let mixes = $state<Mix[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    const [forYouResult, mixesResult] = await Promise.allSettled([
      api.getForYou(),
      api.getFavoriteMixes(),
    ]);
    forYou = forYouResult.status === "fulfilled" ? forYouResult.value : [];
    if (mixesResult.status === "fulfilled") {
      mixes = mixesResult.value;
    } else {
      error = String(mixesResult.reason);
    }
    loading = false;
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
{:else if forYou.length === 0 && mixes.length === 0}
  <EmptyState message="No mixes yet." />
{:else}
  {#if forYou.length > 0}
    <PageSections sections={forYou} {nav} onPlayMix={(id) => void playMix(id, toasts)} />
  {/if}
  {#if mixes.length > 0}
    <h3>Favorite mixes</h3>
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

In `ui/src/App.svelte:391`, change:

```svelte
      {:else if nav.route.name === "mixes"}<Mixes {toasts} />
```

to:

```svelte
      {:else if nav.route.name === "mixes"}<Mixes {nav} {toasts} />
```

- [ ] **Step 2: Type-check**

Run: `npm --prefix ui run check`

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/views/Mixes.svelte ui/src/App.svelte
git commit -m "feat(ui): show for-you mixes in Mixes and Radio"
```

---

### Task 6: Artist radio button and Similar Artists row

**Files:**
- Modify: `ui/src/views/Artist.svelte` (full rewrite below)
- Modify: `ui/src/App.svelte:397` (`<Artist id={nav.route.id} {favorites} />` → pass `{nav} {toasts}`)
- Modify: `ui/src/lib/components/Icon.svelte` (add the Lucide `radio` path)

**Interfaces:**
- Consumes: Task 4's `api.getSimilarArtists` and `playArtistRadio`; `Nav` (`nav.goto({ name: "artist", id })`), `Cover` props `{ uuid, size, radius, alt }`.
- Produces: the `radio` icon name, consumed by the rewrite below; no cross-task symbol.

- [ ] **Step 1: Rewrite `Artist.svelte`**

Full new file content:

```svelte
<script lang="ts">
  import { api, type Artist as ArtistT } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import { playArtistRadio } from "../lib/radio";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    id,
    nav,
    toasts,
    favorites,
  }: { id: number; nav: Nav; toasts: Toasts; favorites: Favorites } = $props();

  let artist = $state<ArtistT | null>(null);
  let similar = $state<ArtistT[]>([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      artist = await api.getArtist(id);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    try {
      similar = await api.getSimilarArtists(id);
    } catch {
      similar = [];
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  {@const artistId = artist.id}
  <header class="view-header">
    <span class="header-art round"><Cover uuid={artist.picture} size={320} radius={80} alt={artist.name} /></span>
    <div class="header-meta">
      <h2>{artist.name}</h2>
      <p class="muted">Artist</p>
      <div class="header-actions">
        <button
          onclick={() => void playArtistRadio(artistId, toasts)}
          aria-label="Start artist radio"
          title="Start artist radio"
        ><Icon name="radio" /></button>
        <button
          class:on={favorites.hasArtist(artistId)}
          onclick={() => favorites.toggleArtist(artistId)}
          aria-label={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
          title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
        ><Icon name="heart" /></button>
      </div>
    </div>
  </header>
  {#if similar.length > 0}
    <h3>Similar artists</h3>
    <div class="similar-grid">
      {#each similar as s (s.id)}
        <button class="similar-card" onclick={() => nav.goto({ name: "artist", id: s.id })} title={s.name}>
          <span class="similar-art"><Cover uuid={s.picture} size={320} radius={80} alt={s.name} /></span>
          <span class="similar-name">{s.name}</span>
        </button>
      {/each}
    </div>
  {/if}
{:else}
  <EmptyState message="Artist not found." />
{/if}

<style>
  .on { color: var(--accent); }
  .on :global(svg) { fill: currentColor; }
  .view-header {
    display: flex;
    align-items: flex-end;
    gap: 20px;
    margin-bottom: 20px;
  }
  .header-art {
    flex: 0 0 auto;
    width: 160px;
  }
  .header-meta {
    min-width: 0;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .similar-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 16px;
  }
  .similar-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    color: inherit;
    text-align: center;
  }
  .similar-card:hover {
    background: var(--surface-2);
  }
  .similar-art {
    display: block;
    width: 100%;
  }
  .similar-name {
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

In `ui/src/App.svelte:397`, change:

```svelte
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {favorites} />
```

to:

```svelte
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {nav} {toasts} {favorites} />
```

- [ ] **Step 2: Add the radio icon**

In `ui/src/lib/components/Icon.svelte`, after the `"disc-3"` entry (Icon.svelte:36), add the Lucide `radio` path verbatim:

```ts
    "radio": '<path d="M16.247 7.761a6 6 0 0 1 0 8.478" /> <path d="M19.075 4.933a10 10 0 0 1 0 14.134" /> <path d="M4.925 19.067a10 10 0 0 1 0-14.134" /> <path d="M7.753 16.239a6 6 0 0 1 0-8.478" /> <circle cx="12" cy="12" r="2" />',
```

- [ ] **Step 3: Type-check**

Run: `npm --prefix ui run check`

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/Artist.svelte ui/src/App.svelte ui/src/lib/components/Icon.svelte
git commit -m "feat(ui): add the artist radio button and similar artists"
```

---

### Task 7: Track Radio in the track context menu

**Files:**
- Modify: `ui/src/lib/menus.ts` (context + item list)
- Modify: `ui/src/lib/menus.test.ts` (expected lists + callback)
- Modify: `ui/src/lib/components/TrackTable.svelte` (props + menu context)
- Modify: `ui/src/views/Explore.svelte:227` (pass `{toasts}`)
- Modify: `ui/src/views/Playlist.svelte:247` (pass `{toasts}`)
- Modify: `ui/src/views/Album.svelte` (Toasts prop + pass `{toasts}`, App.svelte:395)
- Modify: `ui/src/views/CollectionTracks.svelte` (Toasts prop + pass `{toasts}`, App.svelte:389)

**Interfaces:**
- Consumes: Task 4's `playTrackRadio`; `TrackMenuContext` (menus.ts:5-22); `Toasts` type.
- Produces: `TrackMenuContext.startRadio(): void`; `TrackTable` prop `toasts: Toasts`.

- [ ] **Step 1: Update the menu tests first (failing test)**

In `ui/src/lib/menus.test.ts`:

Add `startRadio: vi.fn(),` to `baseCtx` after `play: vi.fn(),` (menus.test.ts:58).

Replace the expected arrays/indices:

```ts
  it("lists the six base actions", () => {
    expect(trackMenuItems(baseCtx()).map((i) => i.id)).toEqual([
      "play",
      "radio",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
    ]);
  });

  it("labels favorite according to state", () => {
    expect(trackMenuItems(baseCtx({ favorites: favorites(true) }))[4].label).toBe("Unfavorite");
    expect(trackMenuItems(baseCtx({ favorites: favorites(false) }))[4].label).toBe("Favorite");
  });
```

and add to the "invokes each callback once" test, after the `ctx.play` assertion:

```ts
    expect(vi.mocked(ctx.startRadio)).toHaveBeenCalledTimes(1);
```

and update the two remaining expected arrays to include `"radio"` after `"play"`:

```ts
    expect(items.map((i) => i.id)).toEqual([
      "play",
      "radio",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "move-up",
      "move-down",
      "remove",
    ]);
```

```ts
    expect(items.map((i) => i.id)).toEqual([
      "play",
      "radio",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "remove",
    ]);
```

Run: `npm --prefix ui test -- src/lib/menus.test.ts`

Expected: FAIL — `radio` missing from the lists.

- [ ] **Step 2: Implement the menu item**

In `ui/src/lib/menus.ts`:

Add to `TrackMenuContext` after `play(): void | Promise<void>;` (menus.ts:10):

```ts
  /// Starts a TIDAL track radio, replacing the queue.
  startRadio(): void;
```

In `trackMenuItems`, insert after the `play` item (menus.ts:28):

```ts
    { id: "radio", label: "Track Radio", run: () => ctx.startRadio() },
```

Run: `npm --prefix ui test -- src/lib/menus.test.ts`

Expected: PASS.

- [ ] **Step 3: Wire `TrackTable` and its call sites**

In `ui/src/lib/components/TrackTable.svelte`:

Add imports near the other lib imports (after `import { trackQualityLabel } from "../quality";`, TrackTable.svelte:8):

```ts
  import { playTrackRadio } from "../radio";
  import type { Toasts } from "../toasts.svelte";
```

Add `toasts,` to the destructured props and `toasts: Toasts;` to the type object (TrackTable.svelte:29-56), next to `favorites,` / `favorites: Favorites;`.

In the `menuFor` context (TrackTable.svelte:201-212), add after `play: () => play(index),`:

```ts
              startRadio: () => void playTrackRadio(t.id, toasts),
```

Then pass the prop:

- `ui/src/views/Explore.svelte:227`: add `{toasts}` to `<TrackTable …>`.
- `ui/src/views/Playlist.svelte:247`: add `{toasts}`.
- `ui/src/views/Album.svelte:96`: add `{toasts}`; in its props (Album.svelte:14-19) add `toasts,` and `toasts: Toasts;` with `import type { Toasts } from "../lib/toasts.svelte";` near the other type imports.
- `ui/src/views/CollectionTracks.svelte:96`: add `{toasts}`; in its props (CollectionTracks.svelte:15-19) add `toasts,` and `toasts: Toasts;` with the same import.
- `ui/src/App.svelte:389`: `<CollectionTracks {player} {favorites} {playlistActions} />` → `<CollectionTracks {player} {favorites} {playlistActions} {toasts} />`.
- `ui/src/App.svelte:395`: `<Album id={nav.route.id} {player} {favorites} {playlistActions} />` → `<Album id={nav.route.id} {player} {favorites} {playlistActions} {toasts} />`.

- [ ] **Step 4: Run the gates**

Run: `npm --prefix ui test` and `npm --prefix ui run check`

Expected: all vitest suites pass; svelte-check reports no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/components/TrackTable.svelte ui/src/views/Explore.svelte ui/src/views/Playlist.svelte ui/src/views/Album.svelte ui/src/views/CollectionTracks.svelte ui/src/App.svelte
git commit -m "feat(ui): add Track Radio to the track context menu"
```

---

### Task 8: Full gates and the human smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-30-track4b-discovery-smoke.md`

**Interfaces:**
- Consumes: everything above; the repo's manual smoke convention (AGENTS.md: "GUI smoke checks require a human, a real account, and a browser login; record them as checklists in `docs/superpowers/plans/notes/`").

- [ ] **Step 1: Run every automated gate**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

Expected: all pass. Fix anything that fails before continuing (clippy failures are treated as errors by the repo).

- [ ] **Step 2: Run the app for human smoke**

Run: `npm --prefix ui run app:dev`

The owner signs in if needed, then walks the checklist below. Record pass/fail and observations (especially region-gap behavior) in the smoke note.

- [ ] **Step 3: Write the smoke note**

Create `docs/superpowers/plans/notes/2026-09-30-track4b-discovery-smoke.md`:

```markdown
# Track 4b discovery — GUI smoke checklist

- **Date:** (fill in)
- **Build:** dev run, account: (fill in country/region if known)
- **Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track4b-discovery-design.md`

## API probe (optional, before clicking)

In the dev WebView inspector console (`Ctrl+Shift+I`, dev builds only):

```js
await window.__TAURI_INTERNALS__.invoke("get_for_you")
await window.__TAURI_INTERNALS__.invoke("get_track_radio", { trackId: <id from any visible track> })
await window.__TAURI_INTERNALS__.invoke("get_artist_radio", { artistId: <id from an artist page> })
await window.__TAURI_INTERNALS__.invoke("get_similar_artists", { artistId: <id> })
```

Record each returned count. Empty results are acceptable only if the UI also degrades
(hidden section / info toast) — note the observed region behavior.

## Checklist

- [ ] Mixes & Radio shows For You section(s) above "Favorite mixes"; cards show art/title/subtitle.
- [ ] Clicking a For You mix replaces the queue and plays it, like a favorite mix.
- [ ] Favorite mixes grid unchanged; both empty → "No mixes yet."
- [ ] Track menu → "Track Radio" from Album, Playlist, Tracks, and Explore; queue replaced, playback starts.
- [ ] Artist page: radio button plays artist radio (queue replaced).
- [ ] Artist page: Similar Artists row renders with round art; clicking navigates to that artist; back works.
- [ ] A radio with no playable tracks (or a region gap) shows the info/error toast and leaves playback untouched.
- [ ] Feed still shows release activity (unchanged).
- [ ] Playback, queue, and the rest of the app behave as before (no regressions).

## Results

- (fill in per item; attach log snippets for any failure)
```

- [ ] **Step 4: Handle failures against the unverified shapes**

If a probe/check comes back empty when a section is expected, capture the raw body before changing code: in `crates/oxitide-tidal/src/discovery.rs`, temporarily change the fetch line in the failing method to log the body, e.g.

```rust
        tracing::debug!(body = %raw, "radio body");
```

run with `$env:RUST_LOG="oxitide_tidal=debug"` before `npm --prefix ui run app:dev`, reproduce, read the app log, then adjust `items_from_value`/mapping **together with its unit tests** (update Task 2 fixtures to the captured shape) and re-run Task 8 Step 1. Do not leave the temporary log line in unless it is genuinely useful at debug level.

If the owner's account/region simply lacks an endpoint (403/404), record it in the smoke note and leave the graceful degradation as shipped.

- [ ] **Step 5: Commit the smoke note**

```bash
git add docs/superpowers/plans/notes/2026-09-30-track4b-discovery-smoke.md
git commit -m "test: record the Track 4b discovery smoke checklist"
```
