# OxiTide — Explore redesign (2026-10-16)

- **Status:** Implemented, gates green; owner check in the running app pending.
- **Owner:** V
- **Scope:** Desktop Explore page and the pages its links open, in every mode. The phone
  remote's Feed page (which can show Explore rows) is out of scope; `get_page` is
  `NOT_EXPOSED` to it.

## 1. Problem

TIDAL's `pages/explore` returns Genres, Moods & Activities and Decades as page links
(`PAGE_LINKS_CLOUD` / `PAGE_LINKS` modules: `title` + `apiPath`, no `imageId`). The page
rendered them as square media cards, so every tile was an empty cover with a music-note
placeholder, and the parser dropped `apiPath`, so the tiles went nowhere. The header was
a small title and a 560px search field over empty space.

## 2. Decisions

| # | Decision |
|---|---|
| EX-D1 | `PageItem` keeps `api_path` (`#[serde(default, skip_serializing_if)]`) when it passes `pages::is_page_path`: `pages/` plus `/`-separated segments of `[A-Za-z0-9_-]`. Anything else (absolute URLs, `..`, query strings) is dropped at parse time, so a page link cannot steer a request elsewhere. |
| EX-D2 | `TidalClient::page(path)` fetches any rows/modules page (same params as home/explore); it refuses invalid paths without a request. The `get_page` command re-checks the path (it comes back from the webview). Not exposed to the phone remote. |
| EX-D3 | New route `{ name: "page", path, title }` (`views/TidalPage.svelte`): an "Explore" eyebrow back-link, the link's name as title, and the page's rows through `PageSections`. `routeForPageItem` routes any item with a valid `api_path`, so nested links (sub-genres) open further pages. Routes persist and are validated on restore like the rest. |
| EX-D4 | "View all" on a linked page reloads from that page: the section route's `source` widens to `"home" \| "explore" \| "forYou" \| PagePath`. |
| EX-D5 | `classifySection` gains a `links` treatment: a row where most items are page links (an `api_path`, or a `PAGE_LINK*` type). This holds even when they carry an `imageId`: in the owner's first run Genres/Moods did, and those ids failed as square covers (empty placeholders). |
| EX-D6 | Link tiles (`PageLinkGrid.svelte`) are a wrapping grid, not a carousel: 2.2:1 name tiles on a two-hue OKLCH wash hashed from the name (`lib/pageLinks.ts`), tuned per theme/mode only through the `--link-wash-*` tokens, with a record-edge motif. A row made only of decades shows them as display numerals. No artwork fetches (deferred; see §4). |
| EX-D7 | Header: the title keeps the shared page-title step (so no other page changes); the search field below it is the page's centred hero (48px, 680px). Under it, one centred row of suggestion chips the field's width (up to 5 recent searches, then top artists from listening history, 16 max, deduped; whatever does not fit is clipped by CSS) runs a search on click. Chips hide while a search is active. A "Browse" row of jump links was tried and removed (owner: it only scrolled). |
| EX-D9 | Order: "New to you", then Explore's untitled page-link rows (New, Top, HiRes, Clean Content, …; `splitQuickLinks`), then the titled rows in TIDAL's order. Video page links (title or path matching `video`) are dropped on Explore and linked pages, since OxiTide cannot play videos; a row left empty disappears. |
| EX-D8 | "New to you" (`lib/exploreSuggest.ts`) is a discovery shelf, distinct from Home's revisiting shelves: similar artists of the top 4 artists from the last 200 plays (ids through the session-cached `resolveTrack`), interleaved one per seed per round, minus anyone in the history (by id or name) or favourites, capped at 16. Cards read "Like {seed}". It loads on its own and hides on an empty history or failure; a failed similar-artists lookup drops only that seed. |

## 3. Files

- Rust: `crates/oxitide-tidal/src/pages.rs` (`api_path`, `is_page_path`, `page`),
  `crates/oxitide-app/src/commands.rs` (`get_page`), `main.rs`, `remote/rpc.rs`.
- UI: `lib/nav.svelte.ts` (`page` route, `PagePath`, `SectionSource`), `lib/media.ts`
  (`isPageLink`, `links`), `lib/pageLinks.ts`, `lib/exploreSuggest.ts`,
  `lib/components/PageLinkGrid.svelte`, `lib/components/PageSections.svelte`,
  `views/Explore.svelte`, `views/TidalPage.svelte`, `views/Section.svelte`,
  `lib/RoutedPage.svelte`, `lib/titles.ts`, `lib/rack/sources.ts` (page → Explore lamp).
- Tokens: `--link-wash-l/-c/-a` in `tokens.css`, `themes.css` (light), `modes.css`
  (Instrument), `rack.css`. Design notes: `ui/design/DESIGN.md` §2.1.

## 4. Deferred

- Artwork on link tiles (fanned covers from each linked page, lazily on scroll, session
  cached).
- Rich search results (cards / top result) instead of the plain album/artist/playlist lists.
- Owner-requested only: moving the plain-`<h2>` pages onto `page-head`.

## 5. Verification

- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test` — green (2026-10-16).
- Owner check in the running app:
  - Explore shows coloured genre/mood tiles and decade numerals instead of empty covers,
    in Atmosphere, Instrument, Rack, and light theme;
  - clicking a genre opens its page (title, rows); "Explore" eyebrow and Back return;
    a sub-genre link (if TIDAL sends one) opens its own page; "View all" on a linked page works;
  - search centred; chips fill one row under it, run a search, hide while searching;
  - the New/Top/HiRes row sits under "New to you", without Videos;
  - "New to you" lists unheard artists ("Like …");
  - restart on a genre page restores it.
