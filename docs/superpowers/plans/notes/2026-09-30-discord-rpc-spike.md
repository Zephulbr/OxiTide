# Discord RPC spike findings (2026-09-30)

- Application: `OxiTide`, ID `1554830262083264532`; Rich Presence art asset key `logo`
  (620×620 PNG) uploaded and resolves (`small_image` appears in the `SET_ACTIVITY` echo).
- External art: Discord proxies the TIDAL CDN URL itself (`mp:external/…` in the echo); no
  uploaded asset is needed for album art.
- Buttons: accepted (`"buttons":["Listen on TIDAL"]` with `metadata.button_urls`).
- Activity type: `type: 2` (Listening); timestamps are unix epoch milliseconds and the bar
  renders from them.
- Echo handling: every `SET_ACTIVITY` is echoed back (opcode 1); the worker's `drain_one` is
  required and works. The handshake response is consumed inside the crate's `connect()`.
- Burst: ten updates 300 ms apart were all accepted (echoed), no ERROR responses.
- The throwaway probe (`crates/oxitide-app/examples/discord_probe.rs`) has been deleted.
