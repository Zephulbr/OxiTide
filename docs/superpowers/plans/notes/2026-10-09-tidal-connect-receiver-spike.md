# TIDAL Connect receiver spike findings (2026-10-09)

Feasibility spike (not committed). Question: can OxiTide appear as a device in the TIDAL
phone app's output picker, so the phone controls playback on the PC (the "desktop Connect"
TIDAL never shipped)? **Answer: yes, and the phone-facing protocol is not ours to
implement.** A binary that ships in every TIDAL desktop install terminates the phone
protocol and hands the host a small plain-JSON websocket. Spike code stayed in `%TEMP%`
(throwaway, no repo changes); the real phone app drove a stand-in host end to end.

## Prior art

- TidaLuna's `DesktopConnect` plugin (`Inrixia/luna-plugins`) turns on TIDAL's own dormant
  receiver code: it advertises `_tidalconnect._tcp` and reuses the app's
  `RemoteDesktopController`. TIDAL's FAQ only ever said desktop control was "coming soon".
- The public `tidal-music` GitHub org (all Apache-2.0) has no Connect device SDK. The binary
  is built from a private `tidal-connect-device-sdk` repo and stays TIDAL's property.

## Architecture (TIDAL 2.43.2, read from the shipped JS and the binary)

```
phone TIDAL app ──mDNS _tidalconnect._tcp──▶ tidal_connect_application.exe (SDK 1.1.8)
                   TLS websocket, port 2019         │ plain JSON websocket, ws://localhost:9000
                                                    ▼
                                       host app (Electron main in TIDAL; OxiTide would be this)
```

- Binary: `resources\app.asar.unpacked\resources\win\tidal_connect_application.exe`. CLI is
  only `-l <0-6>` (log level) and `-p <port>` (the phone-facing port, default 2019). Listens
  on `[::]:2019` and `[::]:9000`. Passing `-p 9000` collides the two.
- TIDAL's `RemotePlayer.js` spawns it with no args and **respawns it on any exit**.
- mDNS is the host's job: the binary does not advertise. Service type `_tidalconnect._tcp`,
  SRV port 2019, TXT `mn=RemoteDesktop`, `id=<md5(hostname)>`, `fn=<friendly name shown in
  the phone>`, `ca=0`, `ve=1` (meanings of `ca`/`ve` unknown).
- The host must **ping the websocket every 15 s** (TIDAL's host does); an unpinged client is
  dropped after ~40 s.

## Protocol (observed live)

Binary → host:

| Command | Notes |
|---|---|
| `notifySessionState {state, appInfo}` | 1 = connecting, 2 = active, 0 = ended |
| `requestResources` | reply `grantResources`; binary then sends `notifyRequestResult {requestId, resultCode}` (0 = ok) |
| `prepare {mediaInfo:{mediaId, mediaType}}` | `mediaId` = TIDAL track id, `mediaType` 0 = track. Only the id |
| `notifyMediaChanged {mediaInfo}` | `itemId` (queue UUID), `mediaId`, `metadata{title, artists[], albumTitle, duration (ms), images{low,medium,high}.url}`, `customData` (JSON string: radio / normalization flags, country) |
| `play`, `pause`, `stop` | transport |
| `seek {position}` | ms; arrives before `play` on resume |
| `setShuffle {shuffle}`, `setRepeatMode {repeatMode}` | `repeatMode` seen as `"ALL"`; echo back to confirm |
| `prepareNext {mediaInfo}` | preload hint (from TIDAL's host code; not yet seen in the spike) |
| `releaseResources`, `notifySessionState {state:0}` | session end |
| `notifyPlayerStatusChanged {playerState, progress, duration}` | the binary's own mirror of our status |

Host → binary: `grantResources`, `revokeResources`,
`onStatusUpdated {playerState: idle|buffering|playing|paused, type:"media"}`,
`onProgressUpdated {progress (ms), duration, type:"media"}`,
`onPlaybackCompleted {hasNextMedia, type:"media"}`, `onRequestNextMedia {type:"media"}`,
and echoes of `setShuffle` / `setRepeatMode` / `setVolumeMute {volume:{level,mute}}`.

The phone owns the queue and sends one item at a time. Skip = `stop`, `prepare(new id)`,
`pause` or `play`. No audio crosses the phone: the host plays from its own account.

## Findings and pitfalls

1. **Race:** `prepare`, `notifyMediaChanged`, `seek` and `play`/`pause` arrive within ~3 ms.
   A host must queue `seek`/`play` until the track is loaded, and must not let a late
   "ready" transition override a `play` that already arrived (the spike's first bug).
2. **The binary crashes at the end of every session** (`0xC0000005`, right after
   `notifySessionState 0`); sending `revokeResources` does not prevent it. A supervisor with
   a ~1 s respawn is therefore mandatory, and the websocket client must reconnect. Without it
   the next connect fails in the phone with "failed to connect to server". Reconnect after a
   respawn worked in the spike.
3. **Firewall:** discovery needs the host process to receive multicast UDP 5353. A Public
   profile inbound **Block** rule for `node.exe` silently prevented discovery; the same
   script from a copy of the exe (new path, new Allow rule) was found immediately. TIDAL
   already installs Allow rules for the binary itself (ports 2019/9000). An OxiTide build
   will need its own Allow rules (installer or first-run prompt).
4. **Killing the host does not kill the child binary:** orphaned instances keep holding
   2019/9000. OxiTide must own the process lifetime (job object / kill on exit).
5. `onProgressUpdated.duration = 0` is accepted (TIDAL's host sends it too); the binary then
   mirrors `duration: 0` back, and the phone seems to use the metadata duration instead.

## Can we replace the binary? (TLS identity test, 2026-10-09)

A stand-in TLS websocket server on :2019 with a **self-signed** certificate, advertised with
the same mDNS record (real binary stopped), was selected from the phone app. The phone
opened TCP, then aborted the handshake: `tlsv1 alert unknown ca` (alert 48), twice. So the
phone validates the receiver's certificate chain against a trust anchor we cannot satisfy.
It connects by IP from mDNS, so a public-CA hostname certificate cannot be the anchor; this
points to a private TIDAL CA (or pinning), consistent with the binary decrypting an embedded
certificate and key at startup (`CMS_decrypt`, `PEM_read_bio_PrivateKey`).

**What the real binary presents** (public leaf certificate seen by a plain TLS handshake to
`127.0.0.1:2019`, TLS 1.2 and 1.3 both accepted; the private key was not touched): subject
`C=no, O=Tidal, OU=Desktop` (no CN, no subjectAltName), issuer `C=no, O=TIDAL, CN=TIDAL TS
CA`, RSA-2048, valid 2021-05-18 to 2031-05-16, leaf only (the CA is not sent). With no name
in it, the phone cannot be doing hostname verification; it must be validating the chain to
the "TIDAL TS CA" trust anchor. Still an inference, not proof: we have not seen the phone's
trust configuration, only that it rejects a self-signed certificate.

**Consequence:** a clean-room replacement of the phone-facing server is not possible without
a TIDAL-issued identity. Extracting the key embedded in the binary would circumvent its
protection and is out of scope. Appearing in the TIDAL phone app therefore requires TIDAL's
binary (the sidecar approach); anything independent of it is a different feature (phone
remote that does not use the TIDAL app).

## Additional static findings

- Host-facing replies the binary understands: `onStatusUpdated`, `onProgressUpdated`,
  `onPlaybackCompleted`, `onRequestNextMedia`, `onPlaybackError`, `onAudioFormatUpdated`,
  plus `setVolumeMute` echo. It also has queue plumbing (`requestQueueItems`,
  `respondQueueItems`, `selectQueueItem`, `notifyQueueChanged`) and `procNotifyTokenExpired`,
  used by the hardware flow where the controller hands over tokens and queue/content server
  URLs; the desktop flow only passes a `mediaId`.
- TIDAL's own controller reads only the `fn` and `id` TXT keys (`mdns/browser.js`,
  `TidalConnectController.js`) and hides receivers on its own machine.

## Open questions

- Natural track end: expected `onPlaybackCompleted` / `onRequestNextMedia` / `prepareNext`;
  not exercised. Needed for gapless behaviour.
- Volume: the phone never sent `setVolumeMute` in the spike; whether the picker shows a
  volume control depends on something the host reports.
- Meaning of `notifyRequestResult` code 1003 (seen once, right after session start).
- Queue edits from the phone, repeat-one, and a second controller joining.
- Whether `ca` / `ve` TXT values or `mn` change which features the phone offers.

## Constraints

- **Not bundleable.** The binary is TIDAL's proprietary SDK, no licence found. OxiTide must
  locate the user's own TIDAL install (`%LOCALAPPDATA%\TIDAL\app-*\...`, glob because the
  path embeds the version) and treat the feature as unavailable without it.
- Cannot run alongside TIDAL desktop with DesktopConnect enabled (same ports).
- Running TIDAL's binary outside its app is a terms-of-service grey area; acceptable for a
  personal-use client, but opt-in and clearly labelled.
- Linux and macOS ship different binaries; scope any first slice to Windows.

## Sketch for OxiTide (if pursued)

- Module (own crate or `oxitide-app`): locate binary → supervise (spawn, 1 s respawn, kill
  on exit) → websocket client (ping 15 s, reconnect) → mDNS advertise (`fn` = configurable
  friendly name) → map messages onto `EngineCmd` / `EngineEvent`.
- Mapping: `prepare` → resolve `mediaId` via `oxitide-tidal` → `PlayTracks`/`Enqueue`;
  `play`/`pause`/`seek`/`stop` → `PlayPause`/`Seek`/`Clear`; shuffle/repeat → `SetShuffle`/
  `SetRepeat`; `EngineEvent::State`/`Position` → `onStatusUpdated`/`onProgressUpdated`;
  track end → `onPlaybackCompleted`.
- Opt-in `[connect]` config block and a Settings toggle; tray/status surface for an active
  session. The session transport it needs is the same primitive the listen-along
  assessment calls for.
- A real design would be its own spec → plan cycle after the open questions are answered.
