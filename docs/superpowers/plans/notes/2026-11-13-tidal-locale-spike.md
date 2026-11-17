# TIDAL page titles: does `locale` translate them? (v1.1 spike)

- **Date:** 2026-11-13
- **Status:** Open (owner). It needs one live request with a signed-in account; tests never
  hit the live API (AGENTS.md), so it cannot be answered offline. Until it is, OxiTide's page
  requests are unchanged and Home/Explore/For You titles stay as TIDAL sends them.
- **Spec:** `2026-11-12-oxitide-v1.1-i18n-a11y-design.md` §10.4. Plan: Phase 3.

## Question

`oxitide-tidal` fetches pages (`pages/home`, `pages/explore`, `pages/for_you`,
`pages/my_collection_my_mixes`, `pages/artist`, `pages/mix`) with `countryCode`,
`deviceType=BROWSER` and `platform=WEB` (`crates/oxitide-tidal/src/pages.rs`), and no
`locale`. TIDAL's own web player sends `locale=en_US` on the same requests. Does a different
`locale` return translated module titles ("Für dich" instead of "For You")?

## Check (about two minutes, in a browser)

1. Sign in at <https://listen.tidal.com> and open DevTools → Network; reload Home.
2. Find the `pages/home?…` request and note whether it carries `locale=`.
3. Right-click it → Copy → Copy as fetch (Node.js or browser). In the Console, run it twice,
   once with `locale=de_DE` and once with `locale=en_US`, and compare `rows[*].modules[*].title`:
   `(await (await fetch(url, opts)).json()).rows.flatMap(r => r.modules.map(m => m.title))`.
4. Optional: repeat with `locale=ja_JP`, and once with an unknown value (`locale=xx_XX`) to
   see whether TIDAL rejects it (an error status) or ignores it.

## Record here

- [ ] Web player sends `locale`: yes / no (value: …)
- [ ] `de_DE` titles differ from `en_US`: yes / no (examples: …)
- [ ] Unknown `locale`: ignored / rejected (status …)

## If the titles are translated

`oxitide-tidal` adds `locale` (`de_DE`, `ja_JP`, `zh_CN`/`zh_TW`, `pt_BR`, `nb_NO`…, mapped
from the UI language) on page requests only; the app passes the resolved language from
`set_shell_language` (which already receives it on every switch) and the UI already clears its
page cache on a switch (`language.set`). A wiremock test pins the parameter. If they are not,
this note closes with "titles stay as TIDAL sends them"; section treatment never keys on titles
(home layout D-4), so layout is unaffected either way.
