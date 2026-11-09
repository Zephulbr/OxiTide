//! The signed-in account's TIDAL profile (internal v2 `profiles/{userId}`).
//!
//! TIDAL keeps no separate user page: a profile is an artist with
//! `ownerType: "USER"`, and the web player's `/user/{id}` opens
//! `/artist/{artistId}` when the profile carries one
//! (`docs/superpowers/plans/notes/2026-11-06-tidal-profiles-research.md`).
//!
//! Editing goes through the public OpenAPI, as TIDAL's web editor does:
//! `PATCH artists/{id}` (name, handle, links; `meta.dryRun` validates),
//! `PATCH artistBiographies/{id}` (bio) and the `artworks` upload flow
//! (picture, banner, logo). Shapes follow the `sone` client's working calls
//! and TIDAL's published OpenAPI spec.

use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::{Error, Result};
use crate::models::Playlist;

/// The artwork slots of a profile: OpenAPI `artists/{id}` relationships
/// `profileArt` (the picture), `bannerArt` (the header banner) and `logoArt`.
/// All three take one uploaded IMAGE artwork through the same flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileArt {
    Picture,
    Banner,
    Logo,
}

impl ProfileArt {
    pub const ALL: [ProfileArt; 3] = [ProfileArt::Picture, ProfileArt::Banner, ProfileArt::Logo];

    pub fn relationship(self) -> &'static str {
        match self {
            ProfileArt::Picture => "profileArt",
            ProfileArt::Banner => "bannerArt",
            ProfileArt::Logo => "logoArt",
        }
    }

    /// The width of the file picked from an artwork's sizes: the 750 px the
    /// artist pages show the picture at, a wide banner, a small logo.
    fn shown_width(self) -> u64 {
        match self {
            ProfileArt::Picture => 750,
            ProfileArt::Banner => 1280,
            ProfileArt::Logo => 640,
        }
    }
}

/// One file TIDAL made from an artwork (`artworks` `files[]`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtFile {
    pub href: String,
    pub width: u64,
    pub height: u64,
}

/// An artist's uploaded artwork, read from OpenAPI `artists/{id}` with the
/// three art relationships included. Not shown on artist pages yet: the v1
/// artist they read carries none of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistArt {
    pub picture: Option<ArtFile>,
    pub banner: Option<ArtFile>,
    pub logo: Option<ArtFile>,
}

/// One link on a profile: TIDAL's `externalLinks[]` `{href, meta: {type}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLink {
    pub href: String,
    /// TIDAL's link type, e.g. `INSTAGRAM`, `TWITTER`, `OFFICIAL_HOMEPAGE`.
    pub kind: String,
}

/// What the profile editor shows, read from OpenAPI `artists/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableProfile {
    pub artist_id: u64,
    pub name: String,
    pub handle: Option<String>,
    pub bio: Option<String>,
    /// The biography resource the bio is written to; `None` when the profile
    /// has none yet (TIDAL's create call is not known, so the bio is then
    /// read-only here).
    pub bio_id: Option<String>,
    pub links: Vec<ProfileLink>,
    pub has_picture: bool,
    /// The current banner and logo, for the editor's previews.
    pub banner: Option<ArtFile>,
    pub logo: Option<ArtFile>,
}

/// The fields a save changes; `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEdit {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(default)]
    pub links: Option<Vec<ProfileLink>>,
}

/// The JSON:API document for an artist edit (`dry_run` validates only).
fn artist_edit_doc(artist_id: u64, edit: &ProfileEdit, dry_run: bool) -> Value {
    let mut attributes = serde_json::Map::new();
    if let Some(name) = &edit.name {
        attributes.insert("name".into(), Value::String(name.trim().to_string()));
    }
    if let Some(handle) = &edit.handle {
        attributes.insert("handle".into(), Value::String(handle.trim().to_string()));
    }
    if let Some(links) = &edit.links {
        let links: Vec<Value> = links
            .iter()
            .map(|link| serde_json::json!({"href": link.href.trim(), "meta": {"type": link.kind}}))
            .collect();
        attributes.insert("externalLinks".into(), Value::Array(links));
    }
    let mut doc = serde_json::json!({
        "data": {"type": "artists", "id": artist_id.to_string(), "attributes": Value::Object(attributes)}
    });
    if dry_run {
        doc["meta"] = serde_json::json!({"dryRun": true});
    }
    doc
}

/// The first `errors[].detail` (else `title`) of a JSON:API error body, so a
/// rejected edit ("handle already taken") reads as TIDAL wrote it.
pub fn api_error_detail(error: &Error) -> Option<String> {
    let Error::Api { message, .. } = error else {
        return None;
    };
    let doc: Value = serde_json::from_str(message).ok()?;
    let first = doc.get("errors")?.as_array()?.first()?;
    text(first.get("detail")).or_else(|| text(first.get("title")))
}

fn editable_from(doc: &Value, artist_id: u64) -> EditableProfile {
    let data = doc.get("data");
    let attrs = data.and_then(|d| d.get("attributes"));
    let included = doc.get("included").and_then(Value::as_array);
    let links = attrs
        .and_then(|a| a.get("externalLinks"))
        .and_then(Value::as_array)
        .map(|links| {
            links
                .iter()
                .filter_map(|link| {
                    Some(ProfileLink {
                        href: text(link.get("href"))?,
                        kind: text(link.get("meta").and_then(|m| m.get("type")))
                            .unwrap_or_else(|| "OFFICIAL_HOMEPAGE".into()),
                    })
                })
                .filter(|link| !is_tidal_own_link(link))
                .collect()
        })
        .unwrap_or_default();
    let relationship = |name: &str| {
        data.and_then(|d| d.get("relationships"))
            .and_then(|r| r.get(name))
            .and_then(|r| r.get("data"))
    };
    let bio_id = relationship("biography").and_then(|b| text(b.get("id")));
    let bio = bio_id.as_deref().and_then(|id| {
        included?.iter().find_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("artistBiographies")
                && item.get("id").and_then(Value::as_str) == Some(id))
            .then(|| text(item.get("attributes").and_then(|a| a.get("text"))))
            .flatten()
        })
    });
    let has_picture = relationship("profileArt")
        .and_then(Value::as_array)
        .is_some_and(|art| !art.is_empty());
    EditableProfile {
        artist_id,
        name: text(attrs.and_then(|a| a.get("name"))).unwrap_or_default(),
        handle: text(attrs.and_then(|a| a.get("handle"))),
        bio,
        bio_id,
        links,
        has_picture,
        banner: art_from(doc, ProfileArt::Banner),
        logo: art_from(doc, ProfileArt::Logo),
    }
}

/// The artworks an `include=` names, by id.
fn included_artwork<'a>(doc: &'a Value, id: &str) -> Option<&'a Value> {
    doc.get("included")?.as_array()?.iter().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("artworks")
            && item.get("id").and_then(Value::as_str) == Some(id)
    })
}

/// Every file of an artwork resource (`attributes.files[]`), with its size.
fn art_files(artwork: &Value) -> Vec<ArtFile> {
    let Some(files) = artwork
        .pointer("/attributes/files")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    files
        .iter()
        .filter_map(|file| {
            let href = text(file.get("href"))?;
            let width = number(file.pointer("/meta/width"))?;
            let height = number(file.pointer("/meta/height")).unwrap_or(width);
            Some(ArtFile {
                href,
                width,
                height,
            })
        })
        .collect()
}

/// The selected artwork of one art relationship, as the file nearest the
/// width it is shown at. `None` when nothing is selected or the artwork was
/// not included.
fn art_from(doc: &Value, art: ProfileArt) -> Option<ArtFile> {
    let selected = doc
        .pointer(&format!("/data/relationships/{}/data", art.relationship()))?
        .as_array()?
        .first()?;
    let artwork = included_artwork(doc, selected.get("id")?.as_str()?)?;
    nearest_file(art_files(artwork), art)
}

/// The file to show for `art`: the picture only from square files (an
/// upload may not exist at every size the uuid-based artist sizes ask for);
/// the banner from its wide files when it has any (TIDAL cuts a banner to
/// both 1:1 and 16:9: 1280x1280 ... and 1280x720 ..., seen live 2026-11-07);
/// the logo at any shape.
fn nearest_file(files: Vec<ArtFile>, art: ProfileArt) -> Option<ArtFile> {
    let has_wide = files.iter().any(|file| file.width > file.height);
    files
        .into_iter()
        .filter(|file| match art {
            ProfileArt::Picture => file.width == file.height,
            ProfileArt::Banner => !has_wide || file.width > file.height,
            ProfileArt::Logo => true,
        })
        .min_by_key(|file| file.width.abs_diff(art.shown_width()))
}

/// The sizes TIDAL made from an artwork, for the log (no URLs).
fn art_sizes(doc: &Value, art: ProfileArt) -> Vec<String> {
    let selected = doc
        .pointer(&format!(
            "/data/relationships/{}/data/0/id",
            art.relationship()
        ))
        .and_then(Value::as_str);
    selected
        .and_then(|id| included_artwork(doc, id))
        .map(|artwork| {
            art_files(artwork)
                .iter()
                .map(|f| format!("{}x{}", f.width, f.height))
                .collect()
        })
        .unwrap_or_default()
}

/// The link TIDAL keeps on every profile to the profile itself
/// (`https://tidal.com/browse/artist/{id}`, a `TIDAL_*` type). TIDAL re-adds
/// it after any links edit, so the editor neither shows nor sends it.
fn is_tidal_own_link(link: &ProfileLink) -> bool {
    if link.kind.to_ascii_uppercase().starts_with("TIDAL") {
        return true;
    }
    let host = link
        .href
        .split_once("://")
        .map_or(link.href.as_str(), |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    host == "tidal.com" || host.ends_with(".tidal.com")
}

/// The file TIDAL made from an upload that the app shows, from the
/// processed `artworks/{id}` document. Kept as the exact file URL, which
/// `coverUrl` passes through.
fn uploaded_file_url(doc: &Value, art: ProfileArt) -> Option<String> {
    nearest_file(art_files(doc.get("data")?), art).map(|file| file.href)
}

/// `items[].playlist` of `v2/user-playlists/{id}/public`, with the follower
/// count from `followInfo`.
fn public_playlists_from(doc: &Value, viewer: u64) -> Vec<Playlist> {
    let Some(items) = doc.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let mut playlist: Playlist =
                serde_json::from_value(item.get("playlist")?.clone()).ok()?;
            playlist.public_playlist = Some(true);
            if playlist.number_of_followers.is_none() {
                playlist.number_of_followers =
                    number(item.get("followInfo").and_then(|f| f.get("nrOfFollowers")))
                        .and_then(|n| u32::try_from(n).ok());
            }
            playlist.mark_owned(viewer, false);
            Some(playlist)
        })
        .collect()
}

/// Pages of public playlists read at most (50 each).
const PUBLIC_PLAYLIST_PAGES: usize = 10;

/// How long a picture upload waits for TIDAL to process it: 20 × 800 ms.
const ARTWORK_POLLS: usize = 20;
const ARTWORK_POLL_MS: u64 = 800;
/// Extra polls for the file list once an artwork reads as processed.
const ARTWORK_FILE_POLLS: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: u64,
    /// The display name; `None` when TIDAL sends none (or only the id).
    pub name: Option<String>,
    pub handle: Option<String>,
    /// Image uuid of the profile picture.
    pub picture: Option<String>,
    /// The artist that is this profile's page. Absent on an account that has
    /// never had one; TIDAL then has no page for the profile either.
    pub artist_id: Option<u64>,
    pub number_of_followers: Option<u64>,
    pub number_of_follows: Option<u64>,
}

use serde_json::Value;

/// A number, a numeric string, or null/anything else as `None`.
fn number(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Trimmed, non-empty text, else `None`.
fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// The profile picture. Live (2026-11-06) it is an object, not the uuid
/// the third-party notes suggested, so every shape is accepted: a string; an
/// object with `url`/`href`/`uuid`/`id`; or an object of sizes keyed by
/// width, from which the largest is taken. A string may be a uuid or a full
/// url, and `coverUrl` handles both.
fn picture(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = text(Some(value)) {
        return Some(s);
    }
    let Value::Object(map) = value else {
        return None;
    };
    for key in ["url", "href", "uuid", "id", "picture", "image"] {
        if let Some(found) = picture(map.get(key)) {
            return Some(found);
        }
    }
    // Sizes keyed by width ("100", "210x210", "LARGE", …): prefer the largest
    // numeric key, else the first value that yields a picture.
    let width = |key: &str| -> u32 {
        key.split(|c: char| !c.is_ascii_digit())
            .find(|part| !part.is_empty())
            .and_then(|part| part.parse().ok())
            .unwrap_or(0)
    };
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(key, _)| std::cmp::Reverse(width(key)));
    let found = entries
        .into_iter()
        .find_map(|(_, inner)| picture(Some(inner)));
    if found.is_none() {
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        tracing::warn!(?keys, "profile picture has an unknown shape");
    }
    found
}

fn profile_from(raw: &Value, user_id: u64) -> UserProfile {
    let get = |key: &str| raw.get(key);
    // A name equal to the id is TIDAL's placeholder, not a name.
    let id_text = user_id.to_string();
    UserProfile {
        user_id: number(get("userId")).unwrap_or(user_id),
        name: text(get("name")).filter(|name| *name != id_text),
        handle: text(get("handle")),
        picture: picture(get("picture")),
        artist_id: number(get("artistId")).filter(|id| *id > 0),
        number_of_followers: number(get("numberOfFollowers")),
        number_of_follows: number(get("numberOfFollows")),
    }
}

impl TidalClient {
    /// The signed-in account's profile, or `None` when TIDAL has none to
    /// show (403/404: private or never created). Read field by field, so an
    /// unexpected shape in one field never hides the whole profile.
    pub async fn my_profile(&self) -> Result<Option<UserProfile>> {
        let user_id = self.session().user_id;
        let raw: Value = match self.get_v2(&format!("profiles/{user_id}"), &[]).await {
            Ok(raw) => raw,
            Err(Error::NotFound(_) | Error::Forbidden) => return Ok(None),
            Err(e) => return Err(e),
        };
        let profile = profile_from(&raw, user_id);
        // Shape only (key names and whether an artist is linked), never values.
        let keys: Vec<&str> = raw
            .as_object()
            .map(|map| map.keys().map(String::as_str).collect())
            .unwrap_or_default();
        tracing::info!(
            ?keys,
            has_artist = profile.artist_id.is_some(),
            "read the account's profile"
        );
        Ok(Some(profile))
    }

    /// A user's public playlists (a profile's "Public playlists"), following
    /// the cursor for at most [`PUBLIC_PLAYLIST_PAGES`] pages.
    pub async fn user_public_playlists(&self, user_id: u64) -> Result<Vec<Playlist>> {
        let viewer = self.session().user_id;
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..PUBLIC_PLAYLIST_PAGES {
            let mut params = vec![("limit", "50".to_string())];
            if let Some(cursor) = &cursor {
                params.push(("cursor", cursor.clone()));
            }
            let doc: Value = self
                .get_v2(&format!("user-playlists/{user_id}/public"), &params)
                .await?;
            out.extend(public_playlists_from(&doc, viewer));
            match text(doc.get("cursor")) {
                Some(next) if Some(&next) != cursor.as_ref() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(out)
    }

    /// The account's profile as the editor shows it.
    pub async fn editable_profile(&self, artist_id: u64) -> Result<EditableProfile> {
        let params = [
            (
                "include",
                "biography,profileArt,bannerArt,logoArt".to_string(),
            ),
            ("countryCode", self.session().country_code.clone()),
        ];
        let doc: Value = self
            .get_openapi(&format!("artists/{artist_id}"), &params)
            .await?;
        // The sizes TIDAL makes a banner and logo at (no URLs): what an
        // upload should be shaped like is not documented.
        tracing::info!(
            banner = ?art_sizes(&doc, ProfileArt::Banner),
            logo = ?art_sizes(&doc, ProfileArt::Logo),
            "read the profile's artwork sizes"
        );
        Ok(editable_from(&doc, artist_id))
    }

    /// Any artist's uploaded picture, banner and logo. Nothing calls this
    /// yet: artist pages read v1, which has no banner or logo, and showing
    /// them would cost this extra request per page.
    pub async fn artist_art(&self, artist_id: u64) -> Result<ArtistArt> {
        let params = [
            ("include", "profileArt,bannerArt,logoArt".to_string()),
            ("countryCode", self.session().country_code.clone()),
        ];
        let doc: Value = self
            .get_openapi(&format!("artists/{artist_id}"), &params)
            .await?;
        Ok(ArtistArt {
            picture: art_from(&doc, ProfileArt::Picture),
            banner: art_from(&doc, ProfileArt::Banner),
            logo: art_from(&doc, ProfileArt::Logo),
        })
    }

    /// Change the profile's name, handle or links. With `dry_run` TIDAL only
    /// validates (a taken handle answers an error) and changes nothing.
    pub async fn update_profile(
        &self,
        artist_id: u64,
        edit: &ProfileEdit,
        dry_run: bool,
    ) -> Result<()> {
        if edit.name.is_none() && edit.handle.is_none() && edit.links.is_none() {
            return Ok(());
        }
        let params = [("countryCode", self.session().country_code.clone())];
        let doc = artist_edit_doc(artist_id, edit, dry_run);
        self.patch_openapi(&format!("artists/{artist_id}"), &params, &doc)
            .await
    }

    /// Rewrite the profile's biography.
    pub async fn update_profile_bio(&self, bio_id: &str, text: &str) -> Result<()> {
        let params = [("countryCode", self.session().country_code.clone())];
        let doc = serde_json::json!({
            "data": {"type": "artistBiographies", "id": bio_id, "attributes": {"text": text.trim()}}
        });
        self.patch_openapi(&format!("artistBiographies/{bio_id}"), &params, &doc)
            .await
    }

    /// Upload `file` (already shaped for `art`; `content_type` is its image
    /// type) as the profile's picture, banner or logo: create an artwork, PUT
    /// the file to its upload link, wait for TIDAL to process it, then point
    /// the artist's relationship for `art` at it. `md5_hex` and `md5_base64`
    /// are the file's MD5 in both encodings TIDAL asks for.
    ///
    /// Answers the URL of the file the app shows when TIDAL lists one: the
    /// v1 artist record keeps an old picture for a while, so the app shows
    /// this one until it catches up.
    pub async fn upload_profile_art(
        &self,
        artist_id: u64,
        art: ProfileArt,
        file: Vec<u8>,
        content_type: &str,
        md5_hex: &str,
        md5_base64: &str,
    ) -> Result<Option<String>> {
        let params = [("countryCode", self.session().country_code.clone())];
        let create = serde_json::json!({
            "data": {"type": "artworks", "attributes": {
                "mediaType": "IMAGE",
                "sourceFile": {"md5Hash": md5_hex, "size": file.len()}
            }}
        });
        let created: Value = self.post_openapi("artworks", &params, &create).await?;
        let artwork_id = text(created.pointer("/data/id")).ok_or_else(|| Error::Api {
            status: 200,
            message: "TIDAL sent no artwork id".into(),
        })?;
        let upload = text(created.pointer("/data/attributes/sourceFile/uploadLink/href"))
            .ok_or_else(|| Error::Api {
                status: 200,
                message: "TIDAL sent no upload link".into(),
            })?;
        self.put_upload(&upload, file, content_type, md5_base64)
            .await?;
        let processed = self.wait_for_artwork(&artwork_id, art).await?;
        self.set_profile_art(artist_id, art, Some(&artwork_id))
            .await?;
        Ok(uploaded_file_url(&processed, art))
    }

    /// Clear the profile's picture, banner or logo.
    pub async fn remove_profile_art(&self, artist_id: u64, art: ProfileArt) -> Result<()> {
        self.set_profile_art(artist_id, art, None).await
    }

    async fn set_profile_art(
        &self,
        artist_id: u64,
        art: ProfileArt,
        artwork_id: Option<&str>,
    ) -> Result<()> {
        let params = [("countryCode", self.session().country_code.clone())];
        let data: Vec<Value> = artwork_id
            .map(|id| serde_json::json!({"type": "artworks", "id": id}))
            .into_iter()
            .collect();
        let doc = serde_json::json!({ "data": data });
        let path = format!("artists/{artist_id}/relationships/{}", art.relationship());
        self.patch_openapi(&path, &params, &doc).await
    }

    /// Poll until TIDAL has processed the artwork; answers its final document
    /// (which lists the files made from it). Once processed, a few more polls
    /// wait for the file list if it is not there yet.
    async fn wait_for_artwork(&self, artwork_id: &str, art: ProfileArt) -> Result<Value> {
        let params = [("countryCode", self.session().country_code.clone())];
        let mut processed: Option<Value> = None;
        let mut file_polls = 0;
        for _ in 0..ARTWORK_POLLS {
            let doc: Value = self
                .get_openapi(&format!("artworks/{artwork_id}"), &params)
                .await?;
            let status =
                text(doc.pointer("/data/attributes/sourceFile/status/technicalFileStatus"));
            match status.as_deref() {
                Some("OK") => {
                    if uploaded_file_url(&doc, art).is_some() || file_polls >= ARTWORK_FILE_POLLS {
                        return Ok(doc);
                    }
                    file_polls += 1;
                    processed = Some(doc);
                    tokio::time::sleep(std::time::Duration::from_millis(ARTWORK_POLL_MS)).await;
                }
                Some("UPLOAD_REQUESTED" | "PROCESSING") | None => {
                    tokio::time::sleep(std::time::Duration::from_millis(ARTWORK_POLL_MS)).await;
                }
                Some(other) => {
                    return Err(Error::Api {
                        status: 200,
                        message: format!("TIDAL could not process the picture ({other})"),
                    })
                }
            }
        }
        processed.ok_or_else(|| Error::Api {
            status: 200,
            message: "TIDAL is still processing the picture; try again in a minute".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::SessionInfo;
    use crate::client::TidalClient;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo {
                session_id: "s".into(),
                country_code: "GR".into(),
                user_id: 42,
            },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn my_profile_reads_the_v2_profile() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/profiles/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": 42, "name": "Vee", "handle": null, "picture": "aa-bb",
                "color": ["#112233"], "profileType": "MY_PROFILE", "artistId": 12345678,
                "numberOfFollowers": 3, "numberOfFollows": 7, "prompts": [], "socialLinks": []
            })))
            .expect(1)
            .mount(&server)
            .await;
        let profile = client(&server).my_profile().await.unwrap().unwrap();
        assert_eq!(profile.user_id, 42);
        assert_eq!(profile.name.as_deref(), Some("Vee"));
        assert_eq!(profile.handle, None);
        assert_eq!(profile.picture.as_deref(), Some("aa-bb"));
        assert_eq!(profile.artist_id, Some(12345678));
        assert_eq!(profile.number_of_followers, Some(3));
        assert_eq!(profile.number_of_follows, Some(7));
    }

    #[tokio::test]
    async fn my_profile_tolerates_a_bare_profile() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/profiles/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": "42", "name": "42", "artistId": null
            })))
            .mount(&server)
            .await;
        let profile = client(&server).my_profile().await.unwrap().unwrap();
        assert_eq!(profile.user_id, 42);
        assert_eq!(
            profile.name, None,
            "a name equal to the id is a placeholder"
        );
        assert_eq!(profile.artist_id, None);
    }

    #[tokio::test]
    async fn my_profile_reads_an_object_picture() {
        // Live 2026-11-06: `picture` is an object, which failed the whole read.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/profiles/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": 42, "name": "Vee", "color": ["#112233"],
                "picture": {"url": "https://resources.tidal.com/images/a/b/600x600.jpg"},
                "artistId": 7
            })))
            .mount(&server)
            .await;
        let profile = client(&server).my_profile().await.unwrap().unwrap();
        assert_eq!(profile.name.as_deref(), Some("Vee"));
        assert_eq!(
            profile.picture.as_deref(),
            Some("https://resources.tidal.com/images/a/b/600x600.jpg")
        );
        assert_eq!(profile.artist_id, Some(7));
    }

    #[test]
    fn picture_takes_the_largest_size_or_gives_up_quietly() {
        use super::picture;
        let sizes = serde_json::json!({"100": "https://img/100.jpg", "600": "https://img/600.jpg", "210": "https://img/210.jpg"});
        assert_eq!(
            picture(Some(&sizes)).as_deref(),
            Some("https://img/600.jpg")
        );
        let nested = serde_json::json!({"LARGE": {"url": "https://img/l.jpg"}});
        assert_eq!(picture(Some(&nested)).as_deref(), Some("https://img/l.jpg"));
        assert_eq!(picture(Some(&serde_json::json!({"width": 3}))), None);
        assert_eq!(picture(Some(&serde_json::json!(null))), None);
        assert_eq!(
            picture(Some(&serde_json::json!("aa-bb"))).as_deref(),
            Some("aa-bb")
        );
    }

    #[tokio::test]
    async fn public_playlists_follow_the_cursor_and_carry_followers() {
        use wiremock::matchers::query_param;
        let server = MockServer::start().await;
        let playlist = |uuid: &str| {
            serde_json::json!({
                "followInfo": {"followType": "PLAYLIST", "followed": false, "nrOfFollowers": 4},
                "playlist": {"uuid": uuid, "title": format!("P {uuid}"), "numberOfTracks": 3,
                             "squareImage": "sq", "creator": {"id": 9, "type": "USER"}},
                "profile": {"userId": 9, "name": "Vee"}
            })
        };
        Mock::given(method("GET"))
            .and(path("/v2/user-playlists/9/public"))
            .and(query_param("cursor", "c2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [playlist("b")], "cursor": null
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/user-playlists/9/public"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [playlist("a"), {"broken": true}], "cursor": "c2"
            })))
            .mount(&server)
            .await;
        let list = client(&server).user_public_playlists(9).await.unwrap();
        assert_eq!(
            list.iter().map(|p| p.uuid.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(list[0].number_of_followers, Some(4));
        assert_eq!(list[0].public_playlist, Some(true));
        assert!(!list[0].owned, "user 9's playlist is not the viewer's (42)");
    }

    #[tokio::test]
    async fn editable_profile_reads_name_handle_bio_links_and_picture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"type": "artists", "id": "7",
                    "attributes": {"name": "CheeseMan", "handle": "cheese",
                        "externalLinks": [
                            {"href": "https://instagram.com/c", "meta": {"type": "INSTAGRAM"}},
                            {"href": "https://tidal.com/browse/artist/7", "meta": {"type": "TIDAL_SHARING"}}
                        ]},
                    "relationships": {
                        "biography": {"data": {"type": "artistBiographies", "id": "bio-1"}},
                        "profileArt": {"data": [{"type": "artworks", "id": "art-1"}]},
                        "bannerArt": {"data": [{"type": "artworks", "id": "ban-1"}]},
                        "logoArt": {"data": []}
                    }},
                "included": [
                    {"type": "artistBiographies", "id": "bio-1", "attributes": {"text": "Cheese?"}},
                    {"type": "artworks", "id": "ban-1", "attributes": {"files": [
                        {"href": "https://img/ban/2400x800.jpg", "meta": {"width": 2400, "height": 800}},
                        {"href": "https://img/ban/1200x400.jpg", "meta": {"width": 1200, "height": 400}}
                    ]}}
                ]
            })))
            .mount(&server)
            .await;
        let profile = client(&server).editable_profile(7).await.unwrap();
        assert_eq!(profile.name, "CheeseMan");
        assert_eq!(profile.handle.as_deref(), Some("cheese"));
        assert_eq!(profile.bio.as_deref(), Some("Cheese?"));
        assert_eq!(profile.bio_id.as_deref(), Some("bio-1"));
        assert_eq!(
            profile.links,
            vec![super::ProfileLink {
                href: "https://instagram.com/c".into(),
                kind: "INSTAGRAM".into()
            }]
        );
        assert!(profile.has_picture);
        let banner = profile.banner.unwrap();
        assert_eq!(
            (banner.href.as_str(), banner.width, banner.height),
            ("https://img/ban/1200x400.jpg", 1200, 400)
        );
        assert_eq!(profile.logo, None);
    }

    #[test]
    fn artist_edit_doc_sends_only_changed_fields() {
        let edit = super::ProfileEdit {
            handle: Some(" cheese ".into()),
            ..Default::default()
        };
        let doc = super::artist_edit_doc(7, &edit, true);
        assert_eq!(doc["data"]["type"], "artists");
        assert_eq!(doc["data"]["id"], "7");
        assert_eq!(doc["data"]["attributes"]["handle"], "cheese");
        assert!(doc["data"]["attributes"].get("name").is_none());
        assert!(doc["data"]["attributes"].get("externalLinks").is_none());
        assert_eq!(doc["meta"]["dryRun"], true);
        let links = super::ProfileEdit {
            links: Some(vec![]),
            ..Default::default()
        };
        let doc = super::artist_edit_doc(7, &links, false);
        assert_eq!(
            doc["data"]["attributes"]["externalLinks"],
            serde_json::json!([])
        );
        assert!(doc.get("meta").is_none());
    }

    #[tokio::test]
    async fn update_profile_patches_the_artist_and_reads_tidals_error() {
        use wiremock::matchers::{body_string_contains, header};
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/openapi/v2/artists/7"))
            .and(header("content-type", "application/vnd.api+json"))
            .and(body_string_contains("\"dryRun\":true"))
            .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                "errors": [{"code": "CONFLICT", "detail": "Handle is already taken"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/openapi/v2/artists/7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        let c = client(&server);
        let edit = super::ProfileEdit {
            handle: Some("taken".into()),
            ..Default::default()
        };
        let err = c.update_profile(7, &edit, true).await.unwrap_err();
        assert_eq!(
            super::api_error_detail(&err).as_deref(),
            Some("Handle is already taken")
        );
        c.update_profile(7, &edit, false).await.unwrap();
        // Nothing to change: no request at all.
        c.update_profile(7, &super::ProfileEdit::default(), false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn update_profile_bio_patches_the_biography() {
        use wiremock::matchers::body_string_contains;
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/openapi/v2/artistBiographies/bio-1"))
            .and(body_string_contains("\"text\":\"New bio\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        client(&server)
            .update_profile_bio("bio-1", " New bio ")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn upload_profile_picture_creates_uploads_waits_and_links() {
        use wiremock::matchers::{body_string_contains, header};
        let server = MockServer::start().await;
        let upload = format!("{}/s3/upload", server.uri());
        Mock::given(method("POST"))
            .and(path("/openapi/v2/artworks"))
            .and(body_string_contains("\"md5Hash\":\"abc\""))
            .and(body_string_contains("\"size\":3"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {"id": "art-9", "type": "artworks",
                    "attributes": {"sourceFile": {"uploadLink": {"href": upload}}}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/s3/upload"))
            .and(header("content-type", "image/jpeg"))
            .and(header("content-md5", "q83v"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artworks/art-9"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"id": "art-9", "attributes": {
                    "sourceFile": {"status": {"technicalFileStatus": "OK"}},
                    "files": [
                        {"href": "https://resources.tidal.com/images/ab12/cd34/1280x1280.jpg", "meta": {"width": 1280, "height": 1280}},
                        {"href": "https://resources.tidal.com/images/ab12/cd34/1280x720.jpg", "meta": {"width": 1280, "height": 720}},
                        {"href": "https://resources.tidal.com/images/ab12/cd34/750x750.jpg", "meta": {"width": 750, "height": 750}},
                        {"href": "https://resources.tidal.com/images/ab12/cd34/320x320.jpg", "meta": {"width": 320, "height": 320}}
                    ]
                }}
            })))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/openapi/v2/artists/7/relationships/profileArt"))
            .and(body_string_contains("\"id\":\"art-9\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        let url = client(&server)
            .upload_profile_art(
                7,
                super::ProfileArt::Picture,
                vec![1, 2, 3],
                "image/jpeg",
                "abc",
                "q83v",
            )
            .await
            .unwrap();
        // The square file nearest 750 px, for the app to show at once.
        assert_eq!(
            url.as_deref(),
            Some("https://resources.tidal.com/images/ab12/cd34/750x750.jpg")
        );
    }

    #[tokio::test]
    async fn a_logo_uploads_as_png_and_links_logo_art() {
        use wiremock::matchers::{body_string_contains, header};
        let server = MockServer::start().await;
        let upload = format!("{}/s3/logo", server.uri());
        Mock::given(method("POST"))
            .and(path("/openapi/v2/artworks"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {"id": "art-5", "attributes": {"sourceFile": {"uploadLink": {"href": upload}}}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/s3/logo"))
            .and(header("content-type", "image/png"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artworks/art-5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"id": "art-5", "attributes": {
                    "sourceFile": {"status": {"technicalFileStatus": "OK"}},
                    "files": [
                        {"href": "https://resources.tidal.com/images/l/o/1280x400.png", "meta": {"width": 1280, "height": 400}},
                        {"href": "https://resources.tidal.com/images/l/o/640x200.png", "meta": {"width": 640, "height": 200}}
                    ]
                }}
            })))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/openapi/v2/artists/7/relationships/logoArt"))
            .and(body_string_contains("\"id\":\"art-5\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        let url = client(&server)
            .upload_profile_art(
                7,
                super::ProfileArt::Logo,
                vec![1],
                "image/png",
                "abc",
                "q83v",
            )
            .await
            .unwrap();
        // Any shape: a logo is not a square.
        assert_eq!(
            url.as_deref(),
            Some("https://resources.tidal.com/images/l/o/640x200.png")
        );
    }

    #[tokio::test]
    async fn artist_art_reads_the_picture_banner_and_logo() {
        let server = MockServer::start().await;
        let artwork = |id: &str, w: u64, h: u64| {
            serde_json::json!({
                "type": "artworks", "id": id,
                "attributes": {"mediaType": "IMAGE", "files": [
                    {"href": format!("https://img/{id}/{w}x{h}.jpg"), "meta": {"width": w, "height": h}},
                    {"href": format!("https://img/{id}/160x160.jpg"), "meta": {"width": 160, "height": 160}}
                ]}
            })
        };
        Mock::given(method("GET"))
            .and(path("/openapi/v2/artists/7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"type": "artists", "id": "7", "attributes": {"name": "C"},
                    "relationships": {
                        "profileArt": {"data": [{"type": "artworks", "id": "p"}]},
                        "bannerArt": {"data": [{"type": "artworks", "id": "b"}]},
                        "logoArt": {"data": []}
                    }},
                "included": [artwork("p", 750, 750), artwork("b", 1280, 427)]
            })))
            .mount(&server)
            .await;
        let art = client(&server).artist_art(7).await.unwrap();
        assert_eq!(art.picture.unwrap().href, "https://img/p/750x750.jpg");
        let banner = art.banner.unwrap();
        assert_eq!((banner.width, banner.height), (1280, 427));
        assert_eq!(art.logo, None);
    }

    #[test]
    fn a_banner_shows_its_wide_cut() {
        use super::{nearest_file, ArtFile, ProfileArt};
        // The sizes TIDAL made from the owner's banner (log, 2026-11-07).
        let files = || {
            [
                (1280, 1280),
                (640, 640),
                (320, 320),
                (1280, 720),
                (640, 360),
                (320, 180),
            ]
            .map(|(width, height)| ArtFile {
                href: format!("{width}x{height}"),
                width,
                height,
            })
            .to_vec()
        };
        assert_eq!(
            nearest_file(files(), ProfileArt::Banner).unwrap().href,
            "1280x720"
        );
        // The picture still takes only square files.
        assert_eq!(
            nearest_file(files(), ProfileArt::Picture).unwrap().href,
            "640x640"
        );
        // A banner with only square files still shows one.
        let square = vec![ArtFile {
            href: "s".into(),
            width: 640,
            height: 640,
        }];
        assert_eq!(nearest_file(square, ProfileArt::Banner).unwrap().href, "s");
    }

    #[test]
    fn the_editor_hides_tidals_own_profile_link() {
        use super::{is_tidal_own_link, ProfileLink};
        let link = |href: &str, kind: &str| ProfileLink {
            href: href.into(),
            kind: kind.into(),
        };
        assert!(is_tidal_own_link(&link(
            "https://tidal.com/browse/artist/58912082",
            "TIDAL_SHARING"
        )));
        assert!(is_tidal_own_link(&link(
            "https://tidal.com/browse/artist/58912082",
            "OFFICIAL_HOMEPAGE"
        )));
        assert!(is_tidal_own_link(&link(
            "https://listen.tidal.com/artist/1",
            "OTHER"
        )));
        assert!(!is_tidal_own_link(&link(
            "https://instagram.com/c",
            "INSTAGRAM"
        )));
        assert!(!is_tidal_own_link(&link(
            "https://nottidal.com/x",
            "OFFICIAL_HOMEPAGE"
        )));
    }

    #[tokio::test]
    async fn removing_art_clears_its_relationship() {
        use wiremock::matchers::body_string_contains;
        let server = MockServer::start().await;
        for relationship in ["profileArt", "bannerArt", "logoArt"] {
            Mock::given(method("PATCH"))
                .and(path(format!(
                    "/openapi/v2/artists/7/relationships/{relationship}"
                )))
                .and(body_string_contains("\"data\":[]"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
                .expect(1)
                .mount(&server)
                .await;
        }
        let c = client(&server);
        for art in super::ProfileArt::ALL {
            c.remove_profile_art(7, art).await.unwrap();
        }
    }

    #[tokio::test]
    async fn my_profile_is_none_when_tidal_has_none() {
        for status in [403, 404] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/v2/profiles/42"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            assert_eq!(
                client(&server).my_profile().await.unwrap(),
                None,
                "status {status}"
            );
        }
    }
}
