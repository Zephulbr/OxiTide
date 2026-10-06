# Track 9.2 — Credits and Album Info Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship track credits in a Now Playing "Info" tab and a "Track info" context-menu modal, plus album release facts and disc-scoped multi-volume grouping on the album page.

**Architecture:** Public-OpenAPI reads go through the existing `TidalClient::get_openapi` seam into new `AlbumInfo` / `TrackCredits` DTOs and two Tauri commands. The UI gets a lazy per-session `info` store (mirroring `lyrics`), an `InfoPanel.svelte` used by both the spine tab and a modal, and `TrackTable` gains an optional `sections` mode for multi-volume albums. Credits are gated on a live spike (Task 0); if the API hides them, the Info tab ships metadata-only and credits are deferred as a documented follow-up.

**Tech Stack:** Rust (serde, Tauri commands, wiremock), Svelte 5 runes, TypeScript, Vitest, JSON:API via `openapi.tidal.com/v2`.

**Spec:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` (§5.3, §6.2, §7, §8, §9)

## Global Constraints

- **Task 0 gates credits.** If the spike shows credits are unavailable (INTERNAL gate, region, or empty), Tasks 3's credits half and Task 4's credits fetch are cancelled: the Info tab ships metadata-only with the "No credits available" empty state and the deferral is recorded in the research note and the smoke checklist. Task 3's `get_track` and Task 4's metadata work still ship.
- **`label` is not in the public album payload** (confirmed against the published OpenAPI: `Albums_Attributes` has `barcodeId`, `copyright.text`, `releaseDate`, `numberOfVolumes`, `numberOfItems` — no label field). The facts row omits label; the spike note records this.
- Per-session in-memory caches only (UI `Map`s); no disk cache, no new remote origins.
- Lazy only: no fetch on browse; album info rides the existing album-open `Promise.all`; credits/`get_track` load on first Info open per track.
- PM-D1: no Track 3 scenario re-run is required (album open adds one request inside the existing wave; `perfHooks.measureAlbumOpen` is synthetic and untouched). The smoke note records this impact statement.
- Rust DTOs are the source of truth; `ui/src/lib/api.ts` is hand-synced. `invoke` args camelCase, Rust params snake_case.
- New commands must be registered in `crates/oxitide-app/src/main.rs` `generate_handler![...]`.
- ARIA: the Now Playing spine ships the full tab pattern from birth (roles, `id` / `aria-selected` / `aria-controls` / `tabpanel`, roving tabindex, Up/Down/Home/End) per spec §6.2; 9.4 verifies it with component tests instead of retrofitting (spec §6.4).
- New strings stay component-local (i18n is a later track).
- Scripted gates after every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.

## Review Focus

1. **Credits unavailable (INTERNAL/region/empty):** the Info tab still shows track metadata and the album link, with an explicit "No credits available" state — never an error loop or a stuck spinner.
2. **Track without an album, or credits without artist ids:** metadata renders without broken links; contributors without ids render as plain text.
3. **Multi-volume album with absent/partial/zero `volumeNumber`s:** grouping never hides or reorders tracks; disc-scoped numbering appears only when more than one volume exists, and sort/filter falls back to the flat table.
4. **`get_album_info` failing or 404 while album+tracks load:** the facts row is simply absent; the album page is unaffected (non-blocking wave).
5. **Fast track/tab switches:** a superseded fetch must never paint the previous track's data; reopening retries failed/missing entries.

---

### Task 0: Live API spike and decision note

**Files:**
- Modify (temporary, reverted in this task): `crates/oxitide-app/src/commands.rs`, `crates/oxitide-app/src/main.rs`
- Create: `docs/superpowers/plans/notes/2026-10-06-track9-2-api-research.md`

**Interfaces:**
- Produces: the spike note (raw payloads + decisions) that Tasks 3/4 follow.
- Consumes: `TidalClient::get_openapi` (`crates/oxitide-tidal/src/client.rs:334`), `require_client` (`commands.rs:327`), `state.fail_tidal` (`state.rs:249`).

- [ ] **Step 1: Add the temporary probe command**

In `commands.rs`, next to `get_artist_profile` (~line 811):

```rust
/// TEMPORARY (Track 9.2 spike): raw public-OpenAPI probe, removed after the
/// research note lands. Never registered outside the spike branch.
#[tauri::command]
pub async fn debug_get_openapi(
    state: State<'_, AppState>,
    path: String,
    include: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = require_client(&state)?;
    let mut params: Vec<(String, String)> = vec![(
        "countryCode".to_string(),
        client.session().country_code.clone(),
    )];
    if let Some(include) = include {
        params.push(("include".to_string(), include));
    }
    let params: Vec<(&str, String)> = params
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    client
        .get_openapi(&path, &params)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

Register `commands::debug_get_openapi,` in `main.rs` `generate_handler![...]` (after `commands::get_artist_profile,` at line 337).

- [ ] **Step 2: Probe with the dev app**

Run: `npm --prefix ui run app:dev` (log in with the real account). In the WebView2 devtools console (`Ctrl+Shift+I`), for one mainstream album and one of its tracks (grab ids from a Now Playing/Album URL):

```js
await window.__TAURI_INTERNALS__.invoke("debug_get_openapi", { path: "tracks/<TRACK_ID>", include: "credits.artist" })
await window.__TAURI_INTERNALS__.invoke("debug_get_openapi", { path: "tracks/<TRACK_ID>", include: null })
await window.__TAURI_INTERNALS__.invoke("debug_get_openapi", { path: "albums/<ALBUM_ID>", include: null })
await window.__TAURI_INTERNALS__.invoke("debug_get_openapi", { path: "tracks/<TRACK_ID>/relationships/credits", include: null })
```

Expected: track + album documents return 200 JSON; the credits include either carries `relationships.credits.data` plus `included` `credits` resources (`attributes.name`/`attributes.role`, optional `relationships.artist.data.id`) or comes back without them. The dedicated relationship endpoint may return an entitlement error — record it verbatim.

- [ ] **Step 3: Write the research note**

Create `docs/superpowers/plans/notes/2026-10-06-track9-2-api-research.md` (adjust the date to the run day) with:

- Status/date/authority lines (spec §5.3).
- The captured JSON for each probe (trimmed to the relevant nodes).
- **Decision 1 — credits:** accessible → Tasks 3/4 run as written; unavailable/empty → credits half cancelled, Info tab ships metadata-only, follow-up recorded here.
- **Decision 2 — album facts:** which of `releaseDate` / `copyright.text` / `barcodeId` / `numberOfVolumes` / `numberOfItems` were present; confirm `label` is absent.
- **Decision 3 — credit artist ids:** whether `credits.artist` returned ids (links ship) or not (names render as text).

- [ ] **Step 4: Remove the temporary probe and commit the note**

Delete `debug_get_openapi` from `commands.rs` and its `generate_handler!` line from `main.rs`.

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: green; `git diff` shows no probe left.

```bash
git add docs/superpowers/plans/notes/2026-10-06-track9-2-api-research.md
git commit -m "docs: spike the credits and album-info payloads"
```

---

### Task 1: `AlbumInfo` backend, command, and IPC type

**Files:**
- Modify: `crates/oxitide-tidal/src/models.rs` (add DTO after `Album`, ~line 70; tests in existing `mod tests`)
- Modify: `crates/oxitide-tidal/src/catalog.rs` (method after `album_tracks_all`, ~line 33; raw structs near `OpenApiArtistProfile`, ~line 250; tests)
- Modify: `crates/oxitide-app/src/commands.rs` (after `get_album_tracks`, ~line 764)
- Modify: `crates/oxitide-app/src/main.rs` (register after `commands::get_album_tracks,` line 324)
- Modify: `ui/src/lib/api.ts` (interface after `Album` line 22; wrapper after `getAlbumTracks` line 318)
- Test: `crates/oxitide-tidal/src/models.rs`, `crates/oxitide-tidal/src/catalog.rs`

**Interfaces:**
- Produces: `AlbumInfo { release_date, copyright, barcode_id, number_of_volumes, number_of_items }` (serde camelCase, all optional), `TidalClient::album_info(id) -> Result<AlbumInfo>`, command `get_album_info(id)`, TS `AlbumInfo`, `api.getAlbumInfo(id)`.
- Consumes: `get_openapi` (`client.rs:334`), `require_client`, `state.fail_tidal`.

- [ ] **Step 1: Write the failing DTO serialization test**

In `models.rs` `mod tests` (next to `full_track_json_parses`):

```rust
#[test]
fn album_info_serializes_camel_case_and_omits_nothing() {
    let info = AlbumInfo {
        release_date: Some("2017-06-30".into()),
        copyright: Some("(C) 2017 S. Carter Enterprises".into()),
        barcode_id: Some("00881377068540".into()),
        number_of_volumes: Some(2),
        number_of_items: Some(24),
    };
    let value = serde_json::to_value(&info).unwrap();
    assert_eq!(value["releaseDate"], "2017-06-30");
    assert_eq!(value["barcodeId"], "00881377068540");
    assert_eq!(value["numberOfVolumes"], 2);
    assert_eq!(value["numberOfItems"], 24);
    assert_eq!(AlbumInfo::default().number_of_volumes, None);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-tidal album_info_serializes_camel_case_and_omits_nothing`
Expected: FAIL — `AlbumInfo` not found.

- [ ] **Step 3: Implement the DTO and method**

In `models.rs` after `Album` (~line 70):

```rust
/// Public v2 album facts (spec §5.3). Absent attributes stay `None` and the
/// UI omits them; the public API exposes no label field (only `copyright`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub barcode_id: Option<String>,
    #[serde(default)]
    pub number_of_volumes: Option<u32>,
    #[serde(default)]
    pub number_of_items: Option<u32>,
}
```

In `catalog.rs`, add `AlbumInfo` to the `use crate::models::{...}` list (line 5) and the method after `album_tracks_all` (~line 33):

```rust
    /// Album facts from the public OpenAPI; every field is optional and a
    /// missing document body yields `AlbumInfo::default()`.
    pub async fn album_info(&self, id: u64) -> Result<AlbumInfo> {
        let params = [("countryCode", self.session().country_code.clone())];
        let raw: OpenApiAlbumInfo = self.get_openapi(&format!("albums/{id}"), &params).await?;
        Ok(raw
            .data
            .and_then(|data| data.attributes)
            .map(|attributes| AlbumInfo {
                release_date: attributes.release_date,
                copyright: attributes.copyright.and_then(|copyright| copyright.text),
                barcode_id: attributes.barcode_id,
                number_of_volumes: attributes.number_of_volumes,
                number_of_items: attributes.number_of_items,
            })
            .unwrap_or_default())
    }
```

Raw structs at the bottom of `catalog.rs` next to `OpenApiArtistProfile` (~line 250):

```rust
#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfo {
    #[serde(default)]
    data: Option<OpenApiAlbumInfoData>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfoData {
    #[serde(default)]
    attributes: Option<OpenApiAlbumInfoAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiAlbumInfoAttributes {
    #[serde(default, rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(default)]
    copyright: Option<OpenApiCopyright>,
    #[serde(default, rename = "barcodeId")]
    barcode_id: Option<String>,
    #[serde(default, rename = "numberOfVolumes")]
    number_of_volumes: Option<u32>,
    #[serde(default, rename = "numberOfItems")]
    number_of_items: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCopyright {
    #[serde(default)]
    text: Option<String>,
}
```

- [ ] **Step 4: Write the failing wiremock mapping tests**

In `catalog.rs` `mod tests` (copy the `client(&server)` helper at lines 300–328):

```rust
    #[tokio::test]
    async fn album_info_reads_facts_and_sends_country_code() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/9"))
            .and(query_param("countryCode", "GR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "9",
                    "type": "albums",
                    "attributes": {
                        "releaseDate": "2017-06-30",
                        "copyright": { "text": "(C) 2017 S. Carter Enterprises" },
                        "barcodeId": "00881377068540",
                        "numberOfVolumes": 2,
                        "numberOfItems": 24,
                        "title": "4:44"
                    }
                }
            })))
            .mount(&server)
            .await;
        let info = client(&server).album_info(9).await.unwrap();
        assert_eq!(info.release_date.as_deref(), Some("2017-06-30"));
        assert_eq!(info.copyright.as_deref(), Some("(C) 2017 S. Carter Enterprises"));
        assert_eq!(info.barcode_id.as_deref(), Some("00881377068540"));
        assert_eq!(info.number_of_volumes, Some(2));
        assert_eq!(info.number_of_items, Some(24));
    }

    #[tokio::test]
    async fn album_info_missing_attributes_default_to_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": "9", "type": "albums", "attributes": { "title": "X" } }
            })))
            .mount(&server)
            .await;
        let info = client(&server).album_info(9).await.unwrap();
        assert_eq!(info, AlbumInfo::default());
    }

    #[tokio::test]
    async fn album_info_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/albums/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).album_info(999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal album_info`, then `cargo test -p oxitide-tidal`.
Expected: PASS.

- [ ] **Step 6: Add the command and IPC type**

In `commands.rs` after `get_album_tracks` (~line 764); add `AlbumInfo` to the existing models import list at the top of the file:

```rust
#[tauri::command]
pub async fn get_album_info(state: State<'_, AppState>, id: u64) -> Result<AlbumInfo, String> {
    require_client(&state)?
        .album_info(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

In `main.rs`, add `commands::get_album_info,` directly after `commands::get_album_tracks,` (line 324).

In `api.ts`, after the `Album` interface (line 22):

```ts
export interface AlbumInfo {
  releaseDate?: string | null;
  copyright?: string | null;
  barcodeId?: string | null;
  numberOfVolumes?: number | null;
  numberOfItems?: number | null;
}
```

and after `getAlbumTracks` (line 318):

```ts
  getAlbumInfo: (id: number) => invoke<AlbumInfo>("get_album_info", { id }),
```

- [ ] **Step 7: Run the gates and commit**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`.
Expected: all green.

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/catalog.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts
git commit -m "feat(tidal): add album info from the public v2 API"
```

---

### Task 2: Album facts row and multi-volume grouping

**Files:**
- Create: `ui/src/lib/albumVolumes.ts`
- Test: `ui/src/lib/albumVolumes.test.ts`
- Modify: `ui/src/views/Album.svelte` (load wave lines 46–56, markup after `</DetailHeader>` line 156, TrackTable props line 157)
- Modify: `ui/src/lib/components/TrackTable.svelte` (props lines 21–65, derived rows lines 78–81, table body lines 214–306, styles at end)

**Interfaces:**
- Produces: `VolumeSection { label: string; tracks: Track[] }`, `volumeSections(tracks) -> VolumeSection[] | null`; `TrackTable` prop `sections?: VolumeSection[]`.
- Consumes: `api.getAlbumInfo` (Task 1), `Track.volumeNumber` (already parsed, `models.rs:37`).

- [ ] **Step 1: Write the failing grouping tests**

Create `ui/src/lib/albumVolumes.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import { volumeSections } from "./albumVolumes";

const track = (id: number, volumeNumber: number, title = `T${id}`): Track => ({
  id, title, duration: 200, artists: [{ id: 1, name: "A" }], album: null,
  audioQuality: null, trackNumber: id, volumeNumber, explicit: false, mediaMetadata: null,
});

describe("volumeSections", () => {
  it("returns null for an empty or single-volume album", () => {
    expect(volumeSections([])).toBeNull();
    expect(volumeSections([track(1, 1), track(2, 1)])).toBeNull();
    expect(volumeSections([track(1, 0), track(2, 0)])).toBeNull();
  });

  it("groups by volume and preserves source order", () => {
    const sections = volumeSections([track(1, 1), track(2, 2), track(3, 1), track(4, 2)]);
    expect(sections?.map((section) => section.label)).toEqual(["Volume 1", "Volume 2"]);
    expect(sections?.[0].tracks.map((t) => t.id)).toEqual([1, 3]);
    expect(sections?.[1].tracks.map((t) => t.id)).toEqual([2, 4]);
  });

  it("treats zero numbering as volume one", () => {
    const sections = volumeSections([track(1, 0), track(2, 2)]);
    expect(sections?.[0].label).toBe("Volume 1");
    expect(sections?.[0].tracks.map((t) => t.id)).toEqual([1]);
    expect(sections?.[1].label).toBe("Volume 2");
  });

  it("keeps sparse volume numbers in first-seen order", () => {
    const sections = volumeSections([track(1, 3), track(2, 1)]);
    expect(sections?.map((section) => section.label)).toEqual(["Volume 3", "Volume 1"]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/albumVolumes.test.ts`
Expected: FAIL — cannot resolve `./albumVolumes`.

- [ ] **Step 3: Implement the helper**

Create `ui/src/lib/albumVolumes.ts`:

```ts
import type { Track } from "./api";

export interface VolumeSection {
  label: string;
  tracks: Track[];
}

/// Multi-volume albums (spec §6.2): group source-order tracks by
/// `volumeNumber`. Returns null for single-volume albums (or all-zero
/// numbering) so callers keep the flat virtualized table. Volumes appear in
/// first-seen order; zero numbering reads as volume one.
export function volumeSections(tracks: readonly Track[]): VolumeSection[] | null {
  const volumes = new Map<number, Track[]>();
  for (const track of tracks) {
    const volume = track.volumeNumber > 0 ? track.volumeNumber : 1;
    const list = volumes.get(volume);
    if (list) list.push(track);
    else volumes.set(volume, [track]);
  }
  if (![...volumes.keys()].some((volume) => volume > 1)) return null;
  return [...volumes.entries()].map(([volume, list]) => ({
    label: `Volume ${volume}`,
    tracks: list,
  }));
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/albumVolumes.test.ts`
Expected: PASS.

- [ ] **Step 5: Teach `TrackTable` grouped rendering**

In `TrackTable.svelte`, extend the prop type/destructure (line 21–65):

```ts
    /// Multi-volume album groups (album page); ignored while a sort/filter is
    /// engaged. Grouped lists render whole (no virtualization) and section
    /// headings reset the row number per volume.
    sections,
```
with `sections?: VolumeSection[];` in the type block, and add the import:

```ts
  import type { VolumeSection } from "../albumVolumes";
```

Replace the derived block at lines 78–81:

```ts
  const rows = $derived(view.items);
  const grouping = $derived(sections !== undefined && !view.engaged ? sections : null);
  const range = $derived(
    grouping
      ? { first: 0, last: rows.length, padTop: 0, padBottom: 0 }
      : windowRange(rows.length, scrollTop, viewport, rowHeight, OVERSCAN),
  );
  const slice = $derived(grouping ? rows : rows.slice(range.first, range.last));

  interface DisplayRow {
    key: string;
    /// Non-null marks a volume heading row.
    label: string | null;
    track?: Track;
    index?: number;
    number?: number;
  }

  /// Flat rows when ungrouped; heading rows interleaved with disc-scoped
  /// numbers when grouped.
  const displayRows = $derived.by<DisplayRow[]>(() => {
    if (!grouping) {
      return rows.slice(range.first, range.last).map((track, i) => ({
        key: `${track.id}-${range.first + i}`,
        label: null,
        track,
        index: range.first + i,
        number: range.first + i + 1,
      }));
    }
    const out: DisplayRow[] = [];
    let index = 0;
    for (const section of grouping) {
      out.push({ key: `section-${section.label}`, label: section.label });
      section.tracks.forEach((track, i) => {
        out.push({ key: `${track.id}-${index}`, label: null, track, index, number: i + 1 });
        index += 1;
      });
    }
    return out;
  });
```

Replace the `{#each slice as t, i (`${t.id}-${range.first + i}`)}` block at lines 218–299 with the row-generic loop (keep the existing cell markup, substituting `t`, `index`, and the number cell):

```svelte
      {#each displayRows as row (row.key)}
        {#if row.label !== null}
          <tr class="section-row">
            <th colspan={columns}>{row.label}</th>
          </tr>
        {:else if row.track !== undefined}
          {@const t = row.track}
          {@const index = row.index ?? 0}
          {@const quality = trackQualityLabel(t.audioQuality)}
          {@const reasons = trackFlags.blockReasons(t)}
          {@const tags = trackFlags.showLabels
            ? [...new Set([...reasons, ...trackFlags.labels(t)])]
            : reasons}
          <tr
            class="row"
            class:blocked={reasons.length > 0}
            class:dragging={dragFrom === index}
            class:playing={playingTrackId !== null && t.id === playingTrackId}
            draggable={canDrag}
            data-drag-index={canDrag ? index : undefined}
            ondblclick={(event) => onRowDoubleClick(event, index)}
            use:menuFor={() =>
              trackMenuItems({
                track: t,
                index,
                trackCount: rows.length,
                favorites,
                blocked: reasons.length > 0,
                play: () => play(index),
                startRadio: () => void playTrackRadio(t.id, toasts),
                addToQueue: () => queueOne(t),
                addToPlaylist: () => onAddToPlaylist?.(t),
                copyTitle: () => void copyText(t.title),
                playlist: playlistActions && { ...playlistActions, canMove: !view.engaged },
              })}
          >
            <td class="muted tnum num-cell">{row.number ?? index + 1}</td>
            <td class="thumb-cell">
              <button class="thumb" onclick={() => play(index)} disabled={loadingAll} aria-label="Play" title="Play">
                <Cover uuid={t.album?.cover} size={80} radius={0} alt="" />
                <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
              </button>
            </td>
            <td class="title-cell"><div class="title-text">
              {t.title}
              {#each tags as tag (tag)}
                <span
                  class="badge"
                  class:block-badge={reasons.includes(tag)}
                  class:tag-badge={!reasons.includes(tag)}
                  title={reasons.includes(tag)
                    ? (tag === "explicit" ? "Hidden: explicit" : "Hidden: AI-generated")
                    : (tag === "explicit" ? "Explicit" : "AI-generated")}
                >{tag === "explicit" ? "E" : "AI"}</span>
              {/each}
            </div></td>
            <td class="text-cell muted"><div class="cell-text">{t.artists.map((a) => a.name).join(", ")}</div></td>
            {#if showAlbum}
              <td class="text-cell muted"><div class="cell-text">{t.album?.title ?? ""}</div></td>
            {/if}
            <td class="muted tnum nowrap-cell time-cell">{formatDuration(t.duration)}</td>
            <td class="nowrap-cell">
              {#if quality}<span class="badge" class:max={quality === "MAX"}>{quality}</span>{/if}
            </td>
            <td class="actions-cell">
              <button
                class="reveal"
                onclick={() => queueOne(t)}
                aria-label="Add to queue"
                title="Add to queue"
              ><Icon name="plus" size={14} /></button>
              {#if onAddToPlaylist}
                <button
                  class="reveal"
                  onclick={() => onAddToPlaylist(t)}
                  aria-label="Add to playlist"
                  title="Add to playlist"
                ><Icon name="list-plus" size={14} /></button>
              {/if}
              <button
                class:on={favorites.hasTrack(t.id)}
                onclick={() => favorites.toggleTrack(t.id)}
                aria-label={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
                title={favorites.hasTrack(t.id) ? "Unfavorite" : "Favorite"}
              ><Icon name="heart" size={14} /></button>
            </td>
          </tr>
        {/if}
      {/each}
```

Add the section-row style at the end of the `<style>` block:

```css
  .section-row th {
    text-align: left;
    padding: 16px 12px 6px 0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
```

- [ ] **Step 6: Album page: facts row and grouped table**

In `Album.svelte`, extend the imports (line 2) with `type AlbumInfo`, add `import { volumeSections } from "../lib/albumVolumes";`, add state `let info = $state<AlbumInfo | null>(null);` (line 33), and replace `load()` (lines 46–56):

```ts
  async function load() {
    loading = true;
    error = "";
    try {
      const [loadedAlbum, loadedTracks, loadedInfo] = await Promise.all([
        api.getAlbum(id),
        api.getAlbumTracks(id),
        api.getAlbumInfo(id).catch(() => null),
      ]);
      album = loadedAlbum;
      tracks = loadedTracks;
      info = loadedInfo;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
```

Add derived facts next to `stats` (line 44):

```ts
  // Release facts from the public v2 album; the public payload has no label,
  // so the row shows what exists and omits the rest (spec §6.2).
  const facts = $derived.by(() => {
    const rows: { label: string; value: string }[] = [];
    if (info?.releaseDate) rows.push({ label: "Released", value: info.releaseDate });
    if (info?.copyright) rows.push({ label: "Copyright", value: info.copyright });
    if (info?.barcodeId) rows.push({ label: "UPC", value: info.barcodeId });
    if (info && info.numberOfVolumes && info.numberOfVolumes > 1) {
      rows.push({ label: "Volumes", value: String(info.numberOfVolumes) });
    }
    return rows;
  });
  const volumes = $derived(volumeSections(tracks));
```

After `</DetailHeader>` (line 156), render the facts row:

```svelte
    {#if facts.length > 0}
      <dl class="facts">
        {#each facts as fact (fact.label)}
          <div>
            <dt>{fact.label}</dt>
            <dd>{fact.value}</dd>
          </div>
        {/each}
      </dl>
    {/if}
```

Pass the sections to `TrackTable` (line 157 block):

```svelte
    <TrackTable
      {view}
      {favorites}
      {toasts}
      fill
      showAlbum={false}
      sections={volumes ?? undefined}
      onPlay={playFrom}
      playingTrackId={player.state.current?.track_id ?? null}
      onAddToPlaylist={(t) => playlistActions.openPicker([t])}
    />
```

and the styles:

```css
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 28px;
    margin: -24px 0 24px;
    padding: 0;
  }
  .facts dt {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .facts dd {
    margin: 2px 0 0;
    font-size: 13.5px;
    overflow-wrap: anywhere;
  }
```

- [ ] **Step 7: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `cargo test --workspace`.
Expected: all green; `svelte-check` accepts the `row.track !== undefined` narrowing.

```bash
git add ui/src/lib/albumVolumes.ts ui/src/lib/albumVolumes.test.ts ui/src/views/Album.svelte ui/src/lib/components/TrackTable.svelte
git commit -m "feat(ui): show album facts and group multi-volume tracks"
```

---

### Task 3: Track detail and credits backend (credits gated on Task 0)

**Files:**
- Modify: `crates/oxitide-tidal/src/models.rs` (after `AlbumInfo`; tests in existing `mod tests`)
- Modify: `crates/oxitide-tidal/src/catalog.rs` (method + pure parser + raw structs; tests)
- Modify: `crates/oxitide-app/src/commands.rs` (after `get_album_info`)
- Modify: `crates/oxitide-app/src/main.rs` (register both commands)
- Modify: `ui/src/lib/api.ts` (interfaces after `AlbumInfo`; wrappers after `getAlbumInfo`)

**Interfaces:**
- Produces: `TrackCredits { roles }`, `CreditRole { role, contributors }`, `CreditContributor { name, artist_id }`; `credits_from_document`; `TidalClient::track_credits(id)`; commands `get_track(id)` and `get_track_credits(id)`; TS types and `api.getTrack` / `api.getTrackCredits`.
- Consumes: Task 0 decision, `TidalClient::track` (`catalog.rs:14`, already implemented), `get_openapi`.

- [ ] **Step 1: Check the Task 0 decision**

Open the research note. If credits are unavailable, execute only the `get_track` parts of this task (Steps 3–4 without credits), skip Step 2 and the credits half of Steps 3–4, and note the skipped steps in the note's decision. Otherwise run every step.

- [ ] **Step 2: Write the failing credits parser tests**

In `catalog.rs` `mod tests`, add the pure-parser tests (no HTTP):

```rust
    #[test]
    fn credits_group_by_role_in_first_seen_order() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": {
                "relationships": { "credits": { "data": [
                    { "id": "c1", "type": "credits" },
                    { "id": "c2", "type": "credits" },
                    { "id": "c3", "type": "credits" }
                ] } }
            },
            "included": [
                { "id": "c1", "type": "credits",
                  "attributes": { "name": "John Lennon", "role": "Vocals" },
                  "relationships": { "artist": { "data": { "id": "13", "type": "artists" } } } },
                { "id": "c2", "type": "credits",
                  "attributes": { "name": "Paul McCartney", "role": "Vocals" } },
                { "id": "c3", "type": "credits",
                  "attributes": { "name": "George Martin", "role": "Producer" } }
            ]
        }))
        .unwrap();
        let credits = credits_from_document(raw);
        assert_eq!(credits.roles.len(), 2);
        assert_eq!(credits.roles[0].role, "Vocals");
        assert_eq!(credits.roles[0].contributors.len(), 2);
        assert_eq!(credits.roles[0].contributors[0].artist_id, Some(13));
        assert_eq!(credits.roles[0].contributors[1].artist_id, None);
        assert_eq!(credits.roles[1].role, "Producer");
    }

    #[test]
    fn credits_ignore_unreferenced_or_non_credit_included() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": { "relationships": { "credits": { "data": [{ "id": "c1", "type": "credits" }] } } },
            "included": [
                { "id": "c9", "type": "credits", "attributes": { "name": "Ghost", "role": "Vocals" } },
                { "id": "a1", "type": "artists", "attributes": { "name": "Artist" } }
            ]
        }))
        .unwrap();
        assert!(credits_from_document(raw).roles.is_empty());
    }

    #[test]
    fn credits_drop_blank_and_duplicate_rows() {
        let raw: OpenApiTrackCredits = serde_json::from_value(serde_json::json!({
            "data": { "relationships": { "credits": { "data": [
                { "id": "c1" }, { "id": "c2" }, { "id": "c3" }, { "id": "c4" }
            ] } } },
            "included": [
                { "id": "c1", "type": "credits", "attributes": { "name": "A", "role": "Vocals" } },
                { "id": "c2", "type": "credits", "attributes": { "name": "A", "role": "Vocals" } },
                { "id": "c3", "type": "credits", "attributes": { "name": "", "role": "Vocals" } },
                { "id": "c4", "type": "credits", "attributes": { "name": "B" } }
            ]
        }))
        .unwrap();
        let credits = credits_from_document(raw);
        assert_eq!(credits.roles.len(), 1);
        assert_eq!(credits.roles[0].contributors.len(), 1);
    }

    #[test]
    fn credits_without_a_relationship_are_empty() {
        let raw: OpenApiTrackCredits =
            serde_json::from_value(serde_json::json!({ "data": { "id": "1" } })).unwrap();
        assert!(credits_from_document(raw).roles.is_empty());
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal credits_`
Expected: FAIL — `OpenApiTrackCredits` / `credits_from_document` not found.

- [ ] **Step 4: Implement the DTOs, parser, method, and commands**

In `models.rs` after `AlbumInfo`:

```rust
/// Track credits from the public v2 `credits` relationship (spec §5.3).
/// An empty `roles` list is a settled "no credits exposed" result.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCredits {
    #[serde(default)]
    pub roles: Vec<CreditRole>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditRole {
    pub role: String,
    #[serde(default)]
    pub contributors: Vec<CreditContributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditContributor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<u64>,
}
```

In `catalog.rs`, add `CreditContributor, CreditRole, TrackCredits` to the models import and the method + parser + raw structs:

```rust
    /// Role-grouped credits from the public v2 track document. An empty
    /// `roles` list means the API exposed no credits for this track.
    pub async fn track_credits(&self, id: u64) -> Result<TrackCredits> {
        let params = [
            ("countryCode", self.session().country_code.clone()),
            ("include", "credits.artist".to_string()),
        ];
        let raw: OpenApiTrackCredits = self.get_openapi(&format!("tracks/{id}"), &params).await?;
        Ok(credits_from_document(raw))
    }
```

```rust
/// Group included `credits` resources by role, preserving first-seen role and
/// contributor order; the credit's `artist` relationship id becomes
/// `artist_id` when present and parseable. Non-credit or unreferenced
/// `included` entries, blank rows, and duplicate (role, name, artist) rows are
/// dropped.
fn credits_from_document(raw: OpenApiTrackCredits) -> TrackCredits {
    let referenced: std::collections::HashSet<String> = raw
        .data
        .and_then(|data| data.relationships)
        .and_then(|relationships| relationships.credits)
        .map(|credits| credits.data.into_iter().map(|entry| entry.id).collect())
        .unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut roles: std::collections::HashMap<String, Vec<CreditContributor>> =
        std::collections::HashMap::new();
    let mut seen: std::collections::HashSet<(String, String, Option<u64>)> =
        std::collections::HashSet::new();
    for resource in raw.included {
        if resource.resource_type != "credits" || !referenced.contains(&resource.id) {
            continue;
        }
        let Some(attributes) = resource.attributes else { continue };
        if attributes.name.trim().is_empty() || attributes.role.trim().is_empty() {
            continue;
        }
        let artist_id = resource
            .relationships
            .and_then(|relationships| relationships.artist)
            .and_then(|artist| artist.data)
            .and_then(|data| data.id.parse::<u64>().ok());
        if !seen.insert((attributes.role.clone(), attributes.name.clone(), artist_id)) {
            continue;
        }
        if !roles.contains_key(&attributes.role) {
            order.push(attributes.role.clone());
        }
        roles.entry(attributes.role).or_default().push(CreditContributor {
            name: attributes.name,
            artist_id,
        });
    }
    TrackCredits {
        roles: order
            .into_iter()
            .map(|role| CreditRole {
                contributors: roles.remove(&role).unwrap_or_default(),
                role,
            })
            .collect(),
    }
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCredits {
    #[serde(default)]
    data: Option<OpenApiTrackCreditsData>,
    #[serde(default)]
    included: Vec<OpenApiCreditResource>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCreditsData {
    #[serde(default)]
    relationships: Option<OpenApiTrackCreditsRelationships>,
}

#[derive(Debug, Deserialize)]
struct OpenApiTrackCreditsRelationships {
    #[serde(default)]
    credits: Option<OpenApiCreditRefs>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRefs {
    #[serde(default)]
    data: Vec<OpenApiCreditRef>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRef {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditResource {
    #[serde(default, rename = "type")]
    resource_type: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Option<OpenApiCreditAttributes>,
    #[serde(default)]
    relationships: Option<OpenApiCreditRelationships>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditAttributes {
    #[serde(default)]
    name: String,
    #[serde(default)]
    role: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditRelationships {
    #[serde(default)]
    artist: Option<OpenApiCreditArtist>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditArtist {
    #[serde(default)]
    data: Option<OpenApiCreditArtistData>,
}

#[derive(Debug, Deserialize)]
struct OpenApiCreditArtistData {
    #[serde(default)]
    id: String,
}
```

In `commands.rs` (add `TrackCredits` to the models import; only `get_track` if credits were cancelled in Task 0):

```rust
#[tauri::command]
pub async fn get_track(state: State<'_, AppState>, id: u64) -> Result<Track, String> {
    require_client(&state)?
        .track(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_track_credits(
    state: State<'_, AppState>,
    id: u64,
) -> Result<TrackCredits, String> {
    require_client(&state)?
        .track_credits(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

Register `commands::get_track,` and `commands::get_track_credits,` in `main.rs` (after `commands::get_album_info,`).

In `api.ts` after `AlbumInfo`:

```ts
export interface CreditContributor { name: string; artistId?: number | null }
export interface CreditRole { role: string; contributors: CreditContributor[] }
export interface TrackCredits { roles: CreditRole[] }
```

and after `getAlbumInfo`:

```ts
  getTrack: (id: number) => invoke<Track>("get_track", { id }),
  getTrackCredits: (id: number) => invoke<TrackCredits>("get_track_credits", { id }),
```

- [ ] **Step 5: Add the wiremock mapping test**

In `catalog.rs` `mod tests`:

```rust
    #[tokio::test]
    async fn track_credits_reads_the_include_and_maps_roles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks/1"))
            .and(query_param("countryCode", "GR"))
            .and(query_param("include", "credits.artist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "1",
                    "type": "tracks",
                    "relationships": { "credits": { "data": [
                        { "id": "c1", "type": "credits" },
                        { "id": "c2", "type": "credits" }
                    ] } }
                },
                "included": [
                    { "id": "c1", "type": "credits",
                      "attributes": { "name": "John Lennon", "role": "Vocals" },
                      "relationships": { "artist": { "data": { "id": "13", "type": "artists" } } } },
                    { "id": "c2", "type": "credits",
                      "attributes": { "name": "George Martin", "role": "Producer" } }
                ]
            })))
            .mount(&server)
            .await;
        let credits = client(&server).track_credits(1).await.unwrap();
        assert_eq!(credits.roles.len(), 2);
        assert_eq!(credits.roles[0].contributors[0].name, "John Lennon");
        assert_eq!(credits.roles[0].contributors[0].artist_id, Some(13));
        assert_eq!(credits.roles[1].contributors[0].artist_id, None);
    }

    #[tokio::test]
    async fn track_credits_without_a_credits_relationship_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": "1", "type": "tracks", "attributes": { "title": "X" } }
            })))
            .mount(&server)
            .await;
        assert!(client(&server).track_credits(1).await.unwrap().roles.is_empty());
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p oxitide-tidal credits`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`.
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/catalog.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts
git commit -m "feat(tidal): add track detail and credits reads"
```

---

### Task 4: Info store, `InfoPanel`, and the Now Playing Info tab

**Files:**
- Create: `ui/src/lib/info.svelte.ts`
- Test: `ui/src/lib/infoStore.test.ts`
- Create: `ui/src/lib/components/InfoPanel.svelte`
- Modify: `ui/src/lib/lyrics.svelte.ts` (tab union lines 6, 13, 19, 68)
- Test: `ui/src/lib/lyricsStore.test.ts` (tab coverage)
- Modify: `ui/src/views/NowPlaying.svelte` (spine lines 215–241, meta column lines 267–337, classes)

**Interfaces:**
- Produces: `info` singleton (`open`, `retry`, `track`, `credits`, `loading`, `error`, `creditsError`, `trackId`), `InfoPanel { trackId, nav, onNavigate? }`, tab union `"lyrics" | "next" | "info"`.
- Consumes: `api.getTrack`, `api.getTrackCredits` (Task 3), `lyrics.setTab`, `nav.goto`, `nav.closeNowPlaying`.

- [ ] **Step 1: Write the failing store tests**

Create `ui/src/lib/infoStore.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, type Track, type TrackCredits } from "./api";
import { createInfo } from "./info.svelte";

vi.mock("./api", () => ({ api: { getTrack: vi.fn(), getTrackCredits: vi.fn() } }));

const track = (id: number): Track => ({
  id, title: `T${id}`, duration: 200, artists: [{ id: 5, name: "A" }],
  album: { id: 7, title: "B", cover: null }, audioQuality: null,
  trackNumber: 1, volumeNumber: 1, explicit: false, mediaMetadata: null,
});

const credits = (): TrackCredits => ({
  roles: [{ role: "Vocals", contributors: [{ name: "A", artistId: 5 }] }],
});

describe("info store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.getTrack).mockResolvedValue(track(1));
    vi.mocked(api.getTrackCredits).mockResolvedValue(credits());
  });

  it("loads track and credits and clears loading", async () => {
    const model = createInfo();
    await model.open(1);
    expect(model.track?.id).toBe(1);
    expect(model.credits?.roles[0].role).toBe("Vocals");
    expect(model.loading).toBe(false);
    expect(model.trackId).toBe(1);
  });

  it("caches both fetches per track id", async () => {
    const model = createInfo();
    await model.open(1);
    await model.open(1);
    expect(api.getTrack).toHaveBeenCalledTimes(1);
    expect(api.getTrackCredits).toHaveBeenCalledTimes(1);
  });

  it("drops a superseded response", async () => {
    let resolveFirst: (value: Track) => void = () => {};
    vi.mocked(api.getTrack)
      .mockImplementationOnce(() => new Promise<Track>((resolve) => (resolveFirst = resolve)))
      .mockResolvedValueOnce(track(2));
    const model = createInfo();
    const first = model.open(1);
    await model.open(2);
    resolveFirst(track(1));
    await first;
    expect(model.track?.id).toBe(2);
    expect(model.trackId).toBe(2);
  });

  it("surfaces a track failure and retries", async () => {
    vi.mocked(api.getTrack).mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(track(1));
    const model = createInfo();
    await model.open(1);
    expect(model.error).toBe("Error: offline");
    expect(api.getTrackCredits).not.toHaveBeenCalled();
    model.retry();
    await vi.waitFor(() => expect(model.track?.id).toBe(1));
    expect(model.error).toBe("");
  });

  it("keeps track metadata when credits fail", async () => {
    vi.mocked(api.getTrackCredits).mockRejectedValue(new Error("credits down"));
    const model = createInfo();
    await model.open(1);
    expect(model.track?.id).toBe(1);
    expect(model.credits).toBeNull();
    expect(model.creditsError).toBe("Error: credits down");
  });

  it("reopens a failed id instead of serving a cache", async () => {
    vi.mocked(api.getTrackCredits).mockRejectedValue(new Error("down"));
    const model = createInfo();
    await model.open(1);
    await model.open(1);
    expect(api.getTrack).toHaveBeenCalledTimes(2);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/infoStore.test.ts`
Expected: FAIL — cannot resolve `./info.svelte`.

- [ ] **Step 3: Implement the store**

Create `ui/src/lib/info.svelte.ts`:

```ts
import { api, type Track, type TrackCredits } from "./api";

/// Per-session track-info cache (spec §5.3): track metadata + credits load
/// once per id, lazily on first Info open; failures stay uncached so reopening
/// retries. No disk cache.
export interface InfoStore {
  readonly trackId: number | null;
  readonly loading: boolean;
  readonly track: Track | null;
  readonly credits: TrackCredits | null;
  readonly error: string;
  readonly creditsError: string;
  open(id: number): Promise<void>;
  retry(): void;
}

interface Entry {
  track: Track;
  credits: TrackCredits | null;
  creditsError: string;
}

export function createInfo(): InfoStore {
  let trackId = $state<number | null>(null);
  let loading = $state(false);
  let track = $state<Track | null>(null);
  let credits = $state<TrackCredits | null>(null);
  let error = $state("");
  let creditsError = $state("");
  const cache = new Map<number, Entry>();
  let requestId = 0;

  async function open(id: number) {
    trackId = id;
    const cached = cache.get(id);
    if (cached) {
      track = cached.track;
      credits = cached.credits;
      creditsError = cached.creditsError;
      error = "";
      loading = false;
      return;
    }
    const token = ++requestId;
    loading = true;
    track = null;
    credits = null;
    error = "";
    creditsError = "";
    let loaded: Track | null = null;
    try {
      loaded = await api.getTrack(id);
    } catch (e) {
      error = String(e);
    }
    let loadedCredits: TrackCredits | null = null;
    let failure = "";
    if (loaded !== null) {
      try {
        loadedCredits = await api.getTrackCredits(id);
      } catch (e) {
        failure = String(e);
      }
    }
    if (token !== requestId) return; // a newer open superseded this fetch
    if (loaded !== null && failure === "") {
      cache.set(id, { track: loaded, credits: loadedCredits, creditsError: "" });
    }
    track = loaded;
    credits = loadedCredits;
    creditsError = failure;
    loading = false;
  }

  function retry() {
    if (trackId === null) return;
    cache.delete(trackId);
    void open(trackId);
  }

  return {
    get trackId() { return trackId; },
    get loading() { return loading; },
    get track() { return track; },
    get credits() { return credits; },
    get error() { return error; },
    get creditsError() { return creditsError; },
    open,
    retry,
  };
}

export const info = createInfo();
```

- [ ] **Step 4: Run the store tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/infoStore.test.ts`
Expected: PASS.

- [ ] **Step 5: Extend the tab union**

In `lyrics.svelte.ts` replace the `tab` type and initial value:

```ts
  readonly tab: "lyrics" | "next" | "info";
```

```ts
  setTab(tab: "lyrics" | "next" | "info"): void;
```

```ts
  let tab = $state<"lyrics" | "next" | "info">("lyrics");
```

In `lyricsStore.test.ts`, add:

```ts
  it("keeps the info tab on a track change and resets on stop", async () => {
    vi.mocked(api.getLyrics).mockResolvedValue(dto(1));
    const model = createLyrics();
    await model.open(track(1));
    model.setTab("info");
    vi.mocked(api.getLyrics).mockResolvedValue(dto(2));
    await model.open(track(2));
    expect(model.tab).toBe("info");
    await model.open(null);
    expect(model.tab).toBe("lyrics");
  });
```

- [ ] **Step 6: Build `InfoPanel`**

Create `ui/src/lib/components/InfoPanel.svelte`:

```svelte
<script lang="ts">
  import { formatDuration } from "../format";
  import { info } from "../info.svelte";
  import type { Route } from "../nav.svelte";
  import type { Nav } from "../nav.svelte";
  import { trackQualityLabel } from "../quality";
  import ErrorState from "./ErrorState.svelte";

  let {
    trackId,
    nav,
    onNavigate,
  }: { trackId: number; nav: Nav; onNavigate?: () => void } = $props();

  $effect(() => {
    void info.open(trackId);
  });

  const track = $derived(info.trackId === trackId ? info.track : null);
  const credits = $derived(info.trackId === trackId ? info.credits : null);
  const loading = $derived(info.trackId !== trackId || info.loading);
  const error = $derived(info.trackId === trackId ? info.error : "");
  const creditsError = $derived(info.trackId === trackId ? info.creditsError : "");
  const quality = $derived(track ? trackQualityLabel(track.audioQuality) : "");

  function go(route: Route) {
    onNavigate?.();
    nav.goto(route);
  }
</script>

{#if loading}
  <p class="muted pad">Loading track info…</p>
{:else if error}
  <ErrorState message={error} onRetry={() => info.retry()} />
{:else if track}
  <div class="info-panel">
    <dl class="facts">
      <div>
        <dt>Album</dt>
        <dd>
          {#if track.album}
            {@const album = track.album}
            <button onclick={() => go({ name: "album", id: album.id })}>{album.title}</button>
          {:else}
            <span class="muted">—</span>
          {/if}
        </dd>
      </div>
      <div>
        <dt>Artists</dt>
        <dd>
          {#each track.artists as artist (artist.id)}
            <button onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
          {/each}
        </dd>
      </div>
      <div>
        <dt>Duration</dt>
        <dd>{formatDuration(track.duration)}</dd>
      </div>
      {#if quality}
        <div>
          <dt>Quality</dt>
          <dd>{quality}</dd>
        </div>
      {/if}
    </dl>

    <section class="credits" aria-label="Credits">
      {#if creditsError}
        <p class="error">{creditsError}</p>
        <button class="retry" onclick={() => info.retry()}>Retry</button>
      {:else if credits && credits.roles.length > 0}
        {#each credits.roles as role (role.role)}
          <div class="role">
            <h3>{role.role}</h3>
            <p>
              {#each role.contributors as contributor, i (contributor.name + i)}
                {#if i > 0}<span class="sep">·</span>{/if}{#if contributor.artistId !== null && contributor.artistId !== undefined}<button onclick={() => go({ name: "artist", id: contributor.artistId! })}>{contributor.name}</button>{:else}<span>{contributor.name}</span>{/if}
              {/each}
            </p>
          </div>
        {/each}
      {:else if credits}
        <p class="muted">No credits available for this track.</p>
      {/if}
    </section>
  </div>
{:else}
  <p class="muted pad">Track not found.</p>
{/if}

<style>
  .info-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
  }
  .pad {
    padding: 8px 0;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
  }
  .facts div {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    gap: 8px;
    align-items: baseline;
  }
  .facts dt {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .facts dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .credits {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }
  .role h3 {
    margin: 0 0 2px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .role p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.5;
  }
  .sep {
    color: var(--text-muted);
    padding: 0 6px;
  }
  .retry {
    align-self: flex-start;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
</style>
```

- [ ] **Step 7: Wire the Now Playing tab**

In `NowPlaying.svelte`:

Add the import (top of `<script>`):

```ts
  import InfoPanel from "../lib/components/InfoPanel.svelte";
```

Add derived flags after `showLyrics` (line 71):

```ts
  const showInfo = $derived(lyrics.tab === "info" && track !== null);
  /// The spine layout (cover + side column) is shared by lyrics and info.
  const wide = $derived(showLyrics || showInfo);

  const NP_TABS = ["lyrics", "next", "info"] as const;

  /// Vertical tablist keyboard pattern (spec §6.2): roving focus with
  /// Up/Down (wrapping) and Home/End; activation follows focus.
  function onTabKeydown(event: KeyboardEvent, current: (typeof NP_TABS)[number]) {
    const index = NP_TABS.indexOf(current);
    let target: (typeof NP_TABS)[number] | null = null;
    if (event.key === "ArrowDown") target = NP_TABS[(index + 1) % NP_TABS.length];
    else if (event.key === "ArrowUp") target = NP_TABS[(index - 1 + NP_TABS.length) % NP_TABS.length];
    else if (event.key === "Home") target = NP_TABS[0];
    else if (event.key === "End") target = NP_TABS[NP_TABS.length - 1];
    if (!target) return;
    event.preventDefault();
    lyrics.setTab(target);
    document.getElementById(`np-tab-${target}`)?.focus();
  }
```

Change the root class (line 172): `class:with-lyrics={wide}`.

Replace the spine condition and tabs (lines 215–241) with a always-on spine and three tabs:

```svelte
        {#if track}
          <div
            class="spine"
            role="tablist"
            aria-orientation="vertical"
            aria-label="Now playing content"
            in:fade={{ duration: dur(200), easing: cubicOut }}
            out:fade={{ duration: dur(160) }}
          >
            <button
              id="np-tab-lyrics"
              role="tab"
              tabindex={lyrics.tab === "lyrics" ? 0 : -1}
              aria-selected={lyrics.tab === "lyrics"}
              aria-controls="np-panel-lyrics"
              class:active={lyrics.tab === "lyrics"}
              onclick={() => lyrics.setTab("lyrics")}
              onkeydown={(event) => onTabKeydown(event, "lyrics")}
            >
              Lyrics
            </button>
            <button
              id="np-tab-next"
              role="tab"
              tabindex={lyrics.tab === "next" ? 0 : -1}
              aria-selected={lyrics.tab === "next"}
              aria-controls="np-panel-next"
              class:active={lyrics.tab === "next"}
              onclick={() => lyrics.setTab("next")}
              onkeydown={(event) => onTabKeydown(event, "next")}
            >
              Next up
            </button>
            <button
              id="np-tab-info"
              role="tab"
              tabindex={lyrics.tab === "info" ? 0 : -1}
              aria-selected={lyrics.tab === "info"}
              aria-controls="np-panel-info"
              class:active={lyrics.tab === "info"}
              onclick={() => lyrics.setTab("info")}
              onkeydown={(event) => onTabKeydown(event, "info")}
            >
              Info
            </button>
          </div>
        {/if}
```

Replace the meta-column branch (lines 267–337) with a three-way branch:

```svelte
  <div class="meta">
    {#if track}
      {#if showLyrics}
        {@const shown = currentLyrics}
        <div
          class="lyrics-slot"
          role="tabpanel"
          id="np-panel-lyrics"
          aria-labelledby="np-tab-lyrics"
          in:fly={{ y: 8, duration: dur(320), delay: dur(80), easing: cubicOut }}
          out:fly={{ y: 12, duration: dur(280), easing: cubicInOut }}
        >
          {#if shown}
            {#if lyricsSettled}
              {#key shown.track_id}
                <LyricsPanel {player} dto={shown} />
              {/key}
            {/if}
          {:else}
            <p class="muted loading">Loading lyrics…</p>
          {/if}
        </div>
      {:else if showInfo}
        <div
          class="lyrics-slot"
          role="tabpanel"
          id="np-panel-info"
          aria-labelledby="np-tab-info"
          in:fly={{ y: 8, duration: dur(320), delay: dur(80), easing: cubicOut }}
          out:fly={{ y: 12, duration: dur(280), easing: cubicInOut }}
        >
          <InfoPanel trackId={track.track_id} {nav} onNavigate={() => nav.closeNowPlaying()} />
        </div>
      {:else}
        <div
          class="meta-details"
          role="tabpanel"
          id="np-panel-next"
          aria-labelledby="np-tab-next"
          in:fly={{ y: -16, duration: dur(260), delay: dur(40), easing: cubicOut }}
          out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
        >
          {#key track.track_id}
            <div class="details-swap" in:fly={{ y: 6, duration: dur(200), easing: cubicOut }}>
              {@render details(track)}
            </div>
          {/key}
        </div>

        <hr
          in:fly={{ y: -16, duration: dur(240), delay: dur(60), easing: cubicOut }}
          out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
        />

        <div
          class="meta-list"
          in:fly={{ y: -16, duration: dur(240), delay: dur(80), easing: cubicOut }}
          out:fly={{ y: -10, duration: dur(420), easing: cubicInOut }}
        >
          {#if nextUp.length > 0}
            <ul class="next-up">
              {#each nextUp as item (item.track_id)}
                <li
                  in:fly={{ y: 10, duration: dur(240), easing: cubicOut }}
                  out:fade={{ duration: dur(160) }}
                  animate:flip={{ duration: dur(240), easing: cubicOut }}
                >
                  <span class="thumb"><Cover uuid={item.cover} size={80} radius={6} alt="" /></span>
                  <span class="next-title">{item.title}</span>
                  <span class="muted next-artist">{item.artist}</span>
                  <span class="muted tnum">{formatDuration(item.duration_secs)}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Queue is empty</p>
          {/if}
        </div>
      {/if}
    {:else}
      <span class="eyebrow">Now playing</span>
      <p class="muted">Nothing playing</p>
    {/if}
  </div>
```

- [ ] **Step 8: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `cargo test --workspace`.
Expected: all green. Manual spot check: `npm --prefix ui run app:dev`, play a track, open Now Playing → Info shows metadata; with credits available it lists roles; with credits unavailable it shows the empty state. Switching tracks updates the panel. Keyboard: Tab to the spine, Arrow Up/Down and Home/End move and activate tabs.

```bash
git add ui/src/lib/info.svelte.ts ui/src/lib/infoStore.test.ts ui/src/lib/lyrics.svelte.ts ui/src/lib/lyricsStore.test.ts ui/src/lib/components/InfoPanel.svelte ui/src/views/NowPlaying.svelte
git commit -m "feat(ui): add the now-playing info tab and panel"
```

---

### Task 5: Track menu "Track info"

**Files:**
- Modify: `ui/src/lib/menus.ts` (context lines 5–26, items lines 30–53)
- Test: `ui/src/lib/menus.test.ts` (baseCtx lines 52–65; id-list assertions 68–77, 106–122)
- Create: `ui/src/lib/trackInfo.svelte.ts`
- Test: `ui/src/lib/trackInfo.test.ts`
- Modify: `ui/src/lib/components/TrackTable.svelte` (menu context lines 233–246)
- Create: `ui/src/lib/components/TrackInfoHost.svelte`
- Modify: `ui/src/App.svelte` (imports, render next to `PlaylistPicker` line 578)

**Interfaces:**
- Produces: `trackMenuItems` item `track-info`, `TrackMenuContext.trackInfo()`, `trackInfo` singleton (`request`, `open(track)`, `close()`), `TrackInfoHost { player, nav }`.
- Consumes: `player.state.current`, `lyrics.setTab("info")`, `nav.nowPlayingOpen` / `nav.toggleNowPlaying`, `Modal`, `InfoPanel` (Task 4).

- [ ] **Step 1: Write the failing menu and store tests**

In `menus.test.ts`, add `trackInfo: vi.fn(),` to `baseCtx`, add `"track-info"` to the expected id lists (after `"copy-title"`), and add:

```ts
  it("track info invokes its callback", () => {
    const ctx = { ...baseCtx, trackInfo: vi.fn() };
    const item = trackMenuItems(ctx).find((entry) => entry.id === "track-info");
    expect(item?.label).toBe("Track info");
    item?.run();
    expect(ctx.trackInfo).toHaveBeenCalledTimes(1);
  });
```

Create `ui/src/lib/trackInfo.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Track } from "./api";
import { createTrackInfo } from "./trackInfo.svelte";

const track = { id: 4, title: "Purple Rain" } as Track;

describe("track info requests", () => {
  it("opens and closes a request slot", () => {
    const model = createTrackInfo();
    model.open(track);
    expect(model.request).toEqual({ id: 4, title: "Purple Rain" });
    model.close();
    expect(model.request).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/menus.test.ts src/lib/trackInfo.test.ts`
Expected: FAIL — `track-info` missing / cannot resolve `./trackInfo.svelte`.

- [ ] **Step 3: Implement the menu item and request store**

In `menus.ts`, add to `TrackMenuContext` after `copyTitle(): void;`:

```ts
  /// Opens the Info surface: the playing track routes to Now Playing's Info
  /// tab, any other row opens the modal (host decides).
  trackInfo(): void;
```

and in `trackMenuItems` after the `copy-title` entry:

```ts
    { id: "track-info", label: "Track info", run: () => ctx.trackInfo() },
```

Create `ui/src/lib/trackInfo.svelte.ts`:

```ts
import type { Track } from "./api";

export interface TrackInfoRequest {
  id: number;
  title: string;
}

export interface TrackInfoState {
  readonly request: TrackInfoRequest | null;
  open(track: Track): void;
  close(): void;
}

export function createTrackInfo(): TrackInfoState {
  let request = $state<TrackInfoRequest | null>(null);
  return {
    get request() {
      return request;
    },
    open: (track) => {
      request = { id: track.id, title: track.title };
    },
    close: () => {
      request = null;
    },
  };
}

/// One app-wide request slot; TrackTable opens it and App hosts the surface.
export const trackInfo = createTrackInfo();
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/menus.test.ts src/lib/trackInfo.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire `TrackTable` and the host**

In `TrackTable.svelte`, import `{ trackInfo } from "../trackInfo.svelte";` and add to the `trackMenuItems({...})` context (line 233):

```ts
              trackInfo: () => trackInfo.open(t),
```

Create `ui/src/lib/components/TrackInfoHost.svelte`:

```svelte
<script lang="ts">
  import { lyrics } from "../lyrics.svelte";
  import type { Nav } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import { trackInfo } from "../trackInfo.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import Modal from "./Modal.svelte";

  let { player, nav }: { player: Player; nav: Nav } = $props();

  const request = $derived(trackInfo.request);
  // The playing track opens Now Playing's Info tab; any other row opens the
  // modal with the same panel (spec §6.2).
  const isCurrent = $derived(
    request !== null && player.state.current?.track_id === request.id,
  );

  $effect(() => {
    if (request && isCurrent) {
      lyrics.setTab("info");
      if (!nav.nowPlayingOpen) nav.toggleNowPlaying();
      trackInfo.close();
    }
  });
</script>

{#if request && !isCurrent}
  <Modal title="Track info" onclose={() => trackInfo.close()}>
    <InfoPanel trackId={request.id} {nav} onNavigate={() => trackInfo.close()} />
  </Modal>
{/if}
```

In `App.svelte`, import the host and render it beside the other app-level overlays:

```ts
  import TrackInfoHost from "./lib/components/TrackInfoHost.svelte";
```

```svelte
    <TrackInfoHost {player} {nav} />
```

(place directly above `<PlaylistPicker ... />` at line 578).

- [ ] **Step 6: Run the gates and commit**

Run: `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: green. Manual: right-click the playing row → "Track info" opens Now Playing on Info; right-click another row → modal; Escape/backdrop closes the modal.

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/trackInfo.svelte.ts ui/src/lib/trackInfo.test.ts ui/src/lib/components/TrackTable.svelte ui/src/lib/components/TrackInfoHost.svelte ui/src/App.svelte
git commit -m "feat(ui): add the track info context action"
```

---

### Task 6: Smoke checklist, gates, and PM-D1 note

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-06-track9-2-smoke.md`

**Interfaces:**
- Consumes: every prior task.

- [ ] **Step 1: Run the full scripted gates**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
Expected: all green. Record the exact commands and result in the note header.

- [ ] **Step 2: Write the owner smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-06-track9-2-smoke.md` (date = run day) following the 9.1 note format:

- **Status/authority** lines (spec §6.2, §5.3; Task 0 decision reference).
- **PM-D1 impact:** no browse-time fetches; one extra `get_album_info` request inside the existing album-open wave; `get_track` + `get_track_credits` once per track per session on first Info open; no Track 3 scenario re-run required (S3/S6 album paths are synthetic) — state this explicitly.
- Checklist sections, each item an observable behavior:
  - **Album facts:** open an album with a release date/copyright/UPC → facts row shows present values, omits absent ones; open a single → no facts row (or minimal) without breaking layout.
  - **Multi-volume:** open a multi-volume album → `Volume N` headings and per-volume numbering; sort or filter → flat table with normal numbering; single-volume albums are unchanged.
  - **Info tab:** play a track → Now Playing spine shows Lyrics / Next up / Info; Info loads once and shows album/artists/duration/quality links; clicking the album or an artist link navigates and closes the overlay.
  - **Credits:** a track with credits lists role groups in order; contributors with ids link to the artist page; a track without credits shows "No credits available for this track."; if Task 0 found credits unavailable, this section records that and the Info tab is metadata-only.
  - **Track info menu:** current track → opens Now Playing Info tab; another row → modal; Escape/backdrop closes and restores focus.
  - **Failure paths:** disconnect the network and open Info → track error with Retry; retry recovers; opening another track while one is loading shows only the new track.
- If Linux checks are skipped, mark them "exempted (Track 8 pattern)".

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-06-track9-2-smoke.md
git commit -m "docs: add the track 9.2 smoke checklist"
```

---

## Plan Self-Review

- **Spec coverage:** §5.3 spike → Task 0; DTOs/methods/commands/api.ts → Tasks 1/3; Info tab (full ARIA pattern from birth) + links + error/retry + empty state + lazy cache → Task 4; track menu entry + modal → Task 5; album facts + multi-volume grouping → Task 2; §6.2 perf statement → Global Constraints + Task 6; §7 tests (serde, wiremock parsing, store behavior, menu) → Tasks 1–5. No spec item is unassigned; the credits-unavailable path is defined by Task 0's decision gate.
- **Placeholders:** none; every step carries code, exact paths, or exact commands.
- **Type consistency:** `AlbumInfo` (Task 1) is consumed by Album.svelte in Task 2; `TrackCredits`/`CreditRole`/`CreditContributor` (Task 3) are consumed by the store and `InfoPanel` in Task 4; `VolumeSection` (Task 2) matches `TrackTable`'s prop; `TrackInfoRequest` (Task 5) matches the host; tab union extended in Task 4 is used by the host in Task 5.
- **Review Focus coverage:** (1) credits unavailable → Task 0 gate + Task 4 empty state + Task 6 smoke; (2) missing album/artist ids → InfoPanel guards + Task 3 parser tests; (3) partial volumes → `volumeSections` tests + engaged fallback; (4) album-info failure → `.catch(() => null)` wave + zero-facts rendering; (5) superseded fetches → info store test + `trackId` guards.
