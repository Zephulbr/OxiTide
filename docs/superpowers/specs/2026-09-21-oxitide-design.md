# OxiTide — Design & Plan

- **Status:** Draft — **Phase 0 spike COMPLETE (DRM-free confirmed)**
- **Date:** 2026-09-21
- **Owner:** V
- **Working name:** OxiTide

---

## 1. Purpose

Build a **fast, lightweight, reliable alternative TIDAL desktop client for Windows** that
just works. The official TIDAL desktop app suffers from buffering, sluggishness, and
general jank; OxiTide aims to beat it on responsiveness, playback reliability, and
buffering/prefetch behaviour while preserving full sound quality.

The intended outcome is a **daily-driver client** that streams the user's real TIDAL
account and library at every quality the official client supports.

## 2. Goals / Success Criteria

1. **Real account playback** of Low, High, Lossless, and Max (24-bit FLAC) tiers.
2. **Bit-perfect output** option (WASAPI exclusive mode) with correct sample-rate handling.
3. **Responsiveness:** near-instant search, browse, and navigation; no UI stalls.
4. **Playback reliability:** gapless playback, aggressive read-ahead/prefetch so playback
   does not stutter where the official client does.
5. **Daily-driver UX:** queue, library, playlists, lyrics, discovery, offline — without
   feeling heavy.

## 3. Non-Goals (deferred, not v1)

- **Dolby Atmos decode** — stretch goal; assess after stereo is solid.
- **Tidal Connect / casting** — later phase.
- **DSP / EQ / crossfeed** — later, and mutually exclusive with bit-perfect mode
  (bit-perfect = no DSP).
- **Non-Windows platforms** — Windows-first.

## 4. Constraints & Risks

### 4.1 Unofficial API (accepted)
TIDAL's official Developer Platform exposes the **SDK Player module as the only
sanctioned third-party playback path, and it plays previews only**. There is no Windows
player SDK. A full client therefore relies on the **community / reverse-engineered TIDAL
API** (as used by `tidalapi`, `tidalrs`, `streamrip`, `OrpheusDL`).

**This is a ToS / DRM grey area. The owner accepts this risk knowingly.** OxiTide is a
personal-use client. No DRM keys are to be extracted if a cleaner path exists.

### 4.2 DRM — RESOLVED: none for stereo
The Phase 0 spike proved that **all stereo tiers are unencrypted** (see §5b). Low/High (AAC),
Lossless (FLAC 16/44.1), and Max/Hi-Res (FLAC 24-bit up to 192 kHz) all arrive as plain
MPEG-DASH fMP4, directly playable. **No CDM/license work is needed for v1.**

The only remaining DRM unknown is **Dolby Atmos** (E-AC-3 / AC-4), which is out of v1 scope
and deferred to Phase 4.

### 4.3 Tauri + audio
Tauri's WebView **cannot** perform exclusive-mode / bit-perfect output. The audio engine
must therefore be **pure Rust in the backend**, independent of the UI. This is planned and
is a good separation.

## 5. Phase 0 Spike — Findings So Far (desk research)

| Tier | Container / manifest | Protection |
|---|---|---|
| Low / High (AAC 96/320) | `vnd.tidal.bts`, direct URL | None (`encryptionType: NONE`) |
| Lossless (FLAC 16/44.1) | `vnd.tidal.bts`, direct `.flac` URL | None |
| **Max / Hi-Res (24-bit FLAC)** | `application/dash+xml` (base64 MPD) | **TBD — `cenc` namespace present; needs empirical check** |
| Dolby Atmos | E-AC-3 / AC-4, separate manifests | DRM'd (out of scope) |

**Building blocks identified:**

- `tidalrs` — Rust TIDAL API client (device-flow OAuth, catalog, DASH `playbackinfo`).
- `wasapi` (henquist) — exclusive-mode, event-driven Windows audio → bit-perfect base.
- `cpal` — alternative Windows audio backend (WASAPI host).
- `widevine` (Rust port of pywidevine) — CDM via extracted `.wvd` device (key extraction).
- `cdm-host` — drives the **system** CDM via the official interface, no key extraction
  (preferred if a CDM is ever needed; currently Linux `.so`, concept ports to Windows
  `widevinecdm.dll`).
- Decoding: `symphonia` (pure-Rust FLAC/AAC) or `ffmpeg`.

## 5b. Phase 0 Spike — VERDICT (live probe, 2026-09-21)

**Every stereo tier is delivered as plain, unencrypted MPEG-DASH fMP4. There is NO DRM.
No Widevine, no CDM, no license server is required for Low/High/Lossless/Max.**

Evidence (track `77646170` *The Golden Age* — Beck, *Sea Change*):

| Tier | Manifest | ContentProtection | pssh | Segment | Result |
|---|---|---|---|---|---|
| HI_RES_LOSSLESS | MPD, codecs=`flac`, `audioSamplingRate=192000`, `bitDepth=24` | **0** | **0** | `ftyp/moov/dfLa/fLaC` init + `moof/mdat`, `mdat` starts `ff f8` (FLAC sync) | plain FLAC ✅ |
| LOSSLESS | MPD, codecs=`flac` | 0 | 0 | same, plain | ✅ |
| HIGH / LOW | MPD, codecs=`mp4a` (AAC) | 0 | 0 | plain fMP4 | ✅ |

The only `cenc`/`schemeIdUri` matches in the MPD are the XML namespace declaration and the
DASH `Role` / `AudioChannelConfiguration` descriptors — **not** encryption.

**End-to-end decode proof:** downloaded all 71 segments (195.8 MB), concatenated to a single
fMP4, then `ffprobe` → `codec=flac, sample_fmt=s32, sample_rate=192000, channels=2,
bits_per_raw_sample=24, duration=276s`; `ffmpeg` decoded 10 s with **exit 0 and no errors**.

**Auth finding:** `HI_RES_LOSSLESS` requires the **PKCE** authorization flow
(`login_pkce`, `appMode=android`). A device-authorization/simple OAuth session does not
unlock Hi-Res. Session is PKCE (`is_pkce=True`), country `GR`.

**Consequence for the architecture:** the planned "pluggable decrypt stage" is
**unnecessary for stereo**. The pipeline simplifies to
`manifest → fetch fMP4 segments → demux FLAC/AAC → decode → WASAPI output`.
The DRM risk is retired; the only remaining DRM unknown is Dolby Atmos (out of v1 scope).

## 6. Open Questions

1. ~~Is the Max / Hi-Res DASH content actually encrypted?~~ **RESOLVED: no — DRM-free.**
2. ~~What auth flow works?~~ **RESOLVED: PKCE for Hi-Res.**
3. Segment structure: **RESOLVED — SegmentTemplate, init (`ftyp/moov/dfLa`) + media
   (`moof/mdat`), FLAC frames in `mdat`, 71 segments for a 4:36 track (~2.8 MB/seg).**
4. (Open, new) Does Dolby Atmos need DRM — deferred to Phase 4.

## 7. Tech Stack (decided)

- **Shell/UI:** Rust + Tauri (WebView2 front end).
- **Audio engine:** pure Rust, backend-side: `manifest → [decrypt if required] → decode →
  WASAPI exclusive/shared output`.
- **Decode:** `symphonia` first; `ffmpeg` fallback if needed.
- **API client:** `tidalrs` if solid, else a thin custom client.
- **DRM (only if proven necessary):** pluggable decrypt backend — system-CDM first,
  `.wvd` CDM fallback.

## 8. Draft Architecture

- **Auth** — OAuth login, token storage/refresh.
- **API client** — catalog, library, playlists, search, lyrics, recommendations.
- **Playback engine** — queue/state machine, prefetch/prefetch buffer, gapless.
- **Stream pipeline** — manifest fetch → decrypt (pluggable) → decode → output.
- **Output** — WASAPI shared/exclusive, device selection, sample-rate handling.
- **Cache / Offline** — download manager + local cache (later phase).
- **UI** — library, search, player, queue, now-playing, lyrics, discovery, settings.
- **Integrations** — Last.fm, Discord RPC, SMTC/media keys (later phase).

## 9. Phasing

| Phase | Scope |
|---|---|
| **0 — Spike (current)** | Prove Max/lossless playback feasibility in Rust/Tauri on Windows. |
| **1 — MVP** | Auth, catalog/library, search, playback (all tiers), queue, WASAPI exclusive + bit-perfect, solid UI, buffering/prefetch. |
| **2 — Library & discovery** | Playlist/favorites editing, synced lyrics, discovery feeds/mixes. |
| **3 — Offline & integrations** | Download manager + cache, Last.fm, Discord RPC, SMTC/media keys. |
| **4 — Advanced audio** | DSP/EQ (opt-in mode), Tidal Connect/casting, Atmos (if feasible). |

Each phase gets its own spec → plan → implementation cycle.

**Phase 1 spec:** `docs/superpowers/specs/2026-09-21-oxitide-phase1-design.md`

## 10. Phase 0 Probe Plan — COMPLETE

All work was throwaway, isolated in `%TEMP%\opencode\oxitide-spike` (nothing committed to
the workspace). Tooling used: portable `uv` + isolated Python 3.12 + `tidalapi` 0.8.11 +
`ffmpeg` 9.0.2 for decode validation.

Steps executed:

1. ✅ Isolated env set up (uv → CPython 3.12 + `tidalapi`).
2. ✅ Owner logged in via PKCE (browser); session stored in throwaway dir only.
3. ✅ Fetched `HI_RES_LOSSLESS` / `LOSSLESS` / `HIGH` / `LOW` `playbackinfo`; dumped MPDs.
4. ✅ Inspected manifests (0 `ContentProtection`, 0 `pssh`) and segments (plain fMP4, FLAC sync).
5. ✅ Verdict: **not encrypted; no license/CDM required**.
6. ✅ Full-track download (195.8 MB) + `ffprobe`/`ffmpeg` decode → 24-bit/192 kHz FLAC, clean.

**Outcome: DRM risk retired. Proceed to Phase 1 design.**

## 11. Decisions Log

| # | Decision | Rationale |
|---|---|---|
| D1 | Real TIDAL account client (not local player). | Owner's goal. |
| D2 | Target all tiers up to 24-bit FLAC; Atmos stretch. | Owner's goal. |
| D3 | Rust + Tauri stack. | Owner's preference + fit. |
| D4 | All ten feature pillars are wanted, **phased** (not one giant v1). | Scope management. |
| D5 | Spike Max/DRM feasibility before committing to Phase 1 design. | De-risk biggest unknown. |
| D6 | Audio engine must be pure-Rust backend (WebView can't do exclusive mode). | Tauri limitation. |
| D7 | Use **PKCE** auth (not device OAuth) — required for Hi-Res. | Spike finding. |
| D8 | **No decrypt stage for v1** — streams are DRM-free. | Spike finding; simplifies pipeline. |
| D9 | Decode via `symphonia` in Rust; `ffmpeg` proven as fallback/validation. | Spike + stack fit. |

## 12. Legal / Ethical Note

OxiTide is a personal-use client built against an unofficial API. It should not extract or
redistribute DRM keys when a cleaner method exists, and it is not to be distributed as a
DRM-circumvention tool. The owner accepts the ToS risk of using their own account.
