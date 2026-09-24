# Plan 8 — UX Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give OxiTide one toast surface, right-click menus, keyboard + mouse shortcuts, in-session back/forward history, and uniform loading/empty/error presentation, plus the polish Plan 5 and Plan 7 deferred.

**Architecture:** Five new pure modules (`errors.ts`, `toasts.svelte.ts`, `menu.svelte.ts`, `menus.ts`, `shortcuts.ts`) carry all logic and are unit-tested; Svelte components (`ToastHost`, `ContextMenu`, `Loading`, `EmptyState`) are thin renderers verified by `svelte-check` and the smoke checklist. `nav.svelte.ts` grows a route-history stack; `App.svelte` becomes the host for toasts, menus, and the global key/mouse handler. `notice.svelte.ts` is deleted and those callers take a `Toasts` dependency.

**Tech Stack:** Svelte 5 runes (`$state`, `$derived`, `$effect`, actions), TypeScript, vitest (node environment, fake timers). No Rust, no new crates, no new npm packages.

**Spec:** `docs/superpowers/specs/2026-09-25-oxitide-plan8-ux-foundations-design.md`

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`.
- No new crates and no new npm packages, including no DOM test environment; component behavior is covered by `svelte-check` and the smoke checklist.
- Version stays 0.2.0; no packaging or config changes.
- `verbatimModuleSyntax` is on: type-only imports must use `import type` (or inline `type` specifiers).
- Toast severity semantics: `success` = an action completed; `info` = neutral outcome ("3 added"); `error` = the action failed and nothing changed. Errors never auto-dismiss.
- Load failures render inline `ErrorState` with Retry; action outcomes render toasts.
- Shortcuts never fire while typing; `Space`/arrows call `preventDefault` only when handled; `Ctrl+F` always `preventDefault`s to suppress the WebView find bar.
- Menus mirror existing inline actions; menu items never introduce behavior the inline controls do not already have.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **Mouse side buttons:** one press can fire both `mousedown` and `auxclick`; the app must navigate exactly once. Task 7 adds a 50 ms guard; smoke verifies with Alt+←/→ as the fallback.
- **Esc while a dialog is open:** the dialog owns Escape; the shortcut host must not also close the queue rail. Task 7 tests `overlayOpen`; smoke checks with the picker open.
- **Shortcuts while typing:** Space and arrows in the Search input must edit text, never control playback or scroll. Task 7 tests the typing guard; smoke checks the live input.
- **Toast cap while an error is pinned:** when the stack is full, the oldest auto-dismissing toast must be evicted, not the error. Task 1 tests both eviction orders.
- **Context menu near a viewport edge:** the menu must clamp on-screen after mount rather than render clipped. Task 4 implements the clamp; smoke checks the four edges (no DOM unit test by design).

---

### Task 1: Error mapper and toast store

**Files:**
- Create: `ui/src/lib/errors.ts`
- Create: `ui/src/lib/errors.test.ts`
- Create: `ui/src/lib/toasts.svelte.ts`
- Create: `ui/src/lib/toasts.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `errorMessage(e: unknown): string`; `ToastSeverity = "success" | "info" | "error"`; `ToastAction { label: string; run(): void }`; `Toast { id: number; message: string; severity: ToastSeverity; action?: ToastAction }`; `Toasts { readonly toasts: readonly Toast[]; show(message, options?): number; dismiss(id): void }`; `createToasts(): Toasts`.

- [ ] **Step 1: Write the failing `errors` test**

Create `ui/src/lib/errors.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { errorMessage } from "./errors";

describe("errorMessage", () => {
  it("maps the forbidden backend string", () => {
    expect(errorMessage("forbidden")).toBe("You can only edit your own playlists");
  });

  it("maps the rate limited backend string", () => {
    expect(errorMessage("rate limited")).toBe(
      "TIDAL is rate-limiting requests — try again in a moment",
    );
  });

  it("passes other backend strings through", () => {
    expect(errorMessage("not found: /playlists/x")).toBe("not found: /playlists/x");
    expect(errorMessage("api error 500: boom")).toBe("api error 500: boom");
  });

  it("falls back to String for non-string errors", () => {
    expect(errorMessage(new Error("offline"))).toBe("Error: offline");
    expect(errorMessage(undefined)).toBe("undefined");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/errors.test.ts`
Expected: FAIL — cannot resolve `./errors`.

- [ ] **Step 3: Write the minimal implementation**

Create `ui/src/lib/errors.ts`:

```ts
/// Map the client's exact backend error strings (crates/oxitide-tidal/src/error.rs)
/// to user-facing copy; anything unrecognized falls through unchanged.
export function errorMessage(e: unknown): string {
  const raw = String(e);
  if (raw === "forbidden") return "You can only edit your own playlists";
  if (raw === "rate limited") {
    return "TIDAL is rate-limiting requests — try again in a moment";
  }
  return raw;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/errors.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Write the failing toast-store test**

Create `ui/src/lib/toasts.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createToasts } from "./toasts.svelte";

describe("toast store", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("defaults to info severity and returns an id", () => {
    const toasts = createToasts();

    const id = toasts.show("hello");

    expect(id).toBeGreaterThan(0);
    expect(toasts.toasts).toHaveLength(1);
    expect(toasts.toasts[0].severity).toBe("info");
  });

  it("auto-dismisses success and info after six seconds", () => {
    const toasts = createToasts();
    toasts.show("ok", { severity: "success" });
    toasts.show("fyi");

    vi.advanceTimersByTime(5999);
    expect(toasts.toasts).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(toasts.toasts).toHaveLength(0);
  });

  it("keeps errors until dismissed", () => {
    const toasts = createToasts();
    toasts.show("boom", { severity: "error" });

    vi.advanceTimersByTime(60_000);
    expect(toasts.toasts).toHaveLength(1);

    toasts.dismiss(toasts.toasts[0].id);
    expect(toasts.toasts).toHaveLength(0);
  });

  it("clears the timer on dismiss and leaves other timers running", () => {
    const toasts = createToasts();
    const first = toasts.show("one");
    toasts.show("two");

    toasts.dismiss(first);

    expect(vi.getTimerCount()).toBe(1);
    expect(toasts.toasts).toHaveLength(1);
  });

  it("caps at three, evicting the oldest auto-dismissing toast first", () => {
    const toasts = createToasts();
    toasts.show("one");
    toasts.show("two");
    toasts.show("boom", { severity: "error" });

    toasts.show("four");

    expect(toasts.toasts.map((t) => t.message)).toEqual(["two", "boom", "four"]);
  });

  it("evicts the oldest error when no auto-dismissing toast is present", () => {
    const toasts = createToasts();
    toasts.show("e1", { severity: "error" });
    toasts.show("e2", { severity: "error" });
    toasts.show("e3", { severity: "error" });

    toasts.show("e4", { severity: "error" });

    expect(toasts.toasts.map((t) => t.message)).toEqual(["e2", "e3", "e4"]);
  });

  it("retains the optional action", () => {
    const toasts = createToasts();
    const run = vi.fn();
    toasts.show("did it", { action: { label: "Undo", run } });

    expect(toasts.toasts[0].action?.label).toBe("Undo");
    toasts.toasts[0].action?.run();
    expect(run).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/toasts.test.ts`
Expected: FAIL — cannot resolve `./toasts.svelte`.

- [ ] **Step 7: Write the minimal implementation**

Create `ui/src/lib/toasts.svelte.ts`:

```ts
export type ToastSeverity = "success" | "info" | "error";

export interface ToastAction {
  label: string;
  run(): void;
}

export interface Toast {
  id: number;
  message: string;
  severity: ToastSeverity;
  action?: ToastAction;
}

export interface Toasts {
  readonly toasts: readonly Toast[];
  show(message: string, options?: { severity?: ToastSeverity; action?: ToastAction }): number;
  dismiss(id: number): void;
}

/// Auto-dismiss delay for success/info toasts; errors persist until dismissed.
const AUTO_DISMISS_MS = 6000;
/// Visible toasts after which the oldest is evicted.
const MAX_TOASTS = 3;

/// Outcome messages shown in the toast host. Errors never auto-dismiss; when
/// the cap is hit the oldest auto-dismissing toast is evicted before any error.
export function createToasts(): Toasts {
  let toasts = $state<Toast[]>([]);
  let nextId = 1;
  const timers = new Map<number, ReturnType<typeof setTimeout>>();

  function clearTimer(id: number) {
    const timer = timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.delete(id);
    }
  }

  function dismiss(id: number) {
    clearTimer(id);
    toasts = toasts.filter((t) => t.id !== id);
  }

  function startTimer(id: number) {
    timers.set(id, setTimeout(() => dismiss(id), AUTO_DISMISS_MS));
  }

  function evictOne() {
    const victim = toasts.find((t) => t.severity !== "error") ?? toasts[0];
    if (victim) dismiss(victim.id);
  }

  return {
    get toasts() {
      return toasts;
    },
    show(message, options = {}) {
      const severity = options.severity ?? "info";
      if (toasts.length >= MAX_TOASTS) evictOne();
      const id = nextId++;
      toasts = [...toasts, { id, message, severity, action: options.action }];
      if (severity !== "error") startTimer(id);
      return id;
    },
    dismiss,
  };
}
```

- [ ] **Step 8: Run the tests and the type check**

Run: `npm --prefix ui test -- src/lib/errors.test.ts src/lib/toasts.test.ts`
Expected: PASS (11 tests).

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/errors.ts ui/src/lib/errors.test.ts ui/src/lib/toasts.svelte.ts ui/src/lib/toasts.test.ts
git commit -m "feat(ui): add the toast store and backend error mapper"
```

---

### Task 2: Toast host, App wiring, and the favorites migration

**Files:**
- Create: `ui/src/lib/components/ToastHost.svelte`
- Modify: `ui/src/lib/favorites.svelte.ts`
- Modify: `ui/src/lib/favorites.test.ts`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `createToasts`, `Toasts`, `errorMessage` (Task 1).
- Produces: `createFavorites(toasts: Toasts): Favorites` (drops `error` and `dismissError`); `ToastHost` component taking `{ toasts: Toasts }`.

- [ ] **Step 1: Migrate the favorites tests to a fake `Toasts`**

In `ui/src/lib/favorites.test.ts`, replace the imports and add the fake helper after the `track` helper:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FavoriteIds, Track, TrackPage } from "./api";
import { api } from "./api";
import { createFavorites } from "./favorites.svelte";
import { clearPagerCache, createTrackPager } from "./pager.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

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

interface FakeToasts extends Toasts {
  readonly calls: { message: string; severity: ToastSeverity }[];
}

function fakeToasts(): FakeToasts {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  return {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
    get calls() {
      return calls;
    },
  };
}
```

Then make these replacements inside the file:

1. Replace every `createFavorites()` call with `createFavorites(fakeToasts())` (10 occurrences). The two error tests below capture the toast fake instead.
2. Replace the body of `"leaves the store hollow after a failed hydrate and retries on the next call"` with:

```ts
  it("leaves the store hollow after a failed hydrate and retries on the next call", async () => {
    vi.mocked(api.getFavoriteIds).mockRejectedValueOnce(new Error("offline"));
    const toasts = fakeToasts();
    const favorites = createFavorites(toasts);

    await favorites.hydrate();
    expect(favorites.hydrated).toBe(false);
    expect(toasts.calls).toEqual([{ message: "Error: offline", severity: "error" }]);

    vi.mocked(api.getFavoriteIds).mockResolvedValueOnce({ tracks: [1], albums: [], artists: [] });
    await favorites.hydrate();
    expect(favorites.hydrated).toBe(true);
    expect(favorites.hasTrack(1)).toBe(true);
    expect(api.getFavoriteIds).toHaveBeenCalledTimes(2);
  });
```

3. Replace the body of `"reverts a failed toggle and surfaces the error"` with:

```ts
  it("reverts a failed toggle and reports a mapped error toast", async () => {
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce("rate limited");
    const toasts = fakeToasts();
    const favorites = createFavorites(toasts);

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(false);
    expect(toasts.calls).toEqual([
      {
        message: "TIDAL is rate-limiting requests — try again in a moment",
        severity: "error",
      },
    ]);

    vi.mocked(api.getFavoriteIds).mockResolvedValue({ tracks: [5], albums: [], artists: [] });
    await favorites.hydrate();
    vi.mocked(api.setFavoriteTrack).mockRejectedValueOnce(new Error("boom"));

    expect(await favorites.toggleTrack(5)).toBeNull();
    expect(favorites.hasTrack(5)).toBe(true);
  });
```

4. In `"clears everything on reset"`, delete the line `expect(favorites.error).toBeNull();`.
5. In `"hydrates to empty sets without error"`, delete the line `expect(favorites.error).toBeNull();`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/favorites.test.ts`
Expected: FAIL — the fake-`Toasts` assertions see no calls (the store still owns `error`).

- [ ] **Step 3: Migrate the favorites store**

In `ui/src/lib/favorites.svelte.ts`:

Add below the existing imports:

```ts
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";
```

Change the interface to drop the error surface:

```ts
export interface Favorites {
  readonly hydrated: boolean;
  readonly trackRevision: number;
  hydrate(): Promise<void>;
  hasTrack(id: number): boolean;
  hasAlbum(id: number): boolean;
  hasArtist(id: number): boolean;
  /// Ids a mounted favorites list should still drop: unfavorited this session,
  /// not re-favorited since, and currently loaded.
  pendingRemovalsToDrop(loaded: ReadonlySet<number>): number[];
  toggleTrack(id: number): Promise<boolean | null>;
  toggleAlbum(id: number): Promise<boolean | null>;
  toggleArtist(id: number): Promise<boolean | null>;
  reset(): void;
}
```

Change the signature and remove the error state:

```ts
/// Favorite state for the signed-in account. Hydrated once per session from
/// `get_favorite_ids`; toggles are optimistic and revert on failure. Failures
/// surface as persistent error toasts.
export function createFavorites(toasts: Toasts): Favorites {
```

Delete `let error = $state<string | null>(null);`.

In `hydrate()`'s catch, replace `error = String(e);` with:

```ts
        toasts.show(errorMessage(e), { severity: "error" });
```

In `toggle()`, replace `error = null;` with nothing (delete the line), and in its catch replace `error = String(e);` with:

```ts
      toasts.show(errorMessage(e), { severity: "error" });
```

In the returned object, delete `get error() {...}` and the `dismissError` entry, and in `reset()` delete `error = null;`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/favorites.test.ts`
Expected: PASS.

- [ ] **Step 5: Create the toast host**

Create `ui/src/lib/components/ToastHost.svelte`:

```svelte
<script lang="ts">
  import type { Toasts } from "../toasts.svelte";
  let { toasts }: { toasts: Toasts } = $props();
</script>

{#if toasts.toasts.length > 0}
  <div class="toast-host" aria-live="polite">
    {#each toasts.toasts as toast (toast.id)}
      <div
        class="toast"
        class:error={toast.severity === "error"}
        class:success={toast.severity === "success"}
        role={toast.severity === "error" ? "alert" : undefined}
      >
        <span class="message">{toast.message}</span>
        {#if toast.action}
          <button
            onclick={() => {
              toast.action?.run();
              toasts.dismiss(toast.id);
            }}>{toast.action.label}</button>
        {/if}
        <button class="dismiss" onclick={() => toasts.dismiss(toast.id)} aria-label="Dismiss">✕</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-host {
    position: fixed;
    top: 12px;
    right: 12px;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(360px, 80vw);
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    box-shadow: 0 4px 12px rgb(0 0 0 / 0.35);
  }
  .toast.error {
    border-color: #ff6b6b;
    color: #ff6b6b;
    align-items: flex-start;
  }
  .toast.success {
    border-color: var(--accent);
  }
  .message {
    flex: 1;
    overflow-wrap: anywhere;
  }
  .dismiss {
    margin-left: auto;
  }
</style>
```

- [ ] **Step 6: Wire the toast store into App**

In `ui/src/App.svelte`:

Add the imports:

```ts
  import ToastHost from "./lib/components/ToastHost.svelte";
  import { createToasts } from "./lib/toasts.svelte";
```

Change the store construction at the top of `<script>`:

```ts
  const toasts = createToasts();
  const favorites = createFavorites(toasts);
```

Delete the favorites auto-dismiss effect entirely:

```ts
  $effect(() => {
    if (!favorites.error) return;
    const timer = setTimeout(() => favorites.dismissError(), 6000);
    return () => clearTimeout(timer);
  });
```

Delete the favorites strip from the main column:

```svelte
      {#if favorites.error}
        <div class="notice">
          <span>{favorites.error}</span>
          <button onclick={() => favorites.dismissError()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
```

Render the toast host once at the file root, after the closing `{/if}` of the auth branches and before `<style>`:

```svelte
<ToastHost {toasts} />
```

Keep the `notice` store, its strip, and the `.notice` style block for now; Task 3 removes them.

- [ ] **Step 7: Run the UI tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/components/ToastHost.svelte ui/src/lib/favorites.svelte.ts ui/src/lib/favorites.test.ts ui/src/App.svelte
git commit -m "feat(ui): add the toast host and migrate favorites to toasts"
```

---

### Task 3: Playlists migration and notice removal

**Files:**
- Modify: `ui/src/lib/playlists.svelte.ts`
- Modify: `ui/src/lib/playlists.test.ts`
- Modify: `ui/src/views/Playlist.svelte`
- Modify: `ui/src/views/Library.svelte`
- Modify: `ui/src/App.svelte`
- Delete: `ui/src/lib/notice.svelte.ts`, `ui/src/lib/notice.test.ts`

**Interfaces:**
- Consumes: `Toasts`, `errorMessage` (Tasks 1–2).
- Produces: `createPlaylistActions(toasts: Toasts): PlaylistActions`; `Playlist` and `Library` views take `{ toasts: Toasts }` instead of `{ notice: Notice }`.

- [ ] **Step 1: Migrate the playlist tests to a fake `Toasts`**

In `ui/src/lib/playlists.test.ts`, replace the imports and add the fake helper after the `playlist` helper:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import { api } from "./api";
import { createPlaylistActions } from "./playlists.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: {
    createPlaylist: vi.fn(),
    addPlaylistTracks: vi.fn(),
    getUserPlaylists: vi.fn(),
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

function playlist(uuid: string, title = "Mix"): Playlist {
  return { uuid, title, description: null, numberOfTracks: 0, duration: 0, image: null };
}

interface FakeToasts extends Toasts {
  readonly calls: { message: string; severity: ToastSeverity }[];
}

function fakeToasts(): FakeToasts {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  return {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
    get calls() {
      return calls;
    },
  };
}
```

Then make these changes:

1. Replace every `createPlaylistActions(createNotice())` with `createPlaylistActions(fakeToasts())` (3 occurrences: the "sends only the add-both choices", "apply-to-all", and "creates then adds" tests).
2. In `"closes and notices when nothing was skipped"` (rename it to `"closes and shows a success toast when nothing was skipped"`), replace `const notice = createNotice();` / `createPlaylistActions(notice)` with `const toasts = fakeToasts();` / `createPlaylistActions(toasts)`, and replace the two message assertions with:

```ts
    expect(toasts.calls).toEqual([{ message: "2 added to Chill", severity: "success" }]);
```

3. In `"keeps one by default and sends no second write"`, use `const toasts = fakeToasts();`, and replace the final assertion with:

```ts
    expect(toasts.calls).toEqual([{ message: "0 added, 1 kept one of", severity: "info" }]);
```

4. In `"dedupes add-both ids in phase two"`, use `const toasts = fakeToasts();`, and replace the two message assertions with:

```ts
    expect(toasts.calls).toEqual([{ message: "1 added, 0 kept one of", severity: "info" }]);
```

5. In `"surfaces a failed add and keeps the picker open with its pending tracks"`, use `const toasts = fakeToasts();`, and replace the message assertion with:

```ts
    expect(toasts.calls).toEqual([{ message: "Error: boom", severity: "error" }]);
```

6. In `"returns the created playlist even when the follow-up add fails"`, use `const toasts = fakeToasts();`, and replace the message assertion with:

```ts
    expect(toasts.calls).toEqual([{ message: "Error: nope", severity: "error" }]);
```

7. In `"creates then adds and bumps the revision"`, use `const toasts = fakeToasts();` and assert the success toast after the revision check:

```ts
    expect(toasts.calls).toEqual([{ message: "Created new with 2 tracks", severity: "success" }]);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/playlists.test.ts`
Expected: FAIL — the store still takes a `Notice` and records nothing in the fake.

- [ ] **Step 3: Migrate the playlist actions store**

In `ui/src/lib/playlists.svelte.ts`:

Replace the notice import with:

```ts
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";
```

Change the signature:

```ts
/// The add-to-playlist picker and playlist creation flow. Track edits
/// (remove/move) are direct API calls in the views; creation, the two-phase
/// duplicate flow, and reset-on-logout live here. Outcomes surface as toasts.
export function createPlaylistActions(toasts: Toasts): PlaylistActions {
```

In `addTo()`, replace the outcome lines:

```ts
      if (skipped.length === 0) {
        toasts.show(`${requested.length} added to ${playlist.title}`, { severity: "success" });
        close();
      } else {
```

and the catch:

```ts
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
```

In `confirmDuplicates()`, replace the success line and catch:

```ts
      toasts.show(
        `${addBoth.length} added, ${uniqueDuplicates.length - addBoth.length} kept one of`,
        { severity: "info" },
      );
      close();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
```

In `createAndAdd()`, replace the three outcome calls and two catches:

```ts
          await api.addPlaylistTracks(created.uuid, ids, false);
          toasts.show(`Created ${created.title} with ${ids.length} tracks`, {
            severity: "success",
          });
        } catch (e) {
          toasts.show(errorMessage(e), { severity: "error" });
        }
      } else {
        toasts.show(`Created ${created.title}`, { severity: "success" });
      }
      close();
      return created;
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
      return null;
    } finally {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/playlists.test.ts`
Expected: PASS.

- [ ] **Step 5: Migrate the Playlist and Library views**

In `ui/src/views/Playlist.svelte`:

Replace the notice import with:

```ts
  import { errorMessage } from "../lib/errors";
  import type { Toasts } from "../lib/toasts.svelte";
```

Replace the prop `notice` with `toasts` in both the destructuring and the type:

```ts
  let {
    nav,
    id,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    nav: Nav;
    id: string;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();
```

Update the four outcome sites:

```ts
      renaming = false;
      toasts.show("Playlist renamed", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
```

```ts
      await api.deletePlaylist(id);
      toasts.show("Playlist deleted", { severity: "success" });
      nav.goto({ name: "library" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
```

```ts
      await api.removePlaylistItems(id, [index]);
      await pager.refresh();
      toasts.show("Removed from playlist", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
```

In `moveAt()`, keep the silent success and change only the catch:

```ts
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
```

In `ui/src/views/Library.svelte`, make the same import and prop swaps, then update the outcome sites:

```ts
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
```

```ts
      playlists = playlists.map((item) => (item.uuid === p.uuid ? { ...item, title } : item));
      editingUuid = null;
      toasts.show("Playlist renamed", { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
```

```ts
      playlists = playlists.filter((item) => item.uuid !== target.uuid);
      toasts.show(`Deleted ${target.title}`, { severity: "success" });
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    } finally {
```

- [ ] **Step 6: Remove the notice store from App**

In `ui/src/App.svelte`:

Delete the import:

```ts
  import { createNotice } from "./lib/notice.svelte";
```

Delete the store and pass `toasts` to the playlist actions:

```ts
  const playlistActions = createPlaylistActions(toasts);
```

Delete the notice strip from the main column:

```svelte
      {#if notice.message}
        <div class="notice">
          <span>{notice.message}</span>
          <button onclick={() => notice.dismiss()} aria-label="Dismiss">✕</button>
        </div>
      {/if}
```

Pass `toasts` instead of `notice` on the two routes:

```svelte
      {:else if nav.route.name === "library"}<Library {nav} {player} {favorites} {playlistActions} {toasts} />
```

```svelte
      {:else if nav.route.name === "playlist"}<Playlist {nav} id={nav.route.id} {player} {favorites} {playlistActions} {toasts} />
```

Delete the now-unused `.notice` style block:

```css
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
```

- [ ] **Step 7: Delete the notice module and its test**

```bash
git rm ui/src/lib/notice.svelte.ts ui/src/lib/notice.test.ts
```

- [ ] **Step 8: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass (the notice suite no longer exists).

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/playlists.svelte.ts ui/src/lib/playlists.test.ts ui/src/views/Playlist.svelte ui/src/views/Library.svelte ui/src/App.svelte
git commit -m "feat(ui): migrate playlists to toasts and remove the notice store"
```

---

### Task 4: Menu store, item builders, and context-menu component

**Files:**
- Create: `ui/src/lib/menu.svelte.ts`
- Create: `ui/src/lib/menu.test.ts`
- Create: `ui/src/lib/menus.ts`
- Create: `ui/src/lib/menus.test.ts`
- Create: `ui/src/lib/components/ContextMenu.svelte`

**Interfaces:**
- Consumes: `Favorites` (Task 2), `Track`/`Playlist` from `./api`.
- Produces: `MenuItem { id, label, disabled?, danger?, run() }`; `MenuState { x, y, items }`; `Menu { open, show(x, y, items), close() }`; `createMenu(): Menu`; app-wide `menu: Menu`; `menuFor(node: HTMLElement, items: () => MenuItem[]): { destroy(): void }`; `trackMenuItems(ctx)`, `queueMenuItems(ctx)`, `libraryPlaylistMenuItems(ctx)`; `ContextMenu` taking `{ menu: Menu }`.

**Design note:** the spec's `menuFor(items)` signature has no place to receive a menu instance, so `menu.svelte.ts` exports one app-wide `menu` singleton; `menuFor` shows on it and `App.svelte` hosts it. Execution correction (Task 5): `menuFor` is a plain Svelte action `(node, items)` — Svelte invokes actions with `(node, param)`, so the curried-factory form does not type-check under `use:menuFor`. `createMenu()` stays exported so tests get isolated instances.

- [ ] **Step 1: Write the failing menu-store test**

Create `ui/src/lib/menu.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createMenu, type MenuItem } from "./menu.svelte";

function item(id: string, extra: Partial<MenuItem> = {}): MenuItem {
  return { id, label: id, run: vi.fn(), ...extra };
}

describe("menu store", () => {
  it("starts closed", () => {
    const menu = createMenu();

    expect(menu.open).toBeNull();
  });

  it("opens at the pointer with the given items", () => {
    const menu = createMenu();
    const items = [item("play")];

    menu.show(10, 20, items);

    expect(menu.open).toEqual({ x: 10, y: 20, items });
  });

  it("replaces the previous menu", () => {
    const menu = createMenu();
    menu.show(1, 2, [item("a")]);
    const next = [item("b")];

    menu.show(3, 4, next);

    expect(menu.open?.items).toBe(next);
    expect(menu.open?.x).toBe(3);
  });

  it("keeps disabled items in the list", () => {
    const menu = createMenu();
    const disabled = item("up", { disabled: true });

    menu.show(0, 0, [disabled, item("down")]);

    expect(menu.open?.items[0].disabled).toBe(true);
    expect(menu.open?.items).toHaveLength(2);
  });

  it("closes", () => {
    const menu = createMenu();
    menu.show(0, 0, [item("a")]);

    menu.close();

    expect(menu.open).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menu.test.ts`
Expected: FAIL — cannot resolve `./menu.svelte`.

- [ ] **Step 3: Implement the menu store and action**

Create `ui/src/lib/menu.svelte.ts`:

```ts
export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  danger?: boolean;
  run(): void;
}

export interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
}

export interface Menu {
  readonly open: MenuState | null;
  show(x: number, y: number, items: MenuItem[]): void;
  close(): void;
}

export function createMenu(): Menu {
  let open = $state<MenuState | null>(null);
  return {
    get open() {
      return open;
    },
    show(x, y, items) {
      open = { x, y, items };
    },
    close() {
      open = null;
    },
  };
}

/// One app-wide menu: rows across every view attach through `menuFor`, and
/// App renders the single ContextMenu host for this instance.
export const menu: Menu = createMenu();

/// Svelte action: `contextmenu` -> preventDefault + show at the pointer.
/// Svelte calls actions with `(node, param)`, so the item closure is the
/// action's parameter, not a curried factory argument.
export function menuFor(node: HTMLElement, items: () => MenuItem[]) {
  function onContextMenu(event: MouseEvent) {
    event.preventDefault();
    menu.show(event.clientX, event.clientY, items());
  }
  node.addEventListener("contextmenu", onContextMenu);
  return {
    destroy() {
      node.removeEventListener("contextmenu", onContextMenu);
    },
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/menu.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Write the failing menu-builder tests**

Create `ui/src/lib/menus.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import {
  libraryPlaylistMenuItems,
  queueMenuItems,
  trackMenuItems,
  type TrackMenuContext,
} from "./menus";

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

function playlist(): Playlist {
  return { uuid: "p", title: "Mix", description: null, numberOfTracks: 0, duration: 0, image: null };
}

function favorites(has: boolean): Favorites {
  return {
    hasTrack: () => has,
    toggleTrack: vi.fn(),
  } as unknown as Favorites;
}

function playlistCtx(overrides: { busy?: boolean; disabled?: boolean } = {}) {
  return {
    onRemove: vi.fn(),
    onMove: vi.fn(),
    busy: false,
    disabled: false,
    ...overrides,
  };
}

function baseCtx(overrides: Partial<TrackMenuContext> = {}): TrackMenuContext {
  return {
    track: track(1),
    index: 0,
    trackCount: 1,
    favorites: favorites(false),
    play: vi.fn(),
    addToQueue: vi.fn(),
    addToPlaylist: vi.fn(),
    ...overrides,
  };
}

describe("trackMenuItems", () => {
  it("lists the four base actions", () => {
    expect(trackMenuItems(baseCtx()).map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
    ]);
  });

  it("labels favorite according to state", () => {
    expect(trackMenuItems(baseCtx({ favorites: favorites(true) }))[3].label).toBe("Unfavorite");
    expect(trackMenuItems(baseCtx({ favorites: favorites(false) }))[3].label).toBe("Favorite");
  });

  it("invokes each callback once", () => {
    const ctx = baseCtx();
    for (const item of trackMenuItems(ctx)) item.run();

    expect(vi.mocked(ctx.play)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToQueue)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(ctx.addToPlaylist)).toHaveBeenCalledTimes(1);
  });

  it("appends playlist editing when the context is editable", () => {
    const items = trackMenuItems(
      baseCtx({ index: 1, trackCount: 3, playlist: playlistCtx() }),
    );

    expect(items.map((i) => i.id)).toEqual([
      "play",
      "add-queue",
      "add-playlist",
      "favorite",
      "move-up",
      "move-down",
      "remove",
    ]);
  });

  it("disables move up at the first row and move down at the last", () => {
    const first = trackMenuItems(baseCtx({ index: 0, trackCount: 3, playlist: playlistCtx() }));
    const last = trackMenuItems(baseCtx({ index: 2, trackCount: 3, playlist: playlistCtx() }));

    expect(first.find((i) => i.id === "move-up")?.disabled).toBe(true);
    expect(first.find((i) => i.id === "move-down")?.disabled).toBe(false);
    expect(last.find((i) => i.id === "move-down")?.disabled).toBe(true);
    expect(last.find((i) => i.id === "move-up")?.disabled).toBe(false);
  });

  it("omits editing items while busy or disabled", () => {
    const busy = trackMenuItems(baseCtx({ playlist: playlistCtx({ busy: true }) }));
    const disabled = trackMenuItems(baseCtx({ playlist: playlistCtx({ disabled: true }) }));

    expect(busy.map((i) => i.id)).not.toContain("remove");
    expect(disabled.map((i) => i.id)).not.toContain("remove");
  });

  it("calls onMove and onRemove with the row index", () => {
    const playlistActions = playlistCtx();
    const items = trackMenuItems(
      baseCtx({ index: 2, trackCount: 3, playlist: playlistActions }),
    );

    items.find((i) => i.id === "move-up")?.run();
    items.find((i) => i.id === "move-down")?.run();
    items.find((i) => i.id === "remove")?.run();

    expect(playlistActions.onMove).toHaveBeenCalledWith(2, -1);
    expect(playlistActions.onMove).toHaveBeenCalledWith(2, 1);
    expect(playlistActions.onRemove).toHaveBeenCalledWith(2);
  });
});

describe("queueMenuItems", () => {
  it("lists play now and remove and invokes them once", () => {
    const playNow = vi.fn();
    const remove = vi.fn();

    const items = queueMenuItems({ index: 0, playNow, remove });

    expect(items.map((i) => i.id)).toEqual(["play-now", "remove"]);
    for (const item of items) item.run();
    expect(playNow).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });
});

describe("libraryPlaylistMenuItems", () => {
  it("lists open, rename, and a danger delete and invokes them once", () => {
    const open = vi.fn();
    const rename = vi.fn();
    const remove = vi.fn();

    const items = libraryPlaylistMenuItems({ playlist: playlist(), open, rename, remove });

    expect(items.map((i) => i.id)).toEqual(["open", "rename", "delete"]);
    expect(items.find((i) => i.id === "delete")?.danger).toBe(true);
    for (const item of items) item.run();
    expect(open).toHaveBeenCalledTimes(1);
    expect(rename).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/menus.test.ts`
Expected: FAIL — cannot resolve `./menus`.

- [ ] **Step 7: Implement the pure item builders**

Create `ui/src/lib/menus.ts`:

```ts
import type { Playlist, Track } from "./api";
import type { Favorites } from "./favorites.svelte";
import type { MenuItem } from "./menu.svelte";

export interface TrackMenuContext {
  track: Track;
  index: number;
  trackCount: number;
  favorites: Favorites;
  play(): void | Promise<void>;
  addToQueue(): void;
  addToPlaylist(): void;
  playlist?: {
    onRemove(index: number): void;
    onMove(index: number, delta: -1 | 1): void;
    busy: boolean;
    disabled: boolean;
  };
}

/// Track row items. The playlist block is present only while the list is
/// editable; edge moves are disabled instead of hidden so the menu is stable.
export function trackMenuItems(ctx: TrackMenuContext): MenuItem[] {
  const items: MenuItem[] = [
    { id: "play", label: "Play", run: () => void ctx.play() },
    { id: "add-queue", label: "Add to queue", run: () => ctx.addToQueue() },
    { id: "add-playlist", label: "Add to playlist", run: () => ctx.addToPlaylist() },
    {
      id: "favorite",
      label: ctx.favorites.hasTrack(ctx.track.id) ? "Unfavorite" : "Favorite",
      run: () => void ctx.favorites.toggleTrack(ctx.track.id),
    },
  ];
  const playlist = ctx.playlist;
  if (playlist && !playlist.busy && !playlist.disabled) {
    items.push(
      {
        id: "move-up",
        label: "Move up",
        disabled: ctx.index === 0,
        run: () => playlist.onMove(ctx.index, -1),
      },
      {
        id: "move-down",
        label: "Move down",
        disabled: ctx.index === ctx.trackCount - 1,
        run: () => playlist.onMove(ctx.index, 1),
      },
      { id: "remove", label: "Remove", run: () => playlist.onRemove(ctx.index) },
    );
  }
  return items;
}

export function queueMenuItems(ctx: {
  index: number;
  playNow(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "play-now", label: "Play now", run: () => ctx.playNow() },
    { id: "remove", label: "Remove", run: () => ctx.remove() },
  ];
}

export function libraryPlaylistMenuItems(ctx: {
  playlist: Playlist;
  open(): void;
  rename(): void;
  remove(): void;
}): MenuItem[] {
  return [
    { id: "open", label: "Open", run: () => ctx.open() },
    { id: "rename", label: "Rename", run: () => ctx.rename() },
    { id: "delete", label: "Delete", danger: true, run: () => ctx.remove() },
  ];
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/menu.test.ts src/lib/menus.test.ts`
Expected: PASS (15 tests).

- [ ] **Step 9: Implement the context-menu component**

Create `ui/src/lib/components/ContextMenu.svelte`:

```svelte
<script lang="ts">
  import type { Menu, MenuItem } from "../menu.svelte";

  let { menu }: { menu: Menu } = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);
  let active = $state(-1);

  function enabledIndexes(items: MenuItem[]): number[] {
    return items.map((item, index) => (item.disabled ? -1 : index)).filter((i) => i >= 0);
  }

  function clampToViewport() {
    const node = panel;
    const state = menu.open;
    if (!node || !state) return;
    const rect = node.getBoundingClientRect();
    left = Math.max(4, Math.min(state.x, window.innerWidth - rect.width - 4));
    top = Math.max(4, Math.min(state.y, window.innerHeight - rect.height - 4));
  }

  function moveActive(delta: -1 | 1) {
    const state = menu.open;
    if (!state) return;
    const enabled = enabledIndexes(state.items);
    if (enabled.length === 0) return;
    const current = enabled.indexOf(active);
    const next = current === -1 ? 0 : (current + delta + enabled.length) % enabled.length;
    active = enabled[next];
  }

  function run(item: MenuItem) {
    if (item.disabled) return;
    menu.close();
    item.run();
  }

  // Cursor reset plus a viewport clamp after the panel has measurable size.
  $effect(() => {
    const state = menu.open;
    if (!state) return;
    const enabled = enabledIndexes(state.items);
    active = enabled.length > 0 ? enabled[0] : -1;
    clampToViewport();
  });

  // Focus the panel so arrows/Enter/Escape stay inside the menu.
  $effect(() => {
    if (menu.open && panel) panel.focus();
  });

  // Close when anything scrolls, including nested scrollers.
  $effect(() => {
    if (!menu.open) return;
    const close = () => menu.close();
    window.addEventListener("scroll", close, true);
    return () => window.removeEventListener("scroll", close, true);
  });

  function onKeydown(event: KeyboardEvent) {
    const state = menu.open;
    if (!state) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      moveActive(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const item = active >= 0 ? state.items[active] : undefined;
      if (item) run(item);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      menu.close();
    }
  }

  function onWindowMouseDown(event: MouseEvent) {
    if (!menu.open) return;
    if (panel && event.target instanceof Node && panel.contains(event.target)) return;
    menu.close();
  }
</script>

<svelte:window onmousedown={onWindowMouseDown} onresize={() => menu.close()} />

{#if menu.open}
  <div
    bind:this={panel}
    class="menu"
    role="menu"
    tabindex="-1"
    style="left:{left}px;top:{top}px"
    onkeydown={onKeydown}
  >
    {#each menu.open.items as item, i (item.id)}
      <button
        type="button"
        role="menuitem"
        class:active={active === i}
        class:danger={item.danger}
        disabled={item.disabled}
        onclick={() => run(item)}
      >{item.label}</button>
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
    outline: none;
  }
  .menu button {
    text-align: left;
    padding: 6px 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .menu button.active:not(:disabled) {
    background: var(--panel-2);
  }
  .menu button.danger {
    color: #ff6b6b;
  }
  .menu button:disabled {
    color: var(--muted);
    cursor: default;
  }
</style>
```

- [ ] **Step 10: Run the type check**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 11: Commit**

```bash
git add ui/src/lib/menu.svelte.ts ui/src/lib/menu.test.ts ui/src/lib/menus.ts ui/src/lib/menus.test.ts ui/src/lib/components/ContextMenu.svelte
git commit -m "feat(ui): add the context menu store, builders, and host"
```

---

### Task 5: Wire context menus into tracks, the queue, and library playlists

**Files:**
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/lib/components/QueueRail.svelte`
- Modify: `ui/src/views/Library.svelte`

**Interfaces:**
- Consumes: `menu`, `menuFor`, `trackMenuItems`, `queueMenuItems`, `libraryPlaylistMenuItems`, `ContextMenu` (Task 4).
- Produces: right-click menus on TrackTable rows, QueueRail items, and Library playlist rows; inline buttons unchanged.

- [ ] **Step 1: Host the menu in App**

In `ui/src/App.svelte`, add the imports:

```ts
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import { menu } from "./lib/menu.svelte";
```

Render it beside the toast host at the file root:

```svelte
<ToastHost {toasts} />
<ContextMenu {menu} />
```

- [ ] **Step 2: Attach the row menu in TrackTable**

In `ui/src/lib/components/TrackTable.svelte`, add the imports:

```ts
  import { menuFor } from "../menu.svelte";
  import { trackMenuItems } from "../menus";
```

Replace the row opening tag:

```svelte
        <tr class="row">
```

with:

```svelte
        <tr
          class="row"
          use:menuFor={() =>
            trackMenuItems({
              track: t,
              index,
              trackCount: tracks.length,
              favorites,
              play: () => play(index),
              addToQueue: () => api.queueAdd([trackToQueueItem(t)]),
              addToPlaylist: () => onAddToPlaylist?.(t),
              playlist: playlistActions,
            })}
        >
```

- [ ] **Step 3: Attach the item menu in QueueRail**

In `ui/src/lib/components/QueueRail.svelte`, add the imports:

```ts
  import { menuFor } from "../menu.svelte";
  import { queueMenuItems } from "../menus";
```

Replace the `<li>` opening tag:

```svelte
      <li
        draggable="true"
```

with:

```svelte
      <li
        draggable="true"
        use:menuFor={() =>
          queueMenuItems({ index: i, playNow: () => jump(i), remove: () => player.remove(i) })}
```

- [ ] **Step 4: Attach the playlist menu in Library**

In `ui/src/views/Library.svelte`, add the imports:

```ts
  import { menuFor } from "../lib/menu.svelte";
  import { libraryPlaylistMenuItems } from "../lib/menus";
```

Replace the playlist row opening tag:

```svelte
      <li>
        {#if editingUuid === p.uuid}
```

with:

```svelte
      <li
        use:menuFor={() =>
          libraryPlaylistMenuItems({
            playlist: p,
            open: () => nav.goto({ name: "playlist", id: p.uuid }),
            rename: () => startRename(p),
            remove: () => (deleting = p),
          })}
      >
        {#if editingUuid === p.uuid}
```

- [ ] **Step 5: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add ui/src/App.svelte ui/src/lib/components/TrackTable.svelte ui/src/lib/components/QueueRail.svelte ui/src/views/Library.svelte
git commit -m "feat(ui): add context menus to tracks, the queue, and library playlists"
```

---

### Task 6: Nav history and Sidebar buttons

**Files:**
- Modify: `ui/src/lib/nav.svelte.ts`
- Create: `ui/src/lib/nav.test.ts`
- Modify: `ui/src/lib/components/Sidebar.svelte`

**Interfaces:**
- Consumes: nothing new.
- Produces: `Nav` gains `canBack`, `canForward`, `back()`, `forward()`; `createNav()` keeps `route`, `queueOpen`, `goto`, `toggleQueue`.

- [ ] **Step 1: Write the failing navigation test**

Create `ui/src/lib/nav.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createNav } from "./nav.svelte";

describe("nav history", () => {
  it("starts at home with no history", () => {
    const nav = createNav();

    expect(nav.route).toEqual({ name: "home" });
    expect(nav.canBack).toBe(false);
    expect(nav.canForward).toBe(false);
  });

  it("ignores a goto to the current route", () => {
    const nav = createNav();

    nav.goto({ name: "home" });

    expect(nav.canBack).toBe(false);
  });

  it("ignores a goto to the same album id", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });

    nav.goto({ name: "album", id: 1 });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("treats the same name with a different id as a new entry", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });
    nav.goto({ name: "album", id: 2 });

    nav.back();

    expect(nav.route).toEqual({ name: "album", id: 1 });
  });

  it("truncates forward entries on a new navigation", () => {
    const nav = createNav();
    nav.goto({ name: "search" });
    nav.goto({ name: "library" });
    nav.back();
    expect(nav.canForward).toBe(true);

    nav.goto({ name: "settings" });

    expect(nav.canForward).toBe(false);
    expect(nav.route).toEqual({ name: "settings" });
  });

  it("clamps back and forward at the ends", () => {
    const nav = createNav();
    nav.goto({ name: "search" });

    nav.back();
    nav.back();
    expect(nav.route).toEqual({ name: "home" });

    nav.forward();
    nav.forward();
    expect(nav.route).toEqual({ name: "search" });
  });

  it("keeps the queue toggle out of history", () => {
    const nav = createNav();

    nav.toggleQueue();

    expect(nav.queueOpen).toBe(true);
    expect(nav.canBack).toBe(false);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: FAIL — `canBack` and `back` do not exist.

- [ ] **Step 3: Implement the history stack**

Replace the whole body of `ui/src/lib/nav.svelte.ts`:

```ts
export type Route =
  | { name: "home" }
  | { name: "search" }
  | { name: "library" }
  | { name: "album"; id: number }
  | { name: "playlist"; id: string }
  | { name: "artist"; id: number }
  | { name: "settings" };

export interface Nav {
  readonly route: Route;
  readonly queueOpen: boolean;
  readonly canBack: boolean;
  readonly canForward: boolean;
  goto(next: Route): void;
  back(): void;
  forward(): void;
  toggleQueue(): void;
}

function sameRoute(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  if ("id" in a && "id" in b) return a.id === b.id;
  return true;
}

/// In-session route history. `goto` on the current route is a no-op; any new
/// navigation truncates forward entries. Queue visibility is orthogonal.
export function createNav(): Nav {
  let history = $state<Route[]>([{ name: "home" }]);
  let index = $state(0);
  let queueOpen = $state(false);

  function goto(next: Route) {
    if (sameRoute(history[index], next)) return;
    history = [...history.slice(0, index + 1), next];
    index = history.length - 1;
  }

  function back() {
    if (index > 0) index -= 1;
  }

  function forward() {
    if (index < history.length - 1) index += 1;
  }

  return {
    get route() {
      return history[index];
    },
    get queueOpen() {
      return queueOpen;
    },
    get canBack() {
      return index > 0;
    },
    get canForward() {
      return index < history.length - 1;
    },
    goto,
    back,
    forward,
    toggleQueue() {
      queueOpen = !queueOpen;
    },
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Add the Sidebar buttons**

Replace the top of `ui/src/lib/components/Sidebar.svelte`:

```svelte
<nav class="sidebar">
  <h1 style="font-size:16px">OxiTide</h1>
  <div class="history">
    <button disabled={!nav.canBack} onclick={() => nav.back()} title="Back (Alt+←)" aria-label="Back">‹</button>
    <button disabled={!nav.canForward} onclick={() => nav.forward()} title="Forward (Alt+→)" aria-label="Forward">›</button>
  </div>
  <button onclick={() => nav.goto({ name: "home" })}>Home</button>
```

Keep the rest of the file unchanged, and add below the closing `</nav>`:

```svelte
<style>
  .history {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
</style>
```

- [ ] **Step 6: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts ui/src/lib/components/Sidebar.svelte
git commit -m "feat(ui): add in-session navigation history and sidebar controls"
```

---

### Task 7: Keyboard shortcuts, mouse side buttons, and the Settings list

**Files:**
- Create: `ui/src/lib/shortcuts.ts`
- Create: `ui/src/lib/shortcuts.test.ts`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/views/Search.svelte`
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: `menu` (Task 4), `nav` history (Task 6), `player`, `Toasts` not needed here.
- Produces: `ShortcutAction`; `isTypingTarget(target): boolean`; `matchShortcut(event, { overlayOpen }): ShortcutAction | null`; `ShortcutHint { keys, description }`; `SHORTCUTS`; App-level `keydown`/`mousedown`/`auxclick` host; `#search-input`; the Settings shortcuts list.

- [ ] **Step 1: Write the failing shortcuts test**

Create `ui/src/lib/shortcuts.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { isTypingTarget, matchShortcut, SHORTCUTS } from "./shortcuts";

function keyEvent(key: string, overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    key,
    repeat: false,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    target: null,
    ...overrides,
  } as unknown as KeyboardEvent;
}

function target(tagName: string, isContentEditable = false): EventTarget {
  return { tagName, isContentEditable } as unknown as EventTarget;
}

const app = { overlayOpen: false };

describe("matchShortcut", () => {
  it("maps every documented key", () => {
    expect(matchShortcut(keyEvent(" "), app)).toBe("play-pause");
    expect(matchShortcut(keyEvent("ArrowLeft"), app)).toBe("seek-back");
    expect(matchShortcut(keyEvent("ArrowRight"), app)).toBe("seek-forward");
    expect(matchShortcut(keyEvent("ArrowLeft", { shiftKey: true }), app)).toBe("prev-track");
    expect(matchShortcut(keyEvent("ArrowRight", { shiftKey: true }), app)).toBe("next-track");
    expect(matchShortcut(keyEvent("ArrowUp"), app)).toBe("volume-up");
    expect(matchShortcut(keyEvent("ArrowDown"), app)).toBe("volume-down");
    expect(matchShortcut(keyEvent("f", { ctrlKey: true }), app)).toBe("focus-search");
    expect(matchShortcut(keyEvent("/"), app)).toBe("focus-search");
    expect(matchShortcut(keyEvent("ArrowLeft", { altKey: true }), app)).toBe("history-back");
    expect(matchShortcut(keyEvent("ArrowRight", { altKey: true }), app)).toBe("history-forward");
    expect(matchShortcut(keyEvent("Escape"), app)).toBe("escape");
  });

  it("ignores shortcuts while typing except Esc and Ctrl+F", () => {
    const input = target("INPUT");

    expect(matchShortcut(keyEvent(" ", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowUp", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowLeft", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("/", { target: input }), app)).toBeNull();
    expect(matchShortcut(keyEvent("Escape", { target: input }), app)).toBe("escape");
    expect(matchShortcut(keyEvent("f", { ctrlKey: true, target: input }), app)).toBe(
      "focus-search",
    );
  });

  it("ignores a held Space repeat but allows seek and volume repeats", () => {
    expect(matchShortcut(keyEvent(" ", { repeat: true }), app)).toBeNull();
    expect(matchShortcut(keyEvent("ArrowRight", { repeat: true }), app)).toBe("seek-forward");
    expect(matchShortcut(keyEvent("ArrowUp", { repeat: true }), app)).toBe("volume-up");
  });

  it("leaves Space on a focused button to the button", () => {
    expect(matchShortcut(keyEvent(" ", { target: target("BUTTON") }), app)).toBeNull();
  });

  it("skips Esc when a dialog owns it", () => {
    expect(matchShortcut(keyEvent("Escape"), { overlayOpen: true })).toBeNull();
  });
});

describe("isTypingTarget", () => {
  it("accepts inputs, textareas, selects, and contenteditable", () => {
    expect(isTypingTarget(target("INPUT"))).toBe(true);
    expect(isTypingTarget(target("TEXTAREA"))).toBe(true);
    expect(isTypingTarget(target("SELECT"))).toBe(true);
    expect(isTypingTarget(target("DIV", true))).toBe(true);
  });

  it("rejects everything else", () => {
    expect(isTypingTarget(null)).toBe(false);
    expect(isTypingTarget(target("DIV"))).toBe(false);
    expect(isTypingTarget(target("BUTTON"))).toBe(false);
  });
});

describe("SHORTCUTS", () => {
  it("documents the seven rows shown in Settings", () => {
    expect(SHORTCUTS).toHaveLength(7);
    for (const hint of SHORTCUTS) {
      expect(hint.keys.length).toBeGreaterThan(0);
      expect(hint.description.length).toBeGreaterThan(0);
    }
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/shortcuts.test.ts`
Expected: FAIL — cannot resolve `./shortcuts`.

- [ ] **Step 3: Implement the shortcut matcher**

Create `ui/src/lib/shortcuts.ts`:

```ts
export type ShortcutAction =
  | "play-pause"
  | "seek-back"
  | "seek-forward"
  | "prev-track"
  | "next-track"
  | "volume-up"
  | "volume-down"
  | "focus-search"
  | "history-back"
  | "history-forward"
  | "escape";

export interface ShortcutHint {
  keys: string;
  description: string;
}

export const SHORTCUTS: ShortcutHint[] = [
  { keys: "Space", description: "Play / pause" },
  { keys: "← / →", description: "Seek −5 s / +5 s" },
  { keys: "Shift+← / Shift+→", description: "Previous / next track" },
  { keys: "↑ / ↓", description: "Volume +5% / −5%" },
  { keys: "Ctrl+F or /", description: "Focus search" },
  { keys: "Alt+← / Alt+→", description: "History back / forward" },
  { keys: "Esc", description: "Close the menu, then the queue" },
];

/// Duck-typed so the matcher stays testable without a DOM environment.
export function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as { tagName?: unknown; isContentEditable?: unknown } | null;
  if (!el || typeof el.tagName !== "string") return false;
  return (
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT" ||
    el.isContentEditable === true
  );
}

function isButtonLike(target: EventTarget | null): boolean {
  const el = target as { tagName?: unknown } | null;
  return el?.tagName === "BUTTON";
}

/// Map a keydown to an app action. Esc and Ctrl+F work while typing; every
/// other binding is ignored then. Esc is skipped entirely when a dialog is
/// open because the dialog owns it.
export function matchShortcut(
  event: KeyboardEvent,
  opts: { overlayOpen: boolean },
): ShortcutAction | null {
  const key = event.key;

  if (key === "Escape") return opts.overlayOpen ? null : "escape";
  if ((key === "f" || key === "F") && (event.ctrlKey || event.metaKey)) return "focus-search";
  if (isTypingTarget(event.target)) return null;

  if (key === " ") {
    if (event.repeat || isButtonLike(event.target)) return null;
    return "play-pause";
  }
  if (event.altKey && key === "ArrowLeft") return "history-back";
  if (event.altKey && key === "ArrowRight") return "history-forward";
  if (event.shiftKey && key === "ArrowLeft") return "prev-track";
  if (event.shiftKey && key === "ArrowRight") return "next-track";
  if (!event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
    if (key === "ArrowLeft") return "seek-back";
    if (key === "ArrowRight") return "seek-forward";
    if (key === "ArrowUp") return "volume-up";
    if (key === "ArrowDown") return "volume-down";
  }
  if (key === "/" && !event.altKey && !event.ctrlKey && !event.metaKey) return "focus-search";
  return null;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/shortcuts.test.ts`
Expected: PASS (8 tests).

- [ ] **Step 5: Add the App-level host**

In `ui/src/App.svelte`, extend the imports:

```ts
  import { tick } from "svelte";
  import { matchShortcut, type ShortcutAction } from "./lib/shortcuts";
```

Add the handlers after `onLogout`:

```ts
  /// Side buttons can fire both mousedown and auxclick for one press; ignore
  /// repeats inside this window so navigation happens exactly once.
  let sideButtonAt = 0;

  async function focusSearch() {
    if (nav.route.name !== "search") nav.goto({ name: "search" });
    await tick();
    document.getElementById("search-input")?.focus();
  }

  function runShortcut(action: ShortcutAction) {
    const state = player.state;
    switch (action) {
      case "play-pause":
        if (state.current) player.playPause();
        break;
      case "seek-back":
        if (state.current) player.seek(Math.max(0, state.position_ms - 5000));
        break;
      case "seek-forward": {
        if (!state.current) break;
        const limit = state.duration_ms > 0 ? state.duration_ms : state.position_ms + 5000;
        player.seek(Math.min(limit, state.position_ms + 5000));
        break;
      }
      case "prev-track":
        if (state.queue.length > 0) player.prev();
        break;
      case "next-track":
        if (state.queue.length > 0) player.next();
        break;
      case "volume-up":
        player.setVolume(Math.min(1, state.volume + 0.05));
        break;
      case "volume-down":
        player.setVolume(Math.max(0, state.volume - 0.05));
        break;
      case "focus-search":
        void focusSearch();
        break;
      case "history-back":
        nav.back();
        break;
      case "history-forward":
        nav.forward();
        break;
      case "escape":
        if (menu.open) menu.close();
        else if (nav.queueOpen) nav.toggleQueue();
        break;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (!loggedIn) return;
    const overlayOpen = document.querySelector('[role="dialog"]') !== null;
    const action = matchShortcut(event, { overlayOpen });
    if (!action) return;
    if (action === "escape" && !menu.open && !nav.queueOpen) return;
    event.preventDefault();
    runShortcut(action);
  }

  function onSideButton(event: MouseEvent) {
    if (!loggedIn) return;
    if (event.button !== 3 && event.button !== 4) return;
    event.preventDefault();
    const now = Date.now();
    if (now - sideButtonAt < 50) return;
    sideButtonAt = now;
    if (event.button === 3) nav.back();
    else nav.forward();
  }
```

Execution correction (Task 7): `<svelte:window>` cannot be nested inside elements or blocks, so render it at the file root beside the other hosts; the `loggedIn` guards above keep shortcuts inert on the login screen:

```svelte
<svelte:window onkeydown={onKeydown} onmousedown={onSideButton} onauxclick={onSideButton} />
<ToastHost {toasts} />
<ContextMenu {menu} />
```

- [ ] **Step 6: Give the Search input its id**

In `ui/src/views/Search.svelte`, replace the input:

```svelte
<input bind:value={query} onkeydown={(e) => e.key === "Enter" && load()} placeholder="Search TIDAL" />
```

with:

```svelte
<input
  id="search-input"
  bind:value={query}
  onkeydown={(e) => e.key === "Enter" && load()}
  placeholder="Search TIDAL"
/>
```

- [ ] **Step 7: Render the shortcuts list in Settings**

In `ui/src/views/Settings.svelte`, add the import:

```ts
  import { SHORTCUTS } from "../lib/shortcuts";
```

Add the section before `{#if player.state.bit_perfect_notice}`:

```svelte
<h3>Keyboard shortcuts</h3>
<ul>
  {#each SHORTCUTS as shortcut (shortcut.keys)}
    <li><kbd>{shortcut.keys}</kbd> <span class="muted">{shortcut.description}</span></li>
  {/each}
</ul>
```

- [ ] **Step 8: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/shortcuts.ts ui/src/lib/shortcuts.test.ts ui/src/App.svelte ui/src/views/Search.svelte ui/src/views/Settings.svelte
git commit -m "feat(ui): add keyboard shortcuts, mouse side buttons, and the settings list"
```

---

### Task 8: Uniform loading, empty, and error presentation

**Files:**
- Create: `ui/src/lib/components/Loading.svelte`
- Create: `ui/src/lib/components/EmptyState.svelte`
- Modify: `ui/src/views/Home.svelte`
- Modify: `ui/src/views/Search.svelte`
- Modify: `ui/src/views/Album.svelte`
- Modify: `ui/src/views/Artist.svelte`
- Modify: `ui/src/views/Playlist.svelte`
- Modify: `ui/src/views/Library.svelte`
- Modify: `ui/src/views/Settings.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `errorMessage`, `Toasts` (Tasks 1–3).
- Produces: `Loading` taking `{ label?: string }` (default `"Loading…"`); `EmptyState` taking `{ message: string; action?: { label: string; run(): void } }`; Settings takes `{ toasts: Toasts }`.

- [ ] **Step 1: Create the two state components**

Create `ui/src/lib/components/Loading.svelte`:

```svelte
<script lang="ts">
  let { label = "Loading…" }: { label?: string } = $props();
</script>

<span class="loading" role="status">
  <span class="spinner" aria-hidden="true"></span>
  {label}
</span>

<style>
  .loading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--line);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
```

Create `ui/src/lib/components/EmptyState.svelte`:

```svelte
<script lang="ts">
  let {
    message,
    action,
  }: { message: string; action?: { label: string; run(): void } } = $props();
</script>

<div class="empty-state">
  <p class="muted">{message}</p>
  {#if action}<button onclick={action.run}>{action.label}</button>{/if}
</div>
```

- [ ] **Step 2: Adopt them in Home**

In `ui/src/views/Home.svelte`, add the imports:

```ts
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Loading from "../lib/components/Loading.svelte";
```

Replace the states:

```svelte
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if sections.length === 0}
  <EmptyState message="Nothing to show yet." />
{:else}
```

- [ ] **Step 3: Adopt them in Search and add the no-results state**

In `ui/src/views/Search.svelte`, add the imports:

```ts
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Loading from "../lib/components/Loading.svelte";
```

Add the searched-query state and the empty derivation after `let error = $state("");`:

```ts
  let searchedQuery = $state("");

  const noResults = $derived(
    results !== null &&
      results.tracks.items.length === 0 &&
      results.albums.items.length === 0 &&
      results.artists.items.length === 0 &&
      results.playlists.items.length === 0,
  );
```

In `load()`, record the query that produced the results:

```ts
    try {
      results = await api.search(query);
      searchedQuery = query;
    } catch (e) {
```

Replace the states:

```svelte
{#if loading}
  <Loading label="Searching…" />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if noResults}
  <EmptyState message={`No results for “${searchedQuery}”`} />
{:else if results}
```

- [ ] **Step 4: Adopt them in Album and Artist**

In `ui/src/views/Album.svelte`, add the imports:

```ts
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Loading from "../lib/components/Loading.svelte";
```

Replace the opening block:

```svelte
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if album}
  {@const albumId = album.id}
```

Delete the old inner `{#if album}` wrapper and its closing `{/if}` so the album content renders directly under the `{:else if album}` branch, and end the block with:

```svelte
{:else}
  <EmptyState message="Album not found." />
{/if}
```

In `ui/src/views/Artist.svelte`, add the imports:

```ts
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Loading from "../lib/components/Loading.svelte";
```

Replace the states:

```svelte
{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
```

and append the empty branch at the end:

```svelte
{:else}
  <EmptyState message="Artist not found." />
{/if}
```

- [ ] **Step 5: Add the pager count spinner in Playlist and Library**

In `ui/src/views/Playlist.svelte`, add the import:

```ts
  import Loading from "../lib/components/Loading.svelte";
```

Replace the header loading state:

```svelte
{#if loading}
  <Loading />
{:else if error}
```

Put the spinner beside the count, replacing the count block:

```svelte
  <p>
    <button onclick={() => pager.refresh()}>Refresh</button>
    <span class="muted">
      {#if pager.unavailable > 0}
        {pager.items.length} tracks ({pager.unavailable} unavailable)
      {:else}
        {pager.items.length} tracks
      {/if}
    </span>
    {#if pager.loading}<Loading />{/if}
  </p>
```

Delete the trailing loading paragraph:

```svelte
  {#if pager.loading}<p class="muted">Loading…</p>{/if}
```

In `ui/src/views/Library.svelte`, add the import and make the same two changes for `favoritesPager`: `<Loading />` for the view loading state, the spinner beside `{favoritesPager.items.length} tracks`, and delete `{#if favoritesPager.loading}<p class="muted">Loading…</p>{/if}`.

- [ ] **Step 6: Move Settings apply failures to toasts**

In `ui/src/views/Settings.svelte`, add the imports:

```ts
  import Loading from "../lib/components/Loading.svelte";
  import { errorMessage } from "../lib/errors";
  import type { Toasts } from "../lib/toasts.svelte";
```

Extend the props:

```ts
  let {
    player,
    toasts,
    onQualityDisplay,
  }: { player: Player; toasts: Toasts; onQualityDisplay: (mode: QualityDisplay) => void } =
    $props();
```

Change `apply` and keep the load failure inline:

```ts
  function apply(failure: unknown) {
    toasts.show(errorMessage(failure), { severity: "error" });
  }
```

In `load()`, replace `apply(e);` in the catch with:

```ts
      error = String(e);
```

Replace the loading state:

```svelte
{#if !config}
  {#if error}
    <ErrorState message={error} onRetry={load} />
  {:else}
    <Loading />
  {/if}
```

Delete the post-load bare error line:

```svelte
  {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
```

- [ ] **Step 7: Pass toasts from App**

In `ui/src/App.svelte`, add the import:

```ts
  import { errorMessage } from "./lib/errors";
```

Update the Settings route:

```svelte
      {:else if nav.route.name === "settings"}<Settings {player} {toasts} onQualityDisplay={(mode) => (qualityDisplay = mode)} />{/if}
```

Route logout failures to a toast (the host renders outside the auth branches, so it stays visible on the login screen):

```ts
  async function onLogout() {
    try {
      await api.authLogout();
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
    clearPagerCache();
    favorites.reset();
    playlistActions.reset();
    loggedIn = false;
  }
```

- [ ] **Step 8: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add ui/src/lib/components/Loading.svelte ui/src/lib/components/EmptyState.svelte ui/src/views/Home.svelte ui/src/views/Search.svelte ui/src/views/Album.svelte ui/src/views/Artist.svelte ui/src/views/Playlist.svelte ui/src/views/Library.svelte ui/src/views/Settings.svelte ui/src/App.svelte
git commit -m "feat(ui): unify loading, empty, and error presentation"
```

---

### Task 9: Modal focus management and picker polish

**Files:**
- Modify: `ui/src/lib/components/Modal.svelte`
- Modify: `ui/src/lib/components/PlaylistPicker.svelte`

**Interfaces:**
- Consumes: nothing new.
- Produces: `Modal` keeps `{ title, onclose, children }` and gains `aria-modal`, first-focusable focus on open, focus restoration on close. `PlaylistPicker` autofocuses the name field and submits on Enter.

- [ ] **Step 1: Add focus management to Modal**

Replace the `<script>` of `ui/src/lib/components/Modal.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    title,
    onclose,
    children,
  }: { title: string; onclose: () => void; children: Snippet } = $props();

  let panel = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const node = panel;
    if (!node) return;
    const previous = document.activeElement as HTMLElement | null;
    const first = node.querySelector<HTMLElement>(
      'input, select, textarea, button, [href], [tabindex]:not([tabindex="-1"])',
    );
    first?.focus();
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  });
</script>
```

Add `aria-modal` and the element binding to the panel:

```svelte
  <div class="panel" role="dialog" aria-modal="true" aria-label={title} bind:this={panel}>
```

- [ ] **Step 2: Autofocus and Enter-submit in PlaylistPicker**

In `ui/src/lib/components/PlaylistPicker.svelte`, add the action after `function close()`:

```ts
  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
```

Replace the create step with a form:

```svelte
    {:else if actions.step === "create"}
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <p><label>Name <input bind:value={name} use:autofocus /></label></p>
        <p><label>Description <input bind:value={description} /></label></p>
        {#if formError}<p class="error">{formError}</p>{/if}
        <p class="actions">
          <button type="button" onclick={close}>Cancel</button>
          <button type="submit" disabled={actions.busy}>Create</button>
        </p>
      </form>
    {:else}
```

- [ ] **Step 3: Run the type check**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/Modal.svelte ui/src/lib/components/PlaylistPicker.svelte
git commit -m "feat(ui): manage modal focus and polish the playlist picker"
```

---

### Task 10: Clickable Home cards and the queue clear confirmation

**Files:**
- Modify: `ui/src/lib/nav.svelte.ts`
- Modify: `ui/src/lib/nav.test.ts`
- Modify: `ui/src/views/Home.svelte`
- Modify: `ui/src/lib/components/QueueRail.svelte`

**Interfaces:**
- Consumes: `HomeItem` from `./api`, `ConfirmDialog` (existing).
- Produces: `routeForHomeItem(item: HomeItem): Route | null`; Home cards navigate when a route exists and are inert otherwise; QueueRail Clear asks for confirmation.

- [ ] **Step 1: Write the failing route-mapping tests**

Append to `ui/src/lib/nav.test.ts`:

```ts
import type { HomeItem } from "./api";
import { routeForHomeItem } from "./nav.svelte";

function homeItem(item_type: string, id: string): HomeItem {
  return { id, title: "Item", item_type };
}

describe("routeForHomeItem", () => {
  it("routes playlist-ish items with a non-numeric id", () => {
    expect(routeForHomeItem(homeItem("PLAYLIST_LIST", "8f5d0e3a-abc"))).toEqual({
      name: "playlist",
      id: "8f5d0e3a-abc",
    });
    expect(routeForHomeItem(homeItem("EDITORIAL", "story-1"))).toEqual({
      name: "playlist",
      id: "story-1",
    });
  });

  it("routes album items with a numeric id", () => {
    expect(routeForHomeItem(homeItem("ALBUM", "12345"))).toEqual({
      name: "album",
      id: 12345,
    });
  });

  it("leaves tracks, mixes, and empty ids inert", () => {
    expect(routeForHomeItem(homeItem("TRACK_LIST", "55"))).toBeNull();
    expect(routeForHomeItem(homeItem("MIX", "mix-1"))).toBeNull();
    expect(routeForHomeItem(homeItem("", "1"))).toBeNull();
    expect(routeForHomeItem(homeItem("ALBUM", ""))).toBeNull();
  });

  it("rejects type and id mismatches", () => {
    expect(routeForHomeItem(homeItem("PLAYLIST_LIST", "55"))).toBeNull();
    expect(routeForHomeItem(homeItem("ALBUM", "not-a-number"))).toBeNull();
  });
});
```

Move the two new imports to the top of the file so it reads:

```ts
import { describe, expect, it } from "vitest";
import type { HomeItem } from "./api";
import { createNav, routeForHomeItem } from "./nav.svelte";
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: FAIL — `routeForHomeItem` is not exported.

- [ ] **Step 3: Implement the mapper**

In `ui/src/lib/nav.svelte.ts`, add the import at the top:

```ts
import type { HomeItem } from "./api";
```

Add after `sameRoute`:

```ts
const NUMERIC_ID = /^\d+$/;

/// Map a home-feed item to a route only when the type and id shape agree.
/// Tracks, mixes, and unknown types have no route and stay inert.
export function routeForHomeItem(item: HomeItem): Route | null {
  if (item.id.length === 0) return null;
  const type = item.item_type.toUpperCase();
  if ((type.includes("PLAYLIST") || type.includes("EDITORIAL")) && !NUMERIC_ID.test(item.id)) {
    return { name: "playlist", id: item.id };
  }
  if (type.includes("ALBUM") && NUMERIC_ID.test(item.id)) {
    return { name: "album", id: Number(item.id) };
  }
  return null;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix ui test -- src/lib/nav.test.ts`
Expected: PASS (11 tests).

- [ ] **Step 5: Make Home cards navigate**

In `ui/src/views/Home.svelte`, add the import:

```ts
  import { routeForHomeItem } from "../lib/nav.svelte";
```

Replace the card loop body:

```svelte
      {#each section.items as item, i (item.id || `${item.title}-${i}`)}
        {@const route = routeForHomeItem(item)}
        {#if route}
          <button
            style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit"
            onclick={() => nav.goto(route)}
          >
            <div>{item.title}</div>
            {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
          </button>
        {:else}
          <div style="text-align:left;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px;color:inherit">
            <div>{item.title}</div>
            {#if item.subtitle}<div class="muted" style="font-size:12px">{item.subtitle}</div>{/if}
          </div>
        {/if}
      {/each}
```

- [ ] **Step 6: Confirm before clearing the queue**

In `ui/src/lib/components/QueueRail.svelte`, add the import:

```ts
  import ConfirmDialog from "./ConfirmDialog.svelte";
```

Add the state beside `dragFrom`:

```ts
  let confirmingClear = $state(false);
```

Replace the Clear button:

```svelte
    <button onclick={() => (confirmingClear = true)}>Clear</button>
```

Add after the closing `</ul>`:

```svelte
  {#if confirmingClear}
    <ConfirmDialog
      title="Clear queue"
      message="Remove all tracks from the queue?"
      onconfirm={() => {
        confirmingClear = false;
        player.clear();
      }}
      oncancel={() => (confirmingClear = false)}
    />
  {/if}
```

- [ ] **Step 7: Run the tests and the type check**

Run: `npm --prefix ui test`
Expected: all tests pass.

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts ui/src/views/Home.svelte ui/src/lib/components/QueueRail.svelte
git commit -m "feat(ui): navigate from home cards and confirm queue clears"
```

---

### Task 11: Full gates, smoke checklist, and docs commit

**Files:**
- Create: `docs/superpowers/plans/notes/plan8-smoke.md`
- Commit: the Plan 8 spec, this plan, and the checklist

- [ ] **Step 1: Run every gate and record the output**

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```

Expected: all green (0 failures; clippy and svelte-check 0 warnings). Record the exact counts in the checklist below.

- [ ] **Step 2: Write the smoke checklist**

Create `docs/superpowers/plans/notes/plan8-smoke.md`:

```markdown
# Plan 8 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and a mouse with
> side buttons. Run with `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at <commit>)
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Toast stack: add tracks to a playlist from Search → success toast appears top-right → another action stacks a second toast → both auto-dismiss after ~6 s.
- [ ] Error toast persistence: turn the network off and toggle a favorite → error toast appears and stays until the ✕ is clicked; a fourth outcome evicts the oldest auto-dismissing toast, never the error.
- [ ] Mapped error copy: attempt to rename a playlist you do not own → the toast reads "You can only edit your own playlists" (not "forbidden").
- [ ] Logout failure: with the network off, click Log out → the error toast is visible on the login screen.
- [ ] Track row menu: right-click a row in Search, an Album, a Playlist, and Library favorites → Play / Add to queue / Add to playlist / Favorite match the inline buttons; playlist rows also show Move up / Move down / Remove with edges disabled; Playlist editing items are absent while the pager is loading.
- [ ] Queue item menu: right-click a queue row → Play now and Remove work.
- [ ] Library playlist menu: right-click a playlist → Open, Rename, and red Delete all do what the inline buttons do.
- [ ] Menu edges and keyboard: open a menu near each of the four window edges → it stays fully on-screen; ↑/↓ wrap and skip disabled items; Enter runs; Esc closes; clicking away or scrolling closes.
- [ ] Shortcuts while playing: Space toggles, ←/→ seek ±5 s, Shift+←/→ change tracks, ↑/↓ change volume by 5%, none act while a text field is focused.
- [ ] Ctrl+F and `/` focus the Search input; Ctrl+F does not open the WebView find bar.
- [ ] Esc precedence: with a menu open it closes the menu; with the queue rail open and no menu it closes the rail; with a dialog open the dialog closes and the rail stays.
- [ ] Alt+←/→ and the mouse side buttons move the history; the Sidebar ‹/› buttons enable/disable correctly; a new navigation clears forward history; toggling the queue does not.
- [ ] Typing guards: with focus in the Search input, Space/arrows edit or move the caret and never control playback.
- [ ] Home cards: clicking a playlist/editorial card opens the playlist view, an album card opens the album; track/mix cards are inert panels.
- [ ] Pager spinner: open a large playlist and Library favorites → a small spinner sits beside the "N tracks" label while paging.
- [ ] Queue clear: Clear asks "Remove all tracks from the queue?"; Cancel keeps the queue; Confirm clears it.
- [ ] Modal focus: open the playlist picker → a control inside the dialog is focused; close it → focus returns to the button that opened it; with the delete confirmation open, focus restores after navigation is skipped safely.
- [ ] Search no-results: search for gibberish → "No results for “<query>”"; Album/Artist/Home loading and empty states all use the shared components (no bare "Loading…" text in views).

## Known caveats
- Mouse side buttons depend on WebView2/driver mapping; Alt+←/→ is the fallback.
- History is in-session only; restarting starts at Home (P8-D5).
- Toast actions exist in the store but no Plan 8 caller renders one yet (deferred undo).
- Menus are additive; inline row buttons are unchanged.
```

- [ ] **Step 3: Fill in the recorded gate counts**

Replace each `<fill in>` with the exact output from Step 1, and record the commit hash in the heading.

- [ ] **Step 4: Commit the docs**

```bash
git add docs/superpowers/specs/2026-09-25-oxitide-plan8-ux-foundations-design.md docs/superpowers/plans/2026-09-25-plan8-ux-foundations.md docs/superpowers/plans/notes/plan8-smoke.md
git commit -m "docs: add the plan 8 spec and plan; record the smoke checklist"
```

---

## Self-Review

- **Spec coverage:** toasts store/host/migration (§5.1 → Tasks 1–3); context menus store/component/builders and all three surfaces (§5.2 → Tasks 4–5); shortcuts, mouse side buttons, Settings list (§5.3 → Task 7); nav history and Sidebar buttons (§5.4 → Task 6); loading/empty/error consistency and `errorMessage` (§5.5 → Tasks 1, 8); carryover polish — pager spinner (Task 8), Modal (Task 9), picker (Task 9), Home cards (Task 10), QueueRail clear (Task 10), Settings list (Task 7), logout toast and forbidden copy (Task 8, Task 1). Repository layout and deletions (§4 → Tasks 1–4, 6, 8). Tests (§9 → every pure module has a suite; smoke in Task 11). Decisions P8-D1…P8-D10 are honored; implementation notes for the three signature adaptations (menu singleton, `show` + severity, `menuFor` action shape) are in Task 4 and Global Constraints.

- **Placeholder scan:** the only `<fill in>` markers are the gate counts in the Task 11 checklist, filled from the actual run in Step 1.

- **Type consistency:** `errorMessage`, `Toasts`/`Toast`/`ToastSeverity`, `createFavorites(toasts)`, `createPlaylistActions(toasts)`, `createMenu`/`menu`/`menuFor`, `MenuItem`/`TrackMenuContext`, `trackMenuItems`/`queueMenuItems`/`libraryPlaylistMenuItems`, `matchShortcut`/`ShortcutAction`/`SHORTCUTS`, `routeForHomeItem`, `Loading`/`EmptyState` names and signatures are identical across their store, component, App wiring, and test tasks.

- **Review Focus coverage:** side-button double fire (Task 7 guard + smoke); Esc vs dialog (Task 7 `overlayOpen` test + smoke); typing guard (Task 7 tests + smoke); toast cap vs errors (Task 1 tests); menu viewport clamp (Task 4 clamp + smoke).
