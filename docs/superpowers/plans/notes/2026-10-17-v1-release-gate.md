# v1.0 release gate (next steps)

- **Date:** 2026-10-17
- **Owner:** V
- **Scope:** what has to happen before `v1.0.0` is tagged. 1.0 is mainly personal; the repo may go
  public. After 1.0: deferred/passed-over features, then V2 (modular sources: other streaming
  services, local files).
- **Decided:** no Windows code signing and no dedicated security contact for 1.0 (contact is via the
  owner's GitHub profile). README screenshots are in scope.

Steps run in this order. Smokes that touch code changed by a later step wait until after it, so
nothing is smoked twice.

## 1. Track 13 (efficiency) + perf debt

Branch `track13-efficiency` (unmerged). Plan `plans/2026-10-16-track13-efficiency.md`, smoke
`plans/notes/2026-10-16-track13-smoke.md`.

- [ ] Finish and merge Track 13; run its smoke.
- [ ] Settle the PM-D1 debt (fix or renegotiate the budgets in `scripts/perf/budgets.json`).
      **Ruled 2026-10-23** (`2026-10-23-v1-perf-rulings.md`: release reference, memory = worst run
      + 15 %, S4 re-measure then debt, S6 p99 re-measure then relax); waits on the release corpus:
  - [ ] the seven pre-existing breaches listed in `2026-10-10-track11-completion.md`;
  - [ ] the 9.3 S5/S6/S7 and Rack S1/S5 re-runs never recorded (`2026-10-13-track9-completion.md`);
  - [ ] the 9.1 `S5-light` `memory_max_mb` 681 vs 675 ruling and the S1/S2 quiet re-run;
  - [ ] `S4.advance_max_gap_ms` (105 vs 100): accept as documented debt in the 1.0 notes, or fix.
- [ ] Measure the shipped artifact at least once (release + LTO profile for S1/S3/memory); the
      harness drives the debug exe today.
- [x] Fix the flaky `oxitide-audio engine::tests::retry_resumes_at_the_recorded_position`
      (`2026-10-13-track12-completion.md`).
      - **Done 2026-10-23** (test-only). The test simulated a stream error with a bare
        `queue.stop()`, which left the failed session running; at 3x real time its track end
        could land between `retry()` resuming the queue and the `playing` assert (`retry()` does
        not hold `playback_lock`), or a stalled test thread let it play out so the retry resumed
        at the end. It now ends the session as the worker does (stop + generation bump), waits
        for the worker to go idle, asserts `playing` straight after `retry()`, and runs the output
        at about real time from a 500 ms seek (1.5 s of margin). Not reproduced locally before
        the fix (0/70 under load on 16 threads); 0/30 after it.
- [x] Underrun counter: count output buffer underruns in the engine and report `underrun_count` in
      the perf rows (S4, S9), so playback glitches are measurable before V2 (V2 spec V2-D42 budgets
      it at 0 for S16). An engine change: it lands before the engine smokes below.
      - **Done 2026-10-23.** WASAPI/ASIO count ring dry-outs (`ring::Starvation`: a short device
        period after a write, once per dry-out); ALSA counts `writei` `EPIPE`. The engine keeps only
        those during playback (`UnderrunTally`: the dry-out a pause, seek, reopen or network stall
        leaves is dropped at the next refill), logs each at `warn`, and fires
        `AudioPerfEvent::Underrun`. Perf rows: `audio` / `event: "underrun"` with `count`;
        `check.mjs` reports `underrun_count` (0 for any run that played, measured from
        `measure_start`). No budget rows added yet: rule on S4/S9 after the first runs.
- [x] Panic hook: route panics (message + location, all threads) into the `tracing` file log. There
      is no `panic::set_hook` today, and the Windows GUI build has no console, so panics vanish.
      - **Done 2026-10-23** (`panic_log.rs`): message, location, thread and backtrace at `error`
        (target `panic`); a main-thread panic flushes the non-blocking file writer first.
- [x] Persisted-data versioning for V2: `history.jsonl` records have no version field, and saved
      items carry bare TIDAL ids. Add a version and a source tag to history records and
      `QueueItem` / `SessionItem` / `session.json`, with tolerant reads of existing files.
      (`session.json` and `track_flags.json` already carry `version`.)
      - **Use the V2 form, not a separate field** (V2 spec `specs/2026-10-17-oxitide-v2-modular-sources-design.md`,
        V2-D19 / V2-D34): write ids as `"tidal:<n>"` strings — `track_id`, `album_id`, the ids in
        `artist_refs` (this also covers `QueuedScrobble.item`); read both that and a bare number.
        Internally 1.x keeps `u64`; only (de)serialisation changes. Anything else (`cover`, map keys,
        `play_source`) can stay as is — V2-D34 reads those shapes.
      - **Done 2026-10-23** (`stored_ref.rs`): `session.json` is version 2 (version 1 still loads),
        `history.jsonl` lines carry `"version": 1` through a file-only `StoredRecord` (the History
        page's `HistoryRecord` IPC shape is unchanged); both read bare numbers too. `artist_refs`
        are written as `{ ref, name }`. `QueuedScrobble` is never persisted, and
        `play_from.track_ids` stays numeric (UI-owned, crosses IPC); both noted in V2-D34.
        Downgrade caveat: a pre-1.0 build backs up a version 2 session and skips the new history
        lines, so its next cap-trim rewrite would drop them.

### Then: engine smokes (Track 13 changes the audio engine)

- [ ] Network resilience — `2026-10-14-network-resilience-smoke.md` (18 rows).
- [ ] ASIO — `2026-10-11-asio-smoke.md` (12 rows).
- [ ] 96 kbps / Low listening rows — `2026-10-16-96kbps-validation.md`.

### Then: start the updater proof

- [ ] Install the published 0.3 build on a clean profile; publish `v0.4.0`; confirm Settings →
      System → Updates finds and installs it (first recorded installed-app update). Every later
      pre-1.0 bump repeats this check. Checklist: `2026-10-13-track12-completion.md`.

## 2. UI pass

- [ ] Visual/animation/text pass across Atmosphere, Instrument and Hi-Fi Rack (light + dark); update
      `ui/design/*/DESIGN.md` with any visual change.
- [ ] Fold in the small UI-side deferred items so they are polished with everything else (e.g. undo
      for destructive ops).
- [ ] If i18n is ever wanted, consider extracting strings during the text pass (about 600–800 strings
      across about 55 files).
- [ ] README screenshots (`docs/images/`, replace the placeholder comment) once the visuals are final.

## 3. Gaps and deferred items

Candidates (pick per value; none block 1.0 on their own):

- [x] "Open logs folder" / "Copy diagnostics" in Settings (version, OS, output mode, recent errors),
      if not already present. **Done 2026-10-23:** "Open logs folder" existed; "Copy diagnostics"
      (`diagnostics.rs`, `get_diagnostics`, not exposed to the phone) copies version, OS, WebView,
      output path, quality, current stream, DSP stages, underruns, enabled switches and the last 20
      WARN/ERROR lines, with URL queries, token/code/state values, bearer tokens and the home
      directory redacted. Smoke: copy once, paste, check nothing personal is in it.
- [x] Recognisable error for TIDAL API drift ("TIDAL changed something — check for updates") instead
      of a generic failure (roadmap §6 backlog). **Done 2026-10-23** (UI, `lib/errors.ts`): a 2xx
      body that no longer decodes (`json error:`, already logged at `warn` with status/URL by
      `decode_json`) or a 410 reads "TIDAL changed something on its side — check for an OxiTide
      update (Settings → System)" in error states and toasts; 404s are left alone (deleted items).
- [x] Credits pagination (Info tab shows the first 20). **Done 2026-10-23** (`catalog.rs`
      `track_credits`): follows `links.meta.nextCursor` through `tracks/{id}/relationships/credits`
      with `include=credits` and `page[cursor]`, up to 10 pages; a later page that fails or names no
      credits keeps what was read. **Live check owed:** the 2026-10-06 probe saw that endpoint
      return linkage only *without* `include`; open Track info on a track with 20+ credits (e.g.
      564178214, "Draw You Out") and confirm the list grows past 20 — if it stays at 20, the log
      has "credits: a later page named no credits".
- [ ] Lyrics in the mini player.
- [ ] AutoMix beat-grid confidence tuning (roadmap, 2026-10-14 revision).
- [ ] Explore feed integration is inert until TIDAL serves track rows — revisit or drop.
- [ ] Optional: one-line `SECURITY.md` pointing to the GitHub profile.

## 4. Final perf run

- [ ] Full scripted scenarios on a quiet machine after the UI pass, ideally against the release build;
      `node scripts/perf/check.mjs` green (or breaches ruled on).

## 5. Remaining smokes

- [ ] Thumbbar — `2026-10-16-thumbbar-smoke.md` (9 rows).
- [ ] Rack 9.5c — `2026-10-10-track9-5c-rack-completion.md` (7 rows, closed without ticks).
- [ ] 96 kbps GUI rows (selector in all three modes, restart persistence) — `2026-10-16-96kbps-validation.md`.
- [ ] Smokes for anything added in steps 2–3.
- [ ] Updater: latest pre-1.0 bump installs over the previous one.

## 6. Linux bare-metal run

After all of the above, in one batch (list in `2026-10-13-track9-completion.md` § Linux bare-metal run):
Track 8 resume lists, the Track 11 `ufw` item, Track 9 surfaces on WebKitGTK, and the AppImage updater
path. Re-check any resulting fixes on Windows. If the run slips, label Linux "preview" in the 1.0
release notes (the README currently says "Supported").

**Run 2026-10-23 (CachyOS / KDE Wayland / PipeWire):** gates, build, MPRIS, deep links, tray, Rack,
credits, remote and the first Linux perf corpus all executed — results and issues in
`2026-10-23-linux-bare-metal-run.md`. A USB-DAC follow-up the same day verified exclusive
bit-perfect playback and hardware mixer volume (after fixing two ALSA bugs in `7498179`) and found that the device picker cannot select a direct
`hw:` device — the last audio-path blocker. Still blocking a
full close: that picker fix, DAC hotplug, AUR (recipe URL/tag drift), the oldest-base checks,
MPRIS desktop-entry resolution, and the MPRIS `Stop` parity issue.

## 7. Tag `v1.0.0`

- [ ] `docs/release.md` runbook; release notes list accepted debt (S4 gap, any renegotiated budgets).

## After 1.0 (not gated)

Deferred/passed-over features, then V2. V2 seams to keep in mind: `QueueItem` ids, `artist_refs` /
`album_id`, `coverUrl` (`resources.tidal.com`), the disk-cache key and `.meta` `StreamPlan`, the phone
remote's TIDAL-only cover rule, and scrobble payloads.
