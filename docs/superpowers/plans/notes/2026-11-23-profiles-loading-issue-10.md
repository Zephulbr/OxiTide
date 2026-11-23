# Profiles stuck on "Loading…" (issue #10)

**Status:** Open. Waiting on the reporter; 1.1.9 adds walk logging to settle it.

Issue: https://github.com/Zephulbr/OxiTide/issues/10 (reporter on 1.1.8, Windows).

## What the report shows

- Screenshot: Profiles' header and "Loading…" under it, nothing else. The
  `favorites:artists` load never settled; the `loading` branch also hides the
  own card (`ui/src/views/Profiles.svelte`).
- The own profile read fine: `read the account's profile … has_artist=true`
  at 16:04:06. Auth and TIDAL were up then.
- The reporter follows artists, not users, so "followed users are missing
  from v1 favorites" (profiles research note, open question) is not this.
- No WARN/ERROR after that line in the pasted log or the diagnostics (which
  quote up to 20): no retried GET, no 401/expiry, no panic (`panic_log` logs
  at ERROR). But the log ends at 16:04:06, so the silent stretch may be
  seconds, not minutes.
- Startup: `api.tidal.com/v1/sessions` failed with rustls
  `InvalidContentType` (a non-TLS reply: HTTPS-scanning antivirus, proxy,
  captive portal, or a network not up yet), so the app started offline; the
  GitHub update check failed too.

## Candidates

1. **The v1.1.6 paging rule** (`ef906b6f`, `7128b156`): `get_all_pages` reads
   every window up to TIDAL's `totalNumberOfItems`, empty ones included, and
   no longer stops at an empty first window. Silent for minutes only if the
   reported total is wildly overstated (tens of thousands of rows at 100 a
   window, 4 at a time). Prediction: Profiles worked on 1.1.5.
2. **A stalled request on that network.** Would log a retry WARN within
   ~30 s and become an error with Retry after ~91 s.

## Shipped in 1.1.9

- `get_all_pages` logs one line per walk when it ends (items, windows, empty
  windows, TIDAL's total, ms), and a walk of 10+ more windows logs the total
  before it starts. Path ids are masked (`users/*/favorites/artists`).

## Asked of the reporter

Whether it turns into an error after ~3 minutes; whether Albums and Tracks
load; roughly how many artists they follow and whether Profiles worked on
1.1.5 or older; VPN/proxy/antivirus HTTPS scanning; the full log from opening
Profiles to 3 minutes later.

## Not done (owner's call)

- `viewCache`'s in-flight dedupe keeps every revisit on a fetch that never
  settles until restart.
- A total in the billions would also allocate the offsets list up front
  (~170 MB at `u32::MAX`); no cap on windows exists.
