# List Interactions and Playlist Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide playlist editing on playlists the user does not own, make double-clicking a track row play it on every list surface, make tapping a queue row play it, and add "Copy title" to the track and queue context menus.

**Architecture:** The TIDAL `Playlist` DTO gains a parsed `creator` and a serialized `owned` flag computed against the session user id (fail-closed on single fetches, assumed-owned on list endpoints). Views gate edit affordances on `owned`. `TrackTable` rows gain a guarded `dblclick`; `QueueRail` rows lose their inner button and gain a guarded row click plus keyboard activation. A tiny `clipboard.ts` module wraps the web Clipboard API with an `execCommand` fallback; the pure menu builders gain a `Copy title` item.

**Tech Stack:** Rust (serde, wiremock), Svelte 5 runes, TypeScript, vitest. No new crates or npm packages.

**Spec:** none — requirements came directly from the owner (2026-09-25): consistent list interactions, no edit UI on unowned playlists, queue tap-to-play, and a Copy title menu item.

## Global Constraints

- Rust tests use `wiremock` only, never the live TIDAL API; `MemoryStore` only; never log token values.
- No new crates and no new npm packages; version stays 0.2.0; no packaging changes.
- Svelte 5 runes only; components are covered by `svelte-check` + smoke (no DOM test environment, P8-D6).
- Menus mirror inline actions and stay additive.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **Missing creator on a single fetch:** must mark the playlist *not* owned (fail closed), while a missing creator on the `user_playlists` list keeps it owned (the endpoint returns only the user's playlists). Tasks 1 tests both.
- **Unowned playlist still offers a path to a write:** dropping `playlistActions` must also disable drag reorder and the row menu's Move/Remove, not just the header buttons. Task 2 covers header, row buttons, drag, and menu items; smoke verifies all four.
- **Double-click on a row control:** double-clicking ♥/＋/⊕/✕ (or the queue ✕/drag handle) must not trigger playback of the row underneath. Tasks 3/4 guard interactive targets.
- **Space on a focused queue row:** row activation must not also toggle playback through the global shortcut host. Task 4 stops propagation; `svelte-check` plus smoke verify.
- **Clipboard failures:** `copyText` must fall back when the async Clipboard API is unavailable/rejected and return `false` rather than throwing. Task 5 implements the fallback; smoke verifies paste (no DOM unit test by design).

---

### Task 1: Ownership signal on the playlist DTO

**Files:**
- Modify: `crates/oxitide-tidal/src/models.rs`
- Modify: `crates/oxitide-tidal/src/playlists.rs`
- Modify: `crates/oxitide-tidal/src/search.rs`
- Modify: `crates/oxitide-app/src/commands.rs` (test literal only)
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: existing `TidalClient::get`, `get_all_pages`, `session()`.
- Produces: `PlaylistCreator { id: u64 }`; `Playlist.creator: Option<PlaylistCreator>` (never serialized); `Playlist.owned: bool` (serialized); `Playlist::mark_owned(&mut self, user_id: u64, assume_owned: bool)`. `api.ts` `Playlist` gains `owned: boolean`.

- [ ] **Step 1: Write the failing model tests**

Append to `models.rs`'s `mod tests`:

```rust
    #[test]
    fn playlist_creator_parses_and_marks_ownership() {
        let raw = r#"{"uuid": "u", "title": "Mine", "creator": {"id": 42}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, false);
        assert!(playlist.owned);

        playlist.mark_owned(7, false);
        assert!(!playlist.owned);
    }

    #[test]
    fn playlist_without_creator_uses_the_assumed_ownership() {
        let raw = r#"{"uuid": "u", "title": "Mine"}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();

        playlist.mark_owned(42, true);
        assert!(playlist.owned);

        playlist.mark_owned(42, false);
        assert!(!playlist.owned);
    }

    #[test]
    fn playlist_serializes_owned_but_not_creator() {
        let raw = r#"{"uuid": "u", "title": "T", "creator": {"id": 1}}"#;
        let mut playlist: Playlist = serde_json::from_str(raw).unwrap();
        playlist.mark_owned(1, false);

        let value = serde_json::to_value(&playlist).unwrap();

        assert_eq!(value["owned"], serde_json::json!(true));
        assert!(value.get("creator").is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal models`
Expected: FAIL — `mark_owned` does not exist / unknown field `creator` is ignored today but `owned` is missing.

- [ ] **Step 3: Implement the model changes**

In `models.rs`, add above `Playlist`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistCreator {
    pub id: u64,
}
```

Extend `Playlist`:

```rust
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
    #[serde(default, skip_serializing)]
    pub creator: Option<PlaylistCreator>,
    #[serde(skip_deserializing, default)]
    pub owned: bool,
}

impl Playlist {
    /// Mark editability from the creator id. `assume_owned` decides the
    /// missing-creator case: list endpoints return only the user's playlists,
    /// single fetches can be anybody's.
    pub fn mark_owned(&mut self, user_id: u64, assume_owned: bool) {
        self.owned = match &self.creator {
            Some(creator) => creator.id == user_id,
            None => assume_owned,
        };
    }
}
```

- [ ] **Step 4: Run the model tests**

Run: `cargo test -p oxitide-tidal models`
Expected: PASS (3 new tests).

- [ ] **Step 5: Write the failing ownership tests**

Append to `playlists.rs`'s `mod tests`:

```rust
    #[tokio::test]
    async fn playlist_marks_owned_when_the_creator_is_the_session_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P", "creator": {"id": 42}
            })))
            .mount(&server).await;
        assert!(client(&server).playlist("p-uuid").await.unwrap().owned);
    }

    #[tokio::test]
    async fn playlist_marks_not_owned_for_another_or_missing_creator() {
        let other = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P", "creator": {"id": 7}
            })))
            .mount(&other).await;
        assert!(!client(&other).playlist("p-uuid").await.unwrap().owned);

        let missing = MockServer::start().await;
        Mock::given(method("GET")).and(path("/playlists/p-uuid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "p-uuid", "title": "P"
            })))
            .mount(&missing).await;
        assert!(!client(&missing).playlist("p-uuid").await.unwrap().owned);
    }

    #[tokio::test]
    async fn user_playlists_marks_items_owned_even_without_a_creator() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/users/42/playlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"uuid": "a", "title": "A"}, {"uuid": "b", "title": "B"}],
                "totalNumberOfItems": 2, "limit": 50, "offset": 0
            })))
            .mount(&server).await;
        let items = client(&server).user_playlists(50, 0).await.unwrap();
        assert!(items.iter().all(|playlist| playlist.owned));
    }
```

In `search.rs`'s `mod tests`, append:

```rust
    #[tokio::test]
    async fn search_marks_playlist_ownership() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tracks": {"items": []}, "albums": {"items": []},
                "artists": {"items": []},
                "playlists": {"items": [
                    {"uuid": "mine", "title": "Mine", "creator": {"id": 1}},
                    {"uuid": "theirs", "title": "Theirs", "creator": {"id": 2}},
                    {"uuid": "unknown", "title": "Unknown"}
                ]}
            })))
            .mount(&server).await;
        let results = client(&server).search("mix", &["playlists"], 20, 0).await.unwrap();
        assert!(results.playlist_items()[0].owned);
        assert!(!results.playlist_items()[1].owned);
        assert!(!results.playlist_items()[2].owned);
    }
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p oxitide-tidal playlist_marks user_playlists_marks search_marks`
Expected: compile error / assertions fail — ownership is not marked yet.

- [ ] **Step 7: Implement the marking**

In `playlists.rs`:

```rust
    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        let mut playlist: Playlist = self.get(&format!("playlists/{id}"), &[]).await?;
        playlist.mark_owned(self.session().user_id, false);
        Ok(playlist)
    }
```

```rust
    pub async fn user_playlists(&self, limit: u32, offset: u32) -> Result<Vec<Playlist>> {
        let page: ItemPage<Playlist> = self
            .get(
                &format!("users/{}/playlists", self.session().user_id),
                &page_params(limit, offset),
            )
            .await?;
        let mut items = page.items;
        for playlist in &mut items {
            playlist.mark_owned(self.session().user_id, true);
        }
        Ok(items)
    }

    /// Walk every page of the user's playlists.
    pub async fn all_user_playlists(&self) -> Result<Vec<Playlist>> {
        let mut items = self
            .get_all_pages(
                &format!("users/{}/playlists", self.session().user_id),
                &[],
                DEFAULT_PAGE_SIZE,
            )
            .await?;
        for playlist in &mut items {
            playlist.mark_owned(self.session().user_id, true);
        }
        Ok(items)
    }
```

In `create_playlist`, after the envelope:

```rust
        let mut playlist = envelope.data;
        playlist.mark_owned(self.session().user_id, true);
        Ok(playlist)
```

In `search.rs`:

```rust
        let mut results: SearchResults = self.get("search", &params).await?;
        for playlist in &mut results.playlists.items {
            playlist.mark_owned(self.session().user_id, false);
        }
        Ok(results)
```

- [ ] **Step 8: Extend the existing create test**

In `playlists.rs`, inside `create_playlist_puts_v2_and_parses_the_envelope`, add after the `number_of_tracks` assertion:

```rust
        assert!(playlist.owned);
```

In `commands.rs`, update the `playlist_dto_uses_camel_case_for_track_count` literal:

```rust
        let dto = oxitide_tidal::models::Playlist {
            uuid: "u".into(),
            title: "T".into(),
            description: None,
            number_of_tracks: 3,
            duration: 0,
            image: None,
            creator: None,
            owned: true,
        };
```

- [ ] **Step 9: Sync the UI interface**

In `ui/src/lib/api.ts`, add the field:

```ts
export interface Playlist {
  uuid: string; title: string; description?: string | null;
  numberOfTracks: number; duration: number; image?: string | null;
  owned: boolean;
}
```

- [ ] **Step 10: Run the Rust tests and the type check**

Run: `cargo test -p oxitide-tidal; cargo test -p oxitide-app; npm --prefix ui run check`
Expected: all pass; `svelte-check` 0 errors (views still compile with the added field).

- [ ] **Step 11: Commit**

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/playlists.rs crates/oxitide-tidal/src/search.rs crates/oxitide-app/src/commands.rs ui/src/lib/api.ts
git commit -m "feat(tidal): expose playlist ownership on the DTO"
```

---

### Task 2: Gate playlist editing on ownership

**Files:**
- Modify: `ui/src/lib/menus.ts`
- Modify: `ui/src/lib/menus.test.ts`
- Modify: `ui/src/views/Playlist.svelte`
- Modify: `ui/src/views/Library.svelte`

**Interfaces:**
- Consumes: `Playlist.owned` (Task 1).
- Produces: `libraryPlaylistMenuItems` context gains `editable: boolean`; Rename/Delete items appear only when true.

- [ ] **Step 1: Update the builder test (RED)**

In `menus.test.ts`, replace the `libraryPlaylistMenuItems` describe block:

```ts
describe("libraryPlaylistMenuItems", () => {
  it("lists open, rename, and a danger delete for an editable playlist", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({
      playlist: playlist(),
      editable: true,
      open,
      rename,
      remove,
    });

    expect(items.map((i) => i.id)).toEqual(["open", "rename", "delete"]);
    expect(items.find((i) => i.id === "delete")?.danger).toBe(true);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });

  it("keeps only Open for a playlist we do not own", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({
      playlist: playlist(),
      editable: false,
      open,
      rename,
      remove,
    });

    expect(items.map((i) => i.id)).toEqual(["open"]);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — the builder does not accept `editable` yet and always emits Rename/Delete.

- [ ] **Step 3: Implement the builder change**

In `menus.ts`:

```ts
export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  editable: boolean;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  const items: MenuItem[] = [{ id: "open", label: "Open", run: () => ctx.open() }];
  if (ctx.editable) {
    items.push(
      { id: "rename", label: "Rename", run: () => ctx.rename() },
      { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
    );
  }
  return items;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: PASS.

- [ ] **Step 5: Gate the Playlist view**

In `Playlist.svelte`, header:

```svelte
    <h2>
      {playlist.title}
      {#if playlist.owned}
        <button onclick={startRename}>Rename</button>
        <button onclick={() => (confirmingDelete = true)}>Delete</button>
      {/if}
    </h2>
```

Table:
```svelte
    playlistActions={playlist.owned
      ? { onRemove: removeAt, onMove: moveAt, busy, disabled: !pager.done, onDrop: moveTrack }
      : undefined}
```

- [ ] **Step 6: Gate the Library rows**

In `Library.svelte`, row buttons:

```svelte
            {:else}
              <button onclick={() => nav.goto({ name: "playlist", id: p.uuid })}>{p.title}</button>
              {#if p.owned}
                <button onclick={() => startRename(p)}>Rename</button>
                <button onclick={() => (deleting = p)}>Delete</button>
              {/if}
            {/if}
```

Menu call:

```svelte
            use:menuFor={() =>
              libraryPlaylistMenuItems({
                playlist: p,
                editable: p.owned,
                open: () => nav.goto({ name: "playlist", id: p.uuid }),
                rename: () => startRename(p),
                remove: () => (deleting = p),
              })}
```

- [ ] **Step 7: Run the UI tests and the type check**

Run: `npm --prefix ui test; npm --prefix ui run check`
Expected: all pass, 0 errors / 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/views/Playlist.svelte ui/src/views/Library.svelte
git commit -m "feat(ui): hide playlist editing on playlists we do not own"
```

---

### Task 3: Double-click a track row to play

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte`

**Interfaces:**
- Consumes: existing `play(index)` and `loadingAll`.
- Produces: row `dblclick` plays that row; no behavior change for buttons.

- [ ] **Step 1: Add the handler and the row binding**

In `TrackTable.svelte`'s script, after `play()`:

```ts
  function onRowDoubleClick(event: MouseEvent, index: number) {
    if (loadingAll) return;
    const target = event.target;
    if (target instanceof Element && target.closest("button, a, input, select, textarea")) return;
    void play(index);
  }
```

On the row element (inside the `{#each}`), alongside `use:menuFor`:

```svelte
          ondblclick={(event) => onRowDoubleClick(event, index)}
```

Add to the `.row` style block:

```css
  .row {
    height: 36px;
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
```

- [ ] **Step 2: Run the type check**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/lib/components/TrackTable.svelte
git commit -m "feat(ui): play a track by double-clicking its row"
```

---

### Task 4: Queue rows play on tap

**Files:**
- Modify: `ui/src/lib/components/QueueRail.svelte`

**Interfaces:**
- Consumes: existing `jump(index)`.
- Produces: row click (anywhere except ✕ and the drag handle) plays the item; Enter/Space on a focused row plays it; the inner title button is gone.

- [ ] **Step 1: Replace the row interaction**

In `QueueRail.svelte`, add after `jump()`:

```ts
  function onRowClick(event: MouseEvent, index: number) {
    const target = event.target;
    if (target instanceof Element && target.closest("button, .handle")) return;
    jump(index);
  }

  function onRowKeydown(event: KeyboardEvent, index: number) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    event.stopPropagation();
    jump(index);
  }
```

Replace the row markup:

```svelte
          <div
            role="button"
            tabindex="0"
            draggable="true"
            data-drag-index={index}
            use:menuFor={() =>
              queueMenuItems({ index, playNow: () => jump(index), remove: () => player.remove(index) })}
            onclick={(event) => onRowClick(event, index)}
            onkeydown={(event) => onRowKeydown(event, index)}
            style="height:{QUEUE_ROW_HEIGHT}px;box-sizing:border-box;display:flex;align-items:center;padding:4px;border-bottom:1px solid var(--line);cursor:pointer;{player.state.current?.track_id === item.track_id ? 'background:var(--panel);' : ''}"
          >
            <span class="muted handle" style="cursor:grab;padding-right:6px" title="Drag to reorder" aria-hidden="true">⠿</span>
            <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
              {item.title} <span class="muted">{item.artist}</span>
            </span>
            <button onclick={() => player.remove(index)} aria-label="Remove">✕</button>
          </div>
```

Also remove the now-invalid `role="list"` from the wrapper `<div>` (rows are buttons now):

```svelte
  <div style="margin:8px 0">
```

- [ ] **Step 2: Run the type check**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/lib/components/QueueRail.svelte
git commit -m "feat(ui): play queue items by tapping the row"
```

---

### Task 5: Copy title in the track and queue menus

**Files:**
- Create: `ui/src/lib/clipboard.ts`
- Modify: `ui/src/lib/menus.ts`
- Modify: `ui/src/lib/menus.test.ts`
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/lib/components/QueueRail.svelte`

**Interfaces:**
- Produces: `copyText(text: string): Promise<boolean>`; `trackMenuItems` and `queueMenuItems` contexts gain `copyTitle(): void` and emit a `Copy title` item.

- [ ] **Step 1: Write the failing builder tests**

In `menus.test.ts`, update the track expectations and add the callback:

```ts
  it("lists the five base actions", () => {
    expect(trackMenuItems(baseCtx()).map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
    ]);
  });
```

In `"invokes each callback once"`:

```ts
  it("invokes each callback once", () => {
    const ctx = baseCtx();
    for (const item of trackMenuItems(ctx)) item.run();

    expect(vi.mocked(ctx.play)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToQueue)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToPlaylist)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.copyTitle)).toHaveBeenCalledTimes(1);
  });
```

In `"appends playlist editing when the context is editable"`:

```ts
    expect(items.map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "copy-title",
      "move-up",
      "move-down",
      "remove",
    ]);
```

Update the queue test:

```ts
  it("lists play now, copy title, and remove and invokes them once", () => {
    const playNow = vi.fn();
    const copyTitle = vi.fn();
    const remove = vi.fn();

    const items = queueMenuItems({ index: 0, playNow, copyTitle, remove });

    expect(items.map((i) => i.id)).toEqual(["play-now", "copy-title", "remove"]);
    for (const item of items) item.run();
    expect(playNow).toHaveBeenCalledTimes(1);
    expect(copyTitle).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });
```

Update the `baseCtx` helper in `menus.test.ts`:

```ts
function baseCtx(overrides: Partial<TrackMenuContext> = {}): TrackMenuContext {
  return {
    track: track(1),
    index: 0,
    trackCount: 1,
    favorites: favorites(false),
    play: vi.fn(),
    addToQueue: vi.fn(),
    addToPlaylist: vi.fn(),
    copyTitle: vi.fn(),
    ...overrides,
  };
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — no `copy-title` items yet.

- [ ] **Step 3: Implement the clipboard module**

Create `ui/src/lib/clipboard.ts`:

```ts
/// Copy `text` to the OS clipboard. The async Clipboard API is available in
/// the Tauri webview; the `execCommand` fallback covers WebView2 states that
/// reject it. Returns false when both paths fail.
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the legacy path
  }
  try {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.top = "-1000px";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const copied = document.execCommand("copy");
    area.remove();
    return copied;
  } catch {
    return false;
  }
}
```

- [ ] **Step 4: Implement the menu items**

In `menus.ts`, add to `TrackMenuContext`:

```ts
  copyTitle(): void;
```

and after the favorite item:

```ts
    { id: "copy-title", label: "Copy title", run: () => ctx.copyTitle() },
```

Update `queueMenuItems`:

```ts
export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  copyTitle(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "play-now", label: "Play now", run: () => ctx.playNow() },
    { id: "copy-title", label: "Copy title", run: () => ctx.copyTitle() },
    { id: "remove", label: "Remove", run: () => ctx.remove() },
  ];
}
```

- [ ] **Step 5: Run the builder tests**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: PASS.

- [ ] **Step 6: Wire the components**

In `TrackTable.svelte`, import and pass the callback:

```ts
  import { copyText } from "../clipboard";
```

```ts
              addToPlaylist: () => onAddToPlaylist?.(t),
              copyTitle: () => void copyText(t.title),
```

In `QueueRail.svelte`, import and pass:

```ts
  import { copyText } from "../clipboard";
```

```svelte
            use:menuFor={() =>
              queueMenuItems({
                index,
                playNow: () => jump(index),
                copyTitle: () => void copyText(item.title),
                remove: () => player.remove(index),
              })}
```

- [ ] **Step 7: Run the UI tests and the type check**

Run: `npm --prefix ui test; npm --prefix ui run check`
Expected: all pass, 0 errors / 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/clipboard.ts ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/components/TrackTable.svelte ui/src/lib/components/QueueRail.svelte
git commit -m "feat(ui): add copy title to the track and queue menus"
```

---

### Task 6: Verification, smoke note, and docs

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-25-list-interactions-smoke.md`
- Modify: `docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md`

- [ ] **Step 1: Run every gate and record the output**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green. Record the exact counts.

- [ ] **Step 2: Mark the superseded hotfix check**

In `docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md`, replace the unowned-rename manual check with:

```markdown
- [ ] Rename or delete a playlist you do not own → superseded by the 2026-09-25 list-interactions smoke: the controls are now hidden; open one to confirm no edit UI appears.
```

- [ ] **Step 3: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-25-list-interactions-smoke.md`:

```markdown
# List Interactions and Ownership Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and the official
> TIDAL app for cross-checking. Run with `npm --prefix ui run app:dev` (or the
> installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Open an editorial/unowned playlist from Home or Search → no Rename/Delete in the header, no Move/Remove row buttons, rows are not draggable, and the row context menu has no Move/Remove items.
- [ ] Open a playlist you own → Rename/Delete, row editing, drag reorder, and menu items are all present and work.
- [ ] Library playlists show Rename/Delete for your own playlists and the row menu lists Open/Rename/Delete.
- [ ] Double-click a track row in Search, Album, Playlist, and Library favorites → playback starts at that row.
- [ ] Double-click on ♥, ＋, ⊕, ▶, or ✕ → no row playback is triggered by the double-click.
- [ ] Single-click ▶ still plays; text in rows cannot be selected (no blue flash on double-click).
- [ ] Queue: single-click anywhere on a row (except ✕ and the drag handle) starts that track; Enter/Space on a focused row starts it; Space does not toggle play/pause when a queue row is focused.
- [ ] Queue: ✕ still removes the row; dragging still reorders; right-click still opens Play now / Copy title / Remove.
- [ ] Copy title on a track row → paste elsewhere shows the track title.
- [ ] Copy title on a queue row → paste elsewhere shows the track title.
- [ ] If the clipboard does not receive text, note the WebView2 state; the fallback path may need the official clipboard plugin as a follow-up.

## Known caveats
- Ownership is creator-based: TIDAL collaborator playlists (editable but created by someone else) are treated as unowned and hide their edit controls.
- If TIDAL omits `creator` on a playlist fetch, the playlist is treated as unowned; on list endpoints the item stays owned.
- Copy title copies the raw title only (no artist).
```

- [ ] **Step 4: Fill in the gate counts and commit**

Replace each `<fill in>` with the Step 1 output and record the commit hash in the heading.

```bash
git add docs/superpowers/plans/2026-09-25-list-interactions-and-ownership.md docs/superpowers/plans/notes/2026-09-25-list-interactions-smoke.md docs/superpowers/plans/notes/2026-09-25-playlist-write-hotfix-smoke.md
git commit -m "docs: record the list interactions plan and smoke checklist"
```

---

## Self-Review

- **Requirement coverage:** ownership flag (Task 1) and gating across Playlist + Library, including drag/menu (Task 2); double-click on every TrackTable surface (Task 3); queue tap-to-play with keyboard support (Task 4); Copy title in both menus with a dependency-free clipboard helper (Task 5); verification/docs (Task 6).
- **Placeholder scan:** only the Task 6 checklist `<fill in>` markers, filled from the recorded run.
- **Type consistency:** `Playlist.owned`, `mark_owned(user_id, assume_owned)`, `libraryPlaylistMenuItems({ editable })`, `trackMenuItems({ copyTitle })`, `queueMenuItems({ copyTitle })`, and `copyText(text)` keep the same shapes across Rust, UI, and tests.
- **Review Focus coverage:** missing-creator rules (Task 1 tests), full unowned gating incl. drag/menu (Task 2 + smoke), double-click guards (Tasks 3/4 + smoke), Space propagation (Task 4), clipboard fallback (Task 5 + smoke).
