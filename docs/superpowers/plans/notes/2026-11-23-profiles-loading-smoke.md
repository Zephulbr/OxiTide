# Profiles loading and network smoke (issue #10, v1.1.9)

**Status:** Pending (owner, on the v1.1.9 draft's installer, before publishing).

Investigation: `2026-11-23-profiles-loading-issue-10.md`. Live account, Windows.

## Profiles

- [ ] Profiles loads: the own card first, then the followed artists; the count under the title
      matches.
- [ ] Throttle or cut the network (`scripts/smoke/`), open Profiles fresh: the own card (if
      cached or quick) shows with "Loading…" or the error under it, not a bare "Loading…".
      Retry after the network returns fills the grid.
- [ ] Today's log has a `read users/*/favorites/artists: N items from W windows (E empty),
      TIDAL reported T, in … ms` line, with no account id in it.
- [ ] A large playlist (1,000+ tracks) and Tracks still load to their last row; a playlist with
      unavailable tracks still shows its last tracks (issue #4 unchanged).
- [ ] Albums, an artist page and Home load as before (shared page cache).

## Network (TLS roots and TLS errors)

Every connection now trusts the OS certificate store as well as the bundled roots.

- [ ] Fresh start signed in: Home loads; a track plays (stream URLs, CDN segments); lyrics and
      cover colours load (the side-services client).
- [ ] Sign out and sign in again (browser login, paste-back).
- [ ] The in-app update from 1.1.8 to this draft (the updater's own client).
- [ ] If an HTTPS-scanning antivirus or an intercepting proxy is at hand (its root installed in
      Windows): with it on, the app reaches TIDAL. 1.1.8 failed there.
- [ ] TLS failure message: point the app at a non-TLS reply (e.g. a hosts-file entry for
      `api.tidal.com` to a local plain HTTP server, or a captive-portal network), open
      Profiles: the error reads "Couldn't connect securely to TIDAL — …" at once, with Retry;
      the log has one failure line, not three retries. Restarting there starts offline with
      the "secure connection failed … intercepting it" line, and the session survives (no
      sign-in screen).
