# Profiles loading and network smoke (issue #10, v1.1.9)

**Status:** Pending (owner, on the v1.1.9 draft's installer, before publishing). The
"Repeated favorites and the page boundary" section: on the v1.1.10 draft's installer.

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

## Repeated favorites and the page boundary (v1.1.10)

`460575ff`: favorites drop repeated rows, and every page sits in a `svelte:boundary`.

- [ ] Profiles, Albums, Tracks: same counts as before; today's log has no `dropped N repeated
      rows` line on an account without repeats.
- [ ] Every page still enters with its animation and restores its scroll on Back, in the
      default shell and in Hi-Fi Rack (the boundary wraps every routed page); Now Playing still
      covers and uncovers the page as it was.
- [ ] A page that throws (dev build: throw from a page's template) shows the error with Retry in
      its place; the sidebar still navigates; the log has `page <route> failed to draw: …`
      with the route name only, no id; Copy diagnostics lists it under "Recent problems".

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
