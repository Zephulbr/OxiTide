# OxiTide V2.0 — Modular sources (design)

- **Date:** 2026-10-17
- **Owner:** V
- **Status:** Draft for owner review. Scope, product and architecture decisions (§2–§3, V2-D1–D30)
  agreed with the owner on 2026-10-17; V2-D31–D35 and the revised V2-D4/D6/D29 (content-derived local
  ids, rebuildable `local.db`, preview-root adoption) come from the owner's first review the same day
  and are agreed. A consistency pass the same day added per-location tags (compilations), the
  reserved `oxitide` / `url` ref keys and `PlaylistRef`, and the playlist/favorites milestones.
  Product details V2-D36–D42 (source states, onboarding, Settings → Sources, tag editor, tag
  health, Local pages, perf budgets) agreed the same day. The only open item is Jellyfin's release
  number (§9).
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (2026-10-17 revision: "then V2 (modular
  sources)"), `plans/notes/2026-10-17-v1-release-gate.md` ("After 1.0" V2 seams; the persisted-data
  versioning step).
- **Research:** `plans/notes/2026-10-17-v2-source-research.md` (Jellyfin, Qobuz, formats, prior art).
- **Starts after:** `v1.0.0` is tagged. Nothing in this spec changes 1.x.

## 1. Goal

Turn OxiTide from "a TIDAL client" into "a player with sources", where TIDAL is one source among
several. V2.0 proves the architecture with one new source — **local files on internal and
external/USB drives** — and leaves every other source to 2.x point releases. The app must be fully
usable with no TIDAL account.

## 2. Scope

**In (V2.0):**

1. **Neutral core** — a source-agnostic track reference (`TrackRef`), quality descriptor,
   ReplayGain, metadata and error types; `oxitide-audio` no longer depends on `oxitide-tidal`.
2. **Source layer** — a source registry with capability flags; TIDAL rewrapped as the first source
   with no loss of 1.x behaviour.
3. **Local source** — library folders on internal and removable drives, an incremental scanner,
   a SQLite index, move-proof track identity, artwork, file playback with real seeking.
4. **Tag management** — a tag-health view and a single/multi-track tag editor with safe writes.
5. **Audio analysis** — a background job that stores per-track features (tempo/beat grid, energy,
   loudness, intro/outro silence) in the index; AutoMix and local mixes consume them.
6. **Local mixes and OxiTide playlists** — rule-based shelves over history and tags, plus
   feature-based mixes; mixed-source app-owned playlists with M3U8 import/export (V2-D21/D22).
7. **Source-aware UI** — onboarding ("choose your sources"), per-source sidebar sections, search
   with per-source tabs, neutral quality badges, Settings → Sources.
8. **Integrations for local tracks** — history/stats, scrobbling, lyrics, content blocking,
   SMTC/MPRIS, Discord (text + logo), phone remote (control, browse, artwork).
9. **Migration and durability** — side-by-side preview builds; the release build adopts the
   preview's data or migrates 1.x one-way with a backup; rolling backups of irreplaceable data.

**2.1 candidates (owner, 2026-10-17):** computed ReplayGain for untagged local tracks, musical key
detection, full-track beat grids (V2-D28); plain-text lyrics editing and the "other tags" panel
with raw tag editing (V2-D39); the Rack INPUT selector (V2-D41, 2.1 or later).

**Out of V2.0 (2.x or later):** Jellyfin (first 2.x source), Qobuz (research-gated), NAS/SMB
libraries as Local folders (use Jellyfin), live folder watching, Opus / APE / WavPack / DSD,
MusicBrainz lookup ("auto-tag"), external similarity (Last.fm / ListenBrainz / TIDAL similar
artists for local tracks), copying TIDAL playlists into OxiTide playlists, cross-source matching
(ISRC), file renaming/organising, Spotify, macOS / Android, a headless daemon, runtime
plugins, network renderers (UPnP / Chromecast / AirPlay), downloads/sync.

## 3. Decisions

- **V2-D1 — release shape.** 2.0 = architecture + Local. Each further source is its own point
  release (Jellyfin first, then Qobuz if V2-D17 clears it), so every release adds one source and
  re-tests the interface at small cost. Platforms stay Windows + Linux.
- **V2-D2 — sources are optional; onboarding.** The `loggedIn` gate in `App.svelte` becomes "at
  least one source is enabled". Onboarding (V2-D37) sets sources up; Settings → Sources (V2-D38)
  adds, disables, removes and reorders them afterwards. Each source reports its own state (V2-D36)
  inside its own UI area, so one failing source never blocks the app.
- **V2-D3 — keep 1.x alive until parity: branches.** At the `v1.0.0` tag, cut `release/1.x`;
  hotfixes are tagged `v1.0.x` there and merged forward into `main`. `main` becomes V2. 2.0
  pre-releases are published as GitHub **prereleases**; the updater endpoint
  (`releases/latest/download/latest.json`) ignores prereleases, so 1.x installs are never offered
  them.
- **V2-D4 — side-by-side preview identity.** Until 2.0 ships, preview builds must not touch the
  daily 1.x install:
  - own Tauri `identifier`, product name ("OxiTide Preview"), data root (`%APPDATA%\OxiTide
    Preview`, Linux equivalent), keyring service, MPRIS bus suffix, tray/SMTC identity;
  - **no `tidal://` scheme registration** (`scheme.rs` backs up and restores the HKCU handler; two
    OxiTide builds doing that would fight) — previews sign in by paste-back only;
  - its own phone-remote default port, and global hotkeys empty by default (a chord can only be
    registered by one process);
  - on first run it **imports a copy** of the 1.x data (config, session, history, flags, scrobble
    queue, palettes) and never writes to the 1.x folder. TIDAL needs a separate sign-in.
  - One build-time switch selects preview vs release identity; the 2.0 release build uses the normal
    identity.
  - **Preview → release carry-over** (agreed 2026-10-17). The preview root already holds V2-shaped
    data (the imported 1.x copy plus everything since), so on first run the 2.0 release build
    **adopts the preview data root** when one exists, and merges in from the 1.x root only what 1.x
    kept writing after the preview's import point: history records newer than that point (append,
    de-duplicated by track + start time) and track flags (a cache, merged as a union). Settings come
    from the preview; a notice says 1.x setting changes made after the import were not carried over.
    TIDAL favorites and playlists live on TIDAL's servers and need no merge. Without a preview root,
    the release build migrates the 1.x data in place (V2-D16). The 1.x root is left untouched in
    both cases.
- **V2-D5 — parity gate.** 2.0 is not tagged until (a) every 1.x smoke checklist that still applies
  passes on 2.0 with TIDAL as the only source, (b) the perf budgets in `scripts/perf/budgets.json`
  show no regression on the TIDAL scenarios, and (c) the V2 smokes and new perf scenarios (§7) pass.
- **V2-D6 — track identity.** Every saved or transported track is a `TrackRef { source, id }`
  with a string form `<source>:<id>` (`tidal:77646170`, `local:f3k9…`). `id` is opaque to everything
  except its source. **A Local track id is derived from the audio itself** — the audio key
  (V2-D32) — not from a database row (agreed 2026-10-17). A file's path is only a location of that
  track. Consequences:
  - Moves and renames need no relinking: a rescan finds the same id at a new path.
  - Tag edits never change the id (the tag-write verification guarantees the audio key is
    unchanged, V2-D33).
  - `local.db` is a **rebuildable cache**: losing it costs a rescan, never a broken ref (V2-D31).
  - Identical audio in several places is **one track with several locations**: an internal disk +
    USB backup copy, but also the same master on an album and on a compilation. It plays from
    whichever location is available, preferring the primary (first found, or picked by the user).
    **Tags belong to locations**, not to the track: album and artist views list locations, so the
    song appears on both the album and the compilation, each with its own tags. History, favorites,
    flags and analysis belong to the track (playing either copy counts the same). Queue, player and
    history show the tags of the location the item was started from (its snapshot), else the
    primary's.
  - Re-encoding, transcoding or frame-rewriting tools produce a new id; the old refs show
    "unavailable" until "Forget missing tracks" (V2-D32 lists the cases).
- **V2-D7 — library roots and removable drives.** A library folder is stored as volume identity
  (Windows volume GUID/serial; Linux filesystem UUID) + path within the volume, not as a drive letter
  or mount path. Roots are settings and live in `config.toml` (`[local] roots`), not in `local.db`.
  A root whose volume is absent marks its locations **unavailable**; a track with no available
  location is greyed, skipped by the queue, and kept in history and favorites. A rescan never
  removes entries from an absent volume. Removal is an explicit user action ("Forget missing
  tracks").
- **V2-D8 — Local index and scanning.** SQLite (`rusqlite`, bundled) in the data root. Scan on
  startup (incremental: size + mtime, then tags and the audio key only for changed files) and on
  "Rescan"; no live watching in 2.0. Scanning runs off the UI path at low priority and reports
  progress. A file is listed as soon as its tags are read but shows "indexing" — not playable, not
  queueable, not favoritable — until its audio key exists. FLAC with a STREAMINFO MD5 gets its key
  from the header at once; other files need one full read (fast on internal disks, slow for large
  MP3 libraries on USB 2 drives).
- **V2-D9 — formats.** FLAC and MP3 are required (MP3 gapless via the LAME header is a smoke row and
  an open check, research §3). ALAC/AAC (M4A), WAV, AIFF and Ogg Vorbis are best effort. Opus, APE,
  WavPack and DSD are deferred.
- **V2-D10 — files are read-only except through the tag editor.** The editor changes tag fields and
  embedded artwork for one or many tracks; it never renames or moves files. Writes are safe: copy →
  write the copy (`lofty`) → re-read and verify → atomic replace; a failure leaves the original
  untouched. The per-OS procedure and metadata rules are V2-D33. The **tag-health view** lists
  missing album artist, missing artwork, inconsistent album names / years / disc numbers, missing
  track numbers, and files that cannot be identified reliably (V2-D32), and opens the editor
  pre-filled. MusicBrainz lookup is 2.x.
- **V2-D11 — browsing per source.** The sidebar groups pages by source (TIDAL: Home, Explore,
  Collection…; Local: Albums, Artists, Tracks, Folders, Tag health). Shared across sources: the
  queue, player, history/stats, and a combined **Home** (personal shelves from every source, TIDAL
  editorial rows labelled and last). OxiTide playlists (V2-D21) are cross-source, so they get their
  own sidebar section beside the per-source groups; TIDAL playlists stay under TIDAL. Search has a
  tab per source plus **Top** (best few hits from each). Settings: default source at startup;
  sidebar order and visibility per source. No merged library.
- **V2-D12 — OxiTide-owned favorites.** Sources without server-side likes (Local) keep favorites in
  `library.db` (V2-D23); TIDAL keeps using TIDAL favorites. The favorite toggle and
  `favorite_changed` stay one path for every source. Local album and artist ids are derived from
  their grouping key (V2-D29), so a favorite album/artist also stores a few of its track refs; if
  its ref stops resolving (renamed outside OxiTide), it is re-found as the album/artist those tracks
  now belong to. Renames made in OxiTide's tag editor update the affected favorites directly.
- **V2-D13 — feature matrix for Local in 2.0.**

  | Feature | Local |
  |---|---|
  | DSP, ReplayGain (from tags), AutoMix, gapless, bit-perfect, ASIO | Yes |
  | History / stats, scrobbling | Yes |
  | Content blocking | Explicit flag from tags where present (MP4 `rtng`, `ITUNESADVISORY`); there is no AI flag for local files |
  | Lyrics | Embedded → sidecar `.lrc` → existing online lookup by name |
  | SMTC / MPRIS | Yes, with embedded/folder art |
  | Discord | Text yes; OxiTide logo instead of art |
  | Phone remote | Control + browse; art served by the remote server behind the pairing cookie |
  | Disk cache / offline | Not used (files are already local) |
  | Mixes, radio, credits, Explore | TIDAL's stay TIDAL-only; Local gets V2-D15 mixes |

- **V2-D14 — audio analysis.** A background job analyses each local track once and stores the
  features in the index (scope in V2-D28; musical key moved to 2.1). It runs at low priority, yields while playback or a scan is busy, can be turned
  off, and is forced off under `OXITIDE_PERF_SCENARIO`. AutoMix uses stored features for local
  tracks instead of analysing the tail live; TIDAL keeps today's live path (caching features of
  played TIDAL tracks is a 2.x option). Feature extraction reuses `automix.rs` code, so the live and
  stored results agree.
- **V2-D15 — local mixes.** Rule-based shelves: Recently added, On repeat, Rediscover (played a lot,
  not lately), Never played, genre and decade mixes, More from this artist. Feature-based: "similar
  vibe" mixes from a seed track and mixes ordered for smooth AutoMix flow. With a ~750-track library
  the pool is small; external similarity (2.x) widens it.
- **V2-D16 — migration.** One-way. When a preview data root exists the release build adopts it
  instead (V2-D4). Otherwise the 2.0 release build backs up the 1.x data root
  (`config.v1-backup-<date>`), then rewrites saved items with `TrackRef` (`123` → `tidal:123`) and
  bumps versions. Reads stay tolerant of 1.x records. No downgrade path; the backup is the way back.
  Depends on the 1.0 gate's "persisted-data versioning" step landing first.
- **V2-D17 — Qobuz stays "?".** Partner-only API with per-app secrets that Qobuz has revoked;
  research §2. Decided after Jellyfin, and only if the owner holds a subscription. A scraped id/secret
  would be user-supplied at runtime and never committed.
- **V2-D18 — quality badges.** The UI shows a neutral format line (codec · sample rate · bit depth,
  bitrate for lossy); a source's own tier name (TIDAL "Max") is an optional extra label.
- **V2-D19 — reference representation** (agreed 2026-10-17). Tracks, albums, artists and artwork
  are typed refs over one string form `<source>:<id>`:
  `Ref<K> { raw: Arc<str>, colon: u16, PhantomData<K> }` with `TrackRef`, `AlbumRef`, `ArtistRef`,
  `PlaylistRef`, `ArtRef`. Clone is a refcount bump, equality a string compare, serde a plain string; the kind
  marker makes mixing an album ref with a track ref a compile error. Today's `ArtistRef { id, name }`
  struct becomes `ArtistLink { ref, name }`; `QueueItem.album_id` / `cover` become `AlbumRef` /
  `ArtRef`. `<source>` is a **source instance key** (`tidal`, `local`, later e.g. `jf-a1b2c3` per
  Jellyfin server), mapped to its kind by the registry; a ref whose source is gone renders as
  "source unavailable". Source keys match `[a-z][a-z0-9-]{0,31}`. Two keys are reserved and never
  name a source: `url` for absolute image URLs (`url:https://…`, V2-D34) and `oxitide` for
  app-owned items (OxiTide playlists: `oxitide:<uuid>`, V2-D21). The part after the first colon is
  the id and may itself contain colons. Deserialisation of 1.x shapes follows the legacy read table
  in V2-D34, so 1.x records load without a rewrite pass.
- **V2-D20 — ref boundary** (agreed). Shared surfaces use refs: queue, player, session, history,
  flags, favorites, playlists, name links, phone remote, SMTC/MPRIS/Discord. TIDAL-only pages (Home
  rows, Explore, Mixes, TIDAL album/artist/playlist views) keep TIDAL's numeric ids internally and
  build refs at the boundary (`tidalRef(id)`). Album/artist routes carry a ref; `App.svelte` picks
  the view by the ref's source.
- **V2-D21 — OxiTide playlists** (agreed). Playlist ids are random UUIDs (never reused, never
  colliding across backups), referenced as `PlaylistRef` `oxitide:<uuid>` (also as the session's
  play source). App-owned playlists hold any `TrackRef`, each entry with
  a metadata snapshot (title, artist, album, duration, art — the `QueueItem` shape), so they render
  with a source disabled (those rows show "unavailable"). TIDAL playlists stay TIDAL server
  playlists. The "Add to playlist" picker lists both kinds; non-TIDAL tracks cannot be added to a
  TIDAL playlist (disabled with a hint). "Save queue as playlist" creates an OxiTide playlist. Copying
  TIDAL playlists into OxiTide ones and Jellyfin playlist sync are 2.x.
- **V2-D22 — M3U8 import/export in 2.0** (agreed, after the core). Export writes local entries as
  paths and other entries as `#` comments; import resolves paths to local refs and reports misses.
- **V2-D23 — two SQLite files** (agreed 2026-10-17). `local.db` (owned by `oxitide-local`: tracks,
  locations, albums/artists, artwork, analysis — a **rebuildable cache**, V2-D6) and `library.db`
  (owned by `oxitide-app`: OxiTide playlists, OxiTide favorites — **irreplaceable**). Each crate
  owns its schema; the app never queries a source's tables and asks the source by ref instead.
  There are no cross-file transactions (WAL is atomic per file only), so cross-file operations
  tolerate partial states — e.g. "Forget missing tracks" leaves favorites/playlist entries as
  "unavailable". Rules for both files:
  - **Copies** (the `library.db` backups) use SQLite's online backup API or `VACUUM INTO`, never a
    file copy of an open database (WAL `-wal`/`-shm` files).
  - **Location:** the app data root only — never on a library volume or a network share.
  - **Threads:** one writer thread per file (a small actor); reads on their own connections; no
    `rusqlite` calls on the async runtime.
  - **Schema versions:** `PRAGMA user_version` + ordered migrations; a file from a newer build is
    handled by the V2-D31 policy.
  - A quick integrity check runs on open; a full check is a Settings → Sources action.
  Backups, restore and corruption handling are V2-D31.
- **V2-D24 — crates and dependencies** (agreed 2026-10-17). New `oxitide-core` (refs, summary
  types, `Quality`, `ReplayGain`, `SourceError`, `StreamSource` / `StreamPlan`, capability traits;
  serde + thiserror only, no I/O). `oxitide-audio`, `oxitide-tidal` and `oxitide-local` depend on
  core; nothing depends on `oxitide-local` or `oxitide-tidal` except `oxitide-app`.
  - TIDAL manifest parsing (`source.rs` `plan_from_info`, the BTS format, `mpd.rs`) moves out of the
    engine into an `oxitide-tidal::source` adapter; the engine keeps only the generic plan type.
  - The engine sees **one** resolver: a router in `oxitide-app` that dispatches by the ref's source.
  - Disk cache key becomes `{TrackRef, Quality, manifest_hash}`; `tidal:<n>` maps onto the 1.x
    cache file names so existing entries (and `find_offline`) survive the upgrade.
  - The analysis job runs in `oxitide-app` (features computed by `oxitide-audio`, stored by
    `oxitide-local`), so `oxitide-local` never depends on the engine.
- **V2-D25 — hybrid source interface** (agreed). Required per source: identity/state, `Playback`
  (sync `resolve(&TrackRef, QualityPref) -> Resolved` on the engine worker) and `Metadata` (batch
  summaries by ref). Optional capabilities through accessors returning `Option<&dyn …>`: `Search`,
  `Library`, `Favorites`, `Lyrics`, `Artwork`, `TagEditing`. Shared surfaces use the shared summary
  types (`TrackSummary`, `AlbumSummary`, `ArtistSummary`, `QueueItem`-shaped); each source keeps its
  rich types for its own pages. **TIDAL's pages and catalog commands are not abstracted** (V2-D20).
  The generic `Library` capability (albums, artists, tracks, genres, folders) drives one set of
  library views shared by Local now and Jellyfin later. `SourceError { Unavailable, NotFound, Auth,
  Network, Offline, Unsupported, Other }` replaces `Error::Tidal`; the engine's track-unavailable
  check and the remote's error codes map from it. Source state reaches the UI through
  `get_sources` (`{key, kind, name, state, caps}`) and one `source_changed` event.
- **V2-D26 — commands and the phone remote** (agreed). TIDAL commands keep their names and DTOs.
  New generic commands: `get_sources` + source settings, `search_source`, `search_top`,
  `get_track_summaries`, `set_favorite(ref, on)`, `library_*(source, …)`. Local-only commands:
  roots, rescan, tag health, tag writes, M3U8 import/export. Queue/play commands keep taking
  `QueueItem` (now with refs). Phone remote scope stays playback, queue, likes and browsing:
  library reads, search and summaries are allowlisted; tag editing, roots, rescans, imports and
  source settings are `NOT_EXPOSED`.
- **V2-D27 — async style** (agreed 2026-10-17). The new source traits use `async-trait` (already in
  `Cargo.lock` transitively): same one-allocation-per-call cost as hand-written boxed futures, but
  methods read as ordinary `async fn` and borrow `self`. Native `async fn` in traits is not used —
  it is not `dyn`-compatible, which V2-D25 needs. `remote/`'s hand-written `BoxFuture` traits stay
  as they are.
- **V2-D28 — audio analysis scope** (agreed; refines V2-D14).
  - **2.0 features per local track** (in `local.db`, tagged with `analysis_version`): the AutoMix
    edge measures for both ends (lead/trailing silence, audible end, ending kind, decline, intro
    length, body loudness — the halves `automix::features()` measures live today), beat grids for
    the first and last ~30 s, a whole-track tempo estimate, a coarse energy curve (RMS per ~2 s) and
    brightness / low-end balance.
  - **2.1:** computed ReplayGain for untagged tracks (EBU R128, stored in the index only; `ebur128`
    crate to be confirmed), musical key, and a full-track beat grid.
  - **AutoMix:** `plan_transition` is split into *measure* and *plan from measures*. Local sides use
    stored measures; TIDAL sides are measured live; with nothing stored the live path is unchanged.
    Stored measures are taken before ReplayGain and gain is applied at plan time; a test checks
    stored == live on the same audio.
  - **Scheduling:** one background thread at the lowest OS priority, one track at a time,
    reduced-rate mono downmix. Order: upcoming queue tracks → recently added → the rest. Yields
    during scans, track starts/seeks and live AutoMix analysis; forced off under
    `OXITIDE_PERF_SCENARIO`. Each `analysis` row stores the track's **audio key** (V2-D32) as its
    content key; a track is re-analysed when `analysis_version` is bumped or its audio key changes.
    A tag-only edit leaves the audio key, and so the analysis, untouched.
  - **Toggles** (Settings → Sources → Local): *Analyse library* (on) and *Pause analysis while
    playing* (off).
  - **Known costs** (documented in Settings help text and the plan): one CPU core at background
    priority for the first pass (~10–30 min estimated for ~750 FLAC tracks, then only new files);
    sequential file reads — on a slow USB HDD this competes with the playing track's reads, so
    analysis **skips files on the volume the current track plays from** while playing; ~10–20 MB of
    transient memory per track; laptops on battery pause analysis. A perf scenario plays through
    analysis and must show no underruns and no change in S4 advance gaps.
- **V2-D29 — Local index schema** (agreed 2026-10-17; ids revised the same day). `local.db` is a
  rebuildable cache (V2-D6); roots live in `config.toml` (V2-D7). Tables:
  - `tracks` — **id = the audio key** (`local:<id>`); format (codec, rate, depth, channels,
    duration); primary location id. Nothing tag-derived.
  - `locations` — one row per file: track id, root, relative path, size, mtime, available, and that
    file's indexed tag fields (title, display artist, album id, disc/track no, year, genre,
    ReplayGain values, explicit flag, lyrics flag, art id). Several locations may share one track
    (V2-D6); album membership comes from locations.
  - `albums`, `artists` + `track_artists` (role, position) — **ids derived from the normalised
    grouping key** (album: album artist + title, or the folder; artist: MBID when present, else the
    normalised name), so a rebuild reproduces them.
  - `art` (content hash, embedded/folder origin, size), `analysis` (version, audio key, features
    JSON), and an FTS5 table over title / artists / album for search (FTS5 in the bundled SQLite
    build to be verified).
  - One file = one location; one track per distinct audio. FTS rows are per location. The tag editor
    reads the full tag set from the file on open; the index keeps only the fields above.
  - **Album grouping:** normalised (case, whitespace) album artist + album title; without an album
    artist, the folder; compilations from `COMPILATION` or "Various Artists".
  - **Album/artist renames** change the derived id. Favorites recover as described in V2-D12;
    history name links already resolve an album/artist through the track when its ref is stale
    (`lib/queueLinks.ts`).
  - **Artist splitting:** only multi-value tag fields (e.g. several FLAC `ARTIST` entries) and `;`.
    Never `/` or `&` by default (AC/DC, Simon & Garfunkel); "feat." stays in the display string in
    2.0.
- **V2-D30 — Local artwork** (agreed).
  - **Choice:** embedded front cover first, folder image (`cover` / `folder` / `front` `.jpg/.png`)
    as fallback; a setting *Prefer folder image over embedded art* (off) flips the order. When an
    album's tracks disagree, the most common image wins. Images are deduplicated by content hash.
  - **Serving:** a custom protocol serves images **by hash only** (never by path). `coverUrl` takes
    an `ArtRef`: `tidal:<uuid>` builds today's CDN URL, `local:<hash>` the protocol URL. WebView2
    exposes custom protocols as `http://<name>.localhost/…`, WebKitGTK as `<name>://…`; the CSP
    allowlist carries both.
  - **Sizes:** thumbnails at the existing `CoverSize` steps (80–1280), generated on first request
    with the `image` crate into a disposable `cache/art/`.
  - **Elsewhere:** `analyze_cover` reads the cached thumbnail (palettes keyed by `ArtRef`); SMTC /
    MPRIS get the thumbnail file; the phone remote serves `/api/art/<ref>` behind the pairing
    cookie; Discord shows the OxiTide logo.
- **V2-D31 — durability of user data** (owner review 2026-10-17, revised the same day). Local track
  ids come from the audio (V2-D6) and album/artist ids from their grouping keys (V2-D29), so
  **`local.db` holds nothing irreplaceable**: losing it costs a rescan and re-analysis, never a
  broken ref. Durability effort goes to what cannot be rebuilt.
  - **Protected set:** `library.db` (OxiTide playlists and favorites) and the files that hold refs
    or settings: `history.jsonl`, `track_flags.json`, `session.json`, `config.toml` (incl. library
    roots). `local.db` and everything under `cache/` are rebuildable and are not backed up.
  - **Rolling backup sets.** A set is one folder `backups/<UTC timestamp>-<reason>/` holding a
    `VACUUM INTO` copy of `library.db`, copies of the protected files, and a `manifest.json` (app
    version, schema version, row counts, reason). The database copy is written as `.tmp`, passes
    `PRAGMA quick_check`, then is renamed; a set that fails is deleted and logged, never half-kept.
    - **When:** daily (first idle moment after startup when the newest daily set is older than
      24 h) and before risky operations: schema migrations, "Forget missing tracks" (when it also
      drops dangling entries), M3U8 import, a restore, playlist deletion, and the V2-D4 / V2-D16
      carry-over and migration.
    - **Retention:** 7 daily + the last 5 pre-operation sets (well under a few MB each).
    - **Location:** the data root (same disk). Settings shows the folder and offers *Export
      backup…* (a zip to any location) for an off-disk copy.
  - **Restore.** Settings → Sources → Backups lists sets (date, reason, counts). Restoring takes a
    pre-restore set first, renames the current files to `*.replaced-<ts>`, copies the chosen set in
    and restarts the app; restoring the JSON/TOML files is a checkbox (on by default). Because local
    ids are content-derived, a restored set of any age resolves against the current library.
  - **Unreadable-file policy** (each database, on open; mirrors the newer-version rule):

    | Condition | `library.db` | `local.db` |
    |---|---|---|
    | `user_version` newer than this build | Not opened; renamed `<file>.newer-v<N>`; warning; offer the newest compatible backup or an empty start | Renamed `<file>.newer-v<N>`; rebuilt by rescanning; brief notice |
    | Open fails, not a database, or `quick_check` fails | Renamed `<file>.corrupt-<ts>` (never deleted); the newest set whose copy passes `quick_check` is restored automatically; the warning states the backup's age | Renamed `<file>.corrupt-<ts>`; rebuilt by rescanning; brief notice |
    | No usable backup | Empty start + warning (playlists and local favorites lost) | — |

    The check runs on the startup background thread, off the S1 critical path; the source shows
    "Checking library" and holds writes until it passes. A `local.db` rebuild shows tracks as
    "indexing" (V2-D8) until their audio keys exist; analysis re-runs in the background.
  - **Cross-file consistency.** Refs between files are weak: every reader tolerates a dangling ref
    (renders "unavailable"; never crashes, never deletes). Nothing is garbage-collected
    automatically; "Forget missing tracks" offers, as a separate checkbox, to also drop dangling
    favorites, playlist entries and flags. A ref is only handed out once its id exists (a track
    still "indexing" has none).
  - **SQLite durability:** WAL everywhere; `synchronous=FULL` for `library.db` (low write rate,
    irreplaceable) and `NORMAL` for `local.db` (scan bursts; a lost commit is rebuilt by the next
    scan).
- **V2-D32 — audio key** (owner review 2026-10-17, revised the same day). One content key per local
  file that changes only when the audio does. It **is** the local track id (V2-D6) and also drives
  analysis invalidation (V2-D28) and tag-write verification (V2-D33). Keys are prefixed by method so
  they never collide (the id's encoding — e.g. one prefix letter + base32 — is a plan detail):
  - **`flacmd5:<hex>`** — FLAC with a non-zero STREAMINFO MD5 (an MD5 of the decoded samples). Free:
    read from the header, no extra I/O. Stable across tag edits, padding changes, and even re-encoding
    at another FLAC compression level (same samples).
  - **`pkt:<xxh3-128>`** — every other 2.0 format (MP3, M4A/ALAC/AAC, WAV, AIFF, Ogg Vorbis) and FLAC
    with a zero MD5: a hash over the compressed audio packets, in order, as the symphonia demuxer
    returns them. Container metadata (ID3v1/v2, APE tags, Vorbis comments, MP4 `ilst`, RIFF `LIST`)
    is not part of a packet. Cost: one full read of the file without decoding, only when size or
    mtime changed (and right after our own tag writes).
  - **"Best effort" means exactly:** the key is stable across moves, renames and any edit that only
    rewrites tag/metadata blocks. It changes on re-encoding, transcoding, trimming, and tools that
    rewrite audio frames in place (MP3Gain-style global-gain edits, frame repairs). There the track
    gets a new id (and is re-analysed); the old refs show "unavailable" until "Forget missing
    tracks".
  - **Verified, not assumed:** whether the MP3 Xing/LAME info frame or Ogg header packets reach the
    hash depends on the demuxer; fixture tests must show that a `lofty` tag-only rewrite leaves the
    `pkt:` key unchanged for every 2.0 format before the scanner plan is done.
  - **Duplicates** (same key in several files) are one track with several locations (V2-D6).
  - **Partly unreadable audio:** a file the demuxer cannot read to the end (e.g. damaged frames)
    gets a `meta:` key (hash of duration + title + artist + album). It plays as far as it decodes,
    but its id would change on any tag edit, so the tag editor is **disabled** for it and the
    tag-health view lists it as "can't be identified reliably".
- **V2-D33 — atomic tag writes per OS** (owner review 2026-10-17). Per file:
  1. **Pre-checks:** the file is writable and not read-only; free space on its volume ≥ file size +
     margin.
  2. **Copy** the original to `.<name>.oxitide-tmp` **in the same directory** (same volume — needed
     for an atomic replace).
  3. **Write** the tags into the copy with `lofty`; flush it (`sync_all`).
  4. **Verify:** re-read the copy — the written fields match the request, the **audio key equals the
     original's** (a write that touched audio is rejected), and the file probes and decodes its
     first frames.
  5. **Replace:**
     - **Windows:** `ReplaceFileW(original, copy, no backup, REPLACEFILE_IGNORE_MERGE_ERRORS)`. It
       keeps the original's creation time, attributes, security descriptor, alternate data streams
       and object id, which `MoveFileExW` would not. Where the volume rejects it (some FAT/exFAT
       cases), fall back to `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` after
       copying attributes and creation time onto the copy.
     - **Linux:** copy the mode and (best effort) owner/group onto the copy, `fsync` it,
       `rename(copy, original)` (atomic within one filesystem), then `fsync` the directory. Extended
       attributes are not carried in 2.0.
  6. **Index:** update that location's row directly (size, mtime, tags; the audio key is
     unchanged).
  - **Timestamps:** the modification time becomes the write time (other tools and backup software
    see the change); creation time is preserved on Windows; Linux has no settable birth time.
    "Preserve modification time" is 2.x.
  - **Files in use:** playback opens local files with `FILE_SHARE_READ | FILE_SHARE_DELETE` on
    Windows, so OxiTide's own playback never blocks a replace; on Linux the playing handle keeps the
    old inode. A Windows test (NTFS and exFAT) must show the playing handle keeps reading cleanly
    after a replace; if it does not, edits to the playing track are applied when playback leaves
    it, and until then the track shows **"Saved — will be written when this track finishes"** so a
    deferred write never looks like a lost one. Another process holding the file without delete
    sharing → retry with backoff, then "file in use"; the original is untouched.
  - **Several locations:** the editor writes the location it was opened from (from an album view,
    that album's file) and says how many other copies exist; they keep their own tags, and the
    track's id is unaffected.
  - **Failure handling:** any failed step deletes the copy and leaves the original byte-identical.
    Multi-file edits are atomic per file, not per batch, and end with a per-file report
    (`library.db` is not touched by tag edits). Leftover `*.oxitide-tmp` files from a crash are
    removed by the next scan of that folder. Undo (journalled previous values) is 2.x.
- **V2-D34 — legacy read table** (owner review 2026-10-17). Every persisted 1.x shape and how 2.0
  reads it; writes always use the canonical 2.0 form. The 1.0 gate's versioning step writes the id
  rows (`track_id`, `album_id`, `artist_refs`) as `tidal:<n>` strings already
  (`plans/notes/2026-10-17-v1-release-gate.md`); 2.0 still reads the bare-number shapes below.

  | 1.x field | 1.x shape | 2.0 read |
  |---|---|---|
  | `track_id` in `QueueItem`, `SessionItem`, `HistoryRecord`, `QueuedScrobble.item` | number | `TrackRef` `tidal:<n>` |
  | `album_id` (same records) | number | `AlbumRef` `tidal:<n>` |
  | `artist_refs[]` | `{ id: number, name }` | `ArtistLink { ref: tidal:<id>, name }`; the old object shape is accepted |
  | `cover` (same records) | dashed TIDAL UUID, an absolute `http(s)` URL (page-feed mix images), or null | UUID (8-4-4-4-12 hex) → `ArtRef` `tidal:<uuid>`; `http(s)://…` → `url:<url>`; anything else → `None` with one log line |
  | `track_flags.json` map keys | `"<n>"` | `tidal:<n>` |
  | `session.json` `play_source` | TIDAL playlist UUID | `PlaylistRef` `tidal:<uuid>` (OxiTide playlists are `oxitide:<uuid>`) |
  | lyrics cache map keys | `"<n>"` | `tidal:<n>` |
  | `cache/palettes.json` keys | TIDAL image UUID | `tidal:<uuid>` (a cache: unreadable entries are dropped) |
  | disk-cache file names | `<n>-<quality>-<hash>.fmp4` | unchanged for `tidal:` refs (V2-D24) |

  - `url:` refs keep the phone remote's rule: only TIDAL image hosts pass the RPC boundary.
  - Every row has a fixture test built from real 1.x files.
  - **What 1.0 writes** (versioning step, 2026-10-23, `crates/oxitide-app/src/stored_ref.rs`):
    `session.json` version 2 and `history.jsonl` lines with `"version": 1` carry `track_id` /
    `album_id` as `"tidal:<n>"` and `artist_refs` as `[{ "ref": "tidal:<n>", "name" }]` (the
    `ArtistLink` shape); earlier files hold the bare-number shapes in the table. IPC payloads keep
    numbers. Not converted, so 2.0 reads them as above: `cover`, `play_source`, map keys, and
    `session.json` `play_from.track_ids` (numbers, the UI's shape; read as `tidal:<n>`).
    `QueuedScrobble` is memory-only in 1.x (nothing persisted).
- **V2-D35 — local file playback: prefetch, gapless, slow drives** (owner review 2026-10-17).
  - **Read-ahead:** `StreamSource::File` is read through a **seekable read-ahead reader** (a
    background thread keeps ~4 MB ahead of the decoder; a seek flushes it and repositions). A slow or
    sleeping drive starves the read-ahead, not the decoder; starvation shows `PlaybackState.buffering`
    exactly like a network stall, on the same session and open device.
  - **Seeking:** symphonia `FormatReader::seek` on the file (accurate seek), not the single-file
    "decode from the start and discard" path.
  - **Prefetch:** at today's prefetch point the next local track is opened, probed and its opening
    pre-read into `TrackStream.pending`, exactly like the HTTP path, so AutoMix and gapless see the
    same shape. Files on removable volumes are opened earlier (drive spin-up can take seconds); the
    plan sets the lead time.
  - **Gapless:** symphonia gapless mode (`enable_gapless`) for every local file. FLAC and WAV/AIFF
    need no trimming; MP3 trims LAME/Xing delay and padding (V2-D9 open check — our own LAME-header
    parse if symphonia does not); M4A uses `iTunSMPB` / edit lists where the demuxer provides them.
    Device continuity across tracks follows today's rules (same format → same open device; the
    shared-mode AutoMix path, AM-D16, for a format change).
  - **Missing files:** a file gone at prefetch or open (drive unplugged, file deleted) marks that
    location unavailable; the track plays from another available location if it has one, else it is
    marked unavailable and the queue moves on with a notice; a read error mid-track after the read-ahead
    drains ends that track the same way. No retries against a removed volume.
  - **No disk cache, no download-ahead, no `AheadGate`** for local files.
- **V2-D36 — source states** (agreed 2026-10-17). `get_sources` / `source_changed` report per
  source a `state` and an independent `activity`:
  - `state`: `ready` · `needs_setup` (enabled, never configured) · `signed_out` (TIDAL session gone)
    · `offline` (TIDAL unreachable; cached tracks still play) · `unavailable` (Local: every library
    drive missing) · `error` (needs attention, with a message) · `disabled` (switched off; hidden
    from sidebar and search; its refs show "source disabled").
  - `activity`: `none` · `scanning {done, total}` · `indexing {n}` · `analysing {done, total}`.
    Background work never blocks: a scanning library is `ready` and browsable.
  - **TIDAL expiry / sign-out:** when TIDAL is the **only** enabled source, the full-screen login
    returns (1.x behaviour). When another source is enabled, the app keeps running, TIDAL's pages
    show an inline "Sign in", and **a notification appears on every app start** while TIDAL stays
    signed out ("TIDAL is signed out" — actions *Sign in* and *Disable TIDAL*; disabling stops the
    reminder).
- **V2-D37 — onboarding** (agreed). Shown on first run and whenever no source is enabled; migrated
  1.x users skip it (TIDAL is enabled with their session). Mini mode exits for it, as for login
  today.
  1. **Choose:** one toggle card per **shipped** source (no "coming later" cards); at least one.
  2. **One step per chosen source**, each with *Skip, set up later*:
     - TIDAL — the `Login.svelte` content as a step (paste-back only in previews, V2-D4) with the
       keyring notice;
     - Local — *Add folder* (native picker), a list of chosen folders with volume label and a
       "USB drive" / "This PC" badge, then *Start*: the scan runs in the background.
  3. **Done:** land on the default source's start page; Local fills in as it scans.
  Skipping every chosen source returns to the choice screen.
  - **Folder picker:** `tauri-plugin-dialog`, called **from Rust** (`local_pick_folder`), so the
    webview gets no file-system capability and only user-picked folders reach the app (the updater
    pattern, T12-D6). Command classified `NOT_EXPOSED`.
- **V2-D38 — Settings → Sources** (agreed). A new **first** Settings tab.
  - **Per source card:** state chip + activity, on/off (disable), *Default at startup*, reorder with
    ↑/↓ buttons (keyboard-operable), *Remove*.
  - **TIDAL:** Account / Log out (moved from System), streaming quality and allow-fallback (moved
    from Playback). Settings search still finds them under their old names. The cache stays in
    Storage.
  - **Local:** folders (path, volume label, USB / This PC, connected or missing, track count,
    remove), *Add folder*, *Rescan*, *Forget missing…*, analysis toggles, folder-art preference, a
    link to Tag health.
  - **Removing a source** keeps its favorites and playlist entries by default (they show
    "unavailable"). The remove dialog offers *Also delete favorites and playlist entries from this
    source*, unchecked; checking it requires a second confirmation that says it is **final** (a
    pre-operation backup set is still taken, V2-D31). Removing TIDAL signs out and clears its
    settings; removing Local forgets its folders and deletes `local.db` (a cache).
  - **"Forget missing tracks"** follows the same pattern: its "also remove dangling favorites /
    playlist entries / flags" checkbox is off by default and checking it needs the final-warning
    confirmation.
  - **Library backups** (list, restore, export, V2-D31) live in **Storage**, not Sources: they
    cover app data, not one source.
- **V2-D39 — tag editor** (agreed 2026-10-17; refines V2-D10, writes per V2-D33).
  - **Entry points:** a track's *Edit tags…* (one file); an album's *Edit album tags…* (its files,
    in album order); a multi-selection in Local Tracks; a Tag health row's *Fix…* (pre-filled).
  - **UI:** a modal dialog (existing `focusTrap` / `dialogs.svelte.ts`) with a cover panel and the
    fields: Title; Artist(s) and Genre (multi-value); Album; Album artist; Track no / total; Disc no /
    total; Year; Composer; Compilation; Comment; embedded front cover (replace, remove, use folder
    image).
  - **Several files:** a field shows the shared value or ‹multiple›; only touched fields are
    written. Helpers: *Number in order* (track numbers + total from the current order), *Apply cover
    to all*, *Use folder image*.
  - **Validation:** track/disc numbers positive and ≤ their totals; year a 4-digit year or ISO
    date; covers JPEG/PNG, warning above 2 MB, never resized automatically.
  - **Saving:** the V2-D33 path file by file with progress, then a report ("11 saved, 1 in use —
    retry"). Files with a `meta:` audio key are disabled with a reason (V2-D32).
  - **Formats:** FLAC — Vorbis comments, multi-value as repeated fields. M4A — `ilst` atoms. MP3 —
    **an existing ID3v2 tag keeps its version** (files are never converted); files with no ID3v2 tag
    get **ID3v2.3** (UTF-16 text) for the widest compatibility (older Windows, car stereos and
    hardware players often read only v2.3; Linux, macOS and Android read both). The editor needs
    nothing v2.4 adds over v2.3 except native multi-value frames, so v2.3 multi-values are joined
    with `"; "`, which V2-D29 splits on (never `/`). An existing ID3v1 tag is updated, never
    added. Whether `lofty` writes v2.3 and preserves a file's version must be verified; if not, MP3
    writes use the `id3` crate (v2.2–2.4 read/write).
  - **2.1:** plain-text lyrics editing and the "other tags" panel (MusicBrainz ids, ReplayGain,
    encoder… — viewing and raw editing).
- **V2-D40 — tag health** (agreed).
  - **Checks:** missing title / artist / album / cover / track numbers; missing album artist on
    albums with more than one track artist; within an album, inconsistent album artist, year, genre,
    disc total or track total; duplicate track numbers on one disc; near-duplicate names across the
    library (case, trailing spaces/punctuation); "can't be identified reliably" (V2-D32).
  - **Rows:** issue, album, file count; *Fix…* opens the editor pre-filled; **one-click fixes**
    where the answer is clear (*Use the most common value*, *Set album artist to "…"*, *Number in
    order*) show the exact change first and go through the same write path and report; *Ignore* is
    stored per album + issue in `library.db` (so it is backed up) and listed under "Ignored" to undo.
  - Results recompute after each scan and save; the issue count shows on the sidebar entry and the
    Local card in Settings → Sources.
- **V2-D41 — Local pages** (agreed 2026-10-17).
  - **Sidebar group:** Albums (grid; sort by artist / title / year / recently added; genre filter
    chip), Artists (album artists by default, *Show all artists* toggle adds featured / track-only
    artists), Tracks (full table, sort + filter, built for 10k+ rows like Collection Tracks),
    Folders (roots → subfolders → files; *Play folder*, *Add to queue*), Tag health (V2-D40). Album
    and artist detail pages reuse today's detail layouts. No Genres page; genre is a filter (genres
    may surface in Explore later).
  - **Extras:** *Show in folder* (reveals the file in Explorer / the file manager, from Rust through
    the existing opener plugin); the Info panel shows file details for local tracks (path, drive,
    format, bitrate, size, number of copies).
  - **Shared components are generalised, not copied:** `TrackTable`, `MediaCard`, `DetailHeader`,
    `Cover` and `ArtistLinks` take source-neutral row types built from the summary types; TIDAL
    views convert at their boundary (V2-D20). They are the S5/S6-sensitive components, so the change
    is re-measured against the current budgets.
  - **UI modes:** no per-mode Local pages (components never branch on mode); Local pages inherit
    Atmosphere / Instrument / Rack through tokens. New elements get per-mode notes in each
    `ui/design/*/DESIGN.md`: source state chip, "indexing" / "unavailable" row states, USB / This PC
    badges, folder rows, tag-health rows, onboarding cards, sidebar scan progress. Dynamic color works
    on local covers through the cached thumbnails (V2-D30).
  - **2.1 or later:** a Rack faceplate INPUT selector (`TIDAL · LOCAL`) inside `RackShell`.
- **V2-D42 — perf scenarios and budgets** (agreed 2026-10-17). Added to `scripts/perf/budgets.json`
  in the M3/M4 plans; existing TIDAL budgets are unchanged (parity gate, V2-D5).
  - **Fixture library:** generated, never the owner's files — 1k and **20k** tracks (20k is the
    headroom target), 90 % FLAC with STREAMINFO MD5 / 10 % MP3, realistic folder depth, compilations,
    and **embedded covers of realistic size** (300–800 KB) on a share of files, since art hashing
    reads image bytes. Audio is short (~5 s) so per-file cost is what is measured.
  - **Memory budgets:** measured baseline + 5 % (PM-D1 method), set in the plans.
  - **Scripted (Windows, T8-D3):**

    | # | Scenario | Budgets |
    |---|---|---|
    | S11 | First scan, internal disk | first tracks listed ≤ 1 s; 1k ≤ 10 s; 20k ≤ 120 s; UI `long_task_max_ms` ≤ 100 during the scan |
    | S12 | Rescan, no changes, 20k | ≤ 5 s |
    | S13 | Local time-to-first-sound | `ttfs_ms` ≤ 300 (= S3-hit), mean ≤ 150 |
    | S14 | Local gapless advance | `advance_max_gap_ms` ≤ 100 (= S4) |
    | S15 | 20k library: scroll, filter, sort, album open | S5/S6 frame budgets per mode; filter ≤ 50 ms; sort ≤ 150 ms; `album_open_ms` ≤ 500 |
    | S16 | Playback during analysis | `underrun_count` = 0; advance gap within S4; `long_task_max_ms` ≤ 100 |
    | S1-local / S2-local | Start with Local enabled (20k index) | S1 / S2 budgets unchanged |
    | S7 (re-run) | 10k queue build, mixed refs | `queue_build_ms` ≤ 250 unchanged |

  - **Manual:** S17 — USB drive: first sound from a sleeping drive (plays, or shows "buffering" and
    recovers in the same session); unplug while playing (skips with a notice).
  - **Not budgeted:** first scans of large non-FLAC libraries on slow USB drives (read-bound by
    design, V2-D8).
  - **Reference points:** S11's 20k ≤ 120 s is ~6 ms per file. A Clementine / Strawberry user report
    puts a from-scratch scan of 26,610 mostly-FLAC files on an SSD at ~9 m 40 s (~22 ms per file),
    and old foobar2000 reports put a 5k rescan near a minute, so the budget is ambitious but
    plausible for header-only reads; it is tightened or ruled on once measured. S13 equals today's
    cache-hit budget (a local file is the same kind of read); S14 and S15 reuse the S4/S5/S6 numbers;
    S15's album open is half of S6's because no network is involved.
  - **`underrun_count`** is a new engine metric; it is added to the **1.x** engine as well (1.0
    release gate) so S4 / S9 runs can report it now.

## 4. Architecture

The overview below was the starting proposal; where it differs from V2-D19–D35, the decisions win.

### 4.1 Crates

| Crate | Role | Change |
|---|---|---|
| `oxitide-core` (new) | `TrackRef`, `SourceId`, neutral `Quality` / `AudioFormat`, `ReplayGain`, `TrackMeta`, `ArtRef`, `SourceError`, capability flags, source traits | Pure types + traits; no I/O |
| `oxitide-audio` | Engine, queue, decode, DSP, AutoMix, outputs, cache | Depends on `oxitide-core` only; `QueueItem.track_id: u64` → `TrackRef`; `Error::Tidal` → `SourceError` |
| `oxitide-tidal` | TIDAL HTTP client | Unchanged API; an adapter implements the source traits |
| `oxitide-local` (new) | Scanner, audio keys, SQLite index (rebuildable), tags (`lofty`), artwork, analysis store, tag writes | Implements the source traits |
| `oxitide-app` | Tauri shell | Owns the source registry; commands dispatch by source |

Sources are compiled in behind cargo features; no runtime plugin system.

### 4.2 Two source traits (sketch)

- **Playback side** (engine-facing, sync, called on the worker thread — today's `StreamResolver`
  generalised): `resolve(&TrackRef, QualityPref) -> Result<StreamSource, SourceError>` where
  `StreamSource` is
  - `Segmented(StreamPlan)` — today's DASH/BTS path (TIDAL), with the disk cache and
    network-resilience machinery unchanged;
  - `File(PathBuf)` — the chosen available location of a local track, read through the seekable
    read-ahead reader with real format seeking (V2-D35); no cache, no download-ahead;
  - `Http { url, headers }` — reserved for 2.x Jellyfin/Qobuz (one file over HTTP, auth headers on
    media requests; Jellyfin 12 rejects tokens in URLs).
  It also returns `ReplayGain` and optional stored analysis features for AutoMix.
- **Catalog side** (app-facing, async): a `Source` with `capabilities()` and optional sub-interfaces
  (search, library browse, favorites, playlists, lyrics, credits, mixes, tag editing, artwork). The
  UI receives the capability set per source and renders only what exists — no lowest-common-
  denominator flattening; TIDAL-only pages stay TIDAL-only.

### 4.3 App, IPC and UI

- **Registry:** `AppState` holds the enabled sources; `[sources]` in the config stores enablement,
  order and the default source; per-source sections hold their settings (`[local]`: roots, analysis
  toggles, cover preference).
- **Commands:** existing TIDAL commands keep their names (no churn for the phone remote); new
  source-generic commands take a `TrackRef` / `SourceId`. Every new command is classified in the
  remote's `ALLOWLIST` / `NOT_EXPOSED` as today.
- **Artwork:** `coverUrl` (dashed TIDAL UUID → `resources.tidal.com`) becomes `ArtRef`-based. Local
  art is served through a custom protocol (new CSP `img-src` entry) and, for the phone, by the remote
  server.
- **IDs over IPC:** `TrackRef` travels as its string form; `api.ts` mirrors the new DTOs by hand.

### 4.4 Data

- Per-source state stays with its source (TIDAL session/tokens; Local's rebuildable `local.db`).
- Cross-source state keyed by refs: queue/session, history, track flags, scrobble queue, palettes,
  lyrics cache, and `library.db` (OxiTide playlists, favorites for sources without server likes).
  The irreplaceable part is backed up per V2-D31.

## 5. Milestones

| M | Content | Visible change |
|---|---|---|
| M0 | `release/1.x` cut; preview identity switch (V2-D4) | Preview installs side by side |
| M1 | `oxitide-core`; `TrackRef` through engine, queue, saved data; neutral errors/quality; tolerant reads | None (TIDAL-only refactor) |
| M2 | Source registry; TIDAL as a source; capability DTOs; `StreamSource` enum | None |
| M3 | `oxitide-local`: roots/volumes, audio keys (content-derived ids), scanner, rebuildable index with locations, `library.db` backup sets + restore + unreadable-file policy (V2-D31), file playback with read-ahead, seeking and gapless (V2-D35) | Local plays (behind a setting) |
| M4 | UI: onboarding, sidebar by source, Local pages, search tabs, badges, Settings → Sources; local favorites (`library.db`) | Local usable end to end |
| M5 | OxiTide playlists + M3U8 (V2-D21/D22); tag health + editor (V2-D33); integrations (V2-D13); phone remote art | Feature matrix complete |
| M6 | Audio analysis + AutoMix stored features; local mixes | Mixes and smoother local transitions |
| M7 | Preview-root adoption (V2-D4) and 1.x migration (V2-D16), parity gate (V2-D5), perf, smokes, 2.0 release identity | `v2.0.0` |

Each milestone gets its own dated plan in `docs/superpowers/plans/`.

## 6. Components (first estimate)

| Piece | File(s) |
|---|---|
| Core types | `crates/oxitide-core/` (new) |
| Engine | `oxitide-audio`: `engine.rs`, `queue.rs`, `source.rs`, `seek.rs`, `cache.rs`, `error.rs`, `decode.rs`, `automix.rs` |
| Local source | `crates/oxitide-local/` (new): scanner, audio keys, index (SQLite, rebuildable), tags, art, analysis store |
| App | `player.rs` (resolver → registry/router), `commands.rs`, `state.rs`, `config.rs` (`[sources]`, `[local]`), `session.rs`, `history.rs`, `track_flags.rs`, `scrobble/`, `lyrics.rs`, `media_controls*.rs`, `discord/`, `remote/`; new: `library.db` store (playlists, favorites), backup sets + restore, analysis job, preview-root adoption |
| Identity | `tauri.conf.json` + a preview overlay; `config.rs` data root; keyring service; `scheme.rs` (off in previews) |
| UI | `App.svelte` (gate, onboarding), `nav.svelte.ts` (source routes), `artwork.ts`, `api.ts`, new Local views, tag editor/health, Settings → Sources, search tabs |
| Docs | `AGENTS.md`, `ui/design/*/DESIGN.md`, README, `THIRD_PARTY_NOTICES.md` (`rusqlite`/SQLite, `lofty`) |

## 7. Testing

- **Fixtures, never the owner's library:** small FLAC/MP3/M4A files generated in tests (tones with
  known tags, a FLAC with and without STREAMINFO MD5, an MP3 with a LAME header) under
  `crates/oxitide-local/tests/fixtures/`.
- **Rust:** identity (move, rename, tag edit, move + tag edit keep the id; duplicates become one
  track with several locations; the same audio on an album and a compilation stays listed on both
  with each file's own tags; a deleted `local.db` rebuilds to the same track, album and artist
  ids), volume absent/present, incremental rescan, `TrackRef` round-trips, file seeking accuracy,
  analysis determinism, the remote allowlist test, and:
  - **audio key:** a `lofty` tag-only rewrite leaves `pkt:` / `flacmd5:` keys unchanged for every
    2.0 format; an audio change alters them; unreadable audio falls back to `meta:` (V2-D32);
  - **durability:** backup sets pass `quick_check`; restore round trip, including an old set
    resolving against the current library; each cell of the unreadable-file table (newer version,
    truncated file, garbage bytes, no backup) for both databases (V2-D31);
  - **carry-over:** preview root adoption merges 1.x history newer than the import point without
    duplicates, and unions flags (V2-D4);
  - **tag writes:** a failure injected at every step leaves the original byte-identical; Windows
    `ReplaceFileW` keeps creation time and attributes; the playing-handle test on NTFS and exFAT;
    leftover temp cleanup (V2-D33);
  - **legacy reads:** one fixture per V2-D34 row, built from real 1.x files;
  - **local playback:** read-ahead starvation → `buffering`, unplug at prefetch / mid-track, a
    missing primary location falling back to another copy, MP3 and M4A gapless trimming (V2-D35).
- **UI (vitest):** onboarding states, source gating, search tabs, capability-driven rendering, tag
  editor validation.
- **Perf (new scenarios):** S11–S17 and the S1/S2/S7 re-runs with their budgets are V2-D42.
  Analysis is forced off in all of them except S16, which plays through analysis (V2-D28).
- **Smokes (owner):** onboarding (local-only and TIDAL), USB drive unplug/replug, file move survives
  history, tag edit round trip, MP3 gapless, AutoMix local transitions, phone remote local art,
  preview side by side with 1.x, migration on a copy of real 1.x data, preview → release adoption,
  deleting `local.db` and confirming history/favorites/playlists still resolve after the rescan.

## 8. Risks

- **Refactor reach:** `track_id` appears in ~140 files; M1 must land as small, test-guarded steps
  while 1.x fixes keep merging forward.
- **Engine regressions:** the TIDAL path (cache, network resilience, AutoMix) must stay behaviour-
  identical; the parity gate and S3/S4 perf scenarios are the check.
- **Tag writes touch the owner's files:** the V2-D33 path and its failure tests are mandatory
  before the editor ships; a one-time "back up your library first" notice.
- **Loss of `library.db`** loses OxiTide playlists and local favorites; V2-D31 (backup sets,
  restore, unreadable-file policy) is a prerequisite of the first preview that writes them, not a
  late milestone. `local.db` loss is a rescan by design (V2-D6).
- **Content-derived ids** depend on the audio key being stable under tag-only rewrites for every
  format; the V2-D32 fixture tests gate the scanner plan. A large non-FLAC library on a slow USB
  drive has a long first scan (full read per file before tracks become playable).
- **Removable-drive identity** differs per OS and filesystem (exFAT/NTFS/ext4); needs early tests on
  real USB drives on both platforms.
- **MP3 gapless** may need our own LAME-header handling if symphonia does not read it.

## 9. Open points for the architecture discussion

1. ~~`oxitide-core`~~ — decided, V2-D24.
2. ~~`TrackRef` representation~~ — decided, V2-D19/D20.
3. ~~Catalog interface~~ — decided, V2-D25; async style V2-D27.
4. ~~Commands / phone remote~~ — decided, V2-D26.
5. ~~Playlists~~ — decided, V2-D21/D22. Storage: V2-D23.
6. ~~Analysis~~ — decided, V2-D24 (where it runs) and V2-D28 (scope, AutoMix, scheduling, costs).
7. ~~Index schema~~ — decided, V2-D29 (schema versions: V2-D23).
8. ~~Artwork~~ — decided, V2-D30. The protocol's name is left to the plan.
9. Release numbering for Jellyfin (2.1 alongside the V2-D28 extras, or 2.2) — owner, later.
10. ~~Product details~~ — decided: onboarding / Settings → Sources (V2-D36–D38), tag editor / tag
    health (V2-D39–D40), Local pages (V2-D41), perf scenarios and budgets (V2-D42).
