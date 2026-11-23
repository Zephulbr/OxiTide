# Profiles loading smoke (issue #10, v1.1.9)

**Status:** Pending (owner, on the v1.1.9 draft's installer, before publishing).

Investigation: `2026-11-23-profiles-loading-issue-10.md`. Live account, Windows.

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
