# Engine smokes — release-gate run (2026-10-24)

Run for `2026-10-17-v1-release-gate.md` step 1 ("Then: engine smokes", because Track 13
changed the audio engine). Owner drives the app; results are recorded here and ticked in each
source checklist when a session closes.

## Build under test

- Branch `main` @ `bc7101c`, version `0.4.0`.
- Binary: `target/release/oxitide-app.exe` (`npm --prefix ui run app:build:perf`, UI embedded,
  no Vite, no installer).
- Machine: Windows, Intel I211 Ethernet (no Wi-Fi), ProtonVPN WireGuard tunnel up, Topping USB
  Audio Device + Realtek ASIO both registered under `HKLM\SOFTWARE\ASIO`.
- Starting config: quality `max`, `exclusive = false`, WASAPI device
  `{0.0.0.00000000}.{d246d610-…}`, volume 16 %, crossfade `duration_ms = 4000` (on), EQ off,
  ReplayGain off. 61 max-quality cache entries with `.fmp4.meta` sidecars.

## How results are verified

- File log: `%APPDATA%\OxiTide\logs\` (daily-rotated, `info` by default; no `RUST_LOG` needed).
  **Rotation uses the UTC date**, so the 2026-10-24 local session writes to
  `oxitide.2026-10-23.log` until 03:00 local (UTC+3).
- Lines to look for: `network down` / `network up` (`connectivity.rs`, logged on every OS
  connectivity change), `playback is waiting for the network`, `playback lost the network (…);
  resuming in …`, `network is back; playback resumed`, `TIDAL unreachable; playing track N from
  the cache`, `output underrun during track N`.
- Cache: `%APPDATA%\OxiTide\config\cache` (`<id>-<quality>-<hash>.fmp4`, `.fmp4.meta`,
  `spill-*.part`).
- The log file is created on the first `info` record, so it does not exist while the app is idle
  at startup. A local `--no-bundle` build has no updater endpoint, so the startup update check
  logs nothing.

## Network toggling (elevated PowerShell)

```powershell
# "network down" (Windows reports the link down)
Disable-NetAdapter -Name "Ethernet" -Confirm:$false
Enable-NetAdapter  -Name "Ethernet" -Confirm:$false

# "link up, no internet" (retry limit counts real time) — firewall block on the exe only
New-NetFirewallRule -DisplayName "OxiTide smoke block" -Direction Outbound `
  -Program "C:\Users\V\Documents\Opencode\OxiTide\target\release\oxitide-app.exe" `
  -Action Block -Profile Any
Remove-NetFirewallRule -DisplayName "OxiTide smoke block"
```

The firewall rule blocks only OxiTide, so the desktop keeps working while the app is isolated.
Block rules win over allow rules, and the block applies to the process regardless of the VPN
tunnel.

## Session 1 — network resilience (18 rows)

Source: `2026-10-14-network-resilience-smoke.md`.

### Download ahead

| # | Row | Result |
|---|-----|--------|
| 1 | Max track complete in cache within ~30 s, still playing | **PASS** (cache was cleared first, so this is the post-clear re-fetch, same `download_ahead` path): track 120744924 re-fetched 78.9 MB → `.fmp4` + `.meta` committed 20 s after the fetch started (02:18:27 → 02:18:47) while it played. The prefetched next track 131334292 committed 29.8 MB with its sidecar at 02:19:02. Its `.part` sat at 0 bytes until the playing track's download finished — that is the `AheadGate` doing its job (only the first download in line runs ahead). |
| 2 | Slow line: playing track does not stutter while next downloads | **Not exercised** — no honest throttle available (QoS on Windows does not shape inbound traffic; no throttle tool installed; owner has no 3G hotspot to use). Owner chose to pass. The `AheadGate` serialization it targets is covered by unit tests. |
| 3 | Seek far into uncached track: `spill-*.part` appears, disappears on track change | **PASS** — seeking into a downloading track created `spill-28356-1.part` (23.6 MB); it vanished 2.4 s later when the next seek superseded it (`ScratchFile::drop`, `fetch.rs:459` — the same drop path a track change takes). Needs tight timing: on this line a 30–80 MB track finishes downloading in ~20 s, so a later seek finds the cache `.part` complete and no scratch is made. Two earlier attempts missed for that reason. |

### Mid-track drop (in-place recovery)

| # | Row | Result |
|---|-----|--------|
| 4 | Fully downloaded track plays to its end with network off | **Attempted 02:23–02:30, not valid** — a cached 22.5 MB track played from disk with the network off for ~6 min with zero errors in the log, but the owner paused before the end, so "to its end" was not observed. Re-run needed. |
| 5 | Drop after a new track starts: buffer, "waiting for network…" (Rack: BUF), resumes within 1–2 s at the same spot, no toast | |
| 6 | Same in exclusive mode: no device re-lock click, no output reopen in the log | |
| 7 | Crossfade/AutoMix on: a drop near the end still crosses into the next track | |
| 8 | Pause while waiting, restore network, play: resumes at the stop point | **Attempted 02:30, not valid** — the pause happened mid-track with the network already off, not while the player was stuck waiting, so the resume-at-pause-point behaviour seen is the ordinary pause path. Re-run needed. |
| 9 | Wi-Fi off >5 min: keeps waiting, continues when it returns | |
| 10 | Link up, no internet >5 min: error appears, Retry resumes at position | |

### Track boundary and offline cache

| # | Row | Result |
|---|-----|--------|
| 11 | Network off while N plays, N+1 already downloaded: N+1 plays | |
| 12 | Network off, previously played track at same quality: plays from cache | |
| 13 | Track cached before the work (no `.meta`): plays, ReplayGain not applied | |
| 14 | Network off, never-played track: waits, plays when the network returns | |

### Offline start

| # | Row | Result |
|---|-----|--------|
| 15 | Start with network off: signs in from saved session, pages fail, queue/cached tracks play | |
| 16 | Network on: browsing works without signing in | |
| 17 | Sign out removes `config\tidal_session.json` | |

### Housekeeping

| # | Row | Result |
|---|-----|--------|
| 18 | Clear cache removes `.fmp4`, `.fmp4.meta`, `.part`; leaves `palettes.json` / `lyrics.json` | **PASS** (run early, at 02:18:27, because the owner cleared the cache before row 1): all 61 `.fmp4` + 61 `.meta` gone; `lyrics.json`, `palettes.json`, `track_flags.json` kept. Two `.part` files present seconds later were new downloads the in-flight playback re-created after the clear (one committed 30 s later), not survivors — `DiskCache::clear` deletes `.part` too (`cache.rs:243`). |

## Session 2 — ASIO (12 rows)

Source: `2026-10-11-asio-smoke.md`. Not started.

## Session 3 — 96 kbps / Low listening rows

Source: `2026-10-16-96kbps-validation.md` rows 104–113 (the restart-persistence and
three-mode selector rows belong to the later GUI pass). Not started.
