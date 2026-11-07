# TIDAL profiles: research (2026-11-06)

Question: the Profiles page (favorite artists, `users/{id}/favorites/artists`)
never shows the account's own profile, and OxiTide has no notion of a TIDAL
user profile. Does TIDAL keep user and artist profiles apart?

Nothing here was called live from OxiTide. Tags: [3P-L] another client's notes
from live calls, [3P] another client's code or docs, [?] open.

## Findings

1. **A profile is an artist.** Every artist record carries `ownerType`
   (`LABEL`, `MIXED`, `USER`) and a nullable `userId`. A TIDAL user's
   profile is an artist with `ownerType: "USER"`; a label artist claimed by
   its owner reads `MIXED` with the owner's `userId` (Prince, 4847). The
   v2 collection payload carries both fields [3P: tidlers fixtures].
2. **The account's profile.** `GET https://api.tidal.com/v2/profiles/{userId}`
   -> `{userId, name, handle, picture, color, profileType, artistId,
   numberOfFollowers, numberOfFollows, prompts, socialLinks}`;
   `profileType` is `"MY_PROFILE"` for one's own [3P-L: tidalfast,
   2026-10-15].
   - Live on the owner's account (2026-11-06): the same eleven keys, and an
     `artistId` is present.
   - `picture` is an **object**, not a uuid string. A typed read failed the
     whole call ("expected a string"), so `profiles.rs` now reads field by
     field and accepts any picture shape. OpenAPI `artists?filter[owners.id]=me` returns the same
   artist (`ownerType: "USER"`) [3P-L].
3. **TIDAL has no separate user page.** The web player's `/user/{userId}`
   redirects to `/artist/{artistId}` when the profile has an `artistId`, and
   to `/` otherwise. The own page is the ordinary artist page with Edit in
   place of Follow, "Add bio", and the empty state "Your profile is empty"
   [3P-L, read from the web bundle].
4. **Reading the own artist.** v2 `artist/{id}` modules come back empty for
   the own artist; v1 answers in full: `artists/{id}`, `/albums`,
   `/albums?filter=EPSANDSINGLES`, `/toptracks` (uploads, including private
   and unlisted ones) [3P-L]. OxiTide's Artist page reads v1, so it should
   render.
5. **Following.**
   - Following a profile favorites its artist id, so followed users should
     already appear in Profiles as artists [? confirm with an account that
     follows a user].
   - `v2/profiles/{userId}/followers` and `/following` (cursor, `limit` up
     to 50) list follow relationships [3P: minim].
   - `v2/follow?trn=trn:user:{id}` (PUT/DELETE) is marked deprecated
     [3P: minim].
6. **Public playlists.** `GET v2/user-playlists/{userId}/public?limit&cursor`
   -> `items[{playlist, followInfo, profile}]` [3P: minim, tidalfast].
7. **The web player's collection list.**
   `v2/my-collection/artists/folders?folderId=root&order=DATE` returns
   `{items[{trn, itemType, addedAt, data: <artist>}], cursor,
   totalNumberOfItems}`. It holds the same artists as v1 favorites, plus
   folders and `addedAt` [3P: tidlers].

## Open questions (one read-only live check on the owner's account)

- Does the account's `v2/profiles/{userId}` carry an `artistId`? A
  listener-only account may have none. tidalfast never saw an account
  without an artist.
- Does TIDAL's own Profiles collection pin the own profile, or only list it
  when you follow yourself? (Screenshot from the official app.)
- Do followed users appear in `users/{id}/favorites/artists`, or only in the
  v2 folders list?
- `v2/profiles/{userId}` for another user: is a private profile a 403 or a
  404?

## Built (2026-11-06)

- Rust: `TidalClient::my_profile()` (`oxitide-tidal/src/profiles.rs`) ->
  `Option<UserProfile>`. 403 and 404 mean `None`, and a name equal to the
  user id counts as no name. The async command `get_my_profile` is listed
  in the remote's `NOT_EXPOSED`, since the phone has no own-profile card.
- UI: `myProfileSource` (`profile:me`) loads the profile, then its artist.
  Pure helpers live in `lib/ownProfile.ts`.
- Profiles: the own profile is the first card, captioned "Your profile",
  and opens `{name: "artist", id: artistId}`. The same artist is dropped
  from the favorites if it is one. With no `artistId` the card is static,
  since TIDAL has no page then.
- Artist page, own artist:
  - The kicker reads "Your profile".
  - "Edit profile" replaces the Favorite heart and opens
    `https://tidal.com/artist/{id}?edit-profile=true` in the browser. No
    profile writes are sent.
  - With no tracks or releases, the page says "Your profile is empty".
- Second round (2026-11-06, owner feedback):
  - **Public playlists:** `TidalClient::user_public_playlists` reads
    `v2/user-playlists/{userId}/public` (cursor, 50 a page, at most 10
    pages). The owner comes from v1 `artists/{id}` `userId`, now
    `Artist.user_id`; the own page falls back to the profile's `userId`.
  - **Artist radio 404:** "not found: /v1/artists/{id}/radio" now reads
    "There's no radio for this artist yet…", shown as info, not an error.
  - **In-app profile editor:** these calls follow `sone`'s working
    implementation and have not been checked live by OxiTide yet.
    - `GET openapi artists/{id}?include=biography,profileArt` reads the
      form.
    - `PATCH openapi artists/{id}` writes name, handle and
      `externalLinks[{href, meta.type}]`. It runs as a `meta.dryRun` first,
      so a rejected edit (for example a taken handle) writes nothing, and
      TIDAL's `errors[].detail` is shown.
    - `PATCH openapi artistBiographies/{id}` writes the bio. A profile
      without a biography can't get one here, because the create call is
      unknown; the editor links to TIDAL's editor instead.
    - The picture is uploaded with `POST openapi artworks` (MD5 hex and
      size), then a PUT of the JPEG to the pre-signed link (`Content-MD5`,
      no bearer token), then polling `GET artworks/{id}` until
      `technicalFileStatus` is OK, then
      `PATCH artists/{id}/relationships/profileArt`. Removing the picture
      PATCHes the relationship to `[]`.
    - The app crate makes the square JPEG (now `profile_art.rs`: centre
      crop, at most 1280 px, quality 90).
    - Edits always target the account's own artist, resolved on the Rust
      side from `v2/profiles/{userId}`.
  - Link types for new links are taken from the host: INSTAGRAM, TWITTER,
    FACEBOOK or TIKTOK (later SNAPCHAT; YOUTUBE was dropped, see the
    fourth round), else OFFICIAL_HOMEPAGE. Links that already
    exist keep the type TIDAL gave them.
- Third round (owner, live, 2026-11-06):
  - Adding and removing links works, and the official app shows the
    change. Uploaded pictures show in both directions (OxiTide to TIDAL's
    apps, and TIDAL's apps to OxiTide).
  - `https://tidal.com/browse/artist/{id}` comes back after any links
    edit, because TIDAL adds it itself. `profiles.rs` no longer lists or
    sends it (`is_tidal_own_link`: a `TIDAL*` type or a tidal.com host),
    and the hero's Links list hides it.
  - A new picture took a while to show in OxiTide, because v1
    `artists/{id}` keeps the old `picture`. The processed
    `GET artworks/{id}` lists `files[] {href, meta{width,height}}`. The
    upload now answers the square file nearest 750 px (waiting up to 3
    more polls for the list), and the UI shows it until v1 changes. The
    palette analysis accepts such a URL, but only on
    `resources.tidal.com/images/`.
  - Handle becomes "Username", with TIDAL's rule (a-z, 0-9, `_`) and its
    "Save username?" confirmation.
- Fourth round (2026-11-07, from TIDAL's published OpenAPI spec,
  `tidal-api-oas.json`; not yet checked live):
  - **Social links are not separate.** TIDAL's "Social media" screen
    edits the same `externalLinks`, one fixed field per type. The spec's
    types (`External_Link_Meta`) are TIDAL_SHARING, TIDAL_USER_SHARING,
    the TIDAL_AUTOPLAY_* set, TWITTER, FACEBOOK, INSTAGRAM, TIKTOK,
    SNAPCHAT, OFFICIAL_HOMEPAGE, CASHAPP_CONTRIBUTIONS and three
    *_REDIRECT types. There is **no YOUTUBE**: the editor used to send it,
    and now types a YouTube link as OFFICIAL_HOMEPAGE. Snapchat links now
    get SNAPCHAT.
  - The editor's quick-adds put in fixed fields for the five social
    types (`SOCIALS` in `lib/profileEdit.ts`). A bare username becomes
    the site's profile link, and a link must be on the site's host.
  - **Banner and logo:** artists have `bannerArt` ("the selected header
    banner") and `logoArt` relationships beside `profileArt`. All three
    take one owned IMAGE artwork through the same `artworks` flow, and
    selecting a new one soft-deletes the one it replaces.
    - `ProfileArt` (picture, banner, logo) picks the relationship.
    - The commands are `upload_my_profile_art` and `remove_my_profile_art`;
      they replace the picture-only pair.
    - `profile_art.rs` shapes the file: the picture as before; the banner
      at its own shape, ≤2560 px wide, JPEG; the logo ≤1280 px, PNG with
      alpha. A prepared file must fit TIDAL's 8 MB.
  - The spec gives no banner shape, so banners are not cropped.
    `editable_profile` logs the sizes TIDAL made from the current banner
    and logo ("read the profile's artwork sizes"; sizes only, no URLs).
    One look at that line with the owner's banner tells us whether to
    crop.
  - **Answered (owner's banner, live 2026-11-07):** TIDAL made 1280x1280,
    640x640, 320x320, 1280x720, 640x360 and 320x180 from it. TIDAL cuts a
    banner to both 1:1 and 16:9 itself, so OxiTide keeps uploading it
    uncropped: cutting to 16:9 first would leave TIDAL's square cut a
    smaller crop of a crop. The editor now previews the 16:9 file
    (`nearest_file` prefers a banner's wide files) in a 16:9 frame.
  - `TidalClient::artist_art` reads any artist's picture, banner and
    logo (`include=profileArt,bannerArt,logoArt`). Nothing calls it yet:
    showing them would cost an extra request per artist page.
- Not yet:
  - A Followers/Following count on the header, from the profile counts.
  - A list of followed users, if they turn out not to be in the
    favorites.
- Smoke (owner, live account):
  - Profiles shows the own card first.
  - It opens your artist page.
  - Edit profile opens the browser editor.

## Sources

- tidalfast `docs/_reference/tidal-api.md`:
  "Profiles", "The account's artist", "The account's own profile".
- minim (dev) `src/minim/api/tidal/_private_api/users.py`:
  `get_user_followers`, `get_user_followings`, `get_user_public_playlists`.
- tidlers `src/client/models/collection/artist.rs`: the v2 collection
  fixtures with `ownerType` and `userId`.
- sone `src-tauri/src/tidal_api.rs`: `v2/profiles/{id}` for follower counts.
