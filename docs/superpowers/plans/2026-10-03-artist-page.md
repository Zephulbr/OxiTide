# Artist Page Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the artist page as a full surface: palette-derived hero with fan count, bio and social links, Play/Shuffle over Top tracks, three discography carousels, and Similar artists — every section degrading independently.

**Architecture:** Three new `TidalClient` methods (`artist_top_tracks`, `artist_albums`, `artist_profile`) plus a pure `clean_bio`; one OpenAPI call supplies fans/socials/bio (spike-confirmed). Four Tauri commands (three reads + a validated `open_external`) are exposed through `api.ts`. The UI rebuilds `Artist.svelte` around a new `ArtistHero.svelte`, reusing `TrackTable`, `Carousel`, `MediaCard`, `DetailAction`, and the existing menu/radio/favorites plumbing.

**Tech Stack:** Rust (Tauri 2, tokio, serde, wiremock), Svelte 5 runes, TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-03-artist-page-design.md`

## Global Constraints

- Windows-first; **no new dependencies** in any crate or the UI (no regex, no URL crates — `reqwest::Url` is already available in `oxitide-app`).
- All Rust HTTP tests use `wiremock`; never hit the live TIDAL API.
- Components read semantic tokens only (`--surface-*`, `--text`, `--line`, `--accent`, `--flag-*`, glass knobs); never base or `--dyn-*` values; never branch on `data-mode`.
- Icons are inlined Lucide paths in `ui/src/lib/components/Icon.svelte`; no unicode glyphs.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case; every new `#[tauri::command]` must be registered in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- Fans, bio, and socials are **optional**; nothing may hard-require the OpenAPI call.
- No CSP or capability changes (the opener call is Rust-side).
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green test run instead.

## Review Focus

- **Sparse artist.** An artist with no bio, no socials, no similar artists, and one album must render a clean hero and only the shelves that exist — never an empty `<h3>` or a broken toggle. This repo has no component-test harness; pinned by the conditional rendering in `ArtistHero.svelte`/`Artist.svelte` plus `clean_bio` tests, and checked in the smoke checklist.
- **Profile fetch failure.** When OpenAPI `artists/{id}` 403s/500s/404s, the page must still render the v1 artist, top tracks, discography and similar. Pinned by the `artist_profile` error test and the UI's `.catch(() => null)`; smoke-covered with the network blocked.
- **Rapid artist → artist navigation.** Section promises from the previous artist must not land on the new page. Pinned by the `loadToken` generation guard in `Artist.svelte` (no component harness); smoke-covered by back-to-back navigation.
- **Bio markup variants.** `[wimpLink]`, `<br>`/`<br/>`/`<br />`, leftover tags, and markup-only text. Pinned by `clean_bio` unit tests (including the missing-fields OpenAPI test).
- **Hostile/odd links and overflow.** `javascript:`/`file:` links must never render or open (`isHttpUrl` + `external_url` tests), and shelves cap at 50 items silently. Smoke-covered.

---

## File Structure

**Rust**

- Modify `crates/oxitide-tidal/src/models.rs` — `ArtistLink`, `ArtistProfile`, `ArtistDiscography` + serde test.
- Modify `crates/oxitide-tidal/src/catalog.rs` — `artist_profile`, `artist_top_tracks`, `artist_albums`, `clean_bio` + helpers + tests.
- Modify `crates/oxitide-app/src/commands.rs` — `external_url` validator, `open_external`, `get_artist_profile`, `get_artist_top_tracks`, `get_artist_albums` + tests.
- Modify `crates/oxitide-app/src/main.rs` — register the four commands.

**UI logic**

- Modify `ui/src/lib/api.ts` — `ArtistLink`, `ArtistProfile`, `ArtistDiscography`, four wrappers.
- Modify `ui/src/lib/share.ts` + `ui/src/lib/share.test.ts` — `"artist"` kind.
- Modify `ui/src/lib/format.ts` + `ui/src/lib/format.test.ts` — `formatCount`.
- Create `ui/src/lib/artistLinks.ts` + `ui/src/lib/artistLinks.test.ts` — label mapping + URL guard.

**UI components**

- Modify `ui/src/lib/components/Icon.svelte` — `external-link` path.
- Create `ui/src/lib/components/ArtistHero.svelte` — hero.
- Rewrite `ui/src/views/Artist.svelte` — orchestration + shelves.
- Modify `ui/src/App.svelte:478` — pass `player` and `playlistActions`.
- Create `docs/superpowers/plans/notes/2026-10-03-artist-page-smoke.md` — human checklist.

---

### Task 1: DTOs and bio cleaning (Rust client)

**Files:**
- Modify: `crates/oxitide-tidal/src/models.rs` (structs after `Artist`; tests inside the existing `mod tests`)
- Modify: `crates/oxitide-tidal/src/catalog.rs` (`clean_bio` + helpers after `impl TidalClient`, tests inside the existing `mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `ArtistLink { site: String, url: String }`; `ArtistProfile { number_of_followers: Option<u64>, external_links: Vec<ArtistLink>, bio: Option<String> }` (serde `camelCase` on serialize); `ArtistDiscography::{Albums, EpsAndSingles, Compilations}` with `tidal_param(&self) -> &'static str`; `pub fn clean_bio(raw: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-tidal/src/models.rs`:

```rust
    #[test]
    fn artist_profile_serializes_camel_case() {
        let profile = ArtistProfile {
            number_of_followers: Some(1106540),
            external_links: vec![ArtistLink {
                site: "TWITTER".into(),
                url: "https://twitter.com/acdc".into(),
            }],
            bio: None,
        };
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json["numberOfFollowers"], 1106540);
        assert_eq!(json["externalLinks"][0]["site"], "TWITTER");
    }
```

Append to the `mod tests` block in `crates/oxitide-tidal/src/catalog.rs`:

```rust
    #[test]
    fn clean_bio_keeps_wimp_link_text_and_breaks_paragraphs() {
        let raw = "See [wimpLink artistId=\"1\"]the band[/wimpLink].<br/>Next line.";
        assert_eq!(
            clean_bio(raw).as_deref(),
            Some("See the band.\n\nNext line.")
        );
    }

    #[test]
    fn clean_bio_strips_other_tags_and_empty_input() {
        assert_eq!(clean_bio("<p>Hi</p>"), Some("Hi".into()));
        assert_eq!(clean_bio("   <br/>  "), None);
        assert_eq!(clean_bio(""), None);
    }

    #[test]
    fn clean_bio_handles_album_links_and_variant_breaks() {
        let raw = "A [wimpLink albumId=\"2\"]record[/wimpLink]<br />B<br>B2";
        assert_eq!(clean_bio(raw).as_deref(), Some("A record\n\nB\n\nB2"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal clean_bio`
Expected: compile error (`cannot find function clean_bio`, missing types) — the failing state.

- [ ] **Step 3: Implement**

In `crates/oxitide-tidal/src/models.rs`, add after the `Artist` struct:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistLink {
    /// Uppercased site key from the API, e.g. "TWITTER".
    pub site: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistProfile {
    #[serde(default)]
    pub number_of_followers: Option<u64>,
    #[serde(default)]
    pub external_links: Vec<ArtistLink>,
    /// Cleaned biography text (no wimpLink/HTML), `None` when absent/empty.
    #[serde(default)]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistDiscography {
    Albums,
    EpsAndSingles,
    Compilations,
}

impl ArtistDiscography {
    /// The `filter` query value TIDAL expects on `artists/{id}/albums`.
    pub fn tidal_param(self) -> &'static str {
        match self {
            ArtistDiscography::Albums => "ALBUMS",
            ArtistDiscography::EpsAndSingles => "EPSANDSINGLES",
            ArtistDiscography::Compilations => "COMPILATIONS",
        }
    }
}
```

In `crates/oxitide-tidal/src/catalog.rs`, extend the imports to:

```rust
use crate::client::{TidalClient, DEFAULT_PAGE_SIZE};
use crate::error::Result;
use crate::models::{Album, Artist, ArtistDiscography, ArtistLink, ArtistProfile, ItemPage, Track};
```

and add after the `impl TidalClient` block (before `#[cfg(test)]`):

```rust
/// Strip TIDAL bio markup: `[wimpLink ...]Name[/wimpLink]` keeps `Name`,
/// `<br>`/`<br/>`/`<br />` become paragraph breaks, and any other tag is
/// dropped. Returns `None` when nothing but markup remains.
pub fn clean_bio(raw: &str) -> Option<String> {
    let text = strip_tags(&strip_line_breaks(&strip_wimp_links(raw)));
    let paragraphs: Vec<&str> = text
        .split('\n')
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .collect();
    if paragraphs.is_empty() {
        None
    } else {
        Some(paragraphs.join("\n\n"))
    }
}

/// `[wimpLink artistId="1"]Name[/wimpLink]` -> `Name`; a malformed open tag
/// drops the remainder rather than leaking raw markup.
fn strip_wimp_links(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("[wimpLink") {
        out.push_str(&rest[..start]);
        let Some(open_end) = rest[start..].find(']') else {
            rest = "";
            break;
        };
        let after_open = &rest[start + open_end + 1..];
        let Some(close) = after_open.find("[/wimpLink]") else {
            out.push_str(after_open);
            rest = "";
            break;
        };
        out.push_str(&after_open[..close]);
        rest = &after_open[close + "[/wimpLink]".len()..];
    }
    out.push_str(rest);
    out
}

/// Case-insensitive `<br>`, `<br/>`, `<br />` -> newline. Operates byte-wise
/// on tag matches only, so non-ASCII text passes through unchanged.
fn strip_line_breaks(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        let tail = &lower[index..];
        if tail.starts_with("<br />") {
            out.push('\n');
            index += 6;
        } else if tail.starts_with("<br/>") {
            out.push('\n');
            index += 5;
        } else if tail.starts_with("<br>") {
            out.push('\n');
            index += 4;
        } else {
            let ch = raw[index..].chars().next().expect("char boundary");
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

fn strip_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal clean_bio artist_profile_serializes_camel_case`
Expected: PASS (3 `clean_bio` tests + 1 serde test).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/models.rs crates/oxitide-tidal/src/catalog.rs
git commit -m "feat(tidal): add artist profile DTOs and bio cleaning"
```

---

### Task 2: Artist top tracks and discography (Rust client)

**Files:**
- Modify: `crates/oxitide-tidal/src/catalog.rs` (methods after `artist`; tests inside `mod tests`)

**Interfaces:**
- Consumes: `ArtistDiscography::tidal_param`, the existing `client(&server)` test helper, `ItemPage`.
- Produces: `pub async fn artist_top_tracks(&self, id: u64, limit: u32) -> Result<Vec<Track>>`; `pub async fn artist_albums(&self, id: u64, filter: ArtistDiscography, limit: u32, offset: u32) -> Result<Vec<Album>>`.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-tidal/src/catalog.rs`:

```rust
    #[test]
    fn discography_filter_params_match_tidal() {
        assert_eq!(ArtistDiscography::Albums.tidal_param(), "ALBUMS");
        assert_eq!(ArtistDiscography::EpsAndSingles.tidal_param(), "EPSANDSINGLES");
        assert_eq!(ArtistDiscography::Compilations.tidal_param(), "COMPILATIONS");
    }

    #[tokio::test]
    async fn artist_top_tracks_sends_limit_and_parses_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/toptracks"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 1, "title": "Hit", "artists": []}],
                "totalNumberOfItems": 200, "limit": 10, "offset": 0
            })))
            .mount(&server)
            .await;
        let tracks = client(&server).artist_top_tracks(9, 10).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Hit");
    }

    #[tokio::test]
    async fn artist_albums_sends_the_filter_and_parses_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/artists/9/albums"))
            .and(query_param("filter", "EPSANDSINGLES"))
            .and(query_param("limit", "50"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"id": 7, "title": "Single", "artists": [], "numberOfTracks": 1}],
                "totalNumberOfItems": 1, "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        let albums = client(&server)
            .artist_albums(9, ArtistDiscography::EpsAndSingles, 50, 0)
            .await
            .unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "Single");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal artist_top_tracks_sends_limit_and_parses_items`
Expected: compile error (`no method named artist_top_tracks`) — the failing state.

- [ ] **Step 3: Implement**

In `crates/oxitide-tidal/src/catalog.rs`, add after `pub async fn artist(...)`:

```rust
    /// One bounded window of the artist's most popular tracks.
    pub async fn artist_top_tracks(&self, id: u64, limit: u32) -> Result<Vec<Track>> {
        let page: ItemPage<Track> = self
            .get(
                &format!("artists/{id}/toptracks"),
                &[("limit", limit.to_string())],
            )
            .await?;
        Ok(page.items)
    }

    /// One page of an artist's discography under a TIDAL filter
    /// (`ALBUMS` / `EPSANDSINGLES` / `COMPILATIONS`).
    pub async fn artist_albums(
        &self,
        id: u64,
        filter: ArtistDiscography,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Album>> {
        let page: ItemPage<Album> = self
            .get(
                &format!("artists/{id}/albums"),
                &[
                    ("filter", filter.tidal_param().to_string()),
                    ("limit", limit.to_string()),
                    ("offset", offset.to_string()),
                ],
            )
            .await?;
        Ok(page.items)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal artist_albums_sends_the_filter_and_parses_items artist_top_tracks_sends_limit_and_parses_items discography_filter_params_match_tidal`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/catalog.rs
git commit -m "feat(tidal): add artist top tracks and discography filters"
```

---

### Task 3: Artist profile via OpenAPI (Rust client)

**Files:**
- Modify: `crates/oxitide-tidal/src/catalog.rs` (private serde types near the top; method after `artist_albums`; tests inside `mod tests`)

**Interfaces:**
- Consumes: `clean_bio`, `ArtistProfile`, `ArtistLink`, `get_openapi`, `client(&server)`.
- Produces: `pub async fn artist_profile(&self, id: u64) -> Result<ArtistProfile>` — one `GET openapi/v2/artists/{id}?countryCode={cc}&include=biography` supplying fans, filtered/deduped external links, and the cleaned bio.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-tidal/src/catalog.rs`:

```rust
    #[tokio::test]
    async fn artist_profile_reads_followers_links_and_bio() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .and(query_param("countryCode", "GR"))
            .and(query_param("include", "biography"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "attributes": {
                        "numberOfFollowers": 1106540,
                        "externalLinks": [
                            {"href": "https://twitter.com/acdc", "meta": {"type": "TWITTER"}},
                            {"href": "https://tidal.com/browse/artist/9", "meta": {"type": "TIDAL_SHARING"}},
                            {"href": "https://twitter.com/acdc", "meta": {"type": "TWITTER"}}
                        ]
                    },
                    "id": "9",
                    "type": "artists"
                },
                "included": [{
                    "attributes": {
                        "source": "TiVo",
                        "text": "See [wimpLink artistId=\"1\"]Band[/wimpLink].<br/>More."
                    },
                    "id": "9",
                    "type": "artistBiographies"
                }]
            })))
            .mount(&server)
            .await;
        let profile = client(&server).artist_profile(9).await.unwrap();
        assert_eq!(profile.number_of_followers, Some(1106540));
        assert_eq!(profile.external_links.len(), 1);
        assert_eq!(profile.external_links[0].site, "TWITTER");
        assert_eq!(profile.external_links[0].url, "https://twitter.com/acdc");
        assert_eq!(profile.bio.as_deref(), Some("See Band.\n\nMore."));
    }

    #[tokio::test]
    async fn artist_profile_tolerates_missing_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"id": "9", "type": "artists"}
            })))
            .mount(&server)
            .await;
        let profile = client(&server).artist_profile(9).await.unwrap();
        assert_eq!(profile.number_of_followers, None);
        assert!(profile.external_links.is_empty());
        assert_eq!(profile.bio, None);
    }

    #[tokio::test]
    async fn artist_profile_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).artist_profile(9).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-tidal artist_profile_reads_followers_links_and_bio`
Expected: compile error (`no method named artist_profile`) — the failing state.

- [ ] **Step 3: Implement**

At the top of `crates/oxitide-tidal/src/catalog.rs`, add the serde import:

```rust
use serde::Deserialize;
```

After the helper functions from Task 1 (before `#[cfg(test)]`), add the private OpenAPI shapes and the method. The shapes follow the `OpenApiPlaylistMeta` pattern in `playlists.rs`:

```rust
#[derive(Debug, Deserialize)]
struct OpenApiArtistProfile {
    #[serde(default)]
    data: Option<OpenApiArtistData>,
    #[serde(default)]
    included: Vec<OpenApiIncluded>,
}

#[derive(Debug, Deserialize)]
struct OpenApiArtistData {
    #[serde(default)]
    attributes: Option<OpenApiArtistAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiArtistAttributes {
    #[serde(default, rename = "numberOfFollowers")]
    number_of_followers: Option<u64>,
    #[serde(default, rename = "externalLinks")]
    external_links: Vec<OpenApiExternalLink>,
}

#[derive(Debug, Deserialize)]
struct OpenApiExternalLink {
    #[serde(default)]
    href: String,
    #[serde(default)]
    meta: Option<OpenApiExternalLinkMeta>,
}

#[derive(Debug, Deserialize)]
struct OpenApiExternalLinkMeta {
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenApiIncluded {
    #[serde(default, rename = "type")]
    resource_type: String,
    #[serde(default)]
    attributes: Option<OpenApiBioAttributes>,
}

#[derive(Debug, Deserialize)]
struct OpenApiBioAttributes {
    #[serde(default)]
    text: Option<String>,
}
```

and inside `impl TidalClient`, after `artist_albums`:

```rust
    /// Fans, external links, and biography from one public-OpenAPI call.
    /// `TIDAL_SHARING` self-links and empty/duplicate hrefs are dropped;
    /// every field is optional.
    pub async fn artist_profile(&self, id: u64) -> Result<ArtistProfile> {
        let params = [
            ("countryCode", self.session().country_code.clone()),
            ("include", "biography".to_string()),
        ];
        let raw: OpenApiArtistProfile = self
            .get_openapi(&format!("artists/{id}"), &params)
            .await?;

        let attributes = raw.data.and_then(|data| data.attributes);
        let mut seen = std::collections::HashSet::new();
        let external_links = attributes
            .as_ref()
            .map(|attributes| attributes.external_links.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|link| {
                let kind = link
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.kind.as_deref())
                    .unwrap_or("");
                !kind.eq_ignore_ascii_case("TIDAL_SHARING")
            })
            .filter(|link| !link.href.trim().is_empty())
            .filter(|link| seen.insert(link.href.clone()))
            .map(|link| ArtistLink {
                site: link
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.kind.clone())
                    .unwrap_or_default()
                    .to_uppercase(),
                url: link.href.clone(),
            })
            .collect();

        let bio = raw
            .included
            .iter()
            .find(|entry| entry.resource_type == "artistBiographies")
            .and_then(|entry| entry.attributes.as_ref())
            .and_then(|attributes| attributes.text.as_deref())
            .and_then(clean_bio);

        Ok(ArtistProfile {
            number_of_followers: attributes.and_then(|attributes| attributes.number_of_followers),
            external_links,
            bio,
        })
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-tidal artist_profile`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/catalog.rs
git commit -m "feat(tidal): fetch artist fans, socials, and bio from OpenAPI"
```

---

### Task 4: Tauri commands and external-link opener

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (imports; commands after `get_artist`; validator above them; tests inside the existing `mod tests`)
- Modify: `crates/oxitide-app/src/main.rs` (`generate_handler!`)

**Interfaces:**
- Consumes: `require_client`, `state.fail_tidal`, `external_url`, `ArtistProfile`, `ArtistDiscography`, `tauri_plugin_opener::OpenerExt`.
- Produces (registered commands): `get_artist_profile(id) -> ArtistProfile`, `get_artist_top_tracks(artist_id, limit) -> Vec<Track>`, `get_artist_albums(artist_id, filter, limit, offset) -> Vec<Album>`, `open_external(url) -> ()`. TS arg names: `{ id }`, `{ artistId, limit }`, `{ artistId, filter, limit, offset }`, `{ url }`.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-app/src/commands.rs`:

```rust
    #[test]
    fn external_url_accepts_http_and_https() {
        assert_eq!(
            external_url("https://twitter.com/acdc").unwrap().as_str(),
            "https://twitter.com/acdc"
        );
        assert!(external_url("http://example.com/").is_ok());
    }

    #[test]
    fn external_url_rejects_unsafe_schemes_and_missing_hosts() {
        for raw in [
            "javascript:alert(1)",
            "file:///C:/secret",
            "data:text/html,x",
            "https://",
            "",
        ] {
            assert!(external_url(raw).is_err(), "{raw} should be rejected");
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-app external_url`
Expected: compile error (`cannot find function external_url`) — the failing state.

- [ ] **Step 3: Implement**

In `crates/oxitide-app/src/commands.rs`, extend the models import (line 10):

```rust
use oxitide_tidal::models::{
    Album, Artist, ArtistDiscography, ArtistProfile, Mix, Playlist, SearchResults, Track,
};
```

Add after `get_artist` (line 728-734):

```rust
/// Only web URLs with a host can be handed to the system browser.
pub(crate) fn external_url(raw: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(raw).map_err(|_| "invalid URL".to_string())?;
    match parsed.scheme() {
        "http" | "https" if parsed.host_str().is_some() => Ok(parsed),
        _ => Err("only http(s) URLs with a host can be opened".to_string()),
    }
}

#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let parsed = external_url(&url)?;
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_artist_profile(
    state: State<'_, AppState>,
    id: u64,
) -> Result<ArtistProfile, String> {
    require_client(&state)?
        .artist_profile(id)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_top_tracks(
    state: State<'_, AppState>,
    artist_id: u64,
    limit: u32,
) -> Result<Vec<Track>, String> {
    require_client(&state)?
        .artist_top_tracks(artist_id, limit)
        .await
        .map_err(|e| state.fail_tidal(e))
}

#[tauri::command]
pub async fn get_artist_albums(
    state: State<'_, AppState>,
    artist_id: u64,
    filter: ArtistDiscography,
    limit: u32,
    offset: u32,
) -> Result<Vec<Album>, String> {
    require_client(&state)?
        .artist_albums(artist_id, filter, limit, offset)
        .await
        .map_err(|e| state.fail_tidal(e))
}
```

In `crates/oxitide-app/src/main.rs`, insert inside `generate_handler![...]` after `commands::get_similar_artists,` (line 284):

```rust
            commands::get_artist_profile,
            commands::get_artist_top_tracks,
            commands::get_artist_albums,
            commands::open_external,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-app external_url`
Expected: PASS (2 tests). Then run `cargo clippy -p oxitide-app -- -D warnings` and expect no warnings (confirms the commands compile with the handler registration).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add artist profile/top-track/discography and external-link commands"
```

---

### Task 5: UI logic modules and API wrappers

**Files:**
- Modify: `ui/src/lib/api.ts` (interfaces after `Artist`; wrappers near `getArtist`)
- Modify: `ui/src/lib/share.ts` + `ui/src/lib/share.test.ts`
- Modify: `ui/src/lib/format.ts` + `ui/src/lib/format.test.ts`
- Create: `ui/src/lib/artistLinks.ts`, `ui/src/lib/artistLinks.test.ts`

**Interfaces:**
- Consumes: existing `invoke`, `Track`, `Album`.
- Produces: `ArtistLink { site: string; url: string }`; `ArtistProfile { numberOfFollowers?: number | null; externalLinks: ArtistLink[]; bio?: string | null }`; `ArtistDiscography = "albums" | "eps_and_singles" | "compilations"`; `api.getArtistProfile`, `api.getArtistTopTracks`, `api.getArtistAlbums`, `api.openExternal`; `shareUrl("artist", id)`; `formatCount(n: number): string`; `linkLabel(site, url): string`, `isHttpUrl(url): boolean`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/artistLinks.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { isHttpUrl, linkLabel } from "./artistLinks";

describe("linkLabel", () => {
  it("maps known site keys", () => {
    expect(linkLabel("TWITTER", "https://twitter.com/acdc")).toBe("Twitter");
    expect(linkLabel("OFFICIAL_HOMEPAGE", "https://acdc.com")).toBe("Website");
    expect(linkLabel("LAST_FM", "https://last.fm/music/AC+DC")).toBe("Last.fm");
  });
  it("falls back to the host for unknown sites", () => {
    expect(linkLabel("SOMETHING_NEW", "https://www.example.com/x")).toBe("example.com");
  });
  it("falls back to the site key for unparseable urls", () => {
    expect(linkLabel("MYSTERY", "not a url")).toBe("MYSTERY");
  });
});

describe("isHttpUrl", () => {
  it("accepts http and https", () => {
    expect(isHttpUrl("https://example.com")).toBe(true);
    expect(isHttpUrl("http://example.com")).toBe(true);
  });
  it("rejects other schemes and junk", () => {
    expect(isHttpUrl("javascript:alert(1)")).toBe(false);
    expect(isHttpUrl("file:///C:/secret")).toBe(false);
    expect(isHttpUrl("")).toBe(false);
  });
});
```

Append to `ui/src/lib/format.test.ts` (add `formatCount` to the import line):

```ts
describe("formatCount", () => {
  it("leaves counts under a thousand exact", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
  });
  it("compacts thousands with one decimal below a hundred", () => {
    expect(formatCount(1000)).toBe("1K");
    expect(formatCount(9672)).toBe("9.7K");
    expect(formatCount(12000)).toBe("12K");
  });
  it("compacts millions and promotes near-boundary thousands", () => {
    expect(formatCount(999999)).toBe("1M");
    expect(formatCount(1106540)).toBe("1.1M");
  });
  it("compacts billions", () => {
    expect(formatCount(1_000_000_000)).toBe("1B");
  });
});
```

Append to the `describe("shareUrl")` block in `ui/src/lib/share.test.ts`:

```ts
  it("builds artist links from the numeric id", () => {
    expect(shareUrl("artist", 945)).toBe("https://tidal.com/artist/945");
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/artistLinks.test.ts src/lib/format.test.ts src/lib/share.test.ts`
Expected: FAIL — cannot resolve `./artistLinks`; `formatCount is not a function`; share type error.

- [ ] **Step 3: Implement**

Create `ui/src/lib/artistLinks.ts`:

```ts
/// Display labels for TIDAL external-link site keys, plus a host fallback.
const LABELS: Record<string, string> = {
  TWITTER: "Twitter",
  FACEBOOK: "Facebook",
  INSTAGRAM: "Instagram",
  YOUTUBE: "YouTube",
  TIKTOK: "TikTok",
  OFFICIAL_HOMEPAGE: "Website",
  WEBSITE: "Website",
  HOMEPAGE: "Website",
  LAST_FM: "Last.fm",
  WIKIPEDIA: "Wikipedia",
  DISCOGS: "Discogs",
  ALLMUSIC: "AllMusic",
  BBC_MUSICPAGE: "BBC Music",
  MYSPACE: "MySpace",
  SOUNDCLOUD: "SoundCloud",
  BANDCAMP: "Bandcamp",
};

export function linkLabel(site: string, url: string): string {
  const known = LABELS[site.toUpperCase()];
  if (known) return known;
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return site || "Link";
  }
}

export function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}
```

In `ui/src/lib/format.ts`, append:

```ts
/// Compact fan/listener counts: 999, 9.7K, 12K, 1.1M, 1B.
export function formatCount(n: number): string {
  const value = Math.max(0, Math.floor(n));
  if (value < 1000) return String(value);
  if (value >= 1e9) return compact(value / 1e9, "B");
  if (value >= 1e6) return compact(value / 1e6, "M");
  const thousands = value / 1e3;
  if (Math.round(thousands) >= 1000) return compact(value / 1e6, "M");
  return compact(thousands, "K");
}

function compact(scaled: number, suffix: string): string {
  const rounded = scaled >= 100 ? Math.round(scaled) : Math.round(scaled * 10) / 10;
  return `${rounded}${suffix}`;
}
```

In `ui/src/lib/share.ts`, change the kind union:

```ts
export type ShareKind = "playlist" | "album" | "mix" | "artist";
```

In `ui/src/lib/api.ts`, add after the `Artist` interface (line 17):

```ts
export interface ArtistLink { site: string; url: string }
export interface ArtistProfile {
  numberOfFollowers?: number | null;
  externalLinks: ArtistLink[];
  bio?: string | null;
}
export type ArtistDiscography = "albums" | "eps_and_singles" | "compilations";
```

and in the `api` object after `getSimilarArtists` (line 216):

```ts
  getArtistProfile: (id: number) => invoke<ArtistProfile>("get_artist_profile", { id }),
  getArtistTopTracks: (artistId: number, limit: number) =>
    invoke<Track[]>("get_artist_top_tracks", { artistId, limit }),
  getArtistAlbums: (artistId: number, filter: ArtistDiscography, limit: number, offset: number) =>
    invoke<Album[]>("get_artist_albums", { artistId, filter, limit, offset }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/artistLinks.test.ts src/lib/format.test.ts src/lib/share.test.ts`
Expected: PASS. Then run `npm --prefix ui run check` and expect 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/artistLinks.ts ui/src/lib/artistLinks.test.ts ui/src/lib/format.ts ui/src/lib/format.test.ts ui/src/lib/share.ts ui/src/lib/share.test.ts ui/src/lib/api.ts
git commit -m "feat(ui): add artist profile API, count formatting, and link helpers"
```

---

### Task 6: `external-link` icon and `ArtistHero`

**Files:**
- Modify: `ui/src/lib/components/Icon.svelte` (ICONS table)
- Create: `ui/src/lib/components/ArtistHero.svelte`

**Interfaces:**
- Consumes: `api.analyzeCover`, `Cover`, `Icon`, `coverUrl`, `formatCount`, `isHttpUrl`, `linkLabel`, `Artist`, `ArtistProfile`.
- Produces: `ArtistHero` with props `{ artist: Artist; profile: ArtistProfile | null; disabled?: boolean; onPlay: () => void; onShuffle: () => void; onOpenLink: (url: string) => void; actions?: Snippet }`.

- [ ] **Step 1: Add the icon path**

In `ui/src/lib/components/Icon.svelte`, add to the `ICONS` table after `"share-2"` (line 56):

```ts
    "external-link": '<path d="M15 3h6v6" /> <path d="M10 14 21 3" /> <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />',
```

- [ ] **Step 2: Create the component**

Create `ui/src/lib/components/ArtistHero.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import { api, type Artist, type ArtistProfile } from "../api";
  import { isHttpUrl, linkLabel } from "../artistLinks";
  import { coverUrl } from "../artwork";
  import { formatCount } from "../format";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  let {
    artist,
    profile,
    disabled = false,
    onPlay,
    onShuffle,
    onOpenLink,
    actions,
  }: {
    artist: Artist;
    profile: ArtistProfile | null;
    disabled?: boolean;
    onPlay: () => void;
    onShuffle: () => void;
    onOpenLink: (url: string) => void;
    actions?: Snippet;
  } = $props();

  let tint = $state<{ primary: string; secondary: string; tertiary: string } | null>(null);
  let bioOpen = $state(false);

  const fans = $derived(profile?.numberOfFollowers ?? null);
  const bio = $derived(profile?.bio ?? null);
  const links = $derived((profile?.externalLinks ?? []).filter((link) => isHttpUrl(link.url)));
  const backdrop = $derived(artist.picture ? coverUrl(artist.picture, 640) : "");
  const heroStyle = $derived(
    tint
      ? `--hero-primary:${tint.primary};--hero-secondary:${tint.secondary};--hero-tertiary:${tint.tertiary}`
      : undefined,
  );

  // Local palette only: the hero must not repaint the app-wide --dyn-* mood.
  $effect(() => {
    const uuid = artist.picture;
    if (!uuid) {
      tint = null;
      return;
    }
    let current = true;
    void api
      .analyzeCover(uuid)
      .then((palette) => {
        if (current) {
          tint = {
            primary: palette.primary,
            secondary: palette.secondary,
            tertiary: palette.tertiary,
          };
        }
      })
      .catch(() => {
        if (current) tint = null;
      });
    return () => {
      current = false;
    };
  });
</script>

<header class="artist-hero" style={heroStyle}>
  {#if backdrop}
    <div class="photo" style="background-image:url({backdrop})" aria-hidden="true"></div>
  {/if}
  <div class="scrim" aria-hidden="true"></div>
  <div class="content">
    <span class="portrait">
      <Cover uuid={artist.picture} size={640} radius={999} alt={artist.name} />
    </span>
    <div class="meta">
      <span class="kicker">Artist</span>
      <h2>{artist.name}</h2>
      {#if fans !== null}
        <p class="stats tnum">{formatCount(fans)} {fans === 1 ? "fan" : "fans"}</p>
      {/if}
      <div class="controls">
        <div class="transport">
          <button class="btn-play" onclick={onPlay} {disabled}>
            <Icon name="play" size={20} /> Play
          </button>
          <button class="btn-shuffle" onclick={onShuffle} {disabled}>
            <Icon name="shuffle" size={20} /> Shuffle
          </button>
        </div>
        {#if actions}
          <div class="actions">{@render actions()}</div>
        {/if}
      </div>
      {#if bio}
        <p class="bio" class:clamped={!bioOpen}>{bio}</p>
        {#if bio.length > 240}
          <button class="more" aria-expanded={bioOpen} onclick={() => (bioOpen = !bioOpen)}>
            {bioOpen ? "Show less" : "Read more"}
          </button>
        {/if}
      {/if}
      {#if links.length > 0}
        <div class="socials">
          {#each links as link (link.url)}
            <button
              class="social"
              title={link.url}
              aria-label={`Open ${linkLabel(link.site, link.url)}`}
              onclick={() => onOpenLink(link.url)}
            >
              <Icon name="external-link" size={14} />
              <span>{linkLabel(link.site, link.url)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</header>

<style>
  .artist-hero {
    --hero-primary: var(--surface-2);
    --hero-secondary: var(--surface-3);
    --hero-tertiary: var(--surface-1);
    position: relative;
    overflow: hidden;
    margin-bottom: 32px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sheet);
    background: linear-gradient(
      135deg,
      var(--hero-primary),
      var(--hero-secondary) 55%,
      var(--hero-tertiary)
    );
  }
  .photo {
    position: absolute;
    inset: -12%;
    background-size: cover;
    background-position: center 30%;
    filter: blur(60px) saturate(1.2);
    transform: scale(1.1);
    opacity: calc(0.45 * var(--flag-backdrop-art));
  }
  .scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--bg) 25%, transparent),
      color-mix(in srgb, var(--bg) 72%, transparent)
    );
  }
  .content {
    position: relative;
    display: flex;
    align-items: flex-end;
    gap: 32px;
    padding: 40px 32px 28px;
  }
  .portrait {
    flex: 0 0 auto;
    width: 200px;
  }
  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .kicker {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  h2 {
    margin: 0;
    font-size: clamp(28px, 3.2vw, 42px);
    line-height: 1.1;
  }
  .stats {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 16px;
    margin-top: 12px;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .transport > button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    height: 44px;
    padding: 0 22px;
    border-radius: var(--radius-control);
    font-weight: 600;
    transition:
      background var(--dur-micro) var(--ease-in),
      color var(--dur-micro) var(--ease-in);
  }
  .btn-play {
    background: var(--accent);
    color: var(--on-accent);
  }
  .btn-play:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-play :global(svg) {
    fill: currentColor;
  }
  .btn-shuffle {
    background: color-mix(in srgb, var(--surface-float) 70%, transparent);
    border: 1px solid var(--line);
    color: var(--text-muted);
  }
  .btn-shuffle:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .transport > button:disabled {
    opacity: 0.5;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .bio {
    margin: 4px 0 0;
    max-width: 72ch;
    color: var(--text-muted);
    white-space: pre-line;
  }
  .bio.clamped {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    overflow: hidden;
  }
  .more {
    align-self: flex-start;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .more:hover {
    color: var(--text);
  }
  .socials {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .social {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--card-badge);
    background: color-mix(in srgb, var(--surface-float) 60%, transparent);
    backdrop-filter: blur(var(--glass-blur));
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
  }
  .social:hover {
    color: var(--text);
    background: var(--surface-2);
  }
</style>
```

- [ ] **Step 3: Verify**

Run: `npm --prefix ui run check`
Expected: 0 errors (component compiles; `external-link` is now a valid `IconName`).

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/Icon.svelte ui/src/lib/components/ArtistHero.svelte
git commit -m "feat(ui): add the artist hero component"
```

---

### Task 7: Rebuild `Artist.svelte` and wire the dispatch

**Files:**
- Rewrite: `ui/src/views/Artist.svelte`
- Modify: `ui/src/App.svelte:478`

**Interfaces:**
- Consumes: `api.getArtist`, `api.getArtistProfile`, `api.getArtistTopTracks`, `api.getArtistAlbums`, `api.getSimilarArtists`, `api.playTracks`, `api.openExternal`, `ArtistHero`, `TrackTable`, `Carousel`, `MediaCard`, `DetailAction`, `Icon`, `createTrackView`, `trackToQueueItem`, `playSource`, `playArtistRadio`, `shareUrl`, `copyText`, `menu`, `isHttpUrl`/`linkLabel` (inside `ArtistHero`).
- Produces: `Artist` props `{ id: number; nav: Nav; player: Player; favorites: Favorites; playlistActions: PlaylistActions; toasts: Toasts }`.

- [ ] **Step 1: Rewrite the view**

Replace `ui/src/views/Artist.svelte` with:

```svelte
<script lang="ts">
  import {
    api,
    trackToQueueItem,
    type Album as AlbumT,
    type Artist as ArtistT,
    type ArtistProfile,
    type PageItem,
    type Track,
  } from "../lib/api";
  import { copyText } from "../lib/clipboard";
  import ArtistHero from "../lib/components/ArtistHero.svelte";
  import Carousel from "../lib/components/Carousel.svelte";
  import DetailAction from "../lib/components/DetailAction.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import ErrorState from "../lib/components/ErrorState.svelte";
  import Loading from "../lib/components/Loading.svelte";
  import MediaCard from "../lib/components/MediaCard.svelte";
  import TrackTable from "../lib/components/TrackTable.svelte";
  import { menu } from "../lib/menu.svelte";
  import { playSource } from "../lib/playSource.svelte";
  import { playArtistRadio } from "../lib/radio";
  import { shareUrl } from "../lib/share";
  import { createTrackView } from "../lib/trackView.svelte";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Nav } from "../lib/nav.svelte";
  import type { PlaylistActions } from "../lib/playlists.svelte";
  import type { Player } from "../lib/player.svelte";
  import type { Toasts } from "../lib/toasts.svelte";

  let {
    id,
    nav,
    player,
    favorites,
    playlistActions,
    toasts,
  }: {
    id: number;
    nav: Nav;
    player: Player;
    favorites: Favorites;
    playlistActions: PlaylistActions;
    toasts: Toasts;
  } = $props();

  let artist = $state<ArtistT | null>(null);
  let profile = $state<ArtistProfile | null>(null);
  let topTracks = $state<Track[]>([]);
  let albums = $state<AlbumT[]>([]);
  let eps = $state<AlbumT[]>([]);
  let comps = $state<AlbumT[]>([]);
  let similar = $state<ArtistT[]>([]);
  let loading = $state(true);
  let error = $state("");
  let expanded = $state(false);
  let loadToken = 0;

  const visibleTop = $derived(expanded ? topTracks : topTracks.slice(0, 5));
  const view = createTrackView(() => visibleTop);

  function albumCard(album: AlbumT): PageItem {
    const year = album.releaseDate?.slice(0, 4) ?? "";
    return {
      id: String(album.id),
      title: album.title,
      subtitle: /^\d{4}$/.test(year) ? year : null,
      item_type: "ALBUM",
      artwork: album.cover,
    };
  }

  function artistCard(artist: ArtistT): PageItem {
    return {
      id: String(artist.id),
      title: artist.name,
      subtitle: null,
      item_type: "ARTIST",
      artwork: artist.picture,
    };
  }

  async function load() {
    const token = ++loadToken;
    loading = true;
    error = "";
    topTracks = [];
    albums = [];
    eps = [];
    comps = [];
    similar = [];
    expanded = false;
    try {
      const [loadedArtist, loadedProfile] = await Promise.all([
        api.getArtist(id),
        api.getArtistProfile(id).catch(() => null),
      ]);
      if (token !== loadToken) return;
      artist = loadedArtist;
      profile = loadedProfile;
    } catch (e) {
      if (token !== loadToken) return;
      error = String(e);
      loading = false;
      return;
    }
    loading = false;
    // Sections load independently: one slow or failed shelf never blocks
    // the hero or its neighbours, and stale responses are dropped.
    void api.getArtistTopTracks(id, 10)
      .then((tracks) => {
        if (token === loadToken) topTracks = tracks;
      })
      .catch(() => {});
    for (const [filter, target] of [
      ["albums", "albums"],
      ["eps_and_singles", "eps"],
      ["compilations", "comps"],
    ] as const) {
      void api.getArtistAlbums(id, filter, 50, 0)
        .then((list) => {
          if (token !== loadToken) return;
          if (target === "albums") albums = list;
          else if (target === "eps") eps = list;
          else comps = list;
        })
        .catch(() => {});
    }
    void api.getSimilarArtists(id)
      .then((list) => {
        if (token === loadToken) similar = list;
      })
      .catch(() => {});
  }

  function queueTop(startIndex: number, shuffle = false) {
    if (topTracks.length === 0) return;
    playSource.set(null);
    void api.playTracks(topTracks.map(trackToQueueItem), startIndex, shuffle);
  }

  function playFrom(index: number) {
    // `view.items` is a prefix of `topTracks` until a filter/sort engages;
    // sourceIndex maps the visible row back to the queue's source order.
    const source = view.sourceIndex(index);
    if (source < 0) return;
    queueTop(source);
  }

  async function share() {
    const copied = await copyText(shareUrl("artist", id));
    toasts.show(copied ? "Link copied" : "Couldn't copy link", {
      severity: copied ? "success" : "error",
    });
  }

  function openMenu(event: MouseEvent) {
    menu.show(event.clientX, event.clientY, [
      { id: "share", label: "Share", run: () => void share() },
    ]);
  }

  async function openLink(url: string) {
    try {
      await api.openExternal(url);
    } catch (e) {
      toasts.show(String(e), { severity: "error" });
    }
  }

  $effect(() => {
    load();
  });
</script>

{#if loading}
  <Loading />
{:else if error}
  <ErrorState message={error} onRetry={load} />
{:else if artist}
  {@const artistId = artist.id}
  <ArtistHero
    artist
    {profile}
    disabled={topTracks.length === 0}
    onPlay={() => queueTop(0)}
    onShuffle={() => queueTop(0, true)}
    onOpenLink={openLink}
  >
    {#snippet actions()}
      <DetailAction
        icon="radio"
        label="Radio"
        title="Start artist radio"
        onclick={() => void playArtistRadio(artistId, toasts)}
      />
      <DetailAction
        icon="heart"
        label={favorites.hasArtist(artistId) ? "Favorited" : "Favorite"}
        active={favorites.hasArtist(artistId)}
        title={favorites.hasArtist(artistId) ? "Unfavorite artist" : "Favorite artist"}
        onclick={() => favorites.toggleArtist(artistId)}
      />
      <DetailAction icon="ellipsis" label="More" title="More options" onclick={openMenu} />
    {/snippet}
  </ArtistHero>

  {#if topTracks.length > 0}
    <section class="shelf">
      <div class="shelf-head">
        <h3>Top tracks</h3>
        {#if topTracks.length > 5}
          <button class="more" onclick={() => (expanded = !expanded)}>
            {expanded ? "Show less" : "Show all"}
          </button>
        {/if}
      </div>
      <TrackTable
        {view}
        {favorites}
        {toasts}
        onPlay={playFrom}
        playingTrackId={player.state.current?.track_id ?? null}
        onAddToPlaylist={(track) => playlistActions.openPicker([track])}
      />
    </section>
  {/if}

  {#if albums.length > 0}
    <section class="shelf">
      <Carousel title="Albums">
        {#each albums as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if eps.length > 0}
    <section class="shelf">
      <Carousel title="Singles & EPs">
        {#each eps as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if comps.length > 0}
    <section class="shelf">
      <Carousel title="Appears on">
        {#each comps as album (album.id)}
          <MediaCard
            variant="carousel"
            item={albumCard(album)}
            onclick={() => nav.goto({ name: "album", id: album.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}

  {#if similar.length > 0}
    <section class="shelf">
      <Carousel title="Similar artists">
        {#each similar as s (s.id)}
          <MediaCard
            variant="carousel"
            item={artistCard(s)}
            onclick={() => nav.goto({ name: "artist", id: s.id })}
          />
        {/each}
      </Carousel>
    </section>
  {/if}
{:else}
  <EmptyState message="Artist not found." />
{/if}

<style>
  .shelf {
    margin-bottom: 28px;
  }
  .shelf-head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .shelf-head h3 {
    flex: 1;
    min-width: 0;
  }
  .more {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .more:hover {
    color: var(--text);
  }
</style>
```

- [ ] **Step 2: Update the dispatch**

In `ui/src/App.svelte` line 478, replace:

```svelte
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {nav} {toasts} {favorites} />
```

with:

```svelte
      {:else if nav.route.name === "artist"}<Artist id={nav.route.id} {nav} {player} {favorites} {playlistActions} {toasts} />
```

- [ ] **Step 3: Verify**

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: all suites pass (including the new `artistLinks`/`formatCount`/`artist` share cases).

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/Artist.svelte ui/src/App.svelte
git commit -m "feat(ui): rebuild the artist page with hero, top tracks, and shelves"
```

---

### Task 8: Full gates and smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-artist-page-smoke.md`

- [ ] **Step 1: Run every gate**

Run: `cargo test --workspace`
Expected: all pass.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings.

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: all pass.

- [ ] **Step 2: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-03-artist-page-smoke.md`:

```markdown
# Artist page smoke checklist (2026-10-03)

Human-in-the-loop checks for the Track 9 artist page. Run with
`npm --prefix ui run app:dev` and a real account.

- [ ] **Rich artist (e.g. AC/DC):** hero renders with the palette backdrop,
      blurred portrait, name, `1.1M fans`, Play/Shuffle enabled.
- [ ] **Sparse artist (e.g. The Notwist):** bio renders with Read more; no
      socials row appears; page is still clean with no empty headings.
- [ ] **Profile failure:** block `openapi.tidal.com` (or disconnect the
      network before opening) → hero still renders, no fans/bio/socials,
      Top tracks and shelves still load once the network returns.
- [ ] **Play vs Shuffle vs Radio:** Play queues the 10 top tracks in order and
      starts row 1; Shuffle queues them randomized; Radio replaces the queue
      with the artist radio (existing behavior).
- [ ] **Show all:** Top tracks shows 5 rows; Show all reveals 10; Show less
      collapses; playing from row 2 queues from that track.
- [ ] **Sections:** Albums / Singles & EPs / Appears on each render as a
      carousel and navigate to the album page; missing shelves are absent.
- [ ] **Similar artists:** circular cards; clicking navigates to that artist;
      back returns to the first artist with correct data.
- [ ] **Rapid navigation:** open artist A, immediately click similar artist B;
      B's page must show B's data only (no flashes of A's shelves).
- [ ] **Heart / Share / More:** heart toggles; More → Share copies
      `https://tidal.com/artist/{id}` and toasts; radio action unchanged.
- [ ] **Socials:** social chips open in the system browser; a chip with a
      non-web URL never renders.
- [ ] **Modes / motion:** atmosphere shows the backdrop; instrument shows the
      flat hero; reduced motion removes transitions.
- [ ] **Playing row:** the currently playing top track is highlighted.
- [ ] **Perf regression check:** with the app warm, run
      `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5`
      then `node scripts/perf/check.mjs`; budgets still pass.
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-03-artist-page-smoke.md
git commit -m "test: add the artist page smoke checklist"
```
