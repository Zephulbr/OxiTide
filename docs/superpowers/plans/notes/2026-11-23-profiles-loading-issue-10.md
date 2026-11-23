# Profiles stuck on "Loading…" (issue #10)

**Status:** Open. 1.1.9 bounds the walk (candidate 1) and logs every walk;
waiting on the reporter to confirm on 1.1.9.

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

## Asked of the reporter

Whether it turns into an error after ~3 minutes; whether Albums and Tracks
load; roughly how many artists they follow and whether Profiles worked on
1.1.5 or older; VPN/proxy/antivirus HTTPS scanning; the full log from opening
Profiles to 3 minutes later.
