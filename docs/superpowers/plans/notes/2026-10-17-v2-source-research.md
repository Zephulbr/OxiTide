# V2 sources and formats — research (2026-10-17)

- **Status:** Research; no code change. Feeds `specs/2026-10-17-oxitide-v2-modular-sources-design.md`
  (V2-D1, V2-D9, V2-D10).
- **Owner:** V
- **Context:** V2.0 ships TIDAL + Local only. Jellyfin and Qobuz are 2.x candidates; this note records
  what is known so far so the 2.x calls can be made without redoing the search. Web sources were
  checked on 2026-10-17; claims marked *unverified* come from third-party docs or forums.

## 1. Jellyfin (2.x candidate — owner will set up a test server)

**Fit:** good. Self-hosted, open API, no ToS exposure, and it covers the owner's "NAS and anything
remote" case: the server scans the share, OxiTide only streams. It is a **byte source** (the engine
gets the original file), so DSP, ReplayGain, AutoMix and bit-perfect output all apply.

| Topic | Finding |
|---|---|
| Auth | Username + password → access token (`/Users/AuthenticateByName`); Quick Connect exists as a code-based sign-in (endpoints not re-checked). Send the token in the standard `Authorization: MediaBrowser Client="…", Device="…", DeviceId="…", Version="…", Token="…"` header. **Jellyfin 12.0 (Sept 2026) disables legacy auth by default** — `X-Emby-Token`, `X-MediaBrowser-Token`, the `api_key` query parameter and the `/emby/` / `/mediabrowser/` paths — on new *and* existing servers. Use only the `Authorization` header; never put the token in a URL. |
| Streaming | `GET /Audio/{id}/stream?static=true` returns the original file without transcoding (documented on the single-item endpoint; the "universal" endpoint has no `static` flag and may transcode). One HTTP file per track → fits a "single file over HTTP" plan; seeking needs HTTP range requests or the existing download-ahead to disk. |
| Token in stream URLs | Because 12.0 drops `api_key`, the engine's fetcher must send the `Authorization` header on media requests too (today `HttpFetcher` only takes a URL). |
| Metadata | Albums/artists/tracks/playlists/genres through `/Items` queries; images through `/Items/{id}/Images/Primary`. |
| Extras | `InstantMix` endpoints (from song/album/artist/genre/playlist) give server-side mixes; a Lyrics API group exists (paths not confirmed); Playstate endpoints (playback start / progress / stopped) let the server record plays and resume points; favorites and playlists are server-side and writable. |
| Versions | Target 10.10+ and 12.x; both accept the `Authorization: MediaBrowser` header. |
| Risks | API drift between majors (12.0 just broke legacy clients); server reachable only on the LAN unless the owner exposes it; self-signed TLS on home servers (allow a per-server "trust this certificate" option, never a global off switch). |

**Rec:** first 2.x source. It reuses most of the Local work (same "file over HTTP" plan shape as
Qobuz, server-side catalog instead of our index) and has no legal exposure.

Sources: [Kotlin SDK — getAudioStream](https://kotlin-sdk.jellyfin.org/dokka/jellyfin-api/org.jellyfin.sdk.api.operations/-audio-api/get-audio-stream.html),
[Kotlin SDK — getUniversalAudioStream](https://kotlin-sdk.jellyfin.org/dokka/jellyfin-api/org.jellyfin.sdk.api.operations/-audio-api/get-universal-audio-stream.html),
[TypeScript SDK — universal audio request](https://typescript-sdk.jellyfin.org/interfaces/generated-client.UniversalAudioApiGetUniversalAudioStreamRequest.html),
[Jellyfin auth overview (mintlify)](https://www.mintlify.com/jellyfin/jellyfin/api/authentication/overview),
[Jellyfin 12.0 security fixes / legacy login removal (Help Net Security)](https://www.helpnetsecurity.com/2026/09/08/jellyfin-12-0-security-fixes/),
[Anchorr v1.6.1 migration note](https://newreleases.io/project/github/openVESSL/Anchorr/release/v1.6.1),
[API mirror — InstantMix / Playstate groups](https://jellyfin.apifox.cn).

## 2. Qobuz (2.x, "?" — research-gated)

**Fit technically:** good — it serves direct FLAC files (up to 24/192), a single-file plan that is
simpler than TIDAL's DASH. **Fit legally / operationally:** poor.

| Topic | Finding |
|---|---|
| Access | The REST API (v0.2) is **partner-only**: an `app_id` + `app_secret` are issued by Qobuz on request (api@qobuz.com) under API Terms of Use governed by French law. Public API docs were taken down. |
| Signing | Every call is signed with the app secret, so Qobuz sees which app makes which calls (*unverified*, forum). Stream URLs come from `track/getFileUrl` with a request signature (endpoint details not re-checked). |
| Unofficial use | Open-source clients pull an app id/secret from the Qobuz web player bundle; libraries warn this "may break at any time". Forum reports say Qobuz has **rejected reused credentials** (the Kodi plugin's id returned 400s) and told developers never to share ids/secrets (*unverified*, date unclear). |
| Official route | Qobuz Connect (100+ partners, Dec 2025) is a hardware/partner programme, not an open API. |
| Account | Requires the owner to hold a Qobuz subscription; none confirmed yet. |

**Risk compared with TIDAL:** higher. TIDAL's PKCE flow uses a public client id; Qobuz signs every
call with a secret it can revoke per app, and it has done so. A scraped secret could stop working
any day, and shipping one in a public repository is a ToS breach on its face.

**Rec:** keep "?" until the owner (a) has a subscription and (b) decides whether to ask Qobuz for
partner credentials (unlikely for a personal AGPL app) or accept the breakage/ToS risk of a scraped
web-player id that is **never committed** (user-supplied at runtime). Revisit after Jellyfin.

Sources: [Qobuz API directory entry](https://providers.apievangelist.com/providers/qobuz/),
[apis.io — Qobuz](https://apis.io/providers/qobuz/),
[moOde forum — Qobuz API credentials](https://moodeaudio.org/forum/showthread.php?pid=63752),
[moOde forum — unofficial usage](https://moodeaudio.org/forum/showthread.php?pid=13681),
[QobuzApiSharp](https://www.nuget.org/packages/QobuzApiSharp),
[Qobuz Connect partners (Shore Fire)](https://shorefire.com/releases/entry/qobuz-connect-celebrates-over-100-partners).

## 3. Formats (owner library: ~750 tracks, ~99 % FLAC, some MP3)

The workspace pins `symphonia = 0.6` with `flac`, `isomp4`, `pcm`, `aac`.

| Format | Status | V2.0 |
|---|---|---|
| FLAC | Supported. STREAMINFO carries an MD5 of the decoded audio (used for track identity, V2-D6). | **Required** |
| MP3 | Needs the `mp3` symphonia feature. Gapless needs the LAME/Xing delay + padding; symphonia's gapless mode is opt-in (`FormatOptions::enable_gapless`) and depends on demuxer + decoder support — **whether the MP3 demuxer reads the LAME tag is unconfirmed** (check `symphonia-bundle-mp3` source before planning). | **Required** (gapless = smoke row) |
| ALAC / AAC (M4A), WAV, AIFF, Ogg Vorbis | Covered by existing or cheap features (`alac`, `vorbis`, `ogg`, `aiff`). | Best effort, lightly tested |
| Opus | symphonia 0.6 lists a separate `symphonia-codec-opus` crate with no quality rating; alternative is a libopus adapter (C). | Deferred |
| APE | `ape-decoder`: pure Rust, all levels, 8–32 bit, seeking; needs file version ≥ 3950. | Deferred (feature-flag candidate) |
| WavPack | Only C bindings (`wavpack` crate, CMake + C++ toolchain). | Deferred |
| DSD (DSF/DFF) | Container readers exist (`dsf`, `dsd-reader`, `dsd-source`); no DoP crate found. Path: DSD → PCM through our DSP first; native DSD/DoP over ASIO much later, if ever. | Deferred |

Tags: **`lofty` 0.23** is the usual cross-format choice. FLAC uses Vorbis comments (writable);
ID3v2 inside FLAC is read-only; MP4 uses `ilst`. Picture writing per format still to be confirmed
in its API docs before the tag-editor plan. `id3` is an MP3-only alternative.

Sources: [Symphonia](https://github.com/pdeljanov/Symphonia),
[Symphonia supported codecs](https://www.mintlify.com/pdeljanov/Symphonia/resources/supported-codecs),
[symphonia 0.6.1](https://docs.rs/crate/symphonia/0.6.1),
[symphonia gapless docs](https://docs.rs/symphonia/^0.5),
[MPD — LAME tag parsing](https://git.pvv.ntnu.no/oysteikt/mpd/commit/1523e8a84290690e67474534a99a250be47d751b),
[ape-decoder](https://docs.rs/ape-decoder),
[wavpack crate](https://rust-digger.code-maven.com/crates/wavpack),
[dsf](https://docs.rs/dsf), [dsd-reader](https://docs.rs/dsd-reader),
[dsd-source](https://docs.rs/crate/dsd-source/0.2.0),
[lofty 0.23.2](https://docs.rs/crate/lofty/latest), [id3](https://docs.rs/crate/id3/latest).

## 4. Prior art for the architecture

Mopidy (backends + frontends + GStreamer output), Music Assistant (music providers + player
providers, capability-driven), Roon (Core / Remote / Output), Strawberry (local + TIDAL/Qobuz/
Subsonic), MPD (headless server + clients). The shared lessons: providers declare capabilities
instead of implementing one large interface, and "player" sits above PCM output once network
renderers exist (out of scope for V2.0).
