# OxiTide — Post-MVP Roadmap (Plan 6+)

- **Status:** Approved (owner-confirmed 2026-09-24); execution order revised 2026-09-28; statuses refreshed 2026-10-01
- **Date:** 2026-09-24
- **Revised:** 2026-09-28 — execution order reordered to Tracks 4 → 5 → 6 → 3 → 7 after Plan 10; Track labels stay stable (see PM-D5).
- **Revised:** 2026-09-30 — Tracks 4, 5, and 6 complete; Track 3 active (spec approved, Phase A planning).
- **Revised:** 2026-10-01 — Track 3 Phase A (harness + baseline) complete; no budget recalibration; Phase B (startup + TTFS) complete; Phase C next.
- **Revised:** 2026-10-01 — ASIO output assessed and parked as an optional, uncommitted Track 7 candidate (spike note: `plans/notes/2026-10-01-asio-spike.md`).
- **Revised:** 2026-10-01 — Track 3 Phase C (UI + large library) complete; Phase D (memory, cache/eviction, stability) next.
- **Revised:** 2026-10-01 — Track 8 (Linux support) added and sequenced before Track 7; findings in `plans/notes/2026-10-01-linux-support-spike.md`.
- **Revised:** 2026-10-01 — Project hygiene batch (LICENSE/notices/CSP/docs) landed; Track 2b closed and its remainder extracted to new **Track 9 (UI completion)**; new **Track 10 (QoL)** added; Track 7 gains ReplayGain and crossfade/automix research; global non-goals recorded (PM-D6).
- **Revised:** 2026-10-02 — Track 3 Phase D (memory, cache/eviction, stability) complete; Phases A–D done, S10 owner smoke + official-client sanity pending.
- **Revised:** 2026-10-02 — Track 3 closed: S10 owner smoke and the official-client sanity note complete.
- **Revised:** 2026-10-04 — Track 8 (Plans 8.1–8.5) implemented and WSL2-verified; closed by owner exemption for the deferred bare-metal checks (`plans/notes/2026-10-04-track8-completion.md`).
- **Revised:** 2026-10-05 — Track 10 (QoL) implemented, merged to `main` at `446e054` (follow-up fixes through `70d0bc9`), and closed with owner smoke passed; 10.4 scrobble love deferred (`plans/notes/2026-10-05-track10-completion.md`).
- **Revised:** 2026-10-07 — Track 7 called by the owner with the **DSP core** scope (ReplayGain, 10-band EQ, crossfade); spec `2026-10-07-oxitide-track7-advanced-audio-design.md`, Plans 7.1–7.2 implemented on `track7-dsp`; owner smoke passed and Track 7 (DSP core) closed the same day. Casting/TIDAL Connect, Atmos, 360 RA, ASIO, listen-along, and automix stay parked. Track 9.4 (a11y) resumes after Track 7.
- **Revised:** 2026-10-07 — AutoMix spike written (`plans/notes/2026-10-07-automix-spike.md`): recommends a small Phase 1 (silence trim + rate-mismatch overlap) before any beat-aligned work; still research-only until the owner calls it.
- **Revised:** 2026-10-08 — Track 9.4 (a11y pass) smoke passed (`plans/notes/DONE-2026-10-07-track9-4-smoke.md`; PM-D1 S5/S6/S7 re-runs show no regression) and merged to `main`. Track 9 closure (plan 9.4 Task 15) waits on the owner confirming the 9.2/9.3 smokes; Hi-Fi Rack runs as its own spec.
- **Revised:** 2026-10-08 — Track 7b (AutoMix Phases 1–2: silence trim, cross-format overlap, beat matching; individually toggleable for A/B) implemented on `track7b-automix` per `2026-10-08-oxitide-track7b-automix-design.md`; owner A/B smoke pending. Phase 3 (pure-Rust tempo stretch) researched in `plans/notes/2026-10-08-tempo-stretch-research.md`.
- **Revised:** 2026-10-09 — Track 9.5 (Hi-Fi Rack, third UI mode) implemented on `track9-5-hifi-rack` and closed after owner polish rounds (`plans/notes/2026-10-09-track9-5-hifi-rack-completion.md`); formal smoke tick and PM-D1 re-runs are owner items; 9.5c (narrow fold, Rack mini layout, queue changer) deferred.
- **Revised:** 2026-10-10 — **Track 11 (Phone Remote)** added: casting/TIDAL Connect research (`plans/notes/2026-10-09-tidal-connect-receiver-spike.md`) showed the TIDAL phone app only accepts TIDAL's own receiver binary, so the owner chose a self-contained LAN web remote instead (spec `2026-10-09-oxitide-track11-phone-remote-design.md`). Plans 11.1–11.5 implemented on `track11-phone-remote`; 11.6 human smoke and PM-D1 re-runs are owner items.
- **Revised:** 2026-10-11 — Track 11 (Phone Remote) closed: owner smoke passed (Linux item deferred to the batched Linux run), merged to `main` (`plans/notes/2026-10-10-track11-completion.md`).
- **Revised:** 2026-10-10 — Track 9.5c (narrow-window fold, Rack mini player, queue changer unit, DSEG7 segment font) implemented on `track9-5c-rack` (Rack spec §9, `plans/notes/2026-10-10-track9-5c-rack-completion.md`).
- **Revised:** 2026-10-13 — ASIO output (Plan 7.3) and Track 9.5c merged to `main`; **Track 9 closed** by the owner (9.2/9.3/9.5 smokes marked done; `plans/notes/2026-10-13-track9-completion.md`). A batched Linux bare-metal run is deferred (Windows first). i18n remains an unspecced follow-on track.
- **Revised:** 2026-10-14 — Track 7b (AutoMix: silence trim, cross-format overlap, beat matching; 7b.2 Classic / Quick / Blend styles, the replay lab, and the Auto style) merged to `main` after two owner A/B passes (`plans/2026-10-08-track7b-2-transition-styles.md`, AM-D8–D18). Owner listening continues on the smoke checklist (rows 12i–12j); beat-grid confidence still needs tuning before beat matching is judged.
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md` (§9 Phasing)
- **Supersedes:** the parent's Phase 2–4 ordering. The shipped Phase 1 MVP (v0.2.0) is
  described in `2026-09-21-oxitide-phase1-design.md`.

---

## 1. Purpose

Fix the order and framing of post-MVP work. Each track (or sub-track) still gets its own
dated spec → plan → implementation cycle; this document only fixes scope, execution order,
decisions, and the research each track needs. Plan numbers continue the existing sequence
(Plan 6 onward) in execution order; a track may span one or more plans.

Owner-confirmed ordering (2026-09-24), execution order revised 2026-09-28 (PM-D5) and
2026-10-01 (Tracks 9–10 added; order Track 9 → Track 8 → Track 10 → Track 7); statuses
refreshed 2026-10-01.

**Track labels are stable; the execution order below is not track-number order.** Tracks
4–6 are complete; Track 3 is complete (Phases A–D, 2026-10-02).

| Exec | Track | Outcome | Status |
|---|---|---|---|
| — | Track 1 | Library editing & favorites: favorite/unfavorite tracks, albums, artists; playlist create/rename/delete; add/remove/reorder tracks | Done (Plans 6–7 + write hotfix) |
| — | Track 2a | UX behavior: shortcuts, context menus, sorting/filtering, virtualization, nav history, toasts, search-as-you-type | Done (Plans 8–9) |
| — | Track 2b | Visual: design language, layout/IA, expanded now-playing, density, responsive window fit | Done (Plans 10–13); remaining polish/Hi-Fi Rack/light mode extracted to Track 9 |
| 1 | Track 4 | Lyrics + discovery: synced lyrics view; discovery feeds/mixes; track radio / similar artists | Done (4a lyrics + 4b discovery) |
| 2 | Track 5 | Scrobbling: Last.fm and ListenBrainz in parallel, independent toggles; now-playing, offline queue, retry | Done (Last.fm + ListenBrainz; smoke closed 2026-09-29) |
| 3 | Track 6 | Remaining integrations: Discord RPC; SMTC + hardware media keys | Done (6a SMTC/media keys + 6b Discord Rich Presence) |
| now | Track 3 | Performance & stability wave: profile and fix startup, time-to-first-sound, UI jank, memory, cache/eviction churn, large-library handling; build a measurement harness | Done — Phases A–D complete 2026-10-02 (`plans/notes/2026-10-02-track3-phase-d.md`) |
| 5 | Track 9 | UI completion: Hi-Fi Rack mode, light mode, artist/credits/album-info surfaces, a11y pass, i18n, remaining UI polish and bug fixes | Done (2026-10-13) - 9.0-9.5c all merged; owner closed the 9.2/9.3/9.5 smokes without row-by-row ticks; i18n handed off as its own track; Linux items batched for later (`plans/notes/2026-10-13-track9-completion.md`) |
| 6 | Track 8 | Linux support: ALSA output backend, MPRIS media controls, Secret Service keyring, deb/AppImage/AUR packaging | Done (2026-10-04) — WSL2-verified; bare-metal checks exempted (`plans/notes/2026-10-04-track8-completion.md`) |
| 7 | Track 10 | Daily-driver QoL: session/queue persistence, playback history + stats, sleep timer, scrobble love (deferred 2026-10-05), AI/explicit content blocking; Phase 2 tray + global hotkeys | Done (2026-10-05) — merged at `446e054` with follow-up fixes through `70d0bc9`; owner smoke passed; 10.4 deferred |
| 8 | Track 7 | Advanced audio: DSP/EQ + ReplayGain (opt-in, mutually exclusive with bit-perfect); crossfade/automix; casting/TIDAL Connect; Dolby Atmos; 360 RA (difficulty-gated); optional ASIO output (maybe, see `plans/notes/2026-10-01-asio-spike.md`); **potential post-casting group listen-along** (see `plans/notes/2026-10-02-listen-along-assessment.md`) | DSP core done 2026-10-07 (ReplayGain, EQ, crossfade; owner smoke passed — `plans/notes/2026-10-07-track7-completion.md`); casting/Atmos/360 RA/ASIO/listen-along/automix parked |
| 9 | Track 11 | Phone remote: opt-in LAN server, QR pairing, a phone web page for playback, queue, search, and library (replaces "control OxiTide from the phone" from Track 7's casting item) | Done (2026-10-11) — owner smoke passed (Linux item deferred), merged to `main`; `plans/notes/2026-10-10-track11-completion.md` |
| 10 | Track 12 | Distribution: GitHub Actions CI, tag-driven draft releases, in-app auto-update (minisign); Windows code signing deferred | Implemented (2026-10-13), unmerged workflows unproven on GitHub — `plans/notes/2026-10-13-track12-completion.md` |

Track 2b is closed; its remainder (Hi-Fi Rack, light mode, leftover polish) is Track 9,
which carries a normal completion gate.

## 2. Carryover (before Plan 6)

- Merge `fix/plan5-pager-short-pages`; re-run the two pending Plan 5 paging smoke items
  (favorites/playlist counts, queue-all).
- Re-check Plan 4's two manual items (cache usage display, HE-AAC Low retirement message).

## 3. Track notes and known constraints

### Track 1 — Library editing & favorites

- The community client (`tidalapi`) confirms playlist create/delete, playlist track
  add/remove, favorites add/remove for tracks/albums/artists, and batch additions;
  playlist create/get/add/delete moved to TIDAL's **v2 endpoints**.
- `TidalClient` is currently GET-only: this track adds POST/DELETE verbs reusing the
  single-flight refresh-on-401 path, plus 429/`Retry-After` handling.
- **Unverified:** explicit track reorder semantics (dedicated endpoint vs delete+re-add) —
  verify before the spec is written.
- Interaction decisions (optimistic updates vs refetch, error reconciliation) belong to
  the plan.

### Track 2 — UX pack + UI/IA overhaul

- Two specs: **2a behavior** (keyboard shortcuts, context menus, sortable/filterable
  tables, list virtualization, back/forward navigation, toasts, search-as-you-type,
  consistent loading/empty/error states) and **2b visual identity + layout/IA**
  (design language, color/typography/icons, library sidebar tree, expanded now-playing,
  queue presentation, density, window fit/responsiveness).
- All five aspects are in scope: visual identity, layout & information architecture,
  player/queue experience, data presentation & interaction polish, window fit.
- The look-and-feel direction (references, light/dark, mockups) is decided during the 2b
  brainstorm; a visual pass is expected.
- **Status:** Done — 2a shipped in Plans 8–9; 2b shipped in Plans 10–13 (plus follow-ups).
  The open-ended remainder (Hi-Fi Rack, light mode, leftover polish/bug fixes) now lives in
  **Track 9 — UI completion** instead of an unbounded 2b bucket.
- Virtualization overlaps the Track 3 budgets; the measurement harness now lands with the
  deferred Track 3 wave (see PM-D5).

### Track 3 — Performance & stability wave

- This operationalizes the project's core promise: beat the official client on
  responsiveness and reliability, not just match its features.
- Deliver a measurement harness plus fixes; candidate budgets: cold start,
  time-to-first-sound, 2k+ row scroll smoothness, memory ceiling, no UI stalls,
  cache/eviction churn, large-library load.
- Standing rule (see §4 PM-D1): every later spec states its performance impact.
- **Status:** Active (2026-09-30); **Phase A (harness + baseline) complete 2026-10-01**
  (`plans/notes/2026-09-30-track3-baseline.md`). The harness (`scripts/perf/`) records
  JSONL runs and gates against the full spec §4.2 budget table, enforced manually via
  `npm --prefix ui run perf:check`. Baseline results: scroll smoothness, long tasks, S1
  cold start, S3-miss TTFS, S3-hit cache-hit floor, and S8 second-pass hit rate pass;
  memory ceilings, `S2.first_paint_ms`, `S3-hit.ttfs_ms`, and `S4.advance_max_gap_ms`
  are recorded misses and the targets for Phases B–D. No T3-D9 recalibration was used
  (owner decision 2026-10-01).
- **Phase B (startup + TTFS) complete 2026-10-01** — `plans/notes/2026-10-01-track3-phase-b.md`:
  window gated on geometry with cache/scheme/keyring work backgrounded; lazy palettes/lyrics/
  keyring loads; event-driven worker wakeups; TTFS attributed by stage. S2 first paint
  1052 → 949 (PASS), S3-hit TTFS 308 → 294 (PASS), S1 first paint 1113 → 938, app-side
  pre-paint steps are now ≤ 2 ms each. No TTFS fix was warranted (no stage > 60% of TTFS;
  resolve/fetch/open = 34/41/24%); memory ceilings and `S4.advance_max_gap_ms` (101 vs 100)
  stay Phase D targets, together with the Phase A cached-seek defect. Owner smoke checklist
  `plans/notes/2026-10-01-track3-phase-b-smoke.md` is open. **Phase C (UI + large library)
  is next.** The PM-D1 standing gate continues to apply to every later change.
- **Phase C (UI + large library) complete 2026-10-01** — `plans/notes/2026-10-01-track3-phase-c.md`:
  player events apply in place (no more 500 ms whole-state invalidation); track-view filter/sort
  keys and the source index are precomputed and the search caches warm off the keystroke; pager
  windows append/remove/move in place and `ensureAll` bursts above the prefetch cap; S6 now
  loads a deterministic 10k synthetic list and gates real filter/sort interaction times. S6
  `filter_max_ms` 23.0 (≤ 50) and `sort_max_ms` 36.1 (≤ 150) PASS; S5-instrument p99 20.9 → 14.0
  and its 65 ms long task → 0; S2 first paint 1052 → 915; the new S1/S2 `startup_step_max_ms`
  row reads 1 ms. Memory ceilings (491–659 MB) and the Phase B `S4`/TTFS carryovers stay Phase D
  targets. The scenario-mode pin now runs after the page leaves `about:blank` (a driver race had
  silently skipped it). Owner smoke checklist
  `plans/notes/2026-10-01-track3-phase-c-smoke.md` is open. **Phase D (memory, cache/eviction,
  stability) is next.**
- **Phase D (memory, cache/eviction, stability) complete 2026-10-02** —
  `plans/notes/2026-10-02-track3-phase-d.md`: the palette cache is bounded and pruned; disk-cache
  byte accounting is incremental (no full-directory rescan on normal play) with the playing and
  prefetched entry protected for the stream lifetime; the baseline cached-seek defect is fixed; the
  harness gained per-process memory attribution, a settled-baseline soak-drift metric with an
  app-only row, page-open/album-open/queue-build gates, and partial-frame-window exclusion. All
  scripted budgets pass except `S4.advance_max_gap_ms` (105 vs 100; owner-ruled documented
  WASAPI-ring finding, no metric change). The owner-approved 2026-10-02 memory renegotiation raised
  the peak ceilings to the WebView2-inclusive p95 + ~15% (S1 570, S2 560, S3/S4 645, S5-atm 675,
  S5-inst 690, S6 740, S7 670, S8 660, S9 755) because the app-owned share is only 52–91 MB; S9
  settled drift is +14 all-process / +1 app (≤ 50 / ≤ 25). S10 owner smoke and the official-client
  sanity note are complete (2026-10-02); the harness is documented in `AGENTS.md`.

### Track 4 — Lyrics + discovery

- **Lyrics:** TIDAL's official API exposes a lyrics resource (`lrcText` synced, plain
  `text`, provider, `technicalStatus`) on the new JSON:API surface; the legacy community
  path exists; `lrclib.net` (free, no auth, synced LRC) is a fallback used by other
  clients. The source choice (TIDAL-first vs lrclib-first vs both) is a Track 4 decision.
- **Discovery:** the official `userDiscoveryMixes` resource and community mix endpoints
  (daily/history/radio) plus artist radio / similar artists.
- Sync availability varies by track and region; handle "no lyrics" cleanly.
- **Status:** Done — 4a lyrics (2026-09-29), 4b discovery (2026-09-30); the 4b smoke
  results are recorded in `plans/notes/2026-09-30-track4b-discovery-smoke.md`.

### Track 5 — Scrobbling

- Both services in parallel, independent toggles; one listen-event feed fans out.
- Last.fm: register an OxiTide API key+secret (owner prerequisite); web auth → session
  key in the OS keyring. ListenBrainz: user token only.
- Scrobble rules: tracks >30 s; scrobble at 50% or 4 min; now-playing pings; offline
  queue + retry per service; privacy toggle.
- **Status:** Done — the Last.fm key/secret is registered; smoke closed 2026-09-29
  (`plans/notes/2026-09-29-track5-scrobbling-smoke.md`).

### Track 6 — Remaining integrations

- Discord RPC: requires registering a Discord application; decide what presence shows
  (track, artist, progress, buttons).
- SMTC + hardware media keys: Windows System Media Transport Controls via the `windows`
  crate; verify key handling and coexistence with the WebView.
- **Status:** Done — 6a implemented and 6b Discord Rich Presence shipped; the 6b smoke is
  closed 2026-09-30. Tracks 1–6 are complete.

### Track 7 — Advanced audio

- **DSP family (opt-in; mutually exclusive with bit-perfect, and the UI must make the
  trade-off explicit):** EQ and ReplayGain / volume normalization.
- **Crossfade / automix:** overlapping crossfade needs two concurrent decoders plus mixing
  in the engine, and is mutually exclusive with gapless and bit-perfect; an Apple
  AutoMix-style beat/time-aligned mixer is research only, not committed. The exact scope is
  fixed in the Track 7 spec.
- Casting / TIDAL Connect (protocol research first) and Dolby Atmos (Phase 0-style DRM
  spike before any commitment). **2026-10-09:** the Connect spike
  (`plans/notes/2026-10-09-tidal-connect-receiver-spike.md`) found that appearing in the
  TIDAL phone app requires TIDAL's own receiver binary (the phone verifies a TIDAL-issued
  certificate). Phone control moved to **Track 11** (own web remote); a Connect sidecar
  stays parked here and could reuse Track 11's `RemoteSession` seam.
- **Listen Along / group listening (potential, not committed):** synchronize playback across
  two or more accounts so people listen together, optionally launched from Discord. No audio
  is streamed — each participant plays from their own account and only transport state (track,
  position, play/pause) is shared. It reuses casting's playback-session/transport primitive
  (discovery + control channel + remote `EngineCmd`/`EngineEvent` mapping), so it is assessed
  only **after** casting / TIDAL Connect lands; the DSP/Atmos/ASIO parts of this track do not
  overlap. Discord's "Listen Along" is Spotify-exclusive and TIDAL's "Live" is host-broadcast
  and mobile-only, so this would be OxiTide's own protocol. See
  `plans/notes/2026-10-02-listen-along-assessment.md`.
- **360 Reality Audio:** currently not relevant or in use; whether it is worth implementing
  is decided by difficulty when Track 7 is scoped.
- **ASIO output (called 2026-10-11, Plan 7.3):** implemented and merged to `main` per
  `specs/2026-10-11-oxitide-track7-asio-output-design.md` (hand-written IASIO binding, no LLVM);
  owner smoke `plans/notes/2026-10-11-asio-smoke.md` pending. Original assessment: feature-parity option, assessed by the
  2026-10-01 spike (`plans/notes/2026-10-01-asio-spike.md`). WASAPI-only today; a contained
  `asio-sys` backend is feasible and AGPL-compatible (SDK GPLv3 option), but costs LLVM at
  build time, in-process driver crash risk, software-only volume (breaks bit-perfect below
  100%), no hotplug, and single-client conflicts. Revisit only if an affected interface
  appears or the owner wants parity; then run the normal spec → plan cycle.
- Parked last, after Track 10, until the owner calls it.
- **Called 2026-10-07 (DSP core):** ReplayGain, 10-band EQ, and crossfade per
  `2026-10-07-oxitide-track7-advanced-audio-design.md` (T7-D1–T7-D8). Bit-perfect is decided
  by reality: identity settings keep the exclusive path sample-exact, and any stage that
  alters samples clears `bit_perfect` and names itself in `dsp_stages`. The other Track 7
  items above keep their parked status.

### Track 8 — Linux support

- **Target:** modern Arch-based (Arch, CachyOS, Manjaro, EndeavourOS) and Debian-based
  (Debian, Ubuntu, Mint) distros; build against the oldest supported base (Ubuntu 22.04 /
  Debian 12 — the WebKitGTK 4.1 floor) per Tauri's AppImage guidance. Kernel choice is a
  non-factor (mainline USB audio); PipeWire vs PulseAudio, X11 vs Wayland, and
  GNOME-Keyring vs KWallet are the real test matrix.
- **Scope:** an ALSA audio backend mapping the existing model (server `default` for shared,
  direct `hw:` for exclusive/bit-perfect); MPRIS media controls replacing the SMTC stub;
  Linux keyring features with a graceful no-Secret-Service fallback; deb + AppImage
  artifacts and an AUR PKGBUILD; Linux port of the Track 3 measurement harness (or an
  explicit exemption).
- **Phases:** (1) playable alpha — audio + keyring + build/packaging; (2) desktop
  integration — MPRIS, deep links/.desktop, Wayland geometry, WebKit CSS pass;
  (3) parity/hardening — perf harness and distro matrix.
- **Status:** Done (2026-10-04). Plans 8.1–8.5 implemented on `track8-linux`, merged to
  `main` at `d14f5f4`, and WSL2-verified; the deferred bare-metal checks were exempted by
  the owner at closure — see `plans/notes/2026-10-04-track8-completion.md` for the
  verified/exempted split. Findings, distro/WM/kernel sensitivity, and effort estimate:
  `plans/notes/2026-10-01-linux-support-spike.md`.

### Track 9 — UI completion

- Closes out the visual/UX program so Track 2b can end: **Hi-Fi Rack** mode (convergence
  doc §2 — needs custom elements and its own design pass), **light mode**, and the remaining
  2b polish and UI bug fixes.
- Surface work: artist page layout/redesign, track credits, album info.
- **A11y pass** with explicit acceptance criteria (keyboard traversal, focus-visible
  states, screen-reader labels for custom widgets, contrast) so the track can actually
  close.
- **i18n** (owner-confirmed placement): locale infrastructure and string extraction; if it
  outgrows this track it becomes its own track.
- **Status:** Done (2026-10-13) - see `plans/notes/2026-10-13-track9-completion.md`; i18n was split out and has no spec yet. Track 3 budgets
  (PM-D1) apply to every new surface.

### Track 10 — Daily-driver QoL

- **Phase 1 (platform-neutral):** session/queue persistence (resume the queue and position
  on restart); playback history / recently-played view plus a stats page (local-first
  data); sleep timer; scrobble-service **love** (Last.fm, ListenBrainz — distinct from the
  TIDAL heart; **deferred 2026-10-05** — owner uses ListenBrainz for stats, not feedback);
  AI-content and explicit-language blocking to match the official app (AI labels and
  filters shipped there in July 2026).
- **Phase 2 (platform integration):** system tray / close-to-tray and system-wide custom
  hotkeys; on Linux these ride the Track 8 seams (MPRIS, AppIndicator, portals).
- **Research before the spec** (`plans/notes/2026-10-01-qol-research.md`): whether the API
  exposes AI-generated flags and user-settings endpoints for the AI/explicit filters;
  playback-history and credits endpoints; ListenBrainz feedback support; history/stats
  data source and retention.
- **Status:** Done (2026-10-05) — merged at `446e054` with follow-up fixes and enhancements through `70d0bc9`; owner smoke passed (`plans/notes/2026-10-05-track10-completion.md`); 10.4 (scrobble love) deferred 2026-10-05.

### Track 11 — Phone remote

- **Outcome:** control a running OxiTide from a phone on the same network with nothing to
  install: an opt-in server (off by default), QR pairing with per-device tokens and desktop
  approval, and a mobile web page for Now Playing, the queue, search, Home/Explore, the
  library, and detail pages. No cloud relay, no account, no TIDAL software involved;
  Tailscale-style VPN addresses are admitted for remote use.
- **Spec:** `2026-10-09-oxitide-track11-phone-remote-design.md` (RM-D1–RM-D13, all
  owner-accepted 2026-10-09). Plan record: `plans/2026-10-09-track11-phone-remote.md`.
- **Status:** Done (2026-10-11). Plans 11.1–11.6 plus smoke round 1 (favorite sync, device
  rename, Atmosphere polish, swipe-on-cover volume) and a final security pass; owner smoke
  passed with the Linux item deferred; PM-D1 shows no remote cost (seven pre-existing
  breaches reproduce with it off). Merged to `main` — see
  `plans/notes/2026-10-10-track11-completion.md`.

## 4. Decisions (post-MVP)

| # | Decision | Rationale |
|---|---|---|
| PM-D1 | Performance is both a dedicated wave (Track 3) and a standing consideration in every track's design decisions; regressions block a track's completion. | Responsiveness/reliability is the product's core promise. |
| PM-D2 | **No explicit downloads.** Offline stays as the existing transient LRU cache; no download-manager UI. Revisit only if the owner changes stance. | TIDAL's terms prohibit downloading outside the official player; a deliberate plain-file library crosses a different line than transient caching, and the cache already covers replays. |
| PM-D3 | Scrobbling feeds Last.fm and ListenBrainz in parallel with independent toggles. | No reason to serialize; both may be wanted. |
| PM-D4 | Track 2 splits into behavior (2a) and visual/IA (2b) specs. | Keeps each plan reviewable and gives the visual direction its own pass. |
| PM-D5 | Execution order is revised to Tracks 4 → 5 → 6 → 3 → 7 after Plan 10 (labelled order unchanged). Track 3 is deferred wholesale — no early harness. | Tracks 4–6 are additive/user-facing and Track 7 is optional, so the dedicated optimization wave targets a stable feature set. PM-D1's standing gate keeps regressions honest meanwhile; Track 5's Last.fm key registration starts now (long lead). |
| PM-D6 | **Global non-goals: multiple accounts and music videos.** i18n, crossfade/automix, and 360 RA remain in scope (placement/difficulty per §3). | Single-account personal use; an audio-first client. |

## 5. Open research questions

- Dolby Atmos DRM status (spike) and casting/TIDAL Connect protocol (Track 7).
- Linux support (Track 8): audio backend choice (ALSA-direct vs server integration),
  keyring fallback when no Secret Service is present, WebKitGTK/CSS floor, packaging
  artifacts and baselines.
- Daily-driver QoL (Track 10): AI-generated flags and the AI/explicit settings endpoints;
  playback-history and credits endpoints; ListenBrainz feedback (love); history/stats data
  source and retention; cross-platform shape of tray and custom hotkeys.
- Resolved since: playlist reorder semantics (Plans 6–7); lyrics source and cache (4a);
  discovery endpoints (4b); Last.fm key registration (5); scrobble offline-queue
  dedupe/failure behavior (5).

## 6. Cross-cutting backlog (fold into whichever plan fits)

- Performance budgets as a standing gate on every plan: cold start,
  time-to-first-sound, 2k+ row scroll smoothness, memory ceiling, no UI stalls —
  measured by the Track 3 harness.
- Performance considered in every track's design decisions (allocations, blocking calls,
  re-renders, API round-trips), not only Track 3; regressions block a track's completion.
- API-drift resilience: version-tolerant parsing, fixture refresh, error taxonomy.
- Packaging: code signing, auto-update (deferred; required only if shared beyond
  friends); CI (none today).
- Logging/diagnostics improvements as needed.

## 7. Notes

- Plan numbering continues from Plan 5 and follows the **execution order** (PM-D5): Track 2b
  spans Plans 10–13, then Tracks 4, 5, 6, then the deferred Track 3 (now active), then
  Track 9 (UI completion), Track 8 (Linux), Track 10 (QoL), then Track 7. **Track labels do not change** — "Track 3" still names Performance & stability
  even though it runs fourth. Track 2 has spanned Plans 8–9 (2a) and Plans 10–13 (2b);
  later tracks may similarly span more than one plan.
- The parent's Phase 4 ordering (DSP → casting → Atmos) is replaced by Track 7; Atmos
  and casting stay parked there.
- Track 2's visual direction (2b) and Track 4's lyrics source (4a) were decided in their
  own specs; both sub-tracks shipped.
