use crate::client::TidalClient;
use crate::error::Result;
use crate::models::ItemPage;
use crate::pages::{page_item_from_value, PageItem};

impl TidalClient {
    /// The user's purchase history. The response shape is unconfirmed; the
    /// permissive mapping accepts wrapped or bare items and degrades to empty
    /// when nothing maps.
    pub async fn purchases(&self, limit: u32, offset: u32) -> Result<(Vec<PageItem>, u32)> {
        let page: ItemPage<serde_json::Value> = self
            .get(
                &format!("users/{}/purchases", self.session().user_id),
                &[("limit", limit.to_string()), ("offset", offset.to_string())],
            )
            .await?;
        let total = page.total_number_of_items;
        let items = page.items.iter().filter_map(page_item_from_value).collect();
        Ok((items, total))
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
    async fn purchases_accepts_wrapped_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42/purchases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {"item": {"id": 7, "title": "Bought Album", "type": "ALBUM", "cover": "c-7"}}
                ],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server)
            .await;
        let (items, total) = client(&server).purchases(100, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].id, "7");
        assert_eq!(items[0].item_type, "ALBUM");
        assert_eq!(items[0].artwork.as_deref(), Some("c-7"));
    }

    #[tokio::test]
    async fn purchases_accepts_bare_items_with_uuids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42/purchases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{"uuid": "p-1", "title": "Bought Playlist", "type": "PLAYLIST"}],
                "totalNumberOfItems": 1, "limit": 100, "offset": 0
            })))
            .mount(&server)
            .await;
        let (items, _) = client(&server).purchases(100, 0).await.unwrap();
        assert_eq!(items[0].id, "p-1");
    }
}
