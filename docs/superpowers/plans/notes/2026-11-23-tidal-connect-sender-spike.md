# TIDAL Connect sender spike: casting from OxiTide to a Connect device (issue #11)

- **Date:** 2026-11-23
- **Status:** Open. The protocol is mapped and the queue server accepts OxiTide's token; the
  one remaining risk (does a receiver accept our token, §5) needs the receiver-binary test bench
  or a real device. No spec yet; Connect stays parked in the roadmap and V2 scope.
- **Trigger:** GitHub issue #11 ("TIDAL Connect so I can use my WiiM Pro").
- **Related:** `2026-10-09-tidal-connect-receiver-spike.md` (the opposite direction).

## Question

Can OxiTide act as a TIDAL Connect **controller**: discover a Connect receiver (WiiM, Bluesound,
…) on the LAN and make it play the user's queue? The 2026-10-09 spike answered the reverse
(OxiTide as a receiver for the TIDAL phone app) and found a TLS identity wall; its conclusion
does not carry over.

## Sources

- TIDAL desktop 2.48.0 main process, `resources\app.asar` → `app/main/tidalConnect/*.js`,
  `app/main/mdns/browser.js` (read locally, not committed).
- TIDAL's web front end served from `https://desktop.tidal.com/` (the desktop app's
  `clientURL`): public JS bundle, main chunk `assets/index-Dl6KHeAX.js` on 2026-11-23. The
  values the main process forwards to the device originate here.
- One live probe against `connectqueue.tidal.com` with OxiTide's own token (§4).

## 1. No identity wall on the sender side

`tidalConnect/websocket.js` opens `wss://<device ip>:<port>` with TIDAL's CA chain embedded
("TIDAL Root CA" → "TIDAL TS CA"), `rejectUnauthorized: true`,
`checkServerIdentity: () => null`, and **no client certificate or key**. Only receivers carry a
TIDAL-issued identity; any app can be a controller. A `disableTls` (plain `ws://`) option exists
in TIDAL's code; OxiTide must never use it (tokens cross this socket, §3).

Discovery: mDNS `_tidalconnect._tcp.local`; TIDAL's browser reads the `fn` (friendly name) and
`id` TXT keys and connects to the first IPv4 address. Heartbeat: WebSocket ping/pong.

## 2. What the controller sends (from the web bundle)

Fixed configuration, passed to `tidalConnect.initialize` at startup:

| Field | Value |
|---|---|
| `authServerUrl` | `https://auth.tidal.com/v1/oauth2/token/` |
| `contentServerUrl` | `https://api.tidal.com/v1/` |
| `queueServerUrl` | `https://connectqueue.tidal.com/v1/queues` |
| `serviceType` | `_tidalconnect._tcp` |

Authentication: `tidalConnect.setAuthentication(String(userId), accessToken)`. The
`sessionCredential` in `startSession` is **the user id as a string**. **No refresh token is
passed** (the main process's `refreshToken` stays `''`); when the controller's token refreshes
it calls `setAuthentication` again and the main process sends `updateServerInfo` to the device.

Server info objects sent to the device (`serverInfo.js`), one for content and one for queue:

```json
{ "serverUrl": "...", "httpHeaderFields": [], "queryParameters": { "audiomode": "STEREO", "audioquality": "<quality>" },
  "authInfo": { "oauthServerInfo": { "serverUrl": "https://auth.tidal.com/v1/oauth2/token/",
    "authInfo": { "headerAuth": "Bearer <access token>", "oauthParameters": { "accessToken": "<access token>", "refreshToken": "" } },
    "formParameters": { "grant_type": "switch_client", "scope": "r_usr" }, "httpHeaderFields": [] } } }
```

So the device exchanges the controller's access token for one under its own client
(`switch_client`) and streams directly from TIDAL. No audio passes through the controller.

## 3. Message sequence (controller → device, JSON over the WebSocket)

1. `{command:"startSession", appId:"tidal", appName:"tidal", sessionCredential:"<userId>"}` →
   device answers `notifySessionStarted {sessionId, joined}` (this reply carries no `requestId`).
   `resumeSession {sessionId}` reattaches; `endSession {sessionId, stopCasting}` ends.
2. Create a cloud queue over HTTP (§4), then
   `{command:"loadCloudQueue", requestId, autoplay, position (ms), currentMediaInfo,
   queueInfo:{queueId, repeatMode, shuffled, maxAfterSize:10, maxBeforeSize:10},
   contentServerInfo, queueServerInfo}`.
   `currentMediaInfo` = `{itemId (queue item id), mediaId (track id string), mediaType (0 track,
   1 video), metadata:{title, artists, albumTitle, duration (ms), images:{low,medium,high}}}`
   (images at 320/640/1280 px).
3. Transport: `play`, `pause`, `stop`, `next`, `previous`, `seek {position ms}`,
   `setVolume`, `setMute`, `selectQueueItem {mediaInfo, policy}`, `setShuffle`,
   `setRepeatMode`, `refreshQueue {queueId}`, `loadMediaInfo`, `updateServerInfo`. Every
   command carries an incrementing `requestId`; the reply echoes it.
4. Device → controller: `notifyPlayerStatusChanged`, `notifyMediaChanged`,
   `notifyQueueChanged`, `notifyQueueItemsChanged`, `notifyDeviceStatusChanged`,
   `notifyContentServerError`, `notifyQueueServerError`, `notifySessionStarted`. TIDAL's
   controller treats a closed socket as the session ending.

A possible OxiTide shape: the HTTP side through `TidalClient` with the existing token, plus a
small WebSocket client whose only trust anchor is the TIDAL CA.

## 4. Cloud queue API (`https://connectqueue.tidal.com/v1/`)

All calls are Bearer-authenticated, with `countryCode`, `locale`, `deviceType` query parameters
like other v1 calls.

- `POST queues` body
  `{items:[{media_id:"<id>", type:"track", properties:{active:"false", original_order:"0",
  source_id:"<album/playlist id>", source_type:"ALBUM"}}], properties:{position:0},
  repeat_mode:"off"|"all"|"one", shuffled:false}` → `{id, items[{id, media_id, properties,
  type}], properties.position, repeat_mode, shuffled}` plus an `ETag` header. TIDAL sends at most
  100 items per request and pages the rest in.
- `GET queues/{id}`, `GET queues/{id}/items?limit=50&offset=` (`total` in the reply).
- `PUT queues/{id}/items` `{item_id?, items, mode:"append"|"prepend"}` with `If-Match: <etag>`;
  `PATCH queues/{id}/items` `{after, ids}` (move); `DELETE queues/{id}/items/{uid}`;
  `PUT queues/{id}/shuffle` `{current_item, shuffle}`.
- `DELETE queues/{id}` requires `If-Match` (400 `input_validation_error` without it).

**Live probe (2026-11-23), OxiTide's own PKCE token:** `GET /v1/sessions` ok →
`POST queues` with one track created a queue (id, one item, `position 0`, `repeat off`, ETag
present) → `GET` queue and items ok (`total 1`) → `DELETE` with `If-Match` → 204, then `GET` →
404 `queue_not_found`. **The queue server accepts OxiTide's token.** Script kept out of the repo;
it read the token from Credential Manager and never printed it.

TIDAL's bundle also has a `cloud-queue-v2` feature flag (off) for a newer OpenAPI-style queue;
v1 may be retired eventually.

## Prior art: `nbonamy/tidal_streamer` (Apache-2.0, last commit 2026-11-08)

A Node server that casts to Connect devices without the TIDAL app; read 2026-11-23 (not run).
It independently matches §2–§4: mDNS `tidalconnect`/tcp via `bonjour-service`, `wss://` to
the advertised port, `startSession {appId:'tidal', appName:'tidal', sessionCredential:<user
id>}`, `loadCloudQueue` with `connectqueue.tidal.com/v1/queues` and
`api.tidal.com/v1`, `switch_client` / `r_usr`. Differences: it turns TLS verification off
(`rejectUnauthorized: false`; OxiTide should not), and it hands the device the **refresh
token** too, sending no `updateServerInfo`. It logs in with a **user-supplied developer-portal
client id** (PKCE or device flow), so `switch_client` apparently accepts tokens from non-official
client ids. That is a strong hint for §5.1, not proof: the README names no tested devices.

## 5. Open questions

1. **Does a receiver accept our token?** The device runs `switch_client` against
   `auth.tidal.com` with a token issued to OxiTide's client id (`pkce.rs`). That is TIDAL server
   policy and the main risk. Test bench: run `tidal_connect_application.exe` (from a TIDAL
   install; it holds a TIDAL-issued receiver identity) with `-l 6`, a stand-in host on `:9000`
   as in the receiver spike, and a throwaway controller script sending §3. TIDAL desktop's own
   copy must not be running (port 9000). Final answer: a real device (the issue author has a
   WiiM Pro).
2. **Refresh token or not:** TIDAL desktop sends none, so OxiTide would stay running and push
   `updateServerInfo` after each refresh while casting; tidal_streamer sends one instead.
   Which the WiiM needs, and whether handing it a refresh token is acceptable, is a design call. What a device does when the token expires and the
   controller is gone (stop, or keep playing the current track) is untested.
3. **Does the handover disturb OxiTide's session?** Check that OxiTide's refresh still works
   after a device switches the token.
4. Error and reconnect behaviour (`notify*ServerError`, device sleep, IP change).

## 6. Consequences for a design

- A second playback backend: while casting, OxiTide's engine is bypassed (no DSP/EQ,
  crossfade, cache, bit-perfect path, CatJam beats), the queue lives in TIDAL's cloud queue,
  and Local tracks (V2) cannot be cast.
- Tokens go to a LAN device: verify the TIDAL CA chain, never the plain `ws://` path.
- Network renderers are out of scope for V2.0 (`2026-10-17-oxitide-v2-modular-sources-design.md`);
  this would be its own spike → spec → plan on `main`.
- Alternative for the same hardware: UPnP/DLNA rendering from OxiTide keeps DSP and Local tracks
  but needs OxiTide to serve the audio; Connect lets the device stream on its own.
