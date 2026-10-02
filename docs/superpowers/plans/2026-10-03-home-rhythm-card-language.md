# Home Rhythm & Card Language Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Home a role-based rhythm (quick-access strip + tile/carousel/list section treatments), a type-keyed card shape language on one shared `MediaCard`, a generic View-all section page, and playable feed tracks.

**Architecture:** Rust exposes each page section's TIDAL module type as `PageSection.kind` and a new `play_track_ids` command resolves track metadata server-side before dispatching the existing audio engine. The UI derives treatment (`classifySection`) and silhouette (`shapeForItemType`) from pure functions in `lib/media.ts`; one `MediaCard` component renders every card; `PageSections` switches between a tile grid, a `Carousel`, and a dense list; a `section` route re-fetches its source and renders uncapped.

**Tech Stack:** Rust (Tauri 2, tokio, wiremock), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-03-home-layout-card-language-design.md`

## Global Constraints

- Windows-first; no new dependencies (tokio is already available; do not add `futures`).
- All Rust HTTP tests use `wiremock`; never hit the live TIDAL API.
- Components read semantic tokens only (`--surface-*`, `--text`, `--line`, `--accent`, …); never base or `--dyn-*` values; never branch on `data-mode`.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case; every new `#[tauri::command]` must be registered in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- Icons are inlined Lucide paths in `Icon.svelte`; no unicode glyphs.
- Caps: quick-access strip 6, tile grid 12, dense list 8.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.

## Review Focus

- **Live module kinds are unverified.** The owner's `pages/home` may use module types outside the classifier's keyword table; unknown kinds must render as carousels with plain shapes and never hide items. Fallbacks are pinned in `media.test.ts`; the table is tuned against real output in Task 9.
- **Track/non-track mixtures.** A section may mix tracks with albums or contain non-numeric track ids; click/Play all must use only numeric `TRACK` ids, in section order. Pinned by `playableTrackIds` tests.
- **Two View-all pages in a row.** `sameRoute` currently treats same-name routes without an `id` as identical, which would make a second View all a silent no-op. Pinned by nav tests.
- **Strip degradation.** If either `getUserPlaylists` or `getFavoriteMixes` rejects, the strip must still render the other source and disappear only when both are empty. Merge/order/cap pinned by `shortcutEntries` tests; the `allSettled` path is in the Task 9 smoke checklist.
- **Carousel edges and short rows.** Arrows must disable at the ends and for rows narrower than the viewport, and stay usable after window resize. No component-test harness exists; verified in the Task 9 smoke checklist.

---

## File Structure

**Rust**

- Modify `crates/oxitide-tidal/src/pages.rs` — `PageSection.kind`, `RawArtist`, track subtitle fallback.
- Modify `crates/oxitide-tidal/src/error.rs` — `Error::Task` for `JoinError`.
- Modify `crates/oxitide-app/src/commands.rs` — `queue_items_from_ids`, `queue_item_from_track`, `play_track_ids`.
- Modify `crates/oxitide-app/src/main.rs` — register `play_track_ids`.

**UI logic**

- Create `ui/src/lib/media.ts` — shape, classification, playable-track, shortcut-merge pure functions.
- Create `ui/src/lib/media.test.ts` — vitest suite.
- Modify `ui/src/lib/api.ts` — `PageSection.kind`, `playTrackIds`.
- Modify `ui/src/lib/nav.svelte.ts` — `section` route, `sameRoute`, `ARTIST` routing.
- Modify `ui/src/lib/nav.test.ts` — new routing/persistence tests.
- Modify `ui/src/styles/tokens.css` — `--radius-card-sharp`, `--card-badge`.

**UI components**

- Create `ui/src/lib/components/MediaCard.svelte` — the shared card.
- Create `ui/src/lib/components/Carousel.svelte` — scroll-snap row + header arrows.
- Rewrite `ui/src/lib/components/PageSections.svelte` — treatments + track playback.
- Create `ui/src/views/Section.svelte` — View-all page.
- Modify `ui/src/views/Home.svelte` — strip + source/toasts.
- Modify `ui/src/views/Explore.svelte` — source/toasts to `PageSections`.
- Modify `ui/src/views/Mixes.svelte` — source/toasts + `MediaCard` favorites grid.
- Modify `ui/src/views/CollectionAlbums.svelte` — `MediaCard`.
- Modify `ui/src/App.svelte` — `Section` dispatch, toasts to Home/Mixes.

---

### Task 1: Expose section kind and track artist subtitles (Rust client)

**Files:**
- Modify: `crates/oxitide-tidal/src/pages.rs`
- Test: `crates/oxitide-tidal/src/pages.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: existing `sections_from_page`, `page_item_from_raw`, `RawItem`.
- Produces: `PageSection { title: String, items: Vec<PageItem>, kind: String }`; track items with no `subTitle`/`description` gain `subtitle = artists.join(", ")`.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-tidal/src/pages.rs` (the module already imports `MockServer`, `Mock`, `ResponseTemplate`, `method`, `path`, and has a `client(&server)` helper):

```rust
    #[tokio::test]
    async fn home_sections_carry_the_module_kind() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIX_LIST", "title": "Mixes", "items": [
                    {"id": "mix-1", "title": "Daily Mix"}
                ]}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections[0].kind, "MIX_LIST");
    }

    #[tokio::test]
    async fn home_sections_without_module_type_default_to_empty_kind() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"title": "Untyped", "items": [{"id": 1, "title": "X"}]}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections[0].kind, "");
    }

    #[tokio::test]
    async fn home_track_items_fall_back_to_artist_names_as_subtitle() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "TRACK_LIST", "title": "New Tracks", "pagedList": {"items": [
                    {"id": 1, "title": "Song", "artists": [{"id": 5, "name": "A"}, {"id": 6, "name": "B"}]},
                    {"id": 2, "title": "Named", "subTitle": "Given", "artists": [{"id": 5, "name": "A"}]},
                    {"id": 3, "title": "Album Item", "type": "ALBUM", "artists": [{"id": 5, "name": "A"}]}
                ]}}]}]
            })))
            .mount(&server).await;
        let items = &client(&server).home_page().await.unwrap()[0].items;
        assert_eq!(items[0].subtitle.as_deref(), Some("A, B"));
        assert_eq!(items[1].subtitle.as_deref(), Some("Given"));
        assert_eq!(items[2].subtitle, None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal home_sections_carry_the_module_kind`
Expected: compile error (`PageSection` has no field `kind`) — the failing state.

- [ ] **Step 3: Implement**

In `crates/oxitide-tidal/src/pages.rs`:

1. Add `kind` to `PageSection` (after `items`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<PageItem>,
    #[serde(default)]
    pub kind: String,
}
```

2. Add the artist shape and field to `RawItem`:

```rust
#[derive(Debug, Deserialize)]
struct RawArtist {
    #[serde(default)]
    pub name: String,
}
```

and inside `RawItem`, after `pub album: Option<RawAlbum>,`:

```rust
    #[serde(default)]
    pub artists: Vec<RawArtist>,
```

3. In `sections_from_page`, pass the module type through:

```rust
            sections.push(PageSection {
                title: module.title,
                items,
                kind: module.module_type,
            });
```

4. In `page_item_from_raw`, replace the `subtitle` line with a computed fallback:

```rust
    let item_type = if raw.item_type.is_empty() {
        module_type.to_string()
    } else {
        raw.item_type
    };
    let subtitle = raw.subtitle.or(raw.description).or_else(|| {
        if item_type.to_uppercase().contains("TRACK") {
            let names: Vec<String> = raw
                .artists
                .iter()
                .map(|a| a.name.clone())
                .filter(|name| !name.is_empty())
                .collect();
            (!names.is_empty()).then(|| names.join(", "))
        } else {
            None
        }
    });
    PageItem {
        id,
        title: raw.title,
        subtitle,
        item_type,
        artwork: raw
            .cover
            // ... existing artwork precedence chain unchanged ...
    }
```

(The `item_type`/`subtitle` computation must happen after the `{type, item}` unwrap block and before the `PageItem` literal; move the existing `let item_type = …` block accordingly if needed.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal`
Expected: all `pages` tests PASS, including the three new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/pages.rs
git commit -m "feat(tidal): expose page section kind and track artist subtitles"
```

---

### Task 2: `play_track_ids` command (Rust app)

**Files:**
- Modify: `crates/oxitide-tidal/src/error.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Test: `crates/oxitide-app/src/commands.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `TidalClient::track(id) -> Result<Track>` (`catalog.rs`), `QueueItem`, `EngineCmd`, `require_client`, `state.fail_tidal`, `state.player()`, `crate::perf::note_play_command`.
- Produces: Tauri command `play_track_ids(ids: Vec<u64>, start: usize, shuffle: Option<bool>) -> Result<(), String>`; helper `queue_items_from_ids(client: &TidalClient, ids: &[u64]) -> Result<Vec<QueueItem>, Error>`.

- [ ] **Step 1: Add the error variant needed to map `JoinError`**

In `crates/oxitide-tidal/src/error.rs`, add to `enum Error` (before `Io`):

```rust
    #[error("task join error: {0}")]
    Task(String),
```

- [ ] **Step 2: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-app/src/commands.rs` (it already imports `MockServer`, `Mock`, `ResponseTemplate`, `method`, `path`, `MemoryStore`; `Arc`, `SessionInfo`, `TokenSet`, `TidalClient` are in scope via `use super::*`):

```rust
    fn tidal_client(server: &MockServer) -> TidalClient {
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
    async fn queue_items_from_ids_preserves_order_and_maps_track_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1, "title": "One", "duration": 210,
                "artists": [{"id": 5, "name": "A"}, {"id": 6, "name": "B"}],
                "album": {"id": 9, "title": "Album", "cover": "c-1"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2, "title": "Two", "duration": 90, "artists": []
            })))
            .mount(&server)
            .await;

        let items = queue_items_from_ids(&tidal_client(&server), &[2, 1])
            .await
            .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track_id, 2);
        assert_eq!(items[0].album, "");
        assert_eq!(items[1].track_id, 1);
        assert_eq!(items[1].title, "One");
        assert_eq!(items[1].artist, "A, B");
        assert_eq!(items[1].album, "Album");
        assert_eq!(items[1].duration_secs, 210);
        assert_eq!(items[1].cover.as_deref(), Some("c-1"));
    }

    #[tokio::test]
    async fn queue_items_from_ids_fails_fast_on_a_mid_list_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1, "title": "One"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tracks/2"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let err = queue_items_from_ids(&tidal_client(&server), &[1, 2])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Api { status: 500, .. }), "got {err:?}");
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p oxitide-app queue_items_from_ids`
Expected: compile error (`queue_items_from_ids` not found) — the failing state.

- [ ] **Step 4: Implement the helper and command**

In `crates/oxitide-app/src/commands.rs`, directly above the `#[tauri::command]` attribute of `play_tracks`:

```rust
/// Build queue items for `ids`, preserving input order, with at most four
/// TIDAL requests in flight. Fails on the first fetch error.
async fn queue_items_from_ids(
    client: &TidalClient,
    ids: &[u64],
) -> Result<Vec<QueueItem>, Error> {
    let mut results: Vec<Option<QueueItem>> = vec![None; ids.len()];
    let mut pending = ids.iter().copied().enumerate();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let Some((index, id)) = pending.next() else {
            break;
        };
        let client = client.clone();
        set.spawn(async move { (index, client.track(id).await) });
    }
    while let Some(joined) = set.join_next().await {
        let (index, result) = joined.map_err(|e| Error::Task(e.to_string()))?;
        results[index] = Some(queue_item_from_track(&result?));
        if let Some((next_index, next_id)) = pending.next() {
            let client = client.clone();
            set.spawn(async move { (next_index, client.track(next_id).await) });
        }
    }
    Ok(results.into_iter().flatten().collect())
}

/// Queue metadata for a track, matching the UI's `trackToQueueItem`.
fn queue_item_from_track(track: &Track) -> QueueItem {
    QueueItem {
        track_id: track.id,
        title: track.title.clone(),
        artist: track
            .artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        album: track
            .album
            .as_ref()
            .map(|a| a.title.clone())
            .unwrap_or_default(),
        duration_secs: track.duration,
        cover: track.album.as_ref().and_then(|a| a.cover.clone()),
    }
}

/// Play a list of track ids (resolved server-side), replacing the queue.
#[tauri::command]
pub async fn play_track_ids(
    state: State<'_, AppState>,
    ids: Vec<u64>,
    start: usize,
    shuffle: Option<bool>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Err("no tracks".to_string());
    }
    let client = require_client(&state)?;
    let items = queue_items_from_ids(&client, &ids)
        .await
        .map_err(|e| state.fail_tidal(e))?;
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let cmd = if shuffle.unwrap_or(false) {
        EngineCmd::PlayShuffled { items }
    } else {
        EngineCmd::PlayTracks { items, start }
    };
    crate::perf::note_play_command();
    player.engine.handle(cmd).map_err(|e| e.to_string())
}
```

`Track` is already imported in `commands.rs` via `use oxitide_tidal::models::{..., Track};`.

- [ ] **Step 5: Register the command**

In `crates/oxitide-app/src/main.rs`, add `commands::play_track_ids,` immediately after `commands::play_tracks,` in `generate_handler![...]`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p oxitide-app`
Expected: all tests PASS, including the two new ones.

- [ ] **Step 7: Lint and commit**

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings.

```bash
git add crates/oxitide-tidal/src/error.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): resolve and play track ids from page feeds"
```

---

### Task 3: Pure media logic (UI)

**Files:**
- Create: `ui/src/lib/media.ts`
- Test: `ui/src/lib/media.test.ts`

**Interfaces:**
- Consumes: `PageItem`, `Playlist`, `Mix` from `./api`.
- Produces: `CardShape`, `SectionTreatment`, `shapeForItemType`, `classifySection`, `isPlayableTrack`, `playableTrackIds`, `ShortcutEntry`, `shortcutEntries`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/media.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Mix, PageItem, Playlist } from "./api";
import {
  classifySection,
  isPlayableTrack,
  playableTrackIds,
  shapeForItemType,
  shortcutEntries,
} from "./media";

function item(item_type: string, id = "1"): PageItem {
  return { id, title: "Item", item_type };
}

describe("shapeForItemType", () => {
  it("maps artists, albums, playlists and mixes", () => {
    expect(shapeForItemType("ARTIST")).toBe("artist");
    expect(shapeForItemType("artist")).toBe("artist");
    expect(shapeForItemType("ALBUM")).toBe("album");
    expect(shapeForItemType("PLAYLIST_LIST")).toBe("playlist");
    expect(shapeForItemType("EDITORIAL")).toBe("playlist");
    expect(shapeForItemType("MIX")).toBe("mix");
    expect(shapeForItemType("MIX_LIST")).toBe("mix");
  });

  it("falls back to plain for tracks, videos and unknown types", () => {
    expect(shapeForItemType("TRACK_LIST")).toBe("plain");
    expect(shapeForItemType("VIDEO")).toBe("plain");
    expect(shapeForItemType("")).toBe("plain");
    expect(shapeForItemType("MIXED_TYPES_LIST")).toBe("plain");
    expect(shapeForItemType("SOMETHING_ELSE")).toBe("plain");
  });
});

describe("classifySection", () => {
  it("returns carousel for an empty section", () => {
    expect(classifySection("ANY", [])).toBe("carousel");
  });

  it("uses list when tracks or videos are the majority", () => {
    expect(
      classifySection("MIXED_TYPES_LIST", [item("TRACK_LIST"), item("TRACK_LIST"), item("ALBUM")]),
    ).toBe("list");
    expect(classifySection("", [item("VIDEO"), item("VIDEO")])).toBe("list");
  });

  it("uses carousel for mixes, radio and albums", () => {
    expect(classifySection("MIX_LIST", [item("MIX")])).toBe("carousel");
    expect(classifySection("RADIO", [item("ALBUM")])).toBe("carousel");
    expect(classifySection("ALBUM_LIST", [item("ALBUM")])).toBe("carousel");
  });

  it("uses tiles for playlist and editorial sections", () => {
    expect(classifySection("PLAYLIST_LIST", [item("PLAYLIST_LIST"), item("EDITORIAL")])).toBe(
      "tiles",
    );
    expect(classifySection("", [item("EDITORIAL")])).toBe("tiles");
  });

  it("falls back to carousel for unknown kinds", () => {
    expect(classifySection("PAGE_LINKS_CLOUD", [item("SOMETHING")])).toBe("carousel");
  });
});

describe("track playback predicates", () => {
  it("only treats numeric track items as playable", () => {
    expect(isPlayableTrack(item("TRACK_LIST", "55"))).toBe(true);
    expect(isPlayableTrack(item("TRACK", "55"))).toBe(true);
    expect(isPlayableTrack(item("TRACK_LIST", "abc"))).toBe(false);
    expect(isPlayableTrack(item("VIDEO", "55"))).toBe(false);
    expect(isPlayableTrack(item("ALBUM", "55"))).toBe(false);
  });

  it("keeps playable ids in order and skips everything else", () => {
    expect(
      playableTrackIds([
        item("TRACK_LIST", "3"),
        item("ALBUM", "9"),
        item("TRACK", "1"),
        item("TRACK_LIST", "x"),
      ]),
    ).toEqual([3, 1]);
  });
});

function playlist(uuid: string, title: string): Playlist {
  return {
    uuid,
    title,
    numberOfTracks: 0,
    duration: 0,
    owned: true,
    squareImage: `sq-${uuid}`,
    image: `im-${uuid}`,
  };
}

function mix(id: string, title: string): Mix {
  return { id, title, image: `m-${id}` };
}

describe("shortcutEntries", () => {
  it("puts playlists first, then saved mixes", () => {
    const entries = shortcutEntries([playlist("p1", "P1")], [mix("m1", "M1")]);
    expect(entries.map((e) => e.route)).toEqual([
      { name: "playlist", id: "p1" },
      { name: "mix", id: "m1" },
    ]);
    expect(entries[0].item.subtitle).toBe("Playlist");
    expect(entries[1].item.subtitle).toBe("Saved mix");
    expect(entries[0].playlist).toBe(true);
    expect(entries[1].playlist).toBe(false);
  });

  it("caps the strip and tolerates missing sources", () => {
    const playlists = Array.from({ length: 5 }, (_, i) => playlist(`p${i}`, `P${i}`));
    const mixes = Array.from({ length: 3 }, (_, i) => mix(`m${i}`, `M${i}`));
    expect(shortcutEntries(playlists, mixes)).toHaveLength(6);
    expect(shortcutEntries([], [])).toEqual([]);
    expect(shortcutEntries([], mixes).map((e) => e.key)).toEqual(["mix-m0", "mix-m1", "mix-m2"]);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/media.test.ts`
Expected: FAIL — cannot resolve `./media`.

- [ ] **Step 3: Implement `ui/src/lib/media.ts`**

```ts
import type { Mix, PageItem, Playlist } from "./api";

export type CardShape = "artist" | "album" | "playlist" | "mix" | "plain";
export type SectionTreatment = "tiles" | "carousel" | "list";

const NUMERIC_ID = /^\d+$/;
const upper = (value: string) => value.toUpperCase();

/// Silhouette for a page item, keyed on TIDAL's item type.
export function shapeForItemType(itemType: string): CardShape {
  const type = upper(itemType);
  if (type.includes("ARTIST")) return "artist";
  if (type.includes("ALBUM")) return "album";
  if (type === "MIX" || type === "MIX_LIST") return "mix";
  if (type.includes("PLAYLIST") || type.includes("EDITORIAL")) return "playlist";
  return "plain";
}

function isTrackish(type: string): boolean {
  return type.includes("TRACK") || type.includes("VIDEO");
}

/// Section treatment from the TIDAL module kind plus item-type composition.
/// Unknown kinds fall back to carousel so new feed rows never break Home.
export function classifySection(kind: string, items: PageItem[]): SectionTreatment {
  if (items.length === 0) return "carousel";
  const k = upper(kind);
  const types = items.map((item) => upper(item.item_type));
  const trackish = types.filter(isTrackish).length;
  if (trackish * 2 > items.length) return "list";
  const mixish =
    k.includes("MIX") ||
    k.includes("RADIO") ||
    types.some((type) => type.includes("MIX") || type.includes("RADIO"));
  if (mixish) return "carousel";
  if (k.includes("ALBUM")) return "carousel";
  const playlistish = types.filter(
    (type) => type.includes("PLAYLIST") || type.includes("EDITORIAL"),
  ).length;
  if (playlistish * 2 > items.length || k.includes("PLAYLIST") || k.includes("EDITORIAL")) {
    return "tiles";
  }
  return "carousel";
}

/// Track items are only playable when their id can be sent to the backend.
export function isPlayableTrack(item: PageItem): boolean {
  return upper(item.item_type).includes("TRACK") && NUMERIC_ID.test(item.id);
}

/// Numeric track ids in section order, skipping everything else.
export function playableTrackIds(items: PageItem[]): number[] {
  return items.filter(isPlayableTrack).map((item) => Number(item.id));
}

export interface ShortcutEntry {
  key: string;
  item: PageItem;
  playlist: boolean;
  route: { name: "playlist"; id: string } | { name: "mix"; id: string };
}

/// Home's quick-access strip: owned playlists first, then saved mixes.
export function shortcutEntries(
  playlists: Playlist[],
  mixes: Mix[],
  cap = 6,
): ShortcutEntry[] {
  const entries: ShortcutEntry[] = [];
  for (const p of playlists) {
    entries.push({
      key: `playlist-${p.uuid}`,
      playlist: true,
      route: { name: "playlist", id: p.uuid },
      item: {
        id: p.uuid,
        title: p.title,
        subtitle: "Playlist",
        item_type: "PLAYLIST_LIST",
        artwork: p.squareImage ?? p.image ?? null,
      },
    });
  }
  for (const m of mixes) {
    entries.push({
      key: `mix-${m.id}`,
      playlist: false,
      route: { name: "mix", id: m.id },
      item: {
        id: m.id,
        title: m.title,
        subtitle: "Saved mix",
        item_type: "MIX_LIST",
        artwork: m.image ?? null,
      },
    });
  }
  return entries.slice(0, cap);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/media.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/media.ts ui/src/lib/media.test.ts
git commit -m "feat(ui): add media shape and section classification helpers"
```

---

### Task 4: API contract and `section` routing

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/nav.svelte.ts`
- Test: `ui/src/lib/nav.test.ts`

**Interfaces:**
- Consumes: existing `Route`, `parseRoute`, `sameRoute`, `routeForPageItem`.
- Produces: `PageSection.kind: string`; `api.playTrackIds`; route `{ name: "section"; source: "home" | "explore" | "forYou"; title: string }`; `ARTIST` item routing.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/nav.test.ts`:

```ts
describe("section routes", () => {
  it("round-trips a section route through storage", () => {
    const storage = fakeStorage();
    const nav = createNav(storage);
    nav.goto({ name: "section", source: "home", title: "Suggested new albums" });

    const restored = createNav(storage);

    expect(restored.route).toEqual({
      name: "section",
      source: "home",
      title: "Suggested new albums",
    });
  });

  it("treats two sections as distinct history entries", () => {
    const nav = createNav();
    nav.goto({ name: "section", source: "home", title: "A" });
    nav.goto({ name: "section", source: "home", title: "B" });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "section", source: "home", title: "A" });
  });

  it("rejects stored sections with an unknown source or empty title", () => {
    const cases = [
      '{"history":[{"name":"section","source":"mixes","title":"A"}],"index":0}',
      '{"history":[{"name":"section","source":"home","title":""}],"index":0}',
      '{"history":[{"name":"section","source":"home"}],"index":0}',
    ];
    for (const raw of cases) {
      expect(createNav(fakeStorage(raw)).route, raw).toEqual({ name: "home" });
    }
  });
});

describe("routeForPageItem artists", () => {
  it("routes artist items with a numeric id", () => {
    expect(routeForPageItem(pageItem("ARTIST", "42"))).toEqual({ name: "artist", id: 42 });
    expect(routeForPageItem(pageItem("ARTIST_LIST", "42"))).toEqual({ name: "artist", id: 42 });
    expect(routeForPageItem(pageItem("ARTIST", "nope"))).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: FAIL — `source` is not a valid route (type error under vitest/esbuild may surface as runtime parse failure) and artist routing returns null.

- [ ] **Step 3: Implement**

`ui/src/lib/api.ts`:

```ts
export interface PageSection { title: string; items: PageItem[]; kind: string }
```

and in the `api` object, next to `playTracks`:

```ts
  playTrackIds: (ids: number[], start: number, shuffle = false) =>
    invoke<void>("play_track_ids", { ids, start, shuffle }),
```

`ui/src/lib/nav.svelte.ts`:

1. Add to the `Route` union:

```ts
  | { name: "section"; source: "home" | "explore" | "forYou"; title: string }
```

2. In `routeForPageItem`, after the mix branch:

```ts
  if (type.includes("ARTIST") && NUMERIC_ID.test(item.id)) {
    return { name: "artist", id: Number(item.id) };
  }
```

3. In `parseRoute`, add a case before `default`:

```ts
    case "section": {
      const source = record.source;
      if (source === "home" || source === "explore" || source === "forYou") {
        const title = record.title;
        if (typeof title === "string" && title.length > 0) {
          return { name: "section", source, title };
        }
      }
      return null;
    }
```

4. Update `sameRoute`:

```ts
function sameRoute(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  if (a.name === "section" && b.name === "section") {
    return a.source === b.source && a.title === b.title;
  }
  if ("id" in a && "id" in b) return a.id === b.id;
  return true;
}
```

- [ ] **Step 4: Run tests and type-check**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: PASS.

Run: `npm --prefix ui run check`
Expected: PASS (no other code constructs `PageSection` literals).

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts
git commit -m "feat(ui): add section route and artist page-item routing"
```

---

### Task 5: Tokens, `MediaCard`, `Carousel`

**Files:**
- Modify: `ui/src/styles/tokens.css`
- Create: `ui/src/lib/components/MediaCard.svelte`
- Create: `ui/src/lib/components/Carousel.svelte`

**Interfaces:**
- Consumes: `shapeForItemType`, `isPlayableTrack` (Task 3), `Cover`, `PlaylistCover`, `Icon`.
- Produces: `MediaCard` props `{ item, variant: "tile" | "carousel" | "row", thumb?, onclick?, collage? }`; `Carousel` props `{ title, controls?, children }`.

- [ ] **Step 1: Add the tokens**

In `ui/src/styles/tokens.css`, after `--radius-card: 12px;`:

```css
  --radius-card-sharp: 4px;
  --card-badge: 20px;
```

- [ ] **Step 2: Create `ui/src/lib/components/MediaCard.svelte`**

```svelte
<script lang="ts">
  import type { PageItem } from "../api";
  import { isPlayableTrack, shapeForItemType } from "../media";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import PlaylistCover from "./PlaylistCover.svelte";

  let {
    item,
    variant,
    thumb = 40,
    onclick,
    collage = false,
  }: {
    item: PageItem;
    variant: "tile" | "carousel" | "row";
    thumb?: number;
    onclick?: () => void;
    collage?: boolean;
  } = $props();

  const shape = $derived(shapeForItemType(item.item_type));
  const playable = $derived(isPlayableTrack(item));
</script>

{#snippet body()}
  <span class="art" style:--thumb="{thumb}px">
    <span class="art-clip">
      {#if collage}
        <PlaylistCover
          owned
          playlist={item.id}
          image={item.artwork}
          size={variant === "row" ? 160 : 320}
          radius={0}
          alt=""
        />
      {:else}
        <Cover uuid={item.artwork} size={variant === "row" ? 160 : 320} radius={0} alt="" />
      {/if}
    </span>
    {#if shape === "mix"}
      <span class="badge" aria-hidden="true"><Icon name="radio" size={12} /></span>
    {/if}
    {#if playable}
      <span class="play-glyph" aria-hidden="true"><Icon name="play" size={14} /></span>
    {/if}
  </span>
  <span class="text">
    <span class="title">{item.title}</span>
    {#if item.subtitle}<span class="sub muted">{item.subtitle}</span>{/if}
  </span>
{/snippet}

{#if onclick}
  <button
    type="button"
    class="media {variant} shape-{shape}"
    onclick={onclick}
    aria-label={playable ? `Play ${item.title}` : undefined}
  >{@render body()}</button>
{:else}
  <div class="media {variant} shape-{shape} inert">{@render body()}</div>
{/if}

<style>
  .media {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    min-width: 0;
    text-align: left;
    color: inherit;
    border-radius: var(--radius-card);
  }
  .tile,
  .carousel {
    padding: 10px;
    border: 1px solid var(--line);
  }
  .carousel {
    width: 168px;
    flex: 0 0 auto;
    scroll-snap-align: start;
  }
  .row {
    flex-direction: row;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
  }
  .media:not(.inert):hover {
    background: var(--surface-2);
  }
  .media.inert {
    opacity: 0.9;
  }
  .art {
    position: relative;
    isolation: isolate;
    display: block;
  }
  .row .art {
    flex: 0 0 auto;
    width: var(--thumb);
  }
  .art-clip {
    display: block;
    overflow: hidden;
    aspect-ratio: 1;
    border-radius: var(--radius-card);
    background: var(--surface-2);
  }
  .shape-artist .art-clip {
    border-radius: 50%;
  }
  .shape-playlist .art-clip {
    border-radius: var(--radius-card-sharp);
  }
  .shape-playlist .art::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    transform: translate(3px, 3px);
    border: 1px solid var(--line);
    border-radius: var(--radius-card-sharp);
    background: var(--surface-3);
  }
  .row.shape-playlist .art::after {
    transform: translate(2px, 2px);
  }
  .art-clip :global(img) {
    transition: transform var(--dur-state) var(--ease-in);
  }
  .media:not(.inert):hover .art-clip :global(img) {
    transform: scale(1.03);
  }
  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: var(--card-badge);
    height: var(--card-badge);
    border: 1px solid transparent;
    border-radius: 50%;
    color: var(--text);
    background:
      linear-gradient(var(--surface-float), var(--surface-float)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
  }
  .play-glyph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius-card);
    color: var(--text);
    background: color-mix(in srgb, var(--surface-float) 65%, transparent);
    opacity: 0;
    transition: opacity var(--dur-state) var(--ease-in);
  }
  .shape-artist .play-glyph {
    border-radius: 50%;
  }
  .shape-playlist .play-glyph {
    border-radius: var(--radius-card-sharp);
  }
  .media:hover .play-glyph,
  .media:focus-visible .play-glyph {
    opacity: 1;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
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

- [ ] **Step 3: Create `ui/src/lib/components/Carousel.svelte`**

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    controls,
    children,
  }: { title: string; controls?: Snippet; children: Snippet } = $props();

  let el = $state<HTMLElement | null>(null);
  let canLeft = $state(false);
  let canRight = $state(false);

  function sync() {
    if (!el) return;
    canLeft = el.scrollLeft > 4;
    canRight = el.scrollLeft + el.clientWidth < el.scrollWidth - 4;
  }

  $effect(() => {
    if (!el) return;
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(el);
    return () => observer.disconnect();
  });

  function step(direction: -1 | 1) {
    if (!el) return;
    const card = el.querySelector<HTMLElement>(".media");
    const amount = card ? card.offsetWidth + 16 : el.clientWidth * 0.8;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollBy({ left: direction * amount, behavior: reduced ? "auto" : "smooth" });
  }
</script>

<div class="head">
  <h3>{title}</h3>
  <div class="controls">
    <button
      type="button"
      class="control"
      onclick={() => step(-1)}
      disabled={!canLeft}
      aria-label="Scroll left"
      title="Scroll left"
    ><Icon name="chevron-left" size={16} /></button>
    <button
      type="button"
      class="control"
      onclick={() => step(1)}
      disabled={!canRight}
      aria-label="Scroll right"
      title="Scroll right"
    ><Icon name="chevron-right" size={16} /></button>
    {@render controls?.()}
  </div>
</div>
<div class="row" bind:this={el} onscroll={sync} role="region" aria-label={title}>
  {@render children()}
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .head h3 {
    flex: 1;
    min-width: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .control {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: 50%;
    color: var(--text-muted);
  }
  .control:hover:not(:disabled) {
    color: var(--text);
  }
  .control:disabled {
    opacity: 0.4;
  }
  .row {
    display: flex;
    gap: 16px;
    overflow-x: auto;
    scroll-snap-type: x proximity;
    padding: 2px 2px 8px;
    scrollbar-width: none;
  }
  .row::-webkit-scrollbar {
    display: none;
  }
</style>
```

- [ ] **Step 4: Type-check**

Run: `npm --prefix ui run check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/styles/tokens.css ui/src/lib/components/MediaCard.svelte ui/src/lib/components/Carousel.svelte
git commit -m "feat(ui): add MediaCard and carousel components"
```

---

### Task 6: Section treatments, View-all page, and wiring

**Files:**
- Rewrite: `ui/src/lib/components/PageSections.svelte`
- Create: `ui/src/views/Section.svelte`
- Modify: `ui/src/views/Home.svelte` (props + `PageSections` call only; the strip arrives in Task 7)
- Modify: `ui/src/views/Explore.svelte`
- Modify: `ui/src/views/Mixes.svelte` (props + `PageSections` call only; the grid arrives in Task 8)
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `classifySection`, `isPlayableTrack`, `playableTrackIds` (Task 3), `MediaCard`, `Carousel` (Task 5), `api.playTrackIds` and `section` route (Task 4).
- Produces: `PageSections` props `{ sections, nav, toasts, source, uncapped?, showTitle? }`; `Section.svelte` props `{ source, title, nav, toasts }`.

- [ ] **Step 1: Rewrite `ui/src/lib/components/PageSections.svelte`**

```svelte
<script lang="ts">
  import { api, type PageItem, type PageSection } from "../api";
  import { errorMessage } from "../errors";
  import { classifySection, isPlayableTrack, playableTrackIds } from "../media";
  import { routeForPageItem, type Nav } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import type { Toasts } from "../toasts.svelte";
  import Carousel from "./Carousel.svelte";
  import Icon from "./Icon.svelte";
  import MediaCard from "./MediaCard.svelte";

  let {
    sections,
    nav,
    toasts,
    source,
    uncapped = false,
    showTitle = true,
  }: {
    sections: PageSection[];
    nav: Nav;
    toasts: Toasts;
    source: "home" | "explore" | "forYou";
    uncapped?: boolean;
    showTitle?: boolean;
  } = $props();

  const TILE_CAP = 12;
  const LIST_CAP = 8;

  let busy = $state(false);

  function play(ids: number[]) {
    playSource.set(null);
    return api.playTrackIds(ids, 0);
  }

  function activate(item: PageItem): (() => void) | undefined {
    const route = routeForPageItem(item);
    if (route) return () => nav.goto(route);
    if (isPlayableTrack(item)) {
      return () => {
        void play([Number(item.id)]).catch((e) =>
          toasts.show(errorMessage(e), { severity: "error" }),
        );
      };
    }
    return undefined;
  }

  async function playAll(ids: number[]) {
    if (busy) return;
    busy = true;
    try {
      await play(ids);
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
      busy = false;
    }
  }
</script>

{#each sections as section, si (`${section.title}-${si}`)}
  {@const treatment = classifySection(section.kind, section.items)}
  {@const effective = uncapped && treatment === "carousel" ? "tiles" : treatment}
  {@const cap = effective === "list" ? LIST_CAP : TILE_CAP}
  {@const shown = uncapped ? section.items : section.items.slice(0, cap)}
  {@const ids = effective === "list" ? playableTrackIds(section.items) : []}
  {@const capped = !uncapped && section.items.length > cap}
  {#if effective === "carousel"}
    <Carousel title={section.title}>
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard {item} variant="carousel" {onclick} />
      {/each}
    </Carousel>
  {:else}
    {#if showTitle || ids.length > 0 || capped}
      <div class="head">
        {#if showTitle}<h3>{section.title}</h3>{/if}
        <div class="controls">
          {#if ids.length > 0}
            <button
              type="button"
              class="control"
              onclick={() => playAll(ids)}
              disabled={busy}
              aria-label="Play all"
              title="Play all"
            ><Icon name="play" size={16} /></button>
          {/if}
          {#if capped}
            <button
              type="button"
              class="view-all"
              onclick={() => nav.goto({ name: "section", source, title: section.title })}
            >View all</button>
          {/if}
        </div>
      </div>
    {/if}
    <div class="grid" class:list={effective === "list"}>
      {#each shown as item, i (item.id || `${item.title}-${i}`)}
        {@const onclick = activate(item)}
        <MediaCard {item} variant={effective === "list" ? "row" : "tile"} {onclick} />
      {/each}
    </div>
  {/if}
{/each}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .head h3 {
    flex: 1;
    min-width: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }
  .control {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: 50%;
    color: var(--text-muted);
  }
  .control:hover:not(:disabled) {
    color: var(--text);
  }
  .control:disabled {
    opacity: 0.4;
  }
  .view-all {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .view-all:hover {
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .grid.list {
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 8px 16px;
  }
</style>
```

- [ ] **Step 2: Create `ui/src/views/Section.svelte`**

```svelte
<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    source,
    title,
    nav,
    toasts,
  }: {
    source: "home" | "explore" | "forYou";
    title: string;
    nav: Nav;
    toasts: Toasts;
  } = $props();

  let section = $state<PageSection | null>(null);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      const sections =
        source === "home"
          ? await api.getHome()
          : source === "explore"
            ? await api.getExplore()
            : await api.getForYou();
      section = sections.find((s) => s.title === title) ?? null;
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

<h2>{title}</h2>
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if !section}
  <EmptyState message="Section no longer available." />
{:else}
  <p class="muted count">{section.items.length} items</p>
  <PageSections sections={[section]} {nav} {toasts} {source} uncapped showTitle={false} />
{/if}

<style>
  .count {
    margin-top: -8px;
  }
</style>
```

- [ ] **Step 3: Wire the three feed views**

`ui/src/views/Home.svelte` — change props and the `PageSections` call only:

```svelte
<script lang="ts">
  // add:
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();
```

and at the bottom:

```svelte
  <PageSections {sections} {nav} {toasts} source="home" />
```

`ui/src/views/Explore.svelte` — change only the `PageSections` call:

```svelte
  <PageSections {sections} {nav} {toasts} source="explore" />
```

`ui/src/views/Mixes.svelte` — add `toasts` to props and change the call:

```svelte
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();
```

```svelte
    <PageSections sections={forYou} {nav} {toasts} source="forYou" />
```

- [ ] **Step 4: Wire `App.svelte`**

Add the import next to the other views:

```ts
  import Section from "./views/Section.svelte";
```

Change the Home and Mixes dispatches:

```svelte
      {:else if nav.route.name === "home"}<Home {nav} {toasts} />
```

```svelte
      {:else if nav.route.name === "mixes"}<Mixes {nav} {toasts} />
```

Add the section dispatch after the mixes line:

```svelte
      {:else if nav.route.name === "section"}<Section source={nav.route.source} title={nav.route.title} {nav} {toasts} />
```

- [ ] **Step 5: Type-check and test**

Run: `npm --prefix ui run check`
Expected: PASS.

Run: `npm --prefix ui test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/PageSections.svelte ui/src/views/Section.svelte ui/src/views/Home.svelte ui/src/views/Explore.svelte ui/src/views/Mixes.svelte ui/src/App.svelte
git commit -m "feat(ui): render feed sections with role-based treatments"
```

---

### Task 7: Home quick-access strip

**Files:**
- Modify: `ui/src/views/Home.svelte`

**Interfaces:**
- Consumes: `shortcutEntries`, `ShortcutEntry` (Task 3), `MediaCard` (Task 5), `api.getUserPlaylists`, `api.getFavoriteMixes`.
- Produces: nothing new for later tasks.

- [ ] **Step 1: Add the strip**

Replace the script top of `ui/src/views/Home.svelte` with:

```svelte
<script lang="ts">
  import { api, type PageSection } from "../lib/api";
  import { shortcutEntries, type ShortcutEntry } from "../lib/media";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import PageSections from "../lib/components/PageSections.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let { nav, toasts }: { nav: Nav; toasts: Toasts } = $props();

  let sections = $state<PageSection[]>([]);
  let shortcuts = $state<ShortcutEntry[]>([]);
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

  async function loadShortcuts() {
    const [playlists, mixes] = await Promise.allSettled([
      api.getUserPlaylists(),
      api.getFavoriteMixes(),
    ]);
    shortcuts = shortcutEntries(
      playlists.status === "fulfilled" ? playlists.value : [],
      mixes.status === "fulfilled" ? mixes.value : [],
    );
  }

  $effect(() => {
    void load();
    void loadShortcuts();
  });
</script>
```

Insert the strip between `<h2>Home</h2>` and the feed states:

```svelte
{#if shortcuts.length > 0}
  <section class="shortcuts" aria-label="Quick access">
    {#each shortcuts as entry (entry.key)}
      <MediaCard
        item={entry.item}
        variant="row"
        thumb={56}
        collage={entry.playlist}
        onclick={() => nav.goto(entry.route)}
      />
    {/each}
  </section>
{/if}
```

Append the style block:

```svelte
<style>
  .shortcuts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 8px 16px;
    margin-bottom: 20px;
  }
</style>
```

- [ ] **Step 2: Type-check**

Run: `npm --prefix ui run check`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add ui/src/views/Home.svelte
git commit -m "feat(ui): add Home quick-access strip"
```

---

### Task 8: Reuse `MediaCard` in collection grids

**Files:**
- Modify: `ui/src/views/CollectionAlbums.svelte`
- Modify: `ui/src/views/Mixes.svelte`

**Interfaces:**
- Consumes: `MediaCard` (Task 5), `PageItem`.
- Produces: nothing new.

- [ ] **Step 1: Rewrite `CollectionAlbums.svelte`'s grid**

Replace the imports (drop `Cover`, add `MediaCard` and `PageItem`):

```svelte
  import { api, type Album, type PageItem } from "../lib/api";
  import MediaCard from "../lib/components/MediaCard.svelte";
```

Replace the grid body:

```svelte
  <div class="grid">
    {#each albums as album (album.id)}
      {@const item: PageItem = {
        id: String(album.id),
        title: album.title,
        subtitle: album.artists.map((a) => a.name).join(", "),
        item_type: "ALBUM",
        artwork: album.cover ?? null,
      }}
      <MediaCard {item} variant="tile" onclick={() => nav.goto({ name: "album", id: album.id })} />
    {/each}
  </div>
```

Keep only the grid rule in the style block:

```svelte
<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
</style>
```

- [ ] **Step 2: Rewrite `Mixes.svelte`'s favorites grid**

Change the imports:

```svelte
  import { api, type Mix, type PageItem, type PageSection } from "../lib/api";
  import MediaCard from "../lib/components/MediaCard.svelte";
```

(Remove the now-unused `Icon` import and its placeholder markup.)

Replace the favorites grid:

```svelte
    <div class="grid">
      {#each mixes as mix (mix.id)}
        {@const item: PageItem = {
          id: mix.id,
          title: mix.title,
          subtitle: mix.subTitle ?? null,
          item_type: "MIX_LIST",
          artwork: mix.image ?? null,
        }}
        <MediaCard {item} variant="tile" onclick={() => nav.goto({ name: "mix", id: mix.id })} />
      {/each}
    </div>
```

Keep only the grid rule in the style block (remove `.card`, `.art`, `.title`, `.sub`, `.placeholder` rules):

```svelte
<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }
</style>
```

- [ ] **Step 3: Type-check and test**

Run: `npm --prefix ui run check`
Expected: PASS (no unused-import errors).

Run: `npm --prefix ui test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/CollectionAlbums.svelte ui/src/views/Mixes.svelte
git commit -m "refactor(ui): reuse MediaCard for album and mix grids"
```

---

### Task 9: Verification, live tuning, and smoke record

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-home-rhythm-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: recorded gate results, tuned classifier (if the live feed disagrees), smoke/perf notes.

- [ ] **Step 1: Run the full gates**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

Expected: all PASS. Record exact outcomes in the notes file.

- [ ] **Step 2: Capture the real feed's section kinds (human-assisted)**

Temporarily add to `PageSections.svelte`'s `{#each}` body (top of the block):

```svelte
  {@const _debug = console.debug("section", section.kind, section.items.map((i) => i.item_type))}
```

Start the app with a warm Vite: `npm --prefix ui run app:dev`. The human opens Home and reads the WebView2 console (right-click → Inspect) or the Vite terminal. Record every `section <kind> [...]` line.

- [ ] **Step 3: Tune `classifySection` if needed**

For each recorded kind, confirm the treatment matches the spec table (§5.2). If a kind needs a different treatment, change only the predicate constants in `ui/src/lib/media.ts` and add a `classifySection` test for that kind in `media.test.ts`. Remove the debug line.

Run: `npm --prefix ui test -- src/lib/media.test.ts`
Expected: PASS.

Commit (only if changed):

```bash
git add ui/src/lib/media.ts ui/src/lib/media.test.ts ui/src/lib/components/PageSections.svelte
git commit -m "fix(ui): tune section classification to the live feed"
```

- [ ] **Step 4: Human GUI smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-03-home-rhythm-smoke.md` with this checklist, then have the human run `npm --prefix ui run app:dev` and tick each line; record results and any deviations:

```markdown
# Home rhythm & card language — smoke checklist (2026-10-03)

- [ ] Quick-access strip shows playlists first, then saved mixes, max 6; hidden when both sources are empty.
- [ ] With the network offline (or TIDAL unreachable), the strip hides and Home still renders its feed error state.
- [ ] Playlist sections render as two-row tile grids with View all when > 12 items.
- [ ] Album/mix/radio sections render as carousels; arrows disable at both ends and on short rows; shift+wheel scrolls.
- [ ] Track-heavy sections render as two-column rows with Play all.
- [ ] Album = rounded square; playlist = sharp square with deck edge; mix = radio badge; artist = circle.
- [ ] Mix cards still open the mix page (no autoplay); artist cards open the artist page.
- [ ] View all opens the section page; back and forward return through each section visited.
- [ ] Track row click plays the track; Play all plays the section from the top; queue metadata (title/artist/album/cover) is correct in the queue rail.
- [ ] Player error toast appears if a track resolve fails (e.g. disable network before clicking).
- [ ] Explore and Mixes render the same treatments and shapes.
- [ ] Reduced motion: carousel arrow clicks jump without smooth scrolling.
```

- [ ] **Step 5: Performance re-check**

Follow `AGENTS.md` §Performance harness (single app, foreground, Vite warm):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 3
node scripts/perf/check.mjs
```

Expected: `check.mjs` exits 0. Record the numbers in the notes file; if the strip pushes S1/S2 over budget, fall back to plain `Cover` thumbs in the strip (drop `collage`) and re-run.

- [ ] **Step 6: Commit the notes**

```bash
git add docs/superpowers/plans/notes/2026-10-03-home-rhythm-smoke.md
git commit -m "docs: record home rhythm smoke checklist and perf results"
```
