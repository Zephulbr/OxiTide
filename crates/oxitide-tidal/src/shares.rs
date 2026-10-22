//! Playlist collaboration through TIDAL shares (public OpenAPI `shares`): an
//! invitation link offers the COLLABORATOR role, and the members come from the
//! internal v2 playlist (OpenAPI `collaborators` as the fallback).

use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaylistInvite {
    pub share_id: String,
    pub code: String,
    pub expires_at: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaylistMember {
    pub id: u64,
    pub name: String,
    /// Image uuid, when TIDAL sends one (it has not been seen yet).
    pub picture: Option<String>,
    pub owner: bool,
}

/// The link the official app copies:
/// `https://tidal.com/playlist/{uuid}/invite?code={code}`.
pub fn invite_url(uuid: &str, code: &str) -> String {
    format!("https://tidal.com/playlist/{uuid}/invite?code={code}")
}

#[derive(Deserialize)]
struct RawShareDoc {
    data: RawShare,
}

#[derive(Deserialize)]
struct RawShare {
    id: String,
    #[serde(default)]
    attributes: RawShareAttributes,
}

#[derive(Deserialize, Default)]
struct RawShareAttributes {
    #[serde(default)]
    code: String,
    #[serde(rename = "expiresAt", default)]
    expires_at: Option<String>,
    #[serde(rename = "externalLinks", default)]
    external_links: Vec<RawExternalLink>,
}

#[derive(Deserialize)]
struct RawExternalLink {
    #[serde(default)]
    href: String,
    #[serde(default)]
    meta: Option<RawLinkMeta>,
}

#[derive(Deserialize)]
struct RawLinkMeta {
    #[serde(rename = "type", default)]
    kind: String,
}

/// A user reference in any of the shapes TIDAL uses: `id` or `userId`,
/// `name` or `username`, optional `picture`.
fn member_from(value: &serde_json::Value, owner: bool) -> Option<PlaylistMember> {
    let id = ["id", "userId"]
        .iter()
        .find_map(|key| match value.get(*key)? {
            serde_json::Value::Number(n) => n.as_u64(),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        })?;
    let name = ["name", "username", "nickname"]
        .iter()
        .find_map(|key| value.get(*key)?.as_str().filter(|s| !s.is_empty()))
        .unwrap_or("TIDAL user")
        .to_string();
    let picture = ["picture", "image", "profilePicture"]
        .iter()
        .find_map(|key| value.get(*key)?.as_str().filter(|s| !s.is_empty()))
        .map(str::to_string);
    Some(PlaylistMember {
        id,
        name,
        picture,
        owner,
    })
}

/// Owner (from `profile`, else `playlist.creator`) then each collaborator.
fn members_from_v2(doc: &serde_json::Value) -> Option<Vec<PlaylistMember>> {
    let playlist = doc.get("playlist")?;
    let owner = doc
        .get("profile")
        .and_then(|profile| member_from(profile, true))
        .or_else(|| member_from(playlist.get("creator")?, true))?;
    let mut members = vec![owner];
    for collaborator in playlist
        .get("collaborators")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        // A nested `{profile: {…}}` / `{user: {…}}` wrapper is accepted too.
        let inner = collaborator
            .get("profile")
            .or_else(|| collaborator.get("user"))
            .unwrap_or(collaborator);
        if let Some(member) = member_from(inner, false) {
            if !members.iter().any(|known| known.id == member.id) {
                members.push(member);
            }
        }
    }
    Some(members)
}

/// Collaborators from the OpenAPI relationship document (`include=collaborators`).
fn members_from_openapi(doc: &serde_json::Value) -> Vec<PlaylistMember> {
    let included = doc
        .get("included")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    doc.get("data")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reference| {
            let id = reference.get("id")?.as_str()?;
            let attributes = included
                .iter()
                .find(|resource| resource.get("id").and_then(|v| v.as_str()) == Some(id))
                .and_then(|resource| resource.get("attributes"));
            let mut value = attributes.cloned().unwrap_or_else(|| serde_json::json!({}));
            value["id"] = serde_json::Value::String(id.to_string());
            member_from(&value, false)
        })
        .collect()
}

impl TidalClient {
    /// Create a reusable COLLABORATOR invitation for an owned playlist. New
    /// shares expire seven days after creation.
    pub async fn create_playlist_invite(&self, uuid: &str) -> Result<PlaylistInvite> {
        let doc = serde_json::json!({
            "data": {
                "type": "shares",
                "attributes": {"role": "COLLABORATOR"},
                "relationships": {
                    "sharedResources": {"data": [{"type": "playlists", "id": uuid}]}
                }
            }
        });
        let params = [("countryCode", self.session().country_code.clone())];
        let raw: RawShareDoc = self.post_openapi("shares", &params, &doc).await?;
        let attributes = raw.data.attributes;
        // TIDAL sends no link today; prefer one if it ever does.
        let url = attributes
            .external_links
            .iter()
            .find(|link| {
                link.meta
                    .as_ref()
                    .is_some_and(|meta| meta.kind == "TIDAL_SHARING")
                    && link.href.starts_with("https://")
            })
            .map(|link| link.href.clone())
            .unwrap_or_else(|| invite_url(uuid, &attributes.code));
        Ok(PlaylistInvite {
            share_id: raw.data.id,
            code: attributes.code,
            expires_at: attributes.expires_at,
            url,
        })
    }

    /// Delete a share: its link stops working. People who already joined keep
    /// access (TIDAL cannot revoke grants yet).
    pub async fn delete_share(&self, share_id: &str) -> Result<()> {
        self.delete_openapi(&format!("shares/{share_id}"), &[], None)
            .await
    }

    /// The owner, then the collaborators.
    pub async fn playlist_members(&self, uuid: &str) -> Result<Vec<PlaylistMember>> {
        let params = [("countryCode", self.session().country_code.clone())];
        let v2 = self
            .get_v2::<serde_json::Value>(&format!("user-playlists/{uuid}"), &params)
            .await;
        match v2.as_ref().ok().and_then(members_from_v2) {
            Some(members) => Ok(members),
            None => {
                if let Err(e) = &v2 {
                    tracing::warn!("playlist members via v2 failed ({e}); using OpenAPI");
                }
                let params = [
                    ("countryCode", self.session().country_code.clone()),
                    ("include", "collaborators".to_string()),
                ];
                let doc: serde_json::Value = self
                    .get_openapi(
                        &format!("playlists/{uuid}/relationships/collaborators"),
                        &params,
                    )
                    .await?;
                Ok(members_from_openapi(&doc))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{body_string_contains, method, path};
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

    #[test]
    fn invite_url_matches_the_official_app() {
        assert_eq!(
            invite_url("p-1", "c-1"),
            "https://tidal.com/playlist/p-1/invite?code=c-1"
        );
    }

    #[tokio::test]
    async fn create_playlist_invite_posts_a_collaborator_share_and_builds_the_link() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openapi/v2/shares"))
            .and(body_string_contains("\"role\":\"COLLABORATOR\""))
            .and(body_string_contains("\"id\":\"p-1\""))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {"id": "s-1", "type": "shares", "attributes": {
                    "code": "c-1", "role": "COLLABORATOR",
                    "createdAt": "2026-10-22T16:09:41Z", "expiresAt": "2026-10-29T16:09:41Z"
                }},
                "links": {"self": "/shares/s-1"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let invite = client(&server).create_playlist_invite("p-1").await.unwrap();
        assert_eq!(invite.share_id, "s-1");
        assert_eq!(invite.url, "https://tidal.com/playlist/p-1/invite?code=c-1");
        assert_eq!(invite.expires_at.as_deref(), Some("2026-10-29T16:09:41Z"));
    }

    #[tokio::test]
    async fn create_playlist_invite_prefers_a_sharing_link_from_tidal() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openapi/v2/shares"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {"id": "s-1", "type": "shares", "attributes": {
                    "code": "c-1",
                    "externalLinks": [
                        {"href": "https://tidal.com/x/c-1", "meta": {"type": "TIDAL_SHARING"}}
                    ]
                }}
            })))
            .mount(&server)
            .await;
        let invite = client(&server).create_playlist_invite("p-1").await.unwrap();
        assert_eq!(invite.url, "https://tidal.com/x/c-1");
    }

    #[tokio::test]
    async fn delete_share_targets_the_share() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/openapi/v2/shares/s-1"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        client(&server).delete_share("s-1").await.unwrap();
    }

    #[tokio::test]
    async fn playlist_members_reads_the_owner_and_collaborators_from_v2() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/user-playlists/p-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "playlist": {
                    "creator": {"id": 42, "name": "Creator", "type": "USER"},
                    "collaborators": [
                        {"id": 7, "name": "Friend", "picture": "pic-7"},
                        {"profile": {"userId": 8, "name": "Nested"}},
                        {"id": 42, "name": "Owner again"}
                    ]
                },
                "profile": {"userId": 42, "name": "CheeseMan", "color": []}
            })))
            .mount(&server)
            .await;
        let members = client(&server).playlist_members("p-1").await.unwrap();
        assert_eq!(members.len(), 3, "the owner is not listed twice");
        assert_eq!(
            members[0],
            PlaylistMember {
                id: 42,
                name: "CheeseMan".into(),
                picture: None,
                owner: true
            }
        );
        assert_eq!(members[1].picture.as_deref(), Some("pic-7"));
        assert_eq!(members[2].name, "Nested");
    }

    #[tokio::test]
    async fn playlist_members_falls_back_to_openapi_collaborators() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/user-playlists/p-1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/openapi/v2/playlists/p-1/relationships/collaborators",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "7", "type": "users"}],
                "included": [{"id": "7", "type": "users", "attributes": {"username": "friend"}}],
                "links": {"self": "/x"}
            })))
            .mount(&server)
            .await;
        let members = client(&server).playlist_members("p-1").await.unwrap();
        assert_eq!(
            members,
            vec![PlaylistMember {
                id: 7,
                name: "friend".into(),
                picture: None,
                owner: false
            }]
        );
    }
}
