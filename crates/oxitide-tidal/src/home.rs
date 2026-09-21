use serde::{Deserialize, Serialize};

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default = "default_kind")]
    pub item_type: String,
}

fn default_kind() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeSection {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<HomeItem>,
}

#[derive(Debug, Deserialize)]
struct RawRow {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub modules: Vec<RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(default)]
    #[allow(dead_code)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<RawItem>,
    #[serde(default, rename = "pagedList")]
    pub paged_list: Option<RawPagedList>,
}

#[derive(Debug, Deserialize)]
struct RawPagedList {
    #[serde(default)]
    pub items: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(rename = "type", default)]
    pub item_type: String,
}

#[derive(Debug, Deserialize)]
struct RawPage {
    #[serde(default)]
    pub rows: Vec<RawRow>,
}

impl TidalClient {
    pub async fn home_page(&self) -> Result<Vec<HomeSection>> {
        let raw: RawPage = self.get("pages/home", &[]).await?;
        let sections = raw
            .rows
            .into_iter()
            .map(|row| {
                let items: Vec<HomeItem> = row
                    .modules
                    .into_iter()
                    .flat_map(|m| {
                        let mut items = m.items;
                        if let Some(paged) = m.paged_list {
                            items.extend(paged.items);
                        }
                        items.into_iter()
                    })
                    .map(|i| HomeItem {
                        id: value_to_id(&i.id),
                        title: i.title,
                        subtitle: i.subtitle,
                        item_type: i.item_type,
                    })
                    .collect();
                HomeSection { title: row.title, items }
            })
            .filter(|s| !s.items.is_empty())
            .collect();
        Ok(sections)
    }
}

fn value_to_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(), server.uri(), format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn home_page_flattens_rows_and_skips_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"title": "For You", "modules": [
                        {"title": "Mixes", "items": [
                            {"id": "m1", "title": "Daily Mix", "subtitle": "Made for you", "type": "MIX"}
                        ]}
                    ]},
                    {"title": "Empty", "modules": [{"title": "x", "items": []}]}
                ]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "For You");
        assert_eq!(sections[0].items[0].id, "m1");
        assert_eq!(sections[0].items[0].item_type, "MIX");
    }

    #[tokio::test]
    async fn home_page_reads_paged_list_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [
                    {"title": "For You", "modules": [
                        {"title": "Paged", "pagedList": {"items": [
                            {"id": 11, "title": "Paged Item", "type": "ALBUM"}
                        ]}}
                    ]}
                ]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].items.len(), 1);
        assert_eq!(sections[0].items[0].title, "Paged Item");
        assert_eq!(sections[0].items[0].item_type, "ALBUM");
    }

    #[tokio::test]
    async fn home_page_with_no_rows_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "rows": [] })))
            .mount(&server).await;
        assert!(client(&server).home_page().await.unwrap().is_empty());
    }
}
