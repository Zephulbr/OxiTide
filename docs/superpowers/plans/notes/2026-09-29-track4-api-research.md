# Track 4 — API research spike (lyrics + discovery)

- **Date:** 2026-09-29
- **Status:** Spike complete; feeds the Track 4 spec brainstorm
- **Parent:** `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 4, §5

Findings ground the two open roadmap decisions: the lyrics source + cache strategy,
and the discovery endpoints to adopt + their auth scopes.

## Method and confidence

- **Live-probed:** `lrclib.net` (`GET /api/get` 200 with synced LRC; 404 on a miss;
  no rate-limit headers; TLS/HSTS via Cloudflare).
- **Verified against source, not live-requested:** TIDAL legacy endpoints via the
  maintained community client `tidalapi` (EbbLabs/python-tidal) and the published
  official OpenAPI (`openapi.tidal.com/v2`, mirrored in api-evangelist/tidal).
  The owner's keyring token was deliberately not read; authenticated endpoint
  shapes need one real capture during implementation.
- **Not tested:** live availability/region behavior of any TIDAL endpoint.

## Lyrics

| Source | Access | Payload | Verdict |
|---|---|---|---|
| Official `openapi.tidal.com/v2` — `GET /lyrics`, `GET /lyrics/{id}`, `tracks … include=lyrics` | `Authorization_Code_PKCE` / Client Credentials, but every lyrics path is `required-access-tier: INTERNAL` | `lrcText` (synced), `text`, `provider`, `technicalStatus`, `direction` | **Out** — first-party only |
| Legacy `api.tidal.com/v1/tracks/{id}/lyrics` | Existing user token, no new scope; 404 = no lyrics | `lyrics` (plain), `subtitles` (synced LRC), `lyricsProvider`, `providerCommontrackId`, `providerLyricsId`, `isRightToLeft` | **Primary** |
| `lrclib.net/api/get` | Public, no auth or key | `syncedLyrics` (LRC), `plainLyrics`, `instrumental`, duration match ±2 s | **Fallback** |

- **Recommendation: TIDAL-first, lrclib fallback on 404/error**, cache per track id
  (positive and negative results).
- lrclib client rules (from its docs): honor `429` + `Retry-After` (their wording:
  "must honor… ignoring it may result in a temporary ban"), send requests
  sequentially with a 200–500 ms gap for batches, and identify with a User-Agent
  like `LRCGET v0.2.0 (https://github.com/tranxuanthang/lrcget)`. The TIDAL
  client already has `Retry-After` handling on its mutate path; the GET path and a
  separate base URL are new work.
- The roadmap's "TIDAL-first vs lrclib-first vs both" question is settled by the
  INTERNAL tier: official JSON:API lyrics are not available to third-party apps.

## Discovery

- Official `userDiscoveryMixes` / `userDailyMixes` / `userNewReleaseMixes`
  (`openapi.tidal.com/v2`) are `THIRD_PARTY` tier but require `Authorization_Code_PKCE`
  scopes `r_usr` + **`recommendations.read`** against a registered official client.
  OxiTide's PKCE is the Android first-party client id with `r_usr w_usr w_sub` on
  `api.tidal.com`. **Out** unless the whole auth story changes.
- Legacy path, all reachable with the existing token and **no new scopes**:
  - `pages/for_you` — the daily/discovery mix feed. Today's `feed_page` probes the
    unconfirmed `pages/feed` / `pages/my_collection_feed` and `Feed.svelte` lands on
    "Feed isn't available for this account."
  - `pages/mix?mixId={id}&deviceType=BROWSER` — mix detail + items (already
    implemented as `mix_page`). `mixType` values include `DAILY_MIX`,
    `DISCOVERY_MIX`, `NEW_RELEASE_MIX`, `HISTORY_ALLTIME_MIX`,
    `HISTORY_MONTHLY_MIX`, `HISTORY_YEARLY_MIX`, `TRACK_MIX`, `ARTIST_MIX`.
  - `tracks/{id}/radio?limit=N` and `tracks/{id}/mix` — track radio (mix id then
    `pages/mix`).
  - `artists/{id}/radio?limit=N` and `artists/{id}/mix` — artist radio.
  - `artists/{id}/similar` — similar artists. No UI exists yet: `Artist.svelte` has
    neither similar artists nor radio, and no track context-menu radio entry.
- **Recommendation: build discovery on the legacy page/mix/radio/similar endpoints**;
  repoint the feed to `pages/for_you`.

## Auth scopes

None of the recommended endpoints need additional scopes — the existing
`r_usr w_usr w_sub` token covers lyrics, pages/mix, radio, and similar artists.

## Open verifications (implementation)

1. Legacy lyrics response shape/content under OxiTide's token — capture one real
   fixture first; this is the linchpin of the lyrics half.
2. `pages/for_you` contents for this account/region (mix ids + `mixType`s).
3. Radio and similar-artists availability and region gaps.
4. lrclib match quality against TIDAL metadata (featuring suffixes, duration
   rounding); `/api/search` fallback may be needed.

## Throwaway

`tidalapi` sources and TIDAL OpenAPI YAMLs were downloaded to
`%TEMP%\opencode\tidalapi` for inspection; nothing was added to the repo, and no
product code was written.
