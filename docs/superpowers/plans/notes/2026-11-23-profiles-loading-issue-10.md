# Profiles stuck on "Loading…" (issue #10)

**Status:** Open. 1.1.9 bounds the walk (candidate 1) and logs every walk.
The reporter's 1.1.9 answer points at the UI (candidate 3 below); the fix
ships in 1.1.10 (`2026-11-24-v1.1.10-release.md`), waiting to be confirmed.

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
- Candidate 1: a walk with a known total stops after `MAX_EMPTY_RUN` (10)
  empty windows in a row, 1,000 rows at 100 a window, ten times issue #4's
  runs, with a WARN ("stopped reading … empty windows in a row"). Offsets are
  made a group at a time, so a huge total allocates nothing up front.
- Candidate 2 needs no code: a stalled GET already ends in an error with
  Retry after ~91 s, and its retries log WARNs.
- The UI pager and the phone pager keep the read-to-total rule: they read
  ahead eagerly only up to a 10,000 total and page on scroll above it.

- Profiles shows the own card while the favorites load or after they fail
  (it used to wait behind "Loading…" with them).
- `viewCache` shares a running fetch for `JOIN_MS` (60 s) only; past it a
  visit starts its own, so a fetch that never settles no longer holds every
  revisit until a restart.

- The startup `InvalidContentType` (owner's call, held the tag):
  - Every client trusts the OS certificate store as well as the bundled roots, so
    HTTPS-scanning antivirus and corporate proxies that install their root in Windows work
    (they failed with `UnknownIssuer` before). This may not be the reporter's case:
    `InvalidContentType` is a reply that is not TLS at all.
  - A TLS failure is `Error::Tls`: not retried per request, shown as "Couldn't connect
    securely to TIDAL — something on your network may be interfering…", and the startup
    offline line names the likely interference. Still transient for the offline paths.

## Reading the reporter's 1.1.9 log

- "stopped reading users/*/favorites/artists …": candidate 1, now fixed;
  the list shows.
- "GET … failed (…); retrying": candidate 2, their network.
- "tls error: …" (UI: "Couldn't connect securely to TIDAL") or "the secure connection
  failed … intercepting it" at startup: something in their path answers TLS with non-TLS
  (captive portal, filter, proxy). Ask what security software, VPN or proxy they run.
- A "read users/*/favorites/artists" line while the page still spins: the
  Rust side finished; look at the IPC/UI side.
- No line at all for it: the call never reached or never left
  `get_all_pages` (e.g. stuck before the first window).

## The reporter's 1.1.9 answer (2026-11-23)

- Profiles: the own card ("Your profile", no picture), then "Loading…" that
  never ends. The log at the same time: `read users/*/favorites/artists: 224
  items from 3 windows (0 empty), TIDAL reported 224, in 215 ms`. The Rust
  side finished in a fraction of a second; the page never drew the list.
- Opening Profiles again from another page "does nothing" until a restart.
- The own card opens the own artist page, whose Top tracks shows
  `error.secureConnection`: the TLS interference goes on under 1.1.9 (the OS
  store could not help; `InvalidContentType` is a non-TLS reply). The
  reporter has not said what antivirus, VPN or proxy they run.
- The 16:xx retry WARNs in their diagnostics predate the update (their 1.1.8
  diagnostics were posted at 16:09).

### Candidate 3: a repeated artist breaks the grid

Profiles' grid is a keyed `{#each … (artist.id)}`. A repeated id makes
Svelte throw `each_key_duplicate`, in production builds too, and nothing
caught it: the page stays on its last frame ("Loading…") and later
navigation can break until a restart. Nothing reaches the log (webview
exceptions were never forwarded), which fits the clean log.

How a favorites list can repeat a row: `get_all_pages` reads offset windows
over `order=DATE`, concurrently. Favorites added in one batch (an import
from another service) share a date, and an order with ties is not stable
between requests, so one row can come back in two windows (and another in
none). The 224 = 224 count does not rule it out.

Not confirmed on the reporter's machine: reproduced in a component test
(the same throw, the page stuck), and the fix logs both halves, so the next
report shows it.

Fix (1.1.10):
- `all_favorite_artists`, `…_albums` and `…_tracks` keep the first row of
  each id and log a WARN when they drop any (`favorite artists: dropped N
  repeated rows`). Not in `get_all_pages`: playlists legitimately repeat
  tracks.
- Profiles draws each artist once whatever it is given.
- `RoutedPage` wraps every page in a `svelte:boundary`: a page that throws
  while drawing shows the error with Retry in its place, and the new
  `log_page_error` command writes `page <route> failed to draw: …` at ERROR,
  so Copy diagnostics' "Recent problems" carries it.

### 1.1.9's TLS fail-fast: a regression (1.1.11 candidate)

The reporter's 1.1.8 log has `InvalidContentType` WARNs ending "retrying in
250ms (1/3)" with no later failure for those calls: the retry went through,
so their interference is intermittent. 1.1.9 stopped retrying `Error::Tls`,
which turns each such glitch into an error (their own artist page's Top
tracks: "Couldn't connect securely to TIDAL"). Fix on `release/1.x` after
`v1.1.10`, not released: a GET gets one quick retry on a TLS failure
(`TLS_GET_ATTEMPTS` = 2, 250 ms); writes unchanged; a network that always
tampers still fails within ~250 ms with the clear message.

Still open (owner-approved angles, 2026-11-24): test the duplicate theory
against the live API by hand (follow a batch at once, read
`favorites/artists?order=DATE&orderDirection=DESC` in windows of 100, look
for repeats); reproduce "Profiles does nothing until restart" in a dev build
with and without the boundary; the own card's doubled "Your profile" when
neither the profile nor the artist has a name (cosmetic).

## Asked of the reporter

Whether it turns into an error after ~3 minutes; whether Albums and Tracks
load; roughly how many artists they follow and whether Profiles worked on
1.1.5 or older; VPN/proxy/antivirus HTTPS scanning; the full log from opening
Profiles to 3 minutes later.
