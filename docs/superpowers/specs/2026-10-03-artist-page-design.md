# OxiTide — Artist page redesign (Track 9 slice)

- **Status:** Draft for review
- **Date:** 2026-10-03
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 9 (UI completion)
- **Related:** `plans/notes/2026-10-03-artist-page-spike.md` (live API probe — all endpoint
  facts below are from it), `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md`
  (backdrop/motion/tokens), `2026-09-30-oxitide-track4b-discovery-design.md` (similar artists,
  artist radio), `2026-10-03-home-layout-card-language-design.md` (`MediaCard`, `Carousel`,
  `PageItem` shapes), `2026-09-24-oxitide-plan6-favorites-design.md` (artist heart).
- **Process note:** designed live with the owner (2026-10-03); the fan-count question was
  settled by the pre-spec spike and is included. Layout/build decisions were reviewed by the
  owner before the spike ran.

---

## 1. Purpose & Outcome

`ui/src/views/Artist.svelte` is a Phase-1 stub: avatar, name, radio and heart buttons, and a
plain grid of similar artists. Every other detail surface (album, playlist, mix) has a real
header and body. This design turns the artist page into the full surface the owner asked for:
a banner-style hero with the artist's real fan count, bio and social links, transport actions,
Top tracks, Albums, Singles & EPs, Appears on, and Similar artists.

**Outcome:** opening an artist shows a hero with a palette-derived backdrop over a blurred
portrait, the artist name, `Artist · {N} fans` when the API reports a count, a clamped bio with
Read more, social chips from the API's external links, Play/Shuffle over the top tracks, and
heart/radio/share actions. Below it: Top tracks (5 rows, Show all → 10), three discography
carousels (each hidden when empty), and Similar artists as a carousel. Every section loads
independently — a failed shelf disappears and never blocks the hero. Both UI modes degrade
cleanly; no new CSP origin; Track 3 budgets apply.

Success = the surfaces above work on Windows 10/11 with the owner's account; parsing/formatting
helpers are unit-tested; navigation and playback semantics are unchanged; `cargo test`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, and
`npm --prefix ui test` pass; a human smoke checklist is recorded.

## 2. Scope

**In scope:** three new Tidal client methods + `ArtistProfile`/`ArtistLink` DTOs +
`clean_bio`; three new read commands and one validated `open_external` command; `generate_handler!`
registration; `api.ts` interfaces/wrappers; `share.ts` artist URL; `formatCount` and a pure
`artistLinks.ts` label mapper; a rebuilt `Artist.svelte`; a new `ArtistHero.svelte`; Top tracks
via `TrackTable`; discography and similar shelves via `Carousel` + `MediaCard`; an
`external-link` icon path; Rust and vitest tests; smoke checklist.

**Out of scope:** v1 `artists/{id}/links` and `artists/{id}/bio` (superseded by the OpenAPI
`include=biography` call — see spike); `profileArt`/blurHash use (square only, no wide banner
exists); artist mixes, videos (PM-D6), credits, "influencers"; See-all discography pages;
clickable links inside bio; per-section retry UI; Hi-Fi Rack and light mode; i18n; sidebar/
queue/player changes; `pages/artist` (exists, but loses full track DTOs and adds no fans/banner).

## 3. Decisions

| # | Decision | Rationale |
|---|---|---|
| D-1 | Hero backdrop is a palette gradient over a blurred portrait, both derived from `artist.picture`. | Owner-approved; the spike confirmed no wide banner art exists anywhere (profileArt and `pages/artist` are the same square image). |
| D-2 | The fan count **is in scope**: OpenAPI `artists/{id}` returns `attributes.numberOfFollowers` (9,672 / 1,106,540 on the two spike samples). | Owner's "spike first, then decide" gate: the spike found it, so it is built. |
| D-3 | Bio comes from the same OpenAPI call with `include=biography`; markup is stripped in Rust (`wimpLink` tags, `<br/>`, leftover HTML). | One request supplies fans + socials + bio; both bio sources share the same markup. |
| D-4 | Socials come from OpenAPI `externalLinks`; `TIDAL_SHARING` entries are dropped in Rust; unknown site types get a generic label. | The v1 `/links` endpoint is unreliable (404 on The Notwist, TWITTER-only on AC/DC) and duplicates `externalLinks`. |
| D-5 | **Play** starts the fetched top tracks in order; **Shuffle** starts the same set shuffled; **radio stays a separate visible action** (existing `playArtistRadio`). | Radio is a different queue than top tracks. |
| D-6 | Top tracks fetch 10, show 5, **Show all** expands inline to 10. | Matches the official client's 5-row top list without a new route. |
| D-7 | Discography is three shelves — Albums, Singles & EPs, Appears on — one page of 50 each, no See-all; empty or failed shelves are hidden. | One API call per shelf; a See-all route is unneeded inventory for a personal client. |
| D-8 | The hero `Play`/`Shuffle` buttons are disabled until top tracks load and when the list is empty; sections never block the hero. | Top tracks is the only transport source; nothing else gates the page. |
| D-9 | Hero renders only after `artist` (fatal) and `profile` (best-effort) settle, both fetched in parallel; section fetches fire in parallel afterward and each render when its own data arrives. | Avoids layout shift from late fans/bio, and keeps the page's first paint early. |
| D-10 | The hero computes its own local palette (`analyzeCover`) and applies scoped `--hero-*` vars; it never writes global `--dyn-*`. | Global dynamic color belongs to playback; a page must not repaint the app mood. |
| D-11 | Photo/blur layers and chips are gated by existing CSS flags (`--flag-backdrop-art`, glass tokens), not by mode branches in JS. | AGENTS: components must not branch on mode; instrument/reduced-motion degrade via CSS. |
| D-12 | Social chips open in the system browser via a new `open_external` command that validates `http(s)` + host in Rust; capability file and CSP unchanged. | Rust-side opener bypasses the webview scope safely and rejects `javascript:`/`file:`/data URLs. |
| D-13 | Options menu (ellipsis) holds **Share** (copies `tidal.com/artist/{id}`); heart and radio stay visible buttons. | Mirrors album/playlist headers; share plumbing already exists. |

## 4. Architecture & components

### 4.1 Client — `crates/oxitide-tidal/src/catalog.rs`, `models.rs`

New DTOs in `models.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistLink {
    /// Uppercased site key from the API, e.g. "TWITTER" or "OFFICIAL_HOMEPAGE".
    pub site: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistProfile {
    #[serde(default)]
    pub number_of_followers: Option<u64>,
    #[serde(default)]
    pub external_links: Vec<ArtistLink>,
    /// Cleaned biography text (no wimpLink/HTML), `None` when absent/empty.
    #[serde(default)]
    pub bio: Option<String>,
}
```

New methods (same file, `impl TidalClient`):

- `pub async fn artist_profile(&self, id: u64) -> Result<ArtistProfile>` —
  `get_openapi("artists/{id}", [("countryCode", cc), ("include", "biography")])`, parsed by
  private serde types following the `OpenApiPlaylistMeta` pattern in `playlists.rs`
  (`data.attributes.numberOfFollowers`, `data.attributes.externalLinks`, and the
  `included[]` entry with `type == "artistBiographies"` for the bio text). `externalLinks`
  entries with `meta.type == "TIDAL_SHARING"` (case-insensitive) or an empty href are dropped,
  duplicates removed by href preserving order. Any missing field defaults; the call errors
  only as the HTTP/parse layer does.
- `pub async fn artist_top_tracks(&self, id: u64, limit: u32) -> Result<Vec<Track>>` —
  `GET artists/{id}/toptracks?limit={limit}` via `ItemPage<Track>` → `items`.
- `pub async fn artist_albums(&self, id: u64, filter: ArtistDiscography, limit: u32, offset: u32)
  -> Result<Vec<Album>>` — `GET artists/{id}/albums` with `limit`, `offset`, and
  `filter = filter.tidal_param()` (`"ALBUMS"` | `"EPSANDSINGLES"` | `"COMPILATIONS"`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistDiscography { Albums, EpsAndSingles, Compilations }
```

- `pub fn clean_bio(raw: &str) -> Option<String>` (free function in `catalog.rs`, unit-tested):
  replace `<br>`, `<br/>`, `<br />` (case-insensitive) with newlines; replace
  `[wimpLink ...]Name[/wimpLink]` with `Name`; strip any remaining `<...>` tag; split on newlines,
  trim each, drop empties, join with `"\n\n"`; `None` when the result is empty.

### 4.2 App commands — `crates/oxitide-app/src/commands.rs`, `main.rs`

Registered in `generate_handler![...]` next to the existing artist commands (`main.rs:275+`):

```rust
#[tauri::command]
pub async fn get_artist_profile(state: State<'_, AppState>, id: u64) -> Result<ArtistProfile, String>;

#[tauri::command]
pub async fn get_artist_top_tracks(
    state: State<'_, AppState>, artist_id: u64, limit: u32,
) -> Result<Vec<Track>, String>;

#[tauri::command]
pub async fn get_artist_albums(
    state: State<'_, AppState>, artist_id: u64, filter: ArtistDiscography,
    limit: u32, offset: u32,
) -> Result<Vec<Album>, String>;

#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String>;
```

- `open_external` uses a pure validator `fn external_url(raw: &str) -> Result<reqwest::Url, String>`
  (reqwest is already a dependency; no new crate): scheme must be `http`/`https`, host must be
  present. On success, `tauri_plugin_opener`'s `opener().open_url(url, None::<&str>)` — the
  existing `open_logs_folder` precedent. Unit tests cover valid https/http and rejected
  `javascript:`, `file:`, `data:`, empty, and hostless inputs.
- The three read commands are thin `state.client()?` wrappers, matching `get_artist`.

### 4.3 API contract — `ui/src/lib/api.ts`, `share.ts`, `format.ts`

```ts
export interface ArtistLink { site: string; url: string }
export interface ArtistProfile {
  numberOfFollowers?: number | null;
  externalLinks: ArtistLink[];
  bio?: string | null;
}
export type ArtistDiscography = "albums" | "eps_and_singles" | "compilations";

getArtistProfile: (id: number) => invoke<ArtistProfile>("get_artist_profile", { id }),
getArtistTopTracks: (artistId: number, limit: number) =>
  invoke<Track[]>("get_artist_top_tracks", { artistId, limit }),
getArtistAlbums: (artistId: number, filter: ArtistDiscography, limit: number, offset: number) =>
  invoke<Album[]>("get_artist_albums", { artistId, filter, limit, offset }),
openExternal: (url: string) => invoke<void>("open_external", { url }),
```

- `share.ts`: `ShareKind` gains `"artist"`; `shareUrl("artist", id)` → `https://tidal.com/artist/{id}`.
- `format.ts`: new `formatCount(n: number): string` for compact counts — `< 1000` exact;
  `< 1e6` → one decimal + `K` (drop `.0`); `< 1e9` → `M`; else `B`. `1106540` → `"1.1M"`,
  `9672` → `"9.7K"`, `999` → `"999"`.
- New pure `ui/src/lib/artistLinks.ts`: `linkLabel(site: string, url: string): string` maps known
  site keys (`TWITTER` → "Twitter", `FACEBOOK`, `INSTAGRAM`, `YOUTUBE`, `TIKTOK`,
  `OFFICIAL_HOMEPAGE`/`WEBSITE`/`HOMEPAGE` → "Website", `LAST_FM` → "Last.fm", `WIKIPEDIA`,
  `DISCOGS`, `ALLMUSIC` → "AllMusic", `BBC_MUSICPAGE` → "BBC Music", `MYSPACE`, `SOUNDCLOUD`,
  `BANDCAMP`) and otherwise returns the URL host without `www.`; `isHttpUrl(url)` guards
  rendering. Vitest-covered.

### 4.4 UI — `Artist.svelte`, `ArtistHero.svelte`

**Orchestration (`Artist.svelte`).** Keep the Loading → ErrorState → EmptyState shell. On
`load()`:

1. `loading = true`; `const [artistResult, profileResult] = await Promise.all([api.getArtist(id),
   api.getArtistProfile(id).catch(() => null)])`. An artist failure sets `error`; a profile
   failure keeps the hero without fans/bio/socials.
2. `loading = false`, then fire all section fetches in parallel without awaiting the batch —
   `getArtistTopTracks(id, 10)`, `getArtistAlbums(id, "albums", 50, 0)`, `…"eps_and_singles"…`,
   `…"compilations"…`, `getSimilarArtists(id)` — each with its own `catch(() => [])`. Sections
   render as their data lands; failed or empty ones stay hidden. The existing `$effect` re-runs
   `load()` on `id` change.

**Top tracks.** `createTrackView(() => expanded ? top.slice(0, 10) : top.slice(0, 5))`;
`TrackTable` with `showAlbum` (tracks span releases), no `fill`, `favorites`, `toasts`,
`playingTrackId`, `onPlay={playFrom}`, `onAddToPlaylist`. Header row: `<h3>Top tracks</h3>` plus
a **Show all / Show less** text button when `top.length > 5`. `playAll`/`shuffleAll` queue the
full fetched 10 (`playTracks(view.items.map(trackToQueueItem), 0, shuffle)` with
`playSource.set(null)`, mirroring `Album.svelte`). Hero transport calls the same two functions
and is disabled while `top.length === 0`.

**Shelves.** A small local helper maps `Album` → `PageItem`:
`{ id: String(album.id), title, subtitle: year(album.release_date), item_type: "ALBUM",
artwork: album.cover }`, where `year` takes the first 4 chars of `release_date` when numeric.
Each shelf is `<Carousel title="Albums|Singles & EPs|Appears on|Similar artists">` with
`MediaCard variant="carousel"`; album cards `nav.goto({ name: "album", id })`, artist cards
`nav.goto({ name: "artist", id })`. The existing similar-artists grid is replaced by the
carousel; its failure behavior (silent empty) is unchanged.

**`ArtistHero.svelte`** (new, `ui/src/lib/components/`):

- Props: `artist`, `profile` (`ArtistProfile | null`), `disabled` (transport), `onPlay`,
  `onShuffle`, `onOpenLink`, plus an `actions` snippet so `Artist.svelte` owns heart/radio/menu
  wiring and URL opening. Social chips render inside the hero component (so their styles stay
  scoped there) and call `onOpenLink`.
- Structure: backdrop layers (photo `<img>` from `coverUrl(artist.picture, 640)`, blurred and
  scrimmed like `Backdrop.svelte`, opacity gated by `--flag-backdrop-art`) over a gradient
  built from scoped vars set by `api.analyzeCover(artist.picture)` (failure → semantic surface
  tokens only); content = circular `Cover` (radius 80 %), kicker `Artist`, name, stats line
  `{formatCount(n)} fans` (singular-aware) when present, transport row (Play, Shuffle as in
  `DetailHeader`), actions row, bio paragraph with 2-line clamp + Read more/Show less
  (aria-expanded; toggle shown when `bio.length > 240`), social chips row.
- Tokens only: chips/backdrop borders use `--surface-float`, `--line`, `--edge-lit`,
  `--glass-blur`; text uses `--text`/`--text-muted`; motion uses existing durations/easings so
  reduced motion collapses automatically. No mode branching and no `--dyn-*` reads.
- Accessibility: actions get aria/title labels as today; social chips are buttons with
  `aria-label="Open {label}"`; the bio toggle is keyboard-reachable.

**Menu.** Ellipsis uses the existing `menu`/`ContextMenu` pattern with a single item, **Share**,
which `copyText(shareUrl("artist", id))` and toasts success/failure like `Album.svelte`.

## 5. Testing & verification

**Rust (`cargo test --workspace`)**

- `catalog.rs` wiremock tests: `artist_top_tracks` sends `limit` and parses items;
  `artist_albums` sends `filter=ALBUMS|EPSANDSINGLES|COMPILATIONS`, `limit`, `offset` (one test
  per filter assertion via `query_param`); `artist_profile` parses a captured-shape JSON
  response — `numberOfFollowers`, deduped/filtered `externalLinks`, bio from `included` — and
  returns `Ok` with defaults when fields are missing; an OpenAPI 404 yields `Err`.
- `clean_bio` unit tests: `wimpLink artistId`, `wimpLink albumId`, `<br/>`/`<br>`, mixed
  markup, empty/whitespace → `None`.
- `open_external` validator tests: https/http pass; `javascript:`, `file:`, `data:`, empty,
  and hostless rejected.
- `cargo clippy --workspace -- -D warnings` clean.

**UI (`npm --prefix ui run check`, `npm --prefix ui test`)**

- `format.test.ts`: `formatCount` boundaries (`999`, `1000`, `9672`, `999999`, `1106540`,
  `1e9`).
- `artistLinks.test.ts`: known labels, unknown → hostname, `isHttpUrl` rejection.
- `share.test.ts`: `shareUrl("artist", 945)`.
- `svelte-check` zero errors.

**Human smoke checklist** (recorded under `docs/superpowers/plans/notes/` per repo convention
after implementation): hero renders with backdrop/fans/bio/socials on a rich artist (AC/DC) and
degrades on a sparse one (The Notwist); Play/Shuffle/radio/favorite/share behave; Show all
expands; shelves navigate; a section failure (offline toggle mid-load) leaves the rest usable;
both modes and reduced motion look right; socials open in the system browser.

**Performance:** the page issues 7 parallel requests; PM-D1 applies. Manual feel check plus a
warm re-check of the Track 3 scenario that covers scrolling (S5) to catch regressions.

## 6. Risks & mitigations

| Risk | Mitigation |
|---|---|
| `numberOfFollowers` is undocumented and could disappear | Field is `Option`; hero line simply absent. |
| `externalLinks` coverage is artist-dependent (sparse) | Chips hidden when empty; `TIDAL_SHARING` filtered; no layout dependency. |
| Bio markup variants (`<br>`, nesting) | Tolerant stripper with unit tests; worst case shows extra whitespace, never raw tags. |
| Slow OpenAPI host delays hero | Profile joins the artist fetch in parallel and is best-effort; hero still renders without it (empty values). |
| `TrackTable` on a 5–10 row list | Already used for full albums/mixes; no new virtualization behavior. |
| `open_external` introduced for user-visible links | Pure validator + unit tests; Rust-side opener only. |

## 7. Out of scope (tracked for later)

Artist mix action (the v1 payload carries `mixes.ARTIST_MIX`, playable via `pages/mix`), bio
in-text navigation, See-all discography pages, credits/influencers, videos, Hi-Fi Rack and light
mode, i18n.
