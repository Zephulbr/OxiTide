//! Everything the probe prints ends up pasted into a public GitHub issue, so
//! every line passes through [`Scrubber::scrub`] and device messages are logged
//! through an allowlist ([`summarize`]) instead of verbatim.

use std::sync::RwLock;

use serde_json::{Map, Value};

/// Replaces known secret values (token, user id, session ids) and anything
/// that looks like a bearer token, a JWT or a country code parameter.
#[derive(Default)]
pub struct Scrubber {
    secrets: RwLock<Vec<(String, &'static str)>>,
}

impl Scrubber {
    /// Registers a literal value to replace with `<label>`. Values shorter than
    /// four characters are ignored: replacing them would mangle ordinary text.
    pub fn add(&self, value: &str, label: &'static str) {
        let value = value.trim();
        if value.len() < 4 {
            return;
        }
        let mut secrets = self.secrets.write().unwrap();
        if secrets.iter().any(|(v, _)| v == value) {
            return;
        }
        secrets.push((value.to_string(), label));
        // Longest first, so a value that contains another is replaced whole.
        secrets.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
    }

    pub fn scrub(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (value, label) in self.secrets.read().unwrap().iter() {
            if out.contains(value.as_str()) {
                out = out.replace(value.as_str(), &format!("<{label}>"));
            }
        }
        let out = mask_after(&out, "Bearer ", "<token>");
        let out = mask_jwts(&out);
        let out = mask_after(&out, "countryCode=", "<cc>");
        let out = mask_after(&out, "\"countryCode\":\"", "<cc>");
        mask_after(&out, "\\\"countryCode\\\":\\\"", "<cc>")
    }
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '/' | '=')
}

/// Replaces the run of token characters after each `marker` with `replacement`.
fn mask_after(text: &str, marker: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos + marker.len()]);
        rest = &rest[pos + marker.len()..];
        let end = rest.find(|c: char| !is_token_char(c)).unwrap_or(rest.len());
        if end > 0 && !rest[..end].starts_with('<') {
            out.push_str(replacement);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

/// TIDAL access tokens are JWTs: `eyJ…` followed by two more dot-separated
/// base64url parts. Masks any such run of 20+ characters.
fn mask_jwts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("eyJ") {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let end = tail.find(|c: char| !is_token_char(c)).unwrap_or(tail.len());
        if end >= 20 && tail[..end].contains('.') {
            out.push_str("<jwt>");
        } else {
            out.push_str(&tail[..end.max(3)]);
        }
        rest = &tail[end.max(3)..];
    }
    out.push_str(rest);
    out
}

/// Top-level keys of a device message that are logged as they are.
const PLAIN_KEYS: &[&str] = &[
    "command",
    "requestId",
    "resultCode",
    "playerState",
    "progress",
    "duration",
    "state",
    "joined",
    "sessionId",
    "errorCode",
    "hasNextMedia",
    "shuffle",
    "repeatMode",
    "position",
    "level",
    "mute",
    "volume",
    "type",
    "result",
    "reason",
];

/// An allowlisted view of a message from a device or the receiver binary.
/// Values outside the allowlist are dropped; their key names are kept so the
/// shape of an unexpected message is still visible.
pub fn summarize(msg: &Value) -> Value {
    let Some(obj) = msg.as_object() else {
        return Value::String(format!("<non-object {}>", type_name(msg)));
    };
    let mut out = Map::new();
    let mut other = Vec::new();
    for (key, value) in obj {
        match key.as_str() {
            k if PLAIN_KEYS.contains(&k) => {
                out.insert(key.clone(), plain(value));
            }
            "mediaInfo" | "currentMediaInfo" => {
                out.insert(key.clone(), media_info(value));
            }
            "queueInfo" => {
                out.insert(
                    key.clone(),
                    pick(value, &["queueId", "repeatMode", "shuffled"]),
                );
            }
            // Server errors are the evidence when a device rejects the
            // login, so they are kept whole (the log scrubs and then caps
            // each line). TIDAL's controller `JSON.parse`s
            // `details.description`; parsing it here too keeps its quotes
            // unescaped, so the scrubber's patterns still match inside it.
            "details" | "error" | "message" | "description" => {
                out.insert(key.clone(), unnest(value));
            }
            _ => other.push(key.clone()),
        }
    }
    if !other.is_empty() {
        out.insert(
            "otherKeys".into(),
            Value::Array(other.into_iter().map(Value::String).collect()),
        );
    }
    Value::Object(out)
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Scalars pass; nested values are replaced by their type.
fn plain(value: &Value) -> Value {
    match value {
        Value::Object(_) => pick(
            value,
            &["level", "mute", "state", "code", "progress", "duration"],
        ),
        Value::Array(a) => Value::String(format!("<array of {}>", a.len())),
        other => other.clone(),
    }
}

fn pick(value: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    if let Some(obj) = value.as_object() {
        for key in keys {
            if let Some(v) = obj.get(*key) {
                if !v.is_object() && !v.is_array() {
                    out.insert((*key).to_string(), v.clone());
                }
            }
        }
    }
    Value::Object(out)
}

fn media_info(value: &Value) -> Value {
    let mut out = match pick(value, &["itemId", "mediaId", "mediaType"]) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if let Some(title) = value.pointer("/metadata/title").and_then(Value::as_str) {
        out.insert("title".into(), Value::String(title.to_string()));
    }
    if value.get("customData").is_some() {
        out.insert("customData".into(), Value::String("<omitted>".into()));
    }
    Value::Object(out)
}

/// Replaces string values that hold JSON with the parsed JSON, recursively.
fn unnest(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            let t = s.trim_start();
            if t.starts_with('{') || t.starts_with('[') {
                if let Ok(inner) = serde_json::from_str::<Value>(s) {
                    return unnest(&inner);
                }
            }
            value.clone()
        }
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), unnest(v))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(unnest).collect()),
        other => other.clone(),
    }
}

/// Caps a line's length (after scrubbing, so no secret is cut in half first).
pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{cut}… (truncated)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJ1aWQiOjEyMzQ1Njc4OX0.c2lnbmF0dXJlX3BhcnQ";

    #[test]
    fn registered_values_are_replaced() {
        let s = Scrubber::default();
        s.add("123456789", "user");
        s.add("secret-access-token-value", "token");
        let line = "user 123456789 sent secret-access-token-value twice: 123456789";
        assert_eq!(s.scrub(line), "user <user> sent <token> twice: <user>");
    }

    #[test]
    fn short_values_are_not_registered() {
        let s = Scrubber::default();
        s.add("ZZ", "cc");
        assert_eq!(s.scrub("ZZTOP"), "ZZTOP");
    }

    #[test]
    fn bearer_jwt_and_country_patterns_are_masked() {
        let s = Scrubber::default();
        let line = format!(
            "Authorization: Bearer abc.def-123 url=?countryCode=ZZ&locale=en_US {JWT} {{\"countryCode\":\"ZZ\"}}"
        );
        let out = s.scrub(&line);
        assert_eq!(
            out,
            "Authorization: Bearer <token> url=?countryCode=<cc>&locale=en_US <jwt> {\"countryCode\":\"<cc>\"}"
        );
    }

    #[test]
    fn an_unrelated_eyj_word_survives() {
        let s = Scrubber::default();
        assert_eq!(s.scrub("eyJx short"), "eyJx short");
    }

    #[test]
    fn summarize_keeps_allowlisted_values_only() {
        let msg = json!({
            "command": "notifyMediaChanged",
            "requestId": 3,
            "mediaInfo": {
                "itemId": "q1", "mediaId": "58990486", "mediaType": 0,
                "metadata": {"title": "Song", "artists": ["A"]},
                "customData": "{\"countryCode\":\"ZZ\",\"userId\":123456789}"
            },
            "contentServerInfo": {"authInfo": {"headerAuth": "Bearer secret"}},
        });
        let out = summarize(&msg);
        let text = out.to_string();
        assert!(!text.contains("secret"));
        assert!(!text.contains("123456789"));
        assert!(!text.contains("ZZ"));
        assert_eq!(out["command"], "notifyMediaChanged");
        assert_eq!(out["mediaInfo"]["mediaId"], "58990486");
        assert_eq!(out["mediaInfo"]["title"], "Song");
        assert_eq!(out["otherKeys"], json!(["contentServerInfo"]));
    }

    /// The failure path on a real device: a server error whose
    /// `details.description` is a JSON string carrying request details.
    #[test]
    fn a_server_error_with_nested_json_does_not_leak() {
        let s = Scrubber::default();
        s.add(JWT, "token");
        s.add("123456789", "user-id");
        let description = json!({
            "status": 401,
            "subStatus": 11003,
            "userMessage": "Token could not be switched",
            "request": {
                "url": "https://api.tidal.com/v1/tracks/1/playbackinfo?countryCode=ZZ&audioquality=LOSSLESS",
                "headers": {"Authorization": format!("Bearer {JWT}")},
                "body": {"countryCode": "ZZ", "userId": 123456789, "access_token": JWT},
            }
        })
        .to_string();
        let msg = json!({
            "command": "notifyContentServerError",
            "errorCode": 3,
            "details": {"description": description, "code": 401},
        });
        let line = s.scrub(&format!("<- {}", summarize(&msg)));
        assert!(!line.contains("eyJ"), "{line}");
        assert!(!line.contains("123456789"), "{line}");
        assert!(!line.contains("ZZ"), "{line}");
        assert!(line.contains("subStatus"), "{line}");
        assert!(line.contains("11003"), "{line}");
        assert!(line.contains("Token could not be switched"), "{line}");
    }

    #[test]
    fn an_escaped_country_code_is_masked() {
        let s = Scrubber::default();
        assert_eq!(
            s.scrub(r#"{\"countryCode\":\"ZZ\"}"#),
            r#"{\"countryCode\":\"<cc>\"}"#
        );
    }

    #[test]
    fn nested_plain_values_do_not_leak() {
        let msg = json!({"state": {"token": "secret", "code": 2}});
        let out = summarize(&msg).to_string();
        assert!(!out.contains("secret"));
        assert!(out.contains("\"code\":2"));
    }
}
