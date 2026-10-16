# OxiTide — Name links (2026-10-15)

- **Status:** Implemented (steps 1–4), gates green; owner check in the running app pending.
- **Owner:** V
- **Scope:** Desktop UI in every mode. The phone remote already links its Now Playing
  artist/album and has Go to album/artist in its track sheet; it is out of scope here.

## 1. Problem

TIDAL lets you click a track title, album name or artist name wherever one is shown (lists,
player bar, queue) to open that page; the cover stays the play target. OxiTide only did
this inside the Track info panel and on cards. Everywhere else names were plain text, so
getting from a playing track to its artist meant opening Track info first.

## 2. Decisions

| # | Decision |
|---|---|
| NL-D1 | Clickable names: artist → artist page, album → album page, track title → its album (OxiTide has no track page; this matches TIDAL). Covers keep their existing action (play / open Now Playing). Now Playing's large title stays plain: its byline already links the album. |
| NL-D2 | Each artist of a multi-artist credit is its own link, comma-separated. |
| NL-D3 | Links look like the text around them (same colour, weight) and underline on hover; muted text lifts to `--text` on hover. Each carries its name as a native tooltip so truncated names stay readable. |
| NL-D4 | No self-links in page content: a name pointing at the page you are on (the album on its album page, the artist on its artist page or full lists) renders as plain text. Derived from `nav.route`, not per-view props. Shell chrome (player bar, queue) always links; `goto` on the current route is already a no-op. |
| NL-D5 | Inline links are `role="link"` spans (`NameLink.svelte`), not buttons. Buttons are atomic inline boxes, so an overflowing one is dropped whole by `text-overflow: ellipsis` instead of being truncated. In track-table rows they are `tabindex="-1"` (a row stays one Tab stop under the roving focus); keyboard users get the same targets from the track menu. Elsewhere they are focusable and open on Enter. |
| NL-D6 | The track menu gains a navigation group: "Go to album" and "Go to artist" (one artist) or "Go to {name}" (several, first three). Self targets are omitted (NL-D4). |
| NL-D7 | Double-clicking a name navigates only; it does not also play the row. |
| NL-D8 | `QueueItem` carries `artist_refs: [{id, name}]` and `album_id` (`#[serde(default, skip_serializing_if)]`), persisted through `SessionItem` and `HistoryRecord`, so an older `session.json` / `history.jsonl` still loads. Pairs, not parallel id lists: the joined `artist` string cannot be split safely ("Crosby, Stills, Nash & Young"). Producers built from a `Track` fill them (`trackToQueueItem`, `queue_item_from_track`); page-feed tracks with embedded metadata do not. |
| NL-D9 | Not linked: the mini players (a click would have to leave mini mode), the Rack VFD text (hardware look), media cards (one `<button>` per card; links cannot nest in it, and feed `PageItem`s carry artist names only — the card opens the album, whose header links the artists), and Home hero's up-next rows (play buttons without a menu; the queue rail covers the same tracks). |
| NL-D10 | Queue and History rows are themselves play buttons. Their names are **nested** links: pointer-only spans with no link role (interactive content cannot nest), whose click stops at the name so the row does not also play. A name click right after a queue drag release is swallowed like the row's play click. The keyboard / screen-reader path is the row menu: the queue menu gains the Go to group, History rows gain a menu (Play + Go to). |
| NL-D11 | Items without ids (older sessions and history, page-feed tracks) still link: the joined artist string is one link to the primary artist, and album/title resolve the album. They resolve **on click** through the session-cached `resolveTrack` (`lib/queueLinks.ts`), never on render. A failed lookup (offline) does nothing. |
| NL-D12 | Navigating from Now Playing, the player bar or the queue closes Now Playing first; the floating (narrow-window) queue closes too, so the opened page is visible. Hosts pass a `go(route)` callback; QueueRail without one keeps plain text. |

## 3. Surfaces

1. **Track table + track menu** (`TrackTable.svelte`, `menus.ts`): every album, playlist,
   mix, favorites, artist and search track list.
2. **Album header** (`DetailHeader` `byline` snippet with `ArtistLinks`).
3. **Playing-track surfaces:** player bar (title, artists), queue rail incl. Rack's queue
   windows (nested), Now Playing byline and Next up.
4. **History** rows (nested + new menu) and the Home hero's current track.

Shared pieces: `NameLink.svelte`, `ArtistLinks.svelte`, `QueueArtistLinks.svelte`,
`lib/nameLinks.ts` (self-link targets, menu artists), `lib/queueLinks.ts` (routes and menu
targets for queue/history items).

## 4. Verification

- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test` — all green (2026-10-16).
- Owner check in the running app:
  - links in a playlist, album, artist page, search results; no self-links on album/artist pages;
  - long names still end in "…"; the playing row keeps its accent title on hover;
  - player bar title/artist open album/artist and close Now Playing if open;
  - queue: clicking a name navigates without playing; clicking elsewhere on the row plays;
    dragging a row by its name does not navigate; right-click → Go to works;
  - an old restored session (no ids): artist/album links still open the right pages;
  - History: names navigate, row click plays, right-click → Go to.
