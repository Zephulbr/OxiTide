//! The HTTP side: OxiTide's saved login (read-only), the account session, a
//! few tracks, and the cloud queue the receiver plays from.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

pub const API: &str = "https://api.tidal.com/v1";
pub const QUEUE_API: &str = "https://connectqueue.tidal.com/v1";
/// What TIDAL desktop sends the receiver (`serverInfo.js` strips the trailing
/// slash from `serverUrl` only, so the auth URL keeps its slash).
pub const AUTH_SERVER_URL: &str = "https://auth.tidal.com/v1/oauth2/token/";
pub const CONTENT_SERVER_URL: &str = "https://api.tidal.com/v1";
pub const QUEUE_SERVER_URL: &str = "https://connectqueue.tidal.com/v1/queues";

/// Minimum lifetime left on the saved access token. The probe never refreshes
/// it (that would write a new login that OxiTide does not know about).
const MIN_TOKEN_SECONDS: i64 = 15 * 60;

#[derive(Deserialize)]
struct SavedTokens {
    access_token: String,
    #[serde(default)]
    expires_at: Option<i64>,
}

pub struct Login {
    pub access_token: String,
    pub minutes_left: Option<i64>,
}

/// Reads OxiTide's saved login from the OS keyring (service `oxitide`, user
/// `tidal`, as OxiTide 1.x writes it). Never writes.
pub fn load_login(service: &str) -> Result<Login> {
    let entry = keyring::Entry::new(service, "tidal").context("keyring unavailable")?;
    let json = match entry.get_password() {
        Ok(json) => json,
        Err(keyring::Error::NoEntry) => bail!(
            "no OxiTide login found (keyring service '{service}'). Open OxiTide, sign in, then rerun."
        ),
        Err(e) => bail!("could not read OxiTide's saved login: {e}"),
    };
    let saved: SavedTokens =
        serde_json::from_str(&json).context("OxiTide's saved login has an unexpected format")?;
    let now = unix_now();
    let minutes_left = saved.expires_at.map(|at| (at - now) / 60);
    if let Some(at) = saved.expires_at {
        if at - now < MIN_TOKEN_SECONDS {
            bail!(
                "OxiTide's saved login expires too soon ({} min left). Open OxiTide and let it \
                 load (it renews the login on start), close it again, then rerun.",
                (at - now).max(0) / 60
            );
        }
    }
    Ok(Login {
        access_token: saved.access_token,
        minutes_left,
    })
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    #[serde(rename = "userId")]
    pub user_id: u64,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

pub async fn fetch_session(http: &reqwest::Client, token: &str) -> Result<Session> {
    let resp = http
        .get(format!("{API}/sessions"))
        .bearer_auth(token)
        .send()
        .await?;
    let body = expect_ok(resp, "GET /v1/sessions").await?;
    Ok(serde_json::from_value(body)?)
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: u64,
    pub title: String,
    pub artists: Vec<String>,
    pub album_id: u64,
    pub album_title: String,
    pub cover: Option<String>,
    pub duration_s: u64,
}

impl Track {
    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v.get("id")?.as_u64()?,
            title: v.get("title")?.as_str()?.to_string(),
            artists: v
                .get("artists")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            album_id: v.pointer("/album/id").and_then(Value::as_u64).unwrap_or(0),
            album_title: v
                .pointer("/album/title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            cover: v
                .pointer("/album/cover")
                .and_then(Value::as_str)
                .map(str::to_string),
            duration_s: v.get("duration").and_then(Value::as_u64).unwrap_or(0),
        })
    }

    fn streamable(v: &Value) -> bool {
        v.get("allowStreaming").and_then(Value::as_bool) != Some(false)
            && v.get("streamReady").and_then(Value::as_bool) != Some(false)
    }

    /// The `images` object TIDAL's web player builds for Connect metadata.
    pub fn images(&self) -> Value {
        let Some(cover) = &self.cover else {
            return Value::Null;
        };
        let base = format!(
            "https://resources.tidal.com/images/{}",
            cover.replace('-', "/")
        );
        let size =
            |px: u32| json!({"height": px, "url": format!("{base}/{px}x{px}.jpg"), "width": px});
        json!({"high": size(1280), "low": size(320), "medium": size(640)})
    }
}

/// Three streamable tracks: the ids given, or the first hits of a search.
pub async fn pick_tracks(
    http: &reqwest::Client,
    token: &str,
    country: &str,
    ids: &[u64],
) -> Result<Vec<Track>> {
    if !ids.is_empty() {
        let mut tracks = Vec::new();
        for id in ids {
            let resp = http
                .get(format!("{API}/tracks/{id}"))
                .query(&[("countryCode", country)])
                .bearer_auth(token)
                .send()
                .await?;
            let body = expect_ok(resp, "GET /v1/tracks/{id}").await?;
            tracks.push(Track::from_json(&body).context("unexpected track shape")?);
        }
        return Ok(tracks);
    }
    let resp = http
        .get(format!("{API}/search/tracks"))
        .query(&[
            ("query", "Daft Punk"),
            ("limit", "20"),
            ("countryCode", country),
        ])
        .bearer_auth(token)
        .send()
        .await?;
    let body = expect_ok(resp, "GET /v1/search/tracks").await?;
    let tracks: Vec<Track> = body
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|v| Track::streamable(v))
                .filter_map(Track::from_json)
                .take(3)
                .collect()
        })
        .unwrap_or_default();
    if tracks.len() < 3 {
        bail!("search returned fewer than three streamable tracks; rerun with --tracks <id,id,id>");
    }
    Ok(tracks)
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: String,
    pub media_id: String,
}

#[derive(Debug, Clone)]
pub struct CloudQueue {
    pub id: String,
    pub items: Vec<QueueItem>,
    pub position: usize,
    pub repeat_mode: String,
    pub shuffled: bool,
}

fn queue_params(country: &str) -> [(&'static str, String); 3] {
    [
        ("countryCode", country.to_string()),
        ("locale", "en_US".to_string()),
        ("deviceType", "DESKTOP".to_string()),
    ]
}

fn as_string(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// `POST /v1/queues`, with the body TIDAL's web player sends.
pub async fn create_queue(
    http: &reqwest::Client,
    token: &str,
    country: &str,
    tracks: &[Track],
) -> Result<CloudQueue> {
    let items: Vec<Value> = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            json!({
                "media_id": t.id.to_string(),
                "properties": {
                    "active": "false",
                    "original_order": i.to_string(),
                    "source_id": t.album_id.to_string(),
                    "source_type": "ALBUM",
                },
                "type": "track",
            })
        })
        .collect();
    let body = json!({
        "items": items,
        "properties": {"position": 0},
        "repeat_mode": "off",
        "shuffled": false,
    });
    let resp = http
        .post(format!("{QUEUE_API}/queues"))
        .query(&queue_params(country))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;
    let v = expect_ok(resp, "POST connectqueue /v1/queues").await?;
    let id = as_string(v.get("id"));
    if id.is_empty() {
        bail!("cloud queue response has no id");
    }
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|it| QueueItem {
                    id: as_string(it.get("id")),
                    media_id: as_string(it.get("media_id")),
                })
                .collect()
        })
        .unwrap_or_default();
    let position = as_string(v.pointer("/properties/position"))
        .parse()
        .unwrap_or(0);
    Ok(CloudQueue {
        id,
        items,
        position,
        repeat_mode: v
            .get("repeat_mode")
            .and_then(Value::as_str)
            .unwrap_or("off")
            .to_string(),
        shuffled: v.get("shuffled").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// `GET` the queue for its ETag, then `DELETE` it with `If-Match`.
pub async fn delete_queue(
    http: &reqwest::Client,
    token: &str,
    country: &str,
    id: &str,
) -> Result<()> {
    let url = format!("{QUEUE_API}/queues/{id}");
    let resp = http
        .get(&url)
        .query(&queue_params(country))
        .bearer_auth(token)
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    expect_ok(resp, "GET connectqueue queue").await?;
    let mut req = http
        .delete(&url)
        .query(&queue_params(country))
        .bearer_auth(token);
    if let Some(etag) = etag {
        req = req.header(reqwest::header::IF_MATCH, etag);
    }
    let resp = req.send().await?;
    if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
        Ok(())
    } else {
        expect_ok(resp, "DELETE connectqueue queue")
            .await
            .map(|_| ())
    }
}

async fn expect_ok(resp: reqwest::Response, what: &str) -> Result<Value> {
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let snippet: String = text.chars().take(300).collect();
        bail!("{what} -> HTTP {status}: {snippet}");
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).with_context(|| format!("{what}: response is not JSON"))
}
