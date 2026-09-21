# OxiTide Plan 2 — Playback Engine & Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure-Rust playback pipeline (resolve → fetch → demux → decode → WASAPI shared output) and a Rust-owned queue, then wire the now-playing bar, queue rail, play actions, and a minimal settings slice.

**Architecture:** A new `oxitide-audio` crate holds the engine, isolated behind traits (`Fetcher`, `AudioOutput`, `StreamResolver`, `PcmSink`) so every piece is testable headless. `oxitide-app` adapts the engine to Tauri commands/events; the Svelte UI renders events and never owns playback state.

**Tech Stack:** Rust 2021, stable toolchain, Tokio, reqwest (+ `blocking`), quick-xml, symphonia 0.6, rtrb, wasapi 0.24; Tauri 2; Svelte 5 + Vite + TypeScript; Vitest.

**Spec:** `docs/superpowers/specs/2026-09-21-oxitide-plan2-playback-design.md`

## Global Constraints

- Rust edition 2021; toolchain pinned by `rust-toolchain.toml` (`stable`). Node 20+. Tauri 2.x. Svelte 5 runes.
- TIDAL API base: `https://api.tidal.com/v1/`. Auth header: `Authorization: Bearer <access_token>` (handled by `TidalClient`).
- Quality parameter values: `LOW`, `HIGH`, `LOSSLESS`, `HI_RES_LOSSLESS`. Pubic quality names: `low`, `high`, `lossless`, `max`.
- **No DRM, no key extraction, no credential/token logging.** Tokens stay in the OS keyring.
- Config/app dir: `%APPDATA%\OxiTide`. Keyring service name: `oxitide`.
- Async work runs on a Tokio multi-thread runtime (Tauri's). Blocking audio work runs on dedicated `std::thread`s.
- Conventional commits (`feat:`, `test:`, `chore:`); commit at the end of every task.
- Every HTTP-facing test uses `wiremock`; every parser test uses an inline fixture. No live network in tests. Audio tests use committed generated-tone fixtures and a `NullOutput`.
- `oxitide-audio` must not depend on Tauri or any UI code.
- Verified dependency versions (checked against crates.io): `quick-xml = "0.42"`, `symphonia = "0.6"` (`flac`, `isomp4`, `pcm` features), `rtrb = "0.4"`, `wasapi = "0.24"`, `base64 = "0.22"`.

## Review Focus

Uncovered input classes most likely to bite a real user; each is pinned to a test in its owning task:

1. **Stream stalls or drops mid-track** — the downloader thread must surface a fetch error and the decode loop must stop without deadlocking the worker; the UI shows `engine_error`. (Tasks 5, 12)
2. **Empty queue / play at the last track** — `next`/`prev`/auto-advance at the bounds must not panic and must stop cleanly when repeat is `off`. (Task 11)
3. **Quality unavailable / unexpected manifest shape** — an unknown `manifestMimeType`, malformed MPD, or non-200 must map to a typed error, never a panic. (Tasks 2, 3, 4)
4. **No output device / device open failure** — `WasapiOutput::open` must return a typed error the engine reports, not panic. (Tasks 9, 10)
5. **Track with missing metadata** — a queue item with no cover/album/artist must serialize and render without breaking the UI. (Tasks 11, 14)

---

### Task 1: `oxitide-audio` crate skeleton and error type

**Files:**
- Create: `crates/oxitide-audio/Cargo.toml`
- Create: `crates/oxitide-audio/src/lib.rs`
- Create: `crates/oxitide-audio/src/error.rs`
- Modify: `Cargo.toml` (workspace members + dependencies)

**Interfaces:**
- Consumes: nothing.
- Produces: `oxitide_audio::{Error, Result}`; `Error` variants used by every later task.

- [ ] **Step 1: Add the crate to the workspace and declare dependencies**

`Cargo.toml` (workspace) — change `members` and add workspace deps:
```toml
[workspace]
resolver = "2"
members = ["crates/oxitide-tidal", "crates/oxitide-audio", "crates/oxitide-app"]

[workspace.dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
url = "2"
quick-xml = "0.42"
symphonia = { version = "0.6", features = ["flac", "isomp4", "pcm"] }
rtrb = "0.4"
base64 = "0.22"
```

`crates/oxitide-audio/Cargo.toml`:
```toml
[package]
name = "oxitide-audio"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
oxitide-tidal = { path = "../oxitide-tidal" }
tokio.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
quick-xml.workspace = true
symphonia.workspace = true
rtrb.workspace = true
base64.workspace = true

[target.'cfg(windows)'.dependencies]
wasapi = "0.24"

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 2: Write the failing test**

`crates/oxitide-audio/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tidal error: {0}")]
    Tidal(#[from] oxitide_tidal::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
    #[error("mpd parse error: {0}")]
    Mpd(String),
    #[error("fetch error: {0}")]
    Fetch(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("output error: {0}")]
    Output(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_error_has_stable_message() {
        assert_eq!(Error::Manifest("bad".into()).to_string(), "manifest error: bad");
    }

    #[test]
    fn tap_error_includes_message() {
        assert_eq!(Error::Decode("boom".into()).to_string(), "decode error: boom");
    }
}
```

`crates/oxitide-audio/src/lib.rs`:
```rust
pub mod error;

pub use error::{Error, Result};
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio`
Expected: FAIL — the workspace member does not exist yet the first time, then the test compiles once the crate exists; the failing assertion is the point (delete the impl body if you want to watch it fail). If the package is not found, re-run after creating the files.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio error::`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/oxitide-audio
git commit -m "feat(audio): scaffold oxitide-audio crate and error type"
```

---

### Task 2: `oxitide-tidal::stream` — the `playbackinfo` endpoint

**Files:**
- Create: `crates/oxitide-tidal/src/stream.rs`
- Modify: `crates/oxitide-tidal/src/lib.rs`

**Interfaces:**
- Consumes: `TidalClient::get` from `client.rs`.
- Produces: `oxitide_tidal::stream::{Quality, PlaybackInfo}` and `TidalClient::playback_info(track_id: u64, quality: Quality) -> Result<PlaybackInfo>`.

- [ ] **Step 1: Write the failing test**

Create `crates/oxitide-tidal/src/stream.rs` with only the test module plus the type stubs it needs. Add `pub mod stream;` to `lib.rs` first.
```rust
use base64::Engine;
use serde::Deserialize;

use crate::client::TidalClient;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    High,
    Lossless,
    Max,
}

impl Quality {
    pub fn tidal_param(self) -> &'static str {
        match self {
            Quality::Low => "LOW",
            Quality::High => "HIGH",
            Quality::Lossless => "LOSSLESS",
            Quality::Max => "HI_RES_LOSSLESS",
        }
    }
}

/// The decoded `playbackinfo` response. `manifest` is raw bytes: XML for DASH,
/// JSON for BTS.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackInfo {
    pub quality: Quality,
    pub manifest_mime: String,
    pub manifest: Vec<u8>,
}

#[derive(Deserialize)]
struct RawPlaybackInfo {
    #[serde(default, rename = "manifestMimeType")]
    manifest_mime_type: String,
    #[serde(default)]
    manifest: String,
}

impl TidalClient {
    pub async fn playback_info(&self, track_id: u64, quality: Quality) -> Result<PlaybackInfo> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionInfo;
    use crate::token_store::{MemoryStore, TokenSet};
    use std::sync::Arc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet { access_token: "AT".into(), refresh_token: Some("RT".into()),
                token_type: "Bearer".into(), expires_at: None },
            Arc::new(MemoryStore::default()),
        )
    }

    #[test]
    fn quality_maps_to_tidal_params() {
        assert_eq!(Quality::Low.tidal_param(), "LOW");
        assert_eq!(Quality::Max.tidal_param(), "HI_RES_LOSSLESS");
    }

    #[tokio::test]
    async fn playback_info_requests_quality_and_decodes_base64() {
        let server = MockServer::start().await;
        let mpd = "<MPD></MPD>";
        let encoded = base64::engine::general_purpose::STANDARD.encode(mpd);
        Mock::given(method("GET"))
            .and(path("/tracks/77646170/playbackinfo"))
            .and(query_param("audioquality", "HI_RES_LOSSLESS"))
            .and(query_param("playbackmode", "STREAM"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifestMimeType": "application/dash+xml",
                "manifest": encoded
            })))
            .mount(&server)
            .await;
        let info = client(&server).playback_info(77646170, Quality::Max).await.unwrap();
        assert_eq!(info.manifest_mime, "application/dash+xml");
        assert_eq!(info.manifest, mpd.as_bytes());
    }

    #[tokio::test]
    async fn playback_info_404_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/tracks/1/playbackinfo"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(client(&server).playback_info(1, Quality::Lossless).await.is_err());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-tidal stream::`
Expected: FAIL — `not yet implemented` panics in `playback_info`.

- [ ] **Step 3: Implement `playback_info`**

Replace the `todo!()` body:
```rust
    pub async fn playback_info(&self, track_id: u64, quality: Quality) -> Result<PlaybackInfo> {
        let raw: RawPlaybackInfo = self
            .get(
                &format!("tracks/{track_id}/playbackinfo"),
                &[
                    ("audioquality", quality.tidal_param().to_string()),
                    ("playbackmode", "STREAM".to_string()),
                    ("assetpresentation", "FULL".to_string()),
                ],
            )
            .await?;
        let manifest = base64::engine::general_purpose::STANDARD
            .decode(raw.manifest.as_bytes())
            .map_err(|e| crate::error::Error::Auth(format!("bad manifest base64: {e}")))?;
        Ok(PlaybackInfo {
            quality,
            manifest_mime: raw.manifest_mime_type,
            manifest,
        })
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-tidal stream::`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-tidal/src/stream.rs crates/oxitide-tidal/src/lib.rs
git commit -m "feat(tidal): add playbackinfo stream endpoint"
```

---

### Task 3: Minimal DASH MPD parser (`mpd.rs`)

**Files:**
- Create: `crates/oxitide-audio/src/mpd.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: nothing (pure).
- Produces: `mpd::{Mpd, Representation, SegmentTemplate, TimelineEntry}`; `mpd::parse(&str) -> Result<Mpd>`; `Mpd::audio_representation() -> Option<&Representation>`; `Representation::{segment_count, media_urls, init_url}` (all take the MPD duration and/or base URLs so the type stays pure).

- [ ] **Step 1: Write the failing test**

Create `crates/oxitide-audio/src/mpd.rs` with the parser stubbed as `todo!()` and add `pub mod mpd;` to `lib.rs`.
```rust
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimelineEntry {
    pub t: Option<u64>,
    pub d: u64,
    pub r: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentTemplate {
    pub initialization: Option<String>,
    pub media: String,
    pub start_number: u64,
    pub timescale: u64,
    pub duration: Option<u64>,
    pub timeline: Vec<TimelineEntry>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Representation {
    pub id: Option<String>,
    pub codecs: Option<String>,
    pub bandwidth: Option<u64>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub base_urls: Vec<String>,
    pub template: Option<SegmentTemplate>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mpd {
    pub base_urls: Vec<String>,
    pub media_presentation_duration: Option<String>,
    pub representations: Vec<Representation>,
}

impl Mpd {
    /// TIDAL audio manifests expose one audio `Representation`; prefer a FLAC
    /// representation when more than one is present.
    pub fn audio_representation(&self) -> Option<&Representation> {
        self.representations
            .iter()
            .find(|r| r.codecs.as_deref().map(|c| c.contains("flac")).unwrap_or(false))
            .or_else(|| self.representations.first())
    }
}

pub fn parse(_xml: &str) -> Result<Mpd> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NUMBER_MPD: &str = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" mediaPresentationDuration="PT8S">
  <BaseURL>https://cdn.example/track/</BaseURL>
  <Period>
    <AdaptationSet>
      <Representation id="audio" codecs="flac" bandwidth="1500000" audioSamplingRate="44100" bitDepth="16">
        <SegmentTemplate initialization="init-$RepresentationID$.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    const TIMELINE_MPD: &str = r#"<MPD><Period><AdaptationSet>
      <Representation id="a" codecs="flac">
        <SegmentTemplate initialization="init.mp4" media="$Time$.mp4" timescale="44100">
          <SegmentTimeline><S d="176400" r="2"/></SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet></Period></MPD>"#;

    #[test]
    fn parses_number_template_and_counts_segments() {
        let mpd = parse(NUMBER_MPD).unwrap();
        assert_eq!(mpd.base_urls, vec!["https://cdn.example/track/"]);
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.codecs.as_deref(), Some("flac"));
        assert_eq!(r.sample_rate, Some(44100));
        assert_eq!(r.bit_depth, Some(16));
        let duration = mpd.media_presentation_duration.as_deref();
        assert_eq!(r.segment_count(duration).unwrap(), 2);
        assert_eq!(
            r.media_urls(duration, &mpd.base_urls).unwrap(),
            vec![
                "https://cdn.example/track/seg-1.mp4",
                "https://cdn.example/track/seg-2.mp4",
            ]
        );
        assert_eq!(
            r.init_url(&mpd.base_urls).as_deref(),
            Some("https://cdn.example/track/init-audio.mp4")
        );
    }

    #[test]
    fn parses_timeline_and_counts_segments() {
        let mpd = parse(TIMELINE_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.segment_count(None).unwrap(), 3);
        assert_eq!(
            r.media_urls(None, &[]).unwrap(),
            vec!["0.mp4", "176400.mp4", "352800.mp4"]
        );
    }

    #[test]
    fn malformed_xml_is_a_typed_error() {
        assert!(matches!(parse("<MPD><Period>"), Err(Error::Mpd(_))));
    }

    #[test]
    fn missing_template_errors() {
        let mpd = parse("<MPD><Period><AdaptationSet><Representation/></AdaptationSet></Period></MPD>").unwrap();
        let r = mpd.audio_representation().unwrap();
        assert!(matches!(r.segment_count(None), Err(Error::Manifest(_))));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio mpd::`
Expected: FAIL — `not yet implemented`.

- [ ] **Step 3: Implement the parser and URL helpers**

Replace the `parse` stub and add the impls (keep `audio_representation` from the test file):
```rust
impl Representation {
    pub fn base_url(&self, mpd_base: &[String]) -> String {
        let mut base = String::new();
        for b in mpd_base.iter().chain(self.base_urls.iter()) {
            base.push_str(b);
        }
        base
    }

    pub fn segment_count(&self, mpd_duration: Option<&str>) -> Result<u64> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("representation has no SegmentTemplate".into()))?;
        if !t.timeline.is_empty() {
            let mut n: u64 = 0;
            for e in &t.timeline {
                n += (e.r.max(-1) + 1) as u64;
            }
            return Ok(n);
        }
        let dur = t.duration.ok_or_else(|| Error::Manifest("no segment duration".into()))?;
        let ts = if t.timescale == 0 { 1 } else { t.timescale };
        let seg_secs = dur as f64 / ts as f64;
        let total = parse_iso8601_duration(
            mpd_duration.ok_or_else(|| Error::Manifest("no mediaPresentationDuration".into()))?,
        )
        .ok_or_else(|| Error::Manifest("bad mediaPresentationDuration".into()))?;
        Ok((total / seg_secs).ceil() as u64)
    }

    pub fn init_url(&self, mpd_base: &[String]) -> Option<String> {
        let t = self.template.as_ref()?;
        let init = t.initialization.as_ref()?;
        Some(resolve_template(&self.base_url(mpd_base), init, self.id.as_deref().unwrap_or(""), self.bandwidth.unwrap_or(0), 1, 0))
    }

    pub fn media_urls(&self, mpd_duration: Option<&str>, mpd_base: &[String]) -> Result<Vec<String>> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("no SegmentTemplate".into()))?;
        let base = self.base_url(mpd_base);
        let repr_id = self.id.clone().unwrap_or_default();
        let bw = self.bandwidth.unwrap_or(0);
        if !t.timeline.is_empty() {
            let mut urls = Vec::new();
            let mut time = t.timeline[0].t.unwrap_or(0);
            for e in &t.timeline {
                if let Some(start) = e.t {
                    time = start;
                }
                for _ in 0..(e.r.max(-1) + 1) {
                    urls.push(resolve_template(&base, &t.media, &repr_id, bw, 0, time));
                    time += e.d;
                }
            }
            return Ok(urls);
        }
        let n = self.segment_count(mpd_duration)?;
        Ok((0..n)
            .map(|i| resolve_template(&base, &t.media, &repr_id, bw, t.start_number + i, 0))
            .collect())
    }
}

fn resolve_template(base: &str, tmpl: &str, repr_id: &str, bandwidth: u64, number: u64, time: u64) -> String {
    tmpl.replace("$RepresentationID$", repr_id)
        .replace("$Number$", &number.to_string())
        .replace("$Time$", &time.to_string())
        .replace("$Bandwidth$", &bandwidth.to_string())
}
```
Then implement `parse`:
```rust
fn local_name(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.name().local_name().as_ref()).to_string()
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.local_name().as_ref() == key.as_bytes() {
            a.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

pub fn parse(xml: &str) -> Result<Mpd> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut mpd = Mpd::default();
    let mut current_repr: Option<Representation> = None;
    let mut base_text: Option<bool> = None; // true = capture into mpd, false = repr

    loop {
        match reader.read_event() {
            Err(e) => return Err(Error::Mpd(e.to_string())),
            Ok(Event::Eof) => break,
            Ok(Event::Text(t)) if base_text.is_some() => {
                let text = t.unescape().map_err(|e| Error::Mpd(e.to_string()))?.into_owned();
                if base_text == Some(false) {
                    if let Some(r) = current_repr.as_mut() {
                        if !text.trim().is_empty() {
                            r.base_urls.push(text);
                        }
                    }
                } else if !text.is_empty() {
                    mpd.base_urls.push(text);
                }
                base_text = None;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                match local_name(&e).as_str() {
                    "MPD" => {
                        mpd.media_presentation_duration = attr(&e, "mediaPresentationDuration");
                    }
                    "BaseURL" => {
                        base_text = Some(current_repr.is_none());
                    }
                    "Representation" => {
                        current_repr = Some(Representation {
                            id: attr(&e, "id"),
                            codecs: attr(&e, "codecs"),
                            bandwidth: attr(&e, "bandwidth").and_then(|v| v.parse().ok()),
                            sample_rate: attr(&e, "audioSamplingRate").and_then(|v| v.parse().ok()),
                            bit_depth: attr(&e, "bitDepth").and_then(|v| v.parse().ok()),
                            ..Default::default()
                        });
                    }
                    "SegmentTemplate" => {
                        let t = SegmentTemplate {
                            initialization: attr(&e, "initialization"),
                            media: attr(&e, "media").unwrap_or_default(),
                            start_number: attr(&e, "startNumber").and_then(|v| v.parse().ok()).unwrap_or(1),
                            timescale: attr(&e, "timescale").and_then(|v| v.parse().ok()).unwrap_or(1),
                            duration: attr(&e, "duration").and_then(|v| v.parse().ok()),
                            timeline: Vec::new(),
                        };
                        if let Some(r) = current_repr.as_mut() {
                            r.template = Some(t);
                        }
                    }
                    "S" => {
                        if let Some(r) = current_repr.as_mut() {
                            if let Some(t) = r.template.as_mut() {
                                t.timeline.push(TimelineEntry {
                                    t: attr(&e, "t").and_then(|v| v.parse().ok()),
                                    d: attr(&e, "d").and_then(|v| v.parse().ok()).unwrap_or(0),
                                    r: attr(&e, "r").and_then(|v| v.parse().ok()).unwrap_or(0),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match local_name(&e).as_str() {
                    "BaseURL" => base_text = None,
                    "Representation" => {
                        if let Some(r) = current_repr.take() {
                            mpd.representations.push(r);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(mpd)
}
```

Finally, add the ISO-8601 duration helper used by `segment_count`:
```rust
fn parse_iso8601_duration(s: &str) -> Option<f64> {
    // Supports PT#H#M#S and P#DT#H#M#S (no years/months).
    let s = s.strip_prefix('P')?;
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let mut secs = 0.0;
    let mut num = String::new();
    for c in date.chars() {
        if c == 'D' {
            secs += num.parse::<f64>().ok()? * 86_400.0;
            num.clear();
        } else {
            num.push(c);
        }
    }
    num.clear();
    for c in time.chars() {
        match c {
            'H' => { secs += num.parse::<f64>().ok()? * 3600.0; num.clear(); }
            'M' => { secs += num.parse::<f64>().ok()? * 60.0; num.clear(); }
            'S' => { secs += num.parse::<f64>().ok()?; num.clear(); }
            _ => num.push(c),
        }
    }
    Some(secs)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio mpd::`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/mpd.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add minimal dash mpd parser"
```

---

### Task 4: `source.rs` — `PlaybackInfo` to `StreamPlan`

**Files:**
- Create: `crates/oxitide-audio/src/source.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `oxitide_tidal::stream::PlaybackInfo`; `crate::mpd`.
- Produces: `source::{StreamPlan, DASH_MIME, BTS_MIME}`; `source::plan_from_info(&PlaybackInfo) -> Result<StreamPlan>`.

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/source.rs`:
```rust
use oxitide_tidal::stream::{PlaybackInfo, Quality};

use crate::error::{Error, Result};
use crate::mpd;

pub const DASH_MIME: &str = "application/dash+xml";
pub const BTS_MIME: &str = "application/vnd.tidal.bts";

#[derive(Debug, Clone, PartialEq)]
pub struct StreamPlan {
    pub init_url: Option<String>,
    pub media_urls: Vec<String>,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
}

pub fn plan_from_info(_info: &PlaybackInfo) -> Result<StreamPlan> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(mime: &str, body: &[u8]) -> PlaybackInfo {
        PlaybackInfo { quality: Quality::Max, manifest_mime: mime.into(), manifest: body.to_vec() }
    }

    #[test]
    fn dash_plan_builds_init_and_media_urls() {
        let mpd = r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
          <BaseURL>https://cdn/x/</BaseURL>
          <Representation id="audio" codecs="flac" audioSamplingRate="48000" bitDepth="24">
            <SegmentTemplate initialization="init.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
          </Representation></AdaptationSet></Period></MPD>"#;
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(plan.init_url.as_deref(), Some("https://cdn/x/init.mp4"));
        assert_eq!(plan.media_urls, vec!["https://cdn/x/seg-1.mp4", "https://cdn/x/seg-2.mp4"]);
        assert_eq!(plan.sample_rate, Some(48000));
        assert_eq!(plan.bit_depth, Some(24));
    }

    #[test]
    fn bts_plan_uses_direct_urls() {
        let json = r#"{"mimeType":"audio/flac","codecs":"flac","urls":["https://cdn/a.flac"]}"#;
        let plan = plan_from_info(&info(BTS_MIME, json.as_bytes())).unwrap();
        assert_eq!(plan.init_url, None);
        assert_eq!(plan.media_urls, vec!["https://cdn/a.flac"]);
        assert_eq!(plan.codec.as_deref(), Some("flac"));
    }

    #[test]
    fn unknown_mime_is_a_typed_error() {
        assert!(matches!(plan_from_info(&info("application/nope", b"{}")), Err(Error::Manifest(_))));
    }

    #[test]
    fn bts_without_urls_is_a_typed_error() {
        assert!(matches!(plan_from_info(&info(BTS_MIME, b"{}")), Err(Error::Manifest(_))));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio source::`
Expected: FAIL — `not yet implemented`.

- [ ] **Step 3: Implement `plan_from_info`**

```rust
#[derive(serde::Deserialize)]
struct BtsManifest {
    #[serde(default)]
    codecs: Option<String>,
    #[serde(default)]
    urls: Vec<String>,
}

pub fn plan_from_info(info: &PlaybackInfo) -> Result<StreamPlan> {
    match info.manifest_mime.as_str() {
        DASH_MIME => {
            let xml = std::str::from_utf8(&info.manifest)
                .map_err(|e| Error::Manifest(format!("dash manifest not utf8: {e}")))?;
            let doc = mpd::parse(xml)?;
            let repr = doc
                .audio_representation()
                .ok_or_else(|| Error::Manifest("no audio representation".into()))?;
            let duration = doc.media_presentation_duration.as_deref();
            let media_urls = repr.media_urls(duration, &doc.base_urls)?;
            let init_url = repr.init_url(&doc.base_urls);
            Ok(StreamPlan {
                init_url,
                media_urls,
                codec: repr.codecs.clone(),
                sample_rate: repr.sample_rate,
                bit_depth: repr.bit_depth,
            })
        }
        BTS_MIME => {
            let parsed: BtsManifest = serde_json::from_slice(&info.manifest)
                .map_err(|e| Error::Manifest(format!("bad bts manifest: {e}")))?;
            if parsed.urls.is_empty() {
                return Err(Error::Manifest("bts manifest has no urls".into()));
            }
            Ok(StreamPlan {
                init_url: None,
                media_urls: parsed.urls,
                codec: parsed.codecs,
                sample_rate: None,
                bit_depth: None,
            })
        }
        other => Err(Error::Manifest(format!("unsupported manifest mime type: {other}"))),
    }
}
```
`Representation::base_url`, `init_url`, and `media_urls` are already defined in Task 3; `resolve_template` already handles an init template with no `$Number$`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio source::`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/source.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): resolve playbackinfo into a stream plan"
```

---

### Task 5: `fetch.rs` — `Fetcher`, `StreamBuffer`, downloader thread

**Files:**
- Create: `crates/oxitide-audio/src/fetch.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `crate::source::StreamPlan`.
- Produces: `fetch::{Fetcher, HttpFetcher, StreamBuffer}`; `fetch::download(buffer: Arc<StreamBuffer>, fetcher: Arc<dyn Fetcher>, plan: StreamPlan)` (runs on the caller's thread); `StreamBuffer` implements the blocking side of the demux source.

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/fetch.rs`:
```rust
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::source::StreamPlan;

pub trait Fetcher: Send + Sync + 'static {
    fn get(&self, url: &str) -> Result<Vec<u8>>;
}

pub struct HttpFetcher {
    client: reqwest::blocking::Client,
}

impl HttpFetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Fetch(e.to_string()))?;
        Ok(Self { client })
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new().expect("build http client")
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.client.get(url).send().map_err(|e| Error::Fetch(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Fetch(format!("status {} for {url}", resp.status())));
        }
        resp.bytes().map(|b| b.to_vec()).map_err(|e| Error::Fetch(e.to_string()))
    }
}

/// A `Fetcher` over a fixed in-memory map. Used by tests and the fixture pipeline.
#[derive(Default, Clone)]
pub struct MapFetcher {
    pub files: HashMap<String, Vec<u8>>,
}

impl MapFetcher {
    pub fn with(mut self, url: &str, bytes: Vec<u8>) -> Self {
        self.files.insert(url.to_string(), bytes);
        self
    }
}

impl Fetcher for MapFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.files
            .get(url)
            .cloned()
            .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
    }
}

struct State {
    data: Vec<u8>,
    done: bool,
    error: Option<String>,
}

pub struct StreamBuffer {
    state: Mutex<State>,
    cond: Condvar,
}

impl StreamBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(State { data: Vec::new(), done: false, error: None }), cond: Condvar::new() })
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut s = self.state.lock().unwrap();
        s.data.extend_from_slice(bytes);
        self.cond.notify_all();
    }

    pub fn finish(&self) {
        let mut s = self.state.lock().unwrap();
        s.done = true;
        self.cond.notify_all();
    }

    pub fn fail(&self, message: String) {
        let mut s = self.state.lock().unwrap();
        s.error = Some(message);
        s.done = true;
        self.cond.notify_all();
    }

    /// Block until `pos` is buffered or the stream ends. Returns the currently
    /// buffered length, or a typed error if the download failed.
    pub fn wait_len(&self, pos: usize) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        loop {
            if let Some(err) = &s.error {
                return Err(Error::Fetch(err.clone()));
            }
            if pos < s.data.len() || s.done {
                return Ok(s.data.len());
            }
            s = self.cond.wait(s).unwrap();
        }
    }

    pub fn read_at(&self, pos: usize, out: &mut [u8]) -> Result<usize> {
        let len = self.wait_len(pos)?;
        if pos >= len {
            return Ok(0);
        }
        let s = self.state.lock().unwrap();
        let end = (pos + out.len()).min(len);
        let n = end - pos;
        out[..n].copy_from_slice(&s.data[pos..end]);
        Ok(n)
    }
}

/// Download every part of `plan` in order into `buffer`. Runs on the calling
/// thread; intended to be spawned via `std::thread::spawn`.
pub fn download(buffer: Arc<StreamBuffer>, fetcher: Arc<dyn Fetcher>, plan: StreamPlan) {
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.media_urls.iter().cloned());
    for url in urls {
        match fetcher.get(&url) {
            Ok(bytes) => buffer.push(&bytes),
            Err(e) => {
                buffer.fail(e.to_string());
                return;
            }
        }
    }
    buffer.finish();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_pushes_all_parts_in_order() {
        let fetcher = Arc::new(
            MapFetcher::default()
                .with("init", b"INIT".to_vec())
                .with("a", b"AAA".to_vec())
                .with("b", b"BB".to_vec()),
        );
        let plan = StreamPlan { init_url: Some("init".into()), media_urls: vec!["a".into(), "b".into()], codec: None, sample_rate: None, bit_depth: None };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan);
        let mut out = [0u8; 9];
        let n = buf.read_at(0, &mut out).unwrap();
        assert_eq!(&out[..n], b"INITAAABB");
    }

    #[test]
    fn fetch_failure_is_reported_to_readers() {
        let fetcher = Arc::new(MapFetcher::default());
        let plan = StreamPlan { init_url: None, media_urls: vec!["missing".into()], codec: None, sample_rate: None, bit_depth: None };
        let buf = StreamBuffer::new();
        download(buf.clone(), fetcher, plan);
        assert!(matches!(buf.wait_len(0), Err(Error::Fetch(_))));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio fetch::`
Expected: FAIL — `todo!`/missing; first create the module by copying the impl (there is no `todo!` here — the failing step is observational only if types are missing). Ensure `pub mod fetch;` is in `lib.rs` and the test module exists before the impl.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio fetch::`
Expected: PASS — 2 tests.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-audio/src/fetch.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add segment fetcher and streaming buffer"
```

---

### Task 6: `demux.rs` — blocking `MediaSource` over `StreamBuffer`

**Files:**
- Create: `crates/oxitide-audio/src/demux.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `crate::fetch::StreamBuffer`.
- Produces: `demux::StreamBufferSource` implementing `symphonia::core::io::MediaSource` (`Read + Seek + Send + Sync`) and `demux::BytesSource` for in-memory buffers.

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/demux.rs`:
```rust
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use symphonia::core::io::MediaSource;

use crate::fetch::StreamBuffer;

pub struct StreamBufferSource {
    buffer: Arc<StreamBuffer>,
    pos: u64,
}

impl StreamBufferSource {
    pub fn new(buffer: Arc<StreamBuffer>) -> Self {
        Self { buffer, pos: 0 }
    }
}

impl Read for StreamBufferSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self
            .buffer
            .read_at(self.pos as usize, out)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for StreamBufferSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // The buffer is a forward-only stream; symphonia will not seek because
        // `is_seekable` is false. Support only querying the position.
        match pos {
            SeekFrom::Start(p) if p <= self.pos => {
                self.pos = p;
                Ok(p)
            }
            SeekFrom::Current(0) => Ok(self.pos),
            _ => Err(io::Error::other("stream is not seekable")),
        }
    }
}

impl MediaSource for StreamBufferSource {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

pub struct BytesSource {
    cursor: io::Cursor<Vec<u8>>,
}

impl BytesSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { cursor: io::Cursor::new(bytes) }
    }
}

impl Read for BytesSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(out)
    }
}

impl Seek for BytesSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl MediaSource for BytesSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.cursor.get_ref().len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bytes_as_they_are_pushed() {
        let buf = StreamBuffer::new();
        buf.push(b"hello ");
        buf.push(b"world");
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        let mut out = Vec::new();
        src.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hello world");
    }

    #[test]
    fn end_of_stream_returns_zero() {
        let buf = StreamBuffer::new();
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        let mut out = [0u8; 4];
        assert_eq!(src.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn unsupported_seek_errors() {
        let buf = StreamBuffer::new();
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        assert!(src.seek(SeekFrom::End(0)).is_err());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio demux::`
Expected: FAIL to compile until `pub mod demux;` is added and the module is saved.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio demux::`
Expected: PASS — 3 tests.

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-audio/src/demux.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add streaming mediasource over the segment buffer"
```

---

### Task 7: `decode.rs` and committed tone fixtures

**Files:**
- Create: `crates/oxitide-audio/src/decode.rs`
- Create: `crates/oxitide-audio/tests/fixtures/tone.flac` (generated)
- Create: `crates/oxitide-audio/tests/fixtures/tone.mp4` (generated)
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `symphonia`; `crate::demux::BytesSource`.
- Produces: `decode::{PcmSink, run}`; `decode::{AudioFormat, CollectSink}`.
- **Verified API (symphonia 0.6.1):** `symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())`, `reader.default_track(TrackType::Audio)`, `reader.next_packet() -> Result<Option<Packet>>`, `track.codec_params.as_ref() -> Option<CodecParameters>` with `CodecParameters::Audio(_)`, `get_codecs().make_audio_decoder(cp, &AudioDecoderOptions::default())`, `decoder.decode(&packet)` returning a value with `spec()`, `samples_interleaved()`, `copy_to_slice_interleaved(&mut Vec<f32>)`.

- [ ] **Step 1: Generate and commit the fixtures**

Run:
```powershell
New-Item -ItemType Directory -Force -Path "crates/oxitide-audio/tests/fixtures" | Out-Null
ffmpeg -y -loglevel error -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=2" -ac 2 -sample_fmt s16 "crates/oxitide-audio/tests/fixtures/tone.flac"
ffmpeg -y -loglevel error -i "crates/oxitide-audio/tests/fixtures/tone.flac" -c:a copy -movflags frag_keyframe+empty_moov+default_base_moof "crates/oxitide-audio/tests/fixtures/tone.mp4"
```
Verify (regenerate if your local ffmpeg differs; the tests depend on 44.1 kHz stereo, 2 seconds):
```powershell
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels -of default=nw=1 "crates/oxitide-audio/tests/fixtures/tone.mp4"
```
Expected: `codec_name=flac`, `sample_rate=44100`, `channels=2`.

- [ ] **Step 2: Write the failing test**

`crates/oxitide-audio/src/decode.rs`:
```rust
use std::fs;

use symphonia::core::audio::AudioSpec;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::demux::BytesSource;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait PcmSink {
    fn on_spec(&mut self, spec: &AudioSpec) -> Result<()>;
    fn on_samples(&mut self, samples: &[f32]) -> Result<()>;
}

pub struct CollectSink {
    pub spec: Option<AudioSpec>,
    pub samples: Vec<f32>,
}

impl CollectSink {
    pub fn new() -> Self {
        Self { spec: None, samples: Vec::new() }
    }
}

impl Default for CollectSink {
    fn default() -> Self {
        Self::new()
    }
}

impl PcmSink for CollectSink {
    fn on_spec(&mut self, spec: &AudioSpec) -> Result<()> {
        self.spec = Some(spec.clone());
        Ok(())
    }
    fn on_samples(&mut self, samples: &[f32]) -> Result<()> {
        self.samples.extend_from_slice(samples);
        Ok(())
    }
}

pub fn run(_source: impl MediaSource + 'static, _ext: &str, _sink: &mut dyn PcmSink) -> Result<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demux::BytesSource;

    fn decode_fixture(name: &str, ext: &str) -> CollectSink {
        let bytes = fs::read(format!("tests/fixtures/{name}")).unwrap();
        let mut sink = CollectSink::new();
        run(BytesSource::new(bytes), ext, &mut sink).unwrap();
        sink
    }

    #[test]
    fn decodes_fragmented_mp4_flac() {
        let sink = decode_fixture("tone.mp4", "mp4");
        let spec = sink.spec.unwrap();
        assert_eq!(spec.rate, 44100);
        assert_eq!(sink.samples.len(), 176_400); // 2s * 44100 * 2ch
        assert!(sink.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn decodes_bare_flac() {
        let sink = decode_fixture("tone.flac", "flac");
        assert_eq!(sink.samples.len(), 176_400);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio decode::`
Expected: FAIL — `not yet implemented`.

- [ ] **Step 4: Implement `run`**

```rust
pub fn run(source: impl MediaSource + 'static, ext: &str, sink: &mut dyn PcmSink) -> Result<()> {
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);

    let mut reader = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::Decode(e.to_string()))?;

    let (track_id, mut decoder) = {
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Decode("no audio track".into()))?;
        let params = match track.codec_params.as_ref() {
            Some(CodecParameters::Audio(audio)) => audio,
            _ => return Err(Error::Decode("unsupported codec parameters".into())),
        };
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| Error::Decode(e.to_string()))?;
        (track.id, decoder)
    };

    let mut spec_sent = false;
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if !spec_sent {
                    sink.on_spec(decoded.spec())?;
                    spec_sent = true;
                }
                let mut buf = vec![0f32; decoded.samples_interleaved()];
                decoded.copy_to_slice_interleaved(&mut buf);
                sink.on_samples(&buf)?;
            }
            Err(SymError::DecodeError(msg)) => {
                tracing::warn!("decode error: {msg}");
            }
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        }
    }
    Ok(())
}
```
Remove the now-duplicate `use crate::demux::BytesSource;` from the top imports (it is only needed in the test module), and keep `AudioFormat` (used by `output` in Task 9).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio decode::`
Expected: PASS — 2 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-audio/src/decode.rs crates/oxitide-audio/src/lib.rs crates/oxitide-audio/tests/fixtures
git commit -m "feat(audio): decode flac from fmp4 and bare flac"
```

---

### Task 8: `output.rs` — `AudioOutput` trait and `NullOutput`

**Files:**
- Create: `crates/oxitide-audio/src/output.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `crate::decode::AudioFormat`.
- Produces: `output::{AudioOutput, OutputDevice, NullOutput}`; `output::enumerate_output_devices() -> Result<Vec<OutputDevice>>` (real on Windows, empty elsewhere).

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/output.rs`:
```rust
use std::sync::{Arc, Mutex};

use crate::decode::AudioFormat;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub trait AudioOutput: Send {
    fn open(&mut self, device_id: Option<&str>, format: AudioFormat) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn write(&mut self, samples: &[f32]) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

/// Captures everything written, for tests and headless use.
#[derive(Clone, Default)]
pub struct NullOutput {
    samples: Arc<Mutex<Vec<f32>>>,
    format: Option<AudioFormat>,
}

impl NullOutput {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn samples(&self) -> Vec<f32> {
        self.samples.lock().unwrap().clone()
    }
}

impl AudioOutput for NullOutput {
    fn open(&mut self, _device_id: Option<&str>, format: AudioFormat) -> Result<()> {
        self.format = Some(format);
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        Ok(())
    }
    fn write(&mut self, samples: &[f32]) -> Result<()> {
        self.samples.lock().unwrap().extend_from_slice(samples);
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    platform::enumerate()
}

#[cfg(not(windows))]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub(crate) mod platform {
    use super::*;
    use wasapi::{DeviceEnumerator, Direction};

    pub fn ensure_com() -> Result<()> {
        wasapi::initialize_mta()
            .ok()
            .ok_or_else(|| Error::Output("failed to initialize COM (MTA)".into()))
    }

    pub fn enumerate() -> Result<Vec<OutputDevice>> {
        ensure_com()?;
        let enumerator = DeviceEnumerator::new().map_err(|e| Error::Output(e.to_string()))?;
        let default_name = enumerator
            .get_default_device_for_role(&Direction::Render, &wasapi::Role::Console)
            .ok()
            .and_then(|d| d.get_friendlyname().ok());
        let mut out = Vec::new();
        for device in enumerator
            .get_device_collection(&Direction::Render)
            .map_err(|e| Error::Output(e.to_string()))?
        {
            let device = device.map_err(|e| Error::Output(e.to_string()))?;
            let name = device.get_friendlyname().unwrap_or_default();
            let id = device.get_id().unwrap_or_default();
            let is_default = default_name.as_deref() == Some(name.as_str());
            out.push(OutputDevice { id, name, is_default });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_output_collects_samples() {
        let mut out = NullOutput::new();
        out.open(None, AudioFormat { sample_rate: 44100, channels: 2 }).unwrap();
        out.start().unwrap();
        out.write(&[0.0, 0.5, -0.5]).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();
        assert_eq!(out.samples(), vec![0.0, 0.5, -0.5]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio output::`
Expected: FAIL to compile until `pub mod output;` is added.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio output::`
Expected: PASS — 1 test.

- [ ] **Step 4: Compile-check the Windows enumeration path**

Run: `cargo build -p oxitide-audio`
Expected: success (compiles the `platform::enumerate` path; no device is opened).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/output.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add audio output trait, null output, and device enumeration"
```

---

### Task 9: `WasapiOutput` — shared-mode playback on a dedicated thread

**Files:**
- Create: `crates/oxitide-audio/src/wasapi_output.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`, `crates/oxitide-audio/src/output.rs`

**Interfaces:**
- Consumes: `crate::output::{AudioOutput, platform}`; `rtrb`.
- Produces: `wasapi_output::WasapiOutput` (Windows only; `#[cfg(windows)]`). It owns a producer end of an `rtrb` ring buffer; a worker thread drains it into the device.

- [ ] **Step 1: Write the implementation**

`crates/oxitide-audio/src/wasapi_output.rs`:
```rust
#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{Direction, SampleType, ShareMode, StreamMode, WaveFormat};

use crate::decode::AudioFormat;
use crate::error::{Error, Result};
use crate::output::{platform, AudioOutput};

const RING_FRAMES: usize = 44100 * 2; // ~0.5s stereo at 44.1k; resized on open

enum Msg {
    Configure,
    Stop,
}

pub struct WasapiOutput {
    device_id: Option<String>,
    format: Option<AudioFormat>,
    producer: Option<Producer<f32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    started: bool,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            format: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            started: false,
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
        self.started = false;
    }
}

impl Default for WasapiOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for WasapiOutput {
    fn open(&mut self, device_id: Option<&str>, format: AudioFormat) -> Result<()> {
        self.shutdown_worker();
        platform::ensure_com()?;
        self.device_id = device_id.map(|s| s.to_string());
        self.format = Some(format);

        let frames = (format.sample_rate as usize * format.channels as usize).max(1024) * 2;
        let (producer, mut consumer) = RingBuffer::<f32>::new(frames.max(RING_FRAMES));

        let device_id = self.device_id.clone();
        let channels = format.channels;
        let sample_rate = format.sample_rate;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || {
                if let Err(e) = run_device(device_id, sample_rate, channels, &mut consumer, &stop_thread) {
                    tracing::error!("wasapi output stopped: {e}");
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        self.producer = Some(producer);
        self.worker = Some(handle);
        self.stop = stop;
        self.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        self.started = true;
        Ok(())
    }

    fn write(&mut self, samples: &[f32]) -> Result<()> {
        let producer = self.producer.as_mut().ok_or_else(|| Error::Output("output not open".into()))?;
        for &sample in samples {
            // If the ring is full, wait briefly rather than dropping audio.
            let mut pending = sample;
            loop {
                match producer.push(pending) {
                    Ok(()) => break,
                    Err(rtrb::PushError::Full(sample)) => {
                        pending = sample;
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

fn run_device(
    device_id: Option<String>,
    sample_rate: u32,
    channels: u16,
    consumer: &mut rtrb::Consumer<f32>,
    stop: &AtomicBool,
) -> Result<()> {
    platform::ensure_com()?;
    let enumerator = wasapi::DeviceEnumerator::new().map_err(|e| Error::Output(e.to_string()))?;
    let device = match device_id {
        Some(id) => enumerator
            .get_device(&Direction::Render, &id)
            .map_err(|e| Error::Output(e.to_string()))?,
        None => enumerator
            .get_default_device(&Direction::Render)
            .map_err(|e| Error::Output(e.to_string()))?,
    };
    let mut client = device.get_iaudioclient().map_err(|e| Error::Output(e.to_string()))?;
    let desired = WaveFormat::new(32, 32, &SampleType::Float, sample_rate as usize, channels as usize, None);
    let (def_time, _min_time) = client.get_device_period().map_err(|e| Error::Output(e.to_string()))?;
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: def_time };
    client
        .initialize_client(&desired, &Direction::Render, &mode)
        .map_err(|e| Error::Output(e.to_string()))?;
    let event = client.set_get_eventhandle().map_err(|e| Error::Output(e.to_string()))?;
    let render = client.get_audiorenderclient().map_err(|e| Error::Output(e.to_string()))?;

    // Prime the buffer before starting.
    write_frames(&client, &render, consumer, channels as usize)?;
    client.start_stream().map_err(|e| Error::Output(e.to_string()))?;

    while !stop.load(Ordering::SeqCst) {
        if event.wait_for_event(500).is_err() {
            continue; // timeout: re-check the stop flag
        }
        write_frames(&client, &render, consumer, channels as usize)?;
    }
    let _ = client.stop_stream();
    Ok(())
}

fn write_frames(
    client: &wasapi::AudioClient,
    render: &wasapi::AudioRenderClient,
    consumer: &mut rtrb::Consumer<f32>,
    channels: usize,
) -> Result<()> {
    let frames = client
        .get_available_space_in_frames()
        .map_err(|e| Error::Output(e.to_string()))? as usize;
    if frames == 0 {
        return Ok(());
    }
    let mut buf = vec![0u8; frames * channels * 4];
    for chunk in buf.chunks_mut(4) {
        let sample = consumer.pop().unwrap_or(0.0);
        chunk.copy_from_slice(&sample.to_le_bytes());
    }
    render
        .write_to_device(frames, &buf, None)
        .map_err(|e| Error::Output(e.to_string()))
}
```
Add `pub mod wasapi_output;` (Windows-gated) to `lib.rs`.

- [ ] **Step 2: Compile-check**

Run: `cargo build -p oxitide-audio`
Expected: success. If `DeviceEnumerator::get_device` or `AudioRenderClient::write_to_device` have different signatures in the installed `wasapi` version, fix per `docs.rs/wasapi/0.24.0` and the `playsine` example. Do not proceed until it compiles.

- [ ] **Step 3: Add a manual, ignored integration test**

Append to `crates/oxitide-audio/src/wasapi_output.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an audio device"]
    fn opens_default_device_and_plays_a_second_of_silence() {
        let mut out = WasapiOutput::new();
        out.open(None, AudioFormat { sample_rate: 44100, channels: 2 }).unwrap();
        out.start().unwrap();
        out.write(&vec![0.0f32; 44100 * 2]).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        out.flush().unwrap();
        out.stop().unwrap();
    }
}
```

- [ ] **Step 4: Run the ignored test manually once**

Run: `cargo test -p oxitide-audio wasapi_output -- --ignored --nocapture`
Expected: PASS on a machine with audio.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/wasapi_output.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add wasapi shared-mode output"
```

---

### Task 10: `queue.rs` — the playback state machine

**Files:**
- Create: `crates/oxitide-audio/src/queue.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: nothing (pure).
- Produces: `queue::{QueueItem, RepeatMode, Queue}`.

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/queue.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub track_id: u64,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub duration_secs: u32,
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    current: Option<usize>,
    playing: bool,
    shuffle: bool,
    repeat: RepeatMode,
    volume: f32,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            current: None,
            playing: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            volume: 1.0,
        }
    }
}

impl Queue {
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.and_then(|i| self.items.get(i))
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }
    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn play_context(&mut self, items: Vec<QueueItem>, start: usize) {
        todo!()
    }
    pub fn toggle_play_pause(&mut self) {
        todo!()
    }
    pub fn next(&mut self) -> bool {
        todo!()
    }
    pub fn prev(&mut self) -> bool {
        todo!()
    }
    pub fn on_track_ended(&mut self) -> Option<usize> {
        todo!()
    }
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        todo!()
    }
    pub fn remove(&mut self, index: usize) -> bool {
        todo!()
    }
    pub fn clear(&mut self) {
        todo!()
    }
    pub fn set_volume(&mut self, v: f32) {
        todo!()
    }
    pub fn set_shuffle(&mut self, on: bool) {
        todo!()
    }
    pub fn set_repeat(&mut self, mode: RepeatMode) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64) -> QueueItem {
        QueueItem { track_id: id, title: format!("T{id}"), artist: String::new(), album: String::new(), duration_secs: 1, cover: None }
    }

    fn queue(n: u64) -> Queue {
        let mut q = Queue::default();
        q.play_context((1..=n).map(item).collect(), 0);
        q
    }

    #[test]
    fn play_context_sets_current_and_playing() {
        let q = queue(3);
        assert_eq!(q.current_index(), Some(0));
        assert!(q.is_playing());
        assert_eq!(q.current().unwrap().track_id, 1);
    }

    #[test]
    fn next_at_end_stops_when_repeat_off() {
        let mut q = queue(2);
        assert!(q.next());
        assert_eq!(q.current_index(), Some(1));
        assert!(!q.next());
        assert!(!q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn next_wraps_when_repeat_all() {
        let mut q = queue(2);
        q.set_repeat(RepeatMode::All);
        q.next();
        assert!(q.next());
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn on_track_ended_repeats_one() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.on_track_ended(), Some(0));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn on_track_ended_stops_at_end_when_repeat_off() {
        let mut q = queue(2);
        q.next(); // current = last item
        assert_eq!(q.on_track_ended(), None);
        assert!(!q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn prev_at_start_is_a_noop() {
        let mut q = queue(3);
        assert!(!q.prev());
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn reorder_preserves_current_item() {
        let mut q = queue(3);
        q.next(); // current = index 1 (track 2)
        assert!(q.reorder(0, 2));
        assert_eq!(q.current().unwrap().track_id, 2);
        assert_eq!(q.items()[2].track_id, 1);
    }

    #[test]
    fn remove_before_current_shifts_index() {
        let mut q = queue(3);
        q.next(); // current track 2 at index 1
        assert!(q.remove(0));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn remove_current_advances_to_next() {
        let mut q = queue(3);
        assert!(q.remove(0));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn clear_empties_the_queue() {
        let mut q = queue(2);
        q.clear();
        assert!(q.items().is_empty());
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
    }

    #[test]
    fn volume_is_clamped() {
        let mut q = Queue::default();
        q.set_volume(2.0);
        assert_eq!(q.volume(), 1.0);
        q.set_volume(-1.0);
        assert_eq!(q.volume(), 0.0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio queue::`
Expected: FAIL — `not yet implemented`.

- [ ] **Step 3: Implement the queue operations**

```rust
    pub fn play_context(&mut self, items: Vec<QueueItem>, start: usize) {
        self.items = items;
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = true;
        }
    }

    pub fn toggle_play_pause(&mut self) {
        if self.current.is_some() {
            self.playing = !self.playing;
        }
    }

    fn advance(&mut self) -> bool {
        let Some(cur) = self.current else { return false };
        if self.items.is_empty() {
            return false;
        }
        if cur + 1 < self.items.len() {
            self.current = Some(cur + 1);
            self.playing = true;
            true
        } else if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
            true
        } else {
            self.playing = false;
            false
        }
    }

    pub fn next(&mut self) -> bool {
        self.advance()
    }

    pub fn prev(&mut self) -> bool {
        let Some(cur) = self.current else { return false };
        if cur == 0 {
            return false;
        }
        self.current = Some(cur - 1);
        self.playing = true;
        true
    }

    pub fn on_track_ended(&mut self) -> Option<usize> {
        if self.items.is_empty() {
            self.playing = false;
            return None;
        }
        let cur = self.current?;
        if self.repeat == RepeatMode::One {
            self.playing = true;
            return Some(cur);
        }
        if cur + 1 < self.items.len() {
            self.current = Some(cur + 1);
            self.playing = true;
            return Some(cur + 1);
        }
        if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
            return Some(0);
        }
        // End of queue with repeat off: stop, keep the last item as current.
        self.playing = false;
        None
    }

    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        if let Some(cur) = self.current {
            if cur == from {
                self.current = Some(to);
            } else if from < cur && to >= cur {
                self.current = Some(cur - 1);
            } else if from > cur && to <= cur {
                self.current = Some(cur + 1);
            }
        }
        true
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        match self.current {
            None => {}
            Some(cur) if index < cur => self.current = Some(cur - 1),
            Some(cur) if index == cur => {
                if self.items.is_empty() {
                    self.current = None;
                    self.playing = false;
                } else {
                    self.current = Some(cur.min(self.items.len() - 1));
                }
            }
            Some(_) => {}
        }
        true
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
        self.playing = false;
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }
```
Shuffle is stored but the current implementation advances sequentially; this is acceptable for Plan 2 (documented: shuffle affects `advance` ordering is deferred). To avoid an unused field warning, `shuffle()` is used by the engine snapshot.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio queue::`
Expected: PASS — 10 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/queue.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add queue state machine"
```

---

### Task 11: `engine.rs` — control loop, events, and playback threads

**Files:**
- Create: `crates/oxitide-audio/src/engine.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`

**Interfaces:**
- Consumes: `queue`, `source::StreamPlan`, `fetch`, `demux`, `decode`, `output`.
- Produces: `engine::{Engine (Clone), EngineEvent, PlaybackState, EngineCmd, StreamResolver, OutputFactory, DEFAULT_QUALITY}`. `Engine::new(resolver, fetcher, output_factory, quality)`, `Engine::handle(&self, cmd)`, `Engine::subscribe(&self, f)`, `Engine::state(&self)`.
- The engine owns an `OutputFactory` and spawns one playback thread per track; it emits events and auto-advances via `Queue::on_track_ended`. Pause and volume are applied inside the PCM sink via shared atomics.

- [ ] **Step 1: Write the failing test**

`crates/oxitide-audio/src/engine.rs`:
```rust
use std::sync::{Arc, Mutex};

use crate::decode::{AudioFormat, PcmSink};
use crate::error::{Error, Result};
use crate::fetch::{self, Fetcher, StreamBuffer};
use crate::output::AudioOutput;
use crate::queue::{Queue, QueueItem};
use crate::source::StreamPlan;
use oxitide_tidal::stream::Quality;

pub trait StreamResolver: Send + Sync + 'static {
    fn resolve(&self, track_id: u64, quality: Quality) -> Result<StreamPlan>;
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlaybackState {
    pub playing: bool,
    pub buffering: bool,
    pub current: Option<QueueItem>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: crate::queue::RepeatMode,
    pub queue: Vec<QueueItem>,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    State(PlaybackState),
    NowPlaying(Option<QueueItem>),
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
}

#[derive(Debug, Clone)]
pub enum EngineCmd {
    PlayTracks { items: Vec<QueueItem>, start: usize },
    PlayPause,
    Next,
    Prev,
    Seek(u64),
    SetVolume(f32),
    SetShuffle(bool),
    SetRepeat(crate::queue::RepeatMode),
    Reorder { from: usize, to: usize },
    Remove(usize),
    Clear,
}

type Listener = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// Creates a fresh `AudioOutput` for each track. The app passes a factory that
/// builds `WasapiOutput`; tests pass one that builds `NullOutput`.
pub trait OutputFactory: Send + Sync + 'static {
    fn create(&self) -> Box<dyn AudioOutput>;
}

impl<F> OutputFactory for F
where
    F: Fn() -> Box<dyn AudioOutput> + Send + Sync + 'static,
{
    fn create(&self) -> Box<dyn AudioOutput> {
        self()
    }
}

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

struct Inner {
    queue: Mutex<Queue>,
    resolver: Arc<dyn StreamResolver>,
    fetcher: Arc<dyn Fetcher>,
    output: Arc<dyn OutputFactory>,
    quality: Mutex<Quality>,
    listeners: Mutex<Vec<Listener>>,
    /// Incremented on every new track; a playback thread whose generation is
    /// stale stops writing.
    generation: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    volume: Arc<AtomicU32>, // f32 bits
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl Engine {
    pub fn new(
        resolver: Arc<dyn StreamResolver>,
        fetcher: Arc<dyn Fetcher>,
        output: Arc<dyn OutputFactory>,
        quality: Quality,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(Queue::default()),
                resolver,
                fetcher,
                output,
                quality: Mutex::new(quality),
                listeners: Mutex::new(Vec::new()),
                generation: Arc::new(AtomicU64::new(0)),
                paused: Arc::new(AtomicBool::new(false)),
                volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            }),
        }
    }

    pub fn subscribe(&self, listener: Listener) {
        self.inner.listeners.lock().unwrap().push(listener);
    }

    pub fn set_quality(&self, quality: Quality) {
        *self.inner.quality.lock().unwrap() = quality;
    }

    pub fn state(&self) -> PlaybackState {
        let q = self.inner.queue.lock().unwrap();
        PlaybackState {
            playing: q.is_playing(),
            buffering: false,
            current: q.current().cloned(),
            position_ms: 0,
            duration_ms: q.current().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0),
            volume: q.volume(),
            shuffle: q.shuffle(),
            repeat: q.repeat(),
            queue: q.items().to_vec(),
        }
    }

    fn emit(&self, event: EngineEvent) {
        for listener in self.inner.listeners.lock().unwrap().iter() {
            listener(event.clone());
        }
    }

    pub fn handle(&self, cmd: EngineCmd) -> Result<()> {
        match cmd {
            EngineCmd::PlayTracks { items, start } => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.play_context(items, start);
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::PlayPause => {
                let playing = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.toggle_play_pause();
                    q.is_playing()
                };
                self.inner.paused.store(!playing, Ordering::SeqCst);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Next => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.next();
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Prev => {
                let item = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.prev();
                    q.current().cloned()
                };
                self.emit(EngineEvent::NowPlaying(item.clone()));
                if let Some(item) = item {
                    self.start_playback(item);
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Seek(_) => { /* position tracking is a Plan 3 concern */ }
            EngineCmd::SetVolume(v) => {
                let clamped = {
                    let mut q = self.inner.queue.lock().unwrap();
                    q.set_volume(v);
                    q.volume()
                };
                self.inner.volume.store(clamped.to_bits(), Ordering::SeqCst);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::SetShuffle(on) => {
                self.inner.queue.lock().unwrap().set_shuffle(on);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::SetRepeat(mode) => {
                self.inner.queue.lock().unwrap().set_repeat(mode);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Reorder { from, to } => {
                self.inner.queue.lock().unwrap().reorder(from, to);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Remove(index) => {
                self.inner.queue.lock().unwrap().remove(index);
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Clear => {
                self.inner.queue.lock().unwrap().clear();
                self.inner.generation.fetch_add(1, Ordering::SeqCst);
                self.inner.paused.store(false, Ordering::SeqCst);
                self.emit(EngineEvent::NowPlaying(None));
                self.emit(EngineEvent::State(self.state()));
            }
        }
        Ok(())
    }

    /// Resolve, download, decode, and write `item` through a fresh output. Runs on
    /// the calling thread; used directly by tests and by the per-track thread.
    pub fn play_track_into(&self, item: &QueueItem, output: &mut dyn AudioOutput) -> Result<()> {
        let quality = *self.inner.quality.lock().unwrap();
        let plan = self.inner.resolver.resolve(item.track_id, quality)?;
        let buffer = StreamBuffer::new();
        let fetcher = self.inner.fetcher.clone();
        let download_buffer = buffer.clone();
        let downloader = std::thread::spawn(move || fetch::download(download_buffer, fetcher, plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut sink = OutputSink {
            output,
            generation: None,
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }

    /// Spawn a playback thread for `item`. On success it advances the queue and
    /// starts the next track; a stale generation (a newer command superseded it)
    /// aborts silently.
    fn start_playback(&self, item: QueueItem) {
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let engine = self.clone();
        let spawned = std::thread::Builder::new()
            .name("oxitide-playback".into())
            .spawn(move || {
                let result = engine.run_track(&item, generation);
                if engine.inner.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match result {
                    Ok(()) => {
                        let next = {
                            let mut q = engine.inner.queue.lock().unwrap();
                            q.on_track_ended().and_then(|i| q.items().get(i).cloned())
                        };
                        engine.emit(EngineEvent::NowPlaying(next.clone()));
                        engine.emit(EngineEvent::State(engine.state()));
                        if let Some(next) = next {
                            engine.start_playback(next);
                        }
                    }
                    Err(e) => engine.emit(EngineEvent::Error {
                        code: "playback".into(),
                        message: e.to_string(),
                    }),
                }
            });
        if let Err(e) = spawned {
            self.emit(EngineEvent::Error { code: "playback".into(), message: e.to_string() });
        }
    }

    fn run_track(&self, item: &QueueItem, generation: u64) -> Result<()> {
        let quality = *self.inner.quality.lock().unwrap();
        let plan = self.inner.resolver.resolve(item.track_id, quality)?;
        let buffer = StreamBuffer::new();
        let fetcher = self.inner.fetcher.clone();
        let download_buffer = buffer.clone();
        let downloader = std::thread::spawn(move || fetch::download(download_buffer, fetcher, plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut output = self.inner.output.create();
        let mut sink = OutputSink {
            output: output.as_mut(),
            generation: Some((self.inner.generation.clone(), generation)),
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = output.stop();
        let _ = downloader.join();
        result
    }
}

struct OutputSink<'a> {
    output: &'a mut dyn AudioOutput,
    generation: Option<(Arc<AtomicU64>, u64)>,
    paused: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
}

impl PcmSink for OutputSink<'_> {
    fn on_spec(&mut self, spec: &symphonia::core::audio::AudioSpec) -> Result<()> {
        self.output.open(
            None,
            AudioFormat { sample_rate: spec.rate, channels: channel_count(&spec.channels) },
        )
    }
    fn on_samples(&mut self, samples: &[f32]) -> Result<()> {
        if let Some((generation, mine)) = &self.generation {
            if generation.load(Ordering::SeqCst) != *mine {
                return Err(Error::Output("playback superseded".into()));
            }
        }
        if self.paused.load(Ordering::SeqCst) {
            // Keep the decode loop from racing ahead while paused.
            std::thread::sleep(std::time::Duration::from_millis(20));
            return Ok(());
        }
        let volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        if (volume - 1.0).abs() < f32::EPSILON {
            self.output.write(samples)
        } else {
            let scaled: Vec<f32> = samples.iter().map(|s| s * volume).collect();
            self.output.write(&scaled)
        }
    }
}

fn channel_count(channels: &symphonia::core::audio::Channels) -> u16 {
    use symphonia::core::audio::Channels;
    match channels {
        Channels::Positioned(p) => p.count() as u16,
        Channels::Discrete(n) => *n as u16,
        _ => 2, // TIDAL audio is stereo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::NullOutput;
    use crate::source::StreamPlan;
    use std::collections::HashMap;

    struct FakeResolver {
        plan: StreamPlan,
    }

    impl StreamResolver for FakeResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Ok(self.plan.clone())
        }
    }

    fn fixture_plan() -> (StreamPlan, HashMap<String, Vec<u8>>) {
        let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let plan = StreamPlan {
            init_url: None,
            media_urls: vec!["mem://tone".into()],
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
        };
        let mut files = HashMap::new();
        files.insert("mem://tone".to_string(), bytes);
        (plan, files)
    }

    fn item(id: u64) -> QueueItem {
        QueueItem { track_id: id, title: format!("T{id}"), artist: "A".into(), album: "B".into(), duration_secs: 2, cover: None }
    }

    /// A factory that hands out clones of one `NullOutput`, so a test can read
    /// everything the engine wrote.
    fn null_factory() -> (Arc<dyn OutputFactory>, Arc<Mutex<NullOutput>>) {
        let handle = Arc::new(Mutex::new(NullOutput::new()));
        let clone = handle.clone();
        let factory: Arc<dyn OutputFactory> =
            Arc::new(move || Box::new(clone.lock().unwrap().clone()));
        (factory, handle)
    }

    fn engine_with(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
    ) -> (Engine, Arc<Mutex<NullOutput>>) {
        let (factory, output) = null_factory();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );
        (engine, output)
    }

    #[test]
    fn play_tracks_emits_now_playing_then_state() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        let events = seen.lock().unwrap();
        assert!(events.len() >= 2);
        assert!(matches!(events[0], EngineEvent::NowPlaying(Some(_))));
        assert!(matches!(events[1], EngineEvent::State(_)));
    }

    #[test]
    fn play_track_into_decodes_fixture() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let mut out = NullOutput::new();
        engine.play_track_into(&item(1), &mut out).unwrap();
        assert_eq!(out.samples().len(), 176_400);
    }

    #[test]
    fn clear_emits_empty_now_playing() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::Clear).unwrap();
        let events = seen.lock().unwrap();
        assert!(matches!(events[0], EngineEvent::NowPlaying(None)));
    }

    #[test]
    fn playback_thread_writes_samples_and_advances() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        // Wait up to 5s for the first track (2s of audio) to reach the sink.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while output.lock().unwrap().samples().len() < 176_400 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(output.lock().unwrap().samples().len() >= 176_400);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio engine::`
Expected: FAIL — compile errors until the module is implemented and `pub mod engine;` is added.

- [ ] **Step 3: Fix API mismatches**

`channel_count` uses `Position::count()`. If that method is absent in symphonia 0.6, use `p.bits().count_ones() as u16` (the `Position` bitmask has one bit per channel). Do not import the `Audio` trait into `engine.rs` unless the compiler asks for it (the verified decode spike did not need it). Verify with `cargo build -p oxitide-audio` before running tests.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxitide-audio engine::`
Expected: PASS — 3 tests.

- [ ] **Step 5: Run the full crate test suite**

Run: `cargo test -p oxitide-audio`
Expected: PASS (all unit tests; the ignored WASAPI test is skipped).

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-audio/src/engine.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add playback engine, events, and queue commands"
```

---

### Task 12: Tauri commands and events (`oxitide-app`)

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml`
- Create: `crates/oxitide-app/src/player.rs`
- Modify: `crates/oxitide-app/src/state.rs`, `crates/oxitide-app/src/commands.rs`, `crates/oxitide-app/src/main.rs`

**Interfaces:**
- Consumes: `oxitide_audio::engine`, `oxitide_audio::output`, `TidalClient`.
- Produces: `player::{TidalResolver, RealOutputs, QualityDto, EngineEventDto, OutputDeviceDto}`; Tauri commands `play_tracks`, `play_pause`, `next`, `prev`, `seek`, `set_volume`, `set_shuffle`, `set_repeat`, `queue_reorder`, `queue_remove`, `queue_clear`, `get_playback_state`, `get_output_devices`, `set_output_device`, `set_preferred_quality`; event `playback_event`.

- [ ] **Step 1: Add the dependency**

`crates/oxitide-app/Cargo.toml` `[dependencies]`:
```toml
oxitide-audio = { path = "../oxitide-audio" }
```

- [ ] **Step 2: Write the failing test**

Create `crates/oxitide-app/src/player.rs`:
```rust
use std::sync::Arc;

use oxitide_audio::engine::{EngineEvent, OutputFactory, StreamResolver};
use oxitide_audio::output::{self, AudioOutput, OutputDevice};
use oxitide_audio::source;
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::stream::Quality;
use serde::{Deserialize, Serialize};

/// Resolves a track's stream plan using a `TidalClient`. Runs the async client
/// on a dedicated Tokio runtime owned by this resolver, so it can be called from
/// the engine's worker thread.
pub struct TidalResolver {
    client: TidalClient,
    runtime: tokio::runtime::Runtime,
}

impl TidalResolver {
    pub fn new(client: TidalClient) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        Ok(Self { client, runtime })
    }
}

impl StreamResolver for TidalResolver {
    fn resolve(&self, track_id: u64, quality: Quality) -> oxitide_audio::Result<oxitide_audio::source::StreamPlan> {
        let info = self
            .runtime
            .block_on(self.client.playback_info(track_id, quality))?;
        source::plan_from_info(&info)
    }
}

/// The real output factory: one `WasapiOutput` per track. Falling back to a
/// silent `NullOutput` on non-Windows keeps the crate building elsewhere.
pub struct RealOutputs;

impl OutputFactory for RealOutputs {
    fn create(&self) -> Box<dyn AudioOutput> {
        #[cfg(windows)]
        {
            Box::new(oxitide_audio::wasapi_output::WasapiOutput::new())
        }
        #[cfg(not(windows))]
        {
            Box::new(oxitide_audio::output::NullOutput::new())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityDto {
    Low,
    High,
    Lossless,
    Max,
}

impl QualityDto {
    pub fn to_quality(self) -> Quality {
        match self {
            QualityDto::Low => Quality::Low,
            QualityDto::High => Quality::High,
            QualityDto::Lossless => Quality::Lossless,
            QualityDto::Max => Quality::Max,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEventDto {
    State(oxitide_audio::engine::PlaybackState),
    NowPlaying(Option<oxitide_audio::queue::QueueItem>),
    Position { position_ms: u64, duration_ms: u64 },
    Error { code: String, message: String },
}

pub fn to_dto(event: EngineEvent) -> EngineEventDto {
    match event {
        EngineEvent::State(s) => EngineEventDto::State(s),
        EngineEvent::NowPlaying(i) => EngineEventDto::NowPlaying(i),
        EngineEvent::Position { position_ms, duration_ms } => EngineEventDto::Position { position_ms, duration_ms },
        EngineEvent::Error { code, message } => EngineEventDto::Error { code, message },
    }
}

#[derive(Serialize)]
pub struct OutputDeviceDto {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn devices() -> Vec<OutputDeviceDto> {
    output::enumerate_output_devices()
        .unwrap_or_default()
        .into_iter()
        .map(|OutputDevice { id, name, is_default }| OutputDeviceDto { id, name, is_default })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_dto_round_trips() {
        assert_eq!(QualityDto::Max.to_quality(), Quality::Max);
        assert_eq!(serde_json::to_value(QualityDto::Lossless).unwrap(), serde_json::json!("lossless"));
    }

    #[test]
    fn engine_event_dto_is_tagged() {
        let dto = to_dto(EngineEvent::Error { code: "auth".into(), message: "no".into() });
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["code"], "auth");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p oxitide-app player::`
Expected: FAIL to compile until `mod player;` is added to `main.rs` and types exist.

- [ ] **Step 4: Wire the engine into `AppState`**

`crates/oxitide-app/src/state.rs` — add an optional player:
```rust
use oxitide_audio::engine::Engine;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    // ...existing fields...
    pub player: RwLock<Option<Arc<Player>>>,
}

pub struct Player {
    pub engine: Engine,
    pub output_device: Mutex<Option<String>>,
}

impl AppState {
    // ...existing methods...

    pub fn set_player(&self, player: Player) {
        *self.player.write().unwrap() = Some(Arc::new(player));
    }

    pub fn player(&self) -> Option<Arc<Player>> {
        self.player.read().unwrap().clone()
    }
}
```
Update the `Default` impl: add `player: RwLock::new(None)` (`RwLock<Option<_>>: Default`). Ensure `Mutex` is imported.

- [ ] **Step 5: Emit engine events to the UI**

In `commands.rs`, create the player inside `auth_complete`/`auth_restore` after the client is set:
```rust
fn build_player(app: &tauri::AppHandle, client: TidalClient) -> Result<Player, String> {
    let resolver = crate::player::TidalResolver::new(client).map_err(|e| e.to_string())?;
    let engine = Engine::new(
        Arc::new(resolver),
        Arc::new(HttpFetcher::default()),
        Arc::new(crate::player::RealOutputs),
        Quality::Lossless,
    );
    let handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
    Ok(Player { engine, output_device: Mutex::new(None) })
}
```
Call `state.set_player(build_player(&app, client.clone())?)` in `auth_complete` and `auth_restore`, each of which gains an `app: tauri::AppHandle` parameter. Add to `commands.rs`: `use crate::state::Player;`, `use crate::player::RealOutputs;`, `use oxitide_audio::fetch::HttpFetcher;`, and `use oxitide_tidal::stream::Quality;`.

- [ ] **Step 6: Add the playback commands**

In `commands.rs`:
```rust
use oxitide_audio::engine::{EngineCmd, PlaybackState};
use oxitide_audio::queue::{QueueItem, RepeatMode};
use crate::player::{devices, OutputDeviceDto, QualityDto};

#[tauri::command]
pub fn play_tracks(state: State<'_, AppState>, items: Vec<QueueItem>, start: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::PlayTracks { items, start }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::PlayPause).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Next).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn prev(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Prev).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Seek(position_ms)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetVolume(volume)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetShuffle(enabled)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_repeat(state: State<'_, AppState>, mode: RepeatMode) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::SetRepeat(mode)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_reorder(state: State<'_, AppState>, from: usize, to: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Reorder { from, to }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, index: usize) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Remove(index)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.handle(EngineCmd::Clear).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackState, String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    Ok(player.engine.state())
}

#[tauri::command]
pub fn get_output_devices() -> Vec<OutputDeviceDto> {
    devices()
}

#[tauri::command]
pub fn set_output_device(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    *player.output_device.lock().unwrap() = Some(device_id);
    Ok(())
}

#[tauri::command]
pub fn set_preferred_quality(state: State<'_, AppState>, quality: QualityDto) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_quality(quality.to_quality());
    Ok(())
}
```

- [ ] **Step 7: Register commands and log out cleanly**

`main.rs`: add `mod player;` and register all new commands in `invoke_handler`. In `auth_logout`, also clear the player (`state.logout()` should set `player` to `None` as well as `client`).

- [ ] **Step 8: Run the tests**

Run: `cargo test -p oxitide-app`
Run: `cargo build --workspace`
Expected: PASS / success.

- [ ] **Step 9: Commit**

```bash
git add crates/oxitide-app
git commit -m "feat(app): wire playback engine commands and events"
```

---

### Task 13: UI event client and player store

**Files:**
- Modify: `ui/src/lib/api.ts`
- Create: `ui/src/lib/player.svelte.ts`
- Create: `ui/src/lib/player.test.ts`

**Interfaces:**
- Consumes: Tauri `invoke`, `listen`.
- Produces: `api.playTracks`, `api.playPause`, `api.next`, `api.prev`, `api.seek`, `api.setVolume`, `api.setShuffle`, `api.setRepeat`, `api.queueReorder`, `api.queueRemove`, `api.queueClear`, `api.getPlaybackState`, `api.getOutputDevices`, `api.setOutputDevice`, `api.setPreferredQuality`; `createPlayer()` store.

- [ ] **Step 1: Add types and API calls to `api.ts`**

Append:
```ts
export interface QueueItem {
  track_id: number; title: string; artist: string; album: string;
  duration_secs: number; cover?: string | null;
}
export type RepeatMode = "off" | "all" | "one";
export interface PlaybackState {
  playing: boolean; buffering: boolean; current?: QueueItem | null;
  position_ms: number; duration_ms: number; volume: number;
  shuffle: boolean; repeat: RepeatMode; queue: QueueItem[];
}
export interface OutputDevice { id: string; name: string; is_default: boolean }
export type EngineEvent =
  | ({ type: "state" } & PlaybackState)
  | { type: "now_playing"; [k: string]: unknown }
  | { type: "position"; position_ms: number; duration_ms: number }
  | { type: "error"; code: string; message: string };

export function trackToQueueItem(t: Track): QueueItem {
  return {
    track_id: t.id,
    title: t.title,
    artist: t.artists.map((a) => a.name).join(", "),
    album: t.album?.title ?? "",
    duration_secs: t.duration,
    cover: t.album?.cover ?? null,
  };
}
```
Add to the `api` object:
```ts
  playTracks: (items: QueueItem[], start: number) => invoke<void>("play_tracks", { items, start }),
  playPause: () => invoke<void>("play_pause"),
  next: () => invoke<void>("next"),
  prev: () => invoke<void>("prev"),
  seek: (positionMs: number) => invoke<void>("seek", { positionMs }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: RepeatMode) => invoke<void>("set_repeat", { mode }),
  queueReorder: (from: number, to: number) => invoke<void>("queue_reorder", { from, to }),
  queueRemove: (index: number) => invoke<void>("queue_remove", { index }),
  queueClear: () => invoke<void>("queue_clear"),
  getPlaybackState: () => invoke<PlaybackState>("get_playback_state"),
  getOutputDevices: () => invoke<OutputDevice[]>("get_output_devices"),
  setOutputDevice: (deviceId: string) => invoke<void>("set_output_device", { deviceId }),
  setPreferredQuality: (quality: "low" | "high" | "lossless" | "max") => invoke<void>("set_preferred_quality", { quality }),
```

- [ ] **Step 2: Write the failing store test**

`ui/src/lib/player.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { reduce, emptyState, type PlayerState } from "./player.svelte";

describe("player reducer", () => {
  it("applies a state event", () => {
    const next = reduce(emptyState(), {
      type: "state", playing: true, buffering: false, current: null,
      position_ms: 0, duration_ms: 0, volume: 0.5, shuffle: false,
      repeat: "off", queue: [],
    });
    expect(next.playing).toBe(true);
    expect(next.volume).toBe(0.5);
  });

  it("applies a position event without clobbering the queue", () => {
    const start: PlayerState = { ...emptyState(), queue: [
      { track_id: 1, title: "T", artist: "A", album: "", duration_secs: 10, cover: null },
    ] };
    const next = reduce(start, { type: "position", position_ms: 1000, duration_ms: 10000 });
    expect(next.position_ms).toBe(1000);
    expect(next.queue).toHaveLength(1);
  });

  it("captures errors", () => {
    const next = reduce(emptyState(), { type: "error", code: "auth", message: "nope" });
    expect(next.error).toBe("nope");
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `npm --prefix ui test`
Expected: FAIL — `./player.svelte` has no `reduce`/`emptyState`.

- [ ] **Step 4: Implement the store**

`ui/src/lib/player.svelte.ts`:
```ts
import { listen } from "@tauri-apps/api/event";
import { api, type EngineEvent, type PlaybackState, type QueueItem, type RepeatMode } from "./api";

export interface PlayerState {
  playing: boolean;
  buffering: boolean;
  current: QueueItem | null;
  position_ms: number;
  duration_ms: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  queue: QueueItem[];
  error: string;
}

export function emptyState(): PlayerState {
  return {
    playing: false, buffering: false, current: null, position_ms: 0,
    duration_ms: 0, volume: 1, shuffle: false, repeat: "off", queue: [], error: "",
  };
}

export function reduce(state: PlayerState, event: EngineEvent): PlayerState {
  switch (event.type) {
    case "state": {
      const { type, ...rest } = event;
      return { ...state, ...rest, error: "" };
    }
    case "position":
      return { ...state, position_ms: event.position_ms, duration_ms: event.duration_ms };
    case "now_playing":
      return state;
    case "error":
      return { ...state, error: event.message };
    default:
      return state;
  }
}

export function createPlayer() {
  let state = $state<PlayerState>(emptyState());

  async function init() {
    await listen<EngineEvent>("playback_event", (event) => {
      state = reduce(state, event.payload);
    });
    try {
      const snapshot = await api.getPlaybackState();
      state = reduce(state, { type: "state", ...snapshot });
    } catch {
      /* not logged in yet */
    }
  }

  return {
    get state() { return state; },
    init,
    playPause: () => api.playPause(),
    next: () => api.next(),
    prev: () => api.prev(),
    seek: (ms: number) => api.seek(ms),
    setVolume: (v: number) => api.setVolume(v),
    setShuffle: (enabled: boolean) => api.setShuffle(enabled),
    setRepeat: (mode: RepeatMode) => api.setRepeat(mode),
    reorder: (from: number, to: number) => api.queueReorder(from, to),
    remove: (index: number) => api.queueRemove(index),
    clear: () => api.queueClear(),
  };
}

export type Player = ReturnType<typeof createPlayer>;
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npm --prefix ui test`
Expected: PASS — existing `format` tests plus 3 new.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/player.svelte.ts ui/src/lib/player.test.ts
git commit -m "feat(ui): add playback api client and player store"
```

---

### Task 14: UI `PlayerBar`

**Files:**
- Modify: `ui/src/lib/components/PlayerBar.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `createPlayer()` store.
- Produces: a functional now-playing bar; `App.svelte` creates the player and passes it to `PlayerBar`/`QueueRail`.

- [ ] **Step 1: Implement `PlayerBar.svelte`**

```svelte
<script lang="ts">
  import { formatDuration } from "../format";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();

  let scrubbing = $state(false);
  let scrubValue = $state(0);

  const position = $derived(
    scrubbing ? scrubValue : player.state.position_ms,
  );
  const duration = $derived(player.state.duration_ms);

  function onSeekInput(e: Event) {
    scrubValue = Number((e.target as HTMLInputElement).value);
  }
  function onSeekCommit() {
    scrubbing = false;
    player.seek(scrubValue);
  }
</script>

<div class="player">
  {#if player.state.current}
    <div class="now">
      <strong>{player.state.current.title}</strong>
      <span class="muted">{player.state.current.artist}</span>
    </div>
    <button onclick={() => player.prev()} aria-label="Previous">⏮</button>
    <button onclick={() => player.playPause()} aria-label="Play/Pause">
      {player.state.playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => player.next()} aria-label="Next">⏭</button>
    <input
      type="range" min="0" max={Math.max(duration, 1)} value={position}
      oninput={onSeekInput}
      onpointerdown={() => (scrubbing = true)}
      onpointerup={onSeekCommit}
      onchange={onSeekCommit}
    />
    <span class="muted">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
    <input
      type="range" min="0" max="1" step="0.01" value={player.state.volume}
      oninput={(e) => player.setVolume(Number((e.target as HTMLInputElement).value))}
      aria-label="Volume"
    />
    {#if player.state.buffering}<span class="muted">buffering…</span>{/if}
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
  {#if player.state.error}<span style="color:#ff6b6b">{player.state.error}</span>{/if}
</div>
```
Add a minimal `Player` type alias in `player.svelte.ts` (`export type Player = ReturnType<typeof createPlayer>;`) and import it in the component.

- [ ] **Step 2: Wire it in `App.svelte`**

```svelte
  import { createPlayer } from "./lib/player.svelte";
  const player = createPlayer();
  let playerReady = $state(false);
  $effect(() => {
    if (loggedIn && !playerReady) {
      playerReady = true;
      player.init();
    }
  });
```
Pass `{player}` to `<PlayerBar />` and `<QueueRail />`.

- [ ] **Step 3: Type-check and build**

Run: `npm --prefix ui run check`
Run: `npm --prefix ui run build`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/PlayerBar.svelte ui/src/lib/player.svelte.ts ui/src/App.svelte
git commit -m "feat(ui): implement now-playing bar"
```

---

### Task 15: UI `QueueRail`, play actions, and settings slice

**Files:**
- Modify: `ui/src/lib/components/QueueRail.svelte`
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/views/Album.svelte`, `ui/src/views/Playlist.svelte`, `ui/src/views/Library.svelte`
- Create: `ui/src/views/Settings.svelte`
- Modify: `ui/src/lib/nav.svelte.ts`, `ui/src/App.svelte`, `ui/src/lib/components/Sidebar.svelte`

**Interfaces:**
- Consumes: `player` store; `api.getOutputDevices`/`setOutputDevice`/`setPreferredQuality`; `trackToQueueItem`.
- Produces: functional queue rail (click to jump, reorder, remove, clear), per-row play and "play all", and a Settings view.

- [ ] **Step 1: Implement `QueueRail.svelte`**

```svelte
<script lang="ts">
  import { api } from "../api";
  import type { Player } from "../player.svelte";
  let { player }: { player: Player } = $props();
  let dragFrom = $state<number | null>(null);

  function jump(index: number) {
    const items = player.state.queue;
    if (index >= 0 && index < items.length) {
      api.playTracks(items, index);
    }
  }
</script>

<div style="padding:12px">
  <div style="display:flex;justify-content:space-between;align-items:center">
    <strong>Queue</strong>
    <button onclick={() => player.clear()}>Clear</button>
  </div>
  <ul style="list-style:none;margin:8px 0;padding:0">
    {#each player.state.queue as item, i (item.track_id + "-" + i)}
      <li
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => { if (dragFrom !== null && dragFrom !== i) player.reorder(dragFrom, i); dragFrom = null; }}
        style="padding:4px;border-bottom:1px solid var(--line)"
      >
        <button onclick={() => jump(i)} style="all:unset;cursor:pointer">
          {item.title} <span class="muted">{item.artist}</span>
        </button>
        <button onclick={() => player.remove(i)} aria-label="Remove">✕</button>
      </li>
    {/each}
    {#if player.state.queue.length === 0}<li class="muted">Queue is empty</li>{/if}
  </ul>
</div>
```
Import the `Player` type from `../player.svelte`.

- [ ] **Step 2: Add a play action to `TrackTable.svelte`**

```svelte
<script lang="ts">
  import type { Player } from "../player.svelte";
  import type { Track } from "../api";
  import { api, trackToQueueItem } from "../api";
  import { formatDuration } from "../format";
  let { tracks, player }: { tracks: Track[]; player: Player } = $props();

  function play(index: number) {
    api.playTracks(tracks.map(trackToQueueItem), index);
  }
</script>
```
Add a header "Play all" button (`onclick={() => play(0)}`) and per-row:
```svelte
        <td><button onclick={() => play(i)} aria-label="Play">▶</button></td>
```
Also update the table's empty-state `colspan` from `3` to `4` and add a matching `<th></th>` to the header row.

- [ ] **Step 3: Pass `player` into `TrackTable` from the views**

In `Album.svelte`, `Playlist.svelte`, `Library.svelte`: add `import type { Player } from "../lib/player.svelte";`, accept `player` as a prop, and render `<TrackTable {tracks} {player} />`. Update `App.svelte` to pass `{player}` into those views (they already receive `nav`; add `player`).

- [ ] **Step 4: Add the Settings view**

`ui/src/views/Settings.svelte`:
```svelte
<script lang="ts">
  import { api, type OutputDevice } from "../lib/api";
  let devices = $state<OutputDevice[]>([]);
  let quality = $state<"low" | "high" | "lossless" | "max">("lossless");
  let error = $state("");

  $effect(() => {
    api.getOutputDevices().then((d) => (devices = d)).catch((e) => (error = String(e)));
  });
</script>

<h2>Settings</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
<h3>Audio output</h3>
<select onchange={(e) => {
  const value = (e.target as HTMLSelectElement).value;
  if (value) api.setOutputDevice(value).catch((err) => (error = String(err)));
}}>
  <option value="">System default</option>
  {#each devices as d (d.id)}
    <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
  {/each}
</select>
<h3>Streaming quality</h3>
<select bind:value={quality} onchange={() => api.setPreferredQuality(quality)}>
  <option value="low">Low</option>
  <option value="high">High</option>
  <option value="lossless">Lossless</option>
  <option value="max">Max (Hi-Res)</option>
</select>
```

- [ ] **Step 5: Add the Settings route**

`nav.svelte.ts`: add `| { name: "settings" }` to `Route`. `Sidebar.svelte`: add a Settings button. `App.svelte`: render `<Settings />` for `nav.route.name === "settings"`.

- [ ] **Step 6: Type-check, build, and test**

Run: `npm --prefix ui run check`
Run: `npm --prefix ui test`
Run: `npm --prefix ui run build`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add ui/src
git commit -m "feat(ui): wire queue rail, play actions, and settings slice"
```

---

### Task 16: End-to-end verification and smoke checklist

**Files:**
- Create: `docs/superpowers/plans/notes/plan2-smoke.md`
- Modify: `docs/superpowers/plans/notes/plan2-smoke.md` (record results)

**Interfaces:**
- Consumes: everything from Tasks 1–15.
- Produces: a verified build and a human smoke checklist.

- [ ] **Step 1: Run the full automated suite**

Run: `cargo test --workspace`
Run: `cargo clippy --workspace -- -D warnings`
Run: `npm --prefix ui test`
Run: `npm --prefix ui run check`
Expected: all green. Fix any warnings before proceeding.

- [ ] **Step 2: Write the smoke checklist**

`docs/superpowers/plans/notes/plan2-smoke.md`:
```markdown
# Plan 2 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and an audio device.
> Run with `npm --prefix ui run app:dev`.

- [ ] Clicking ▶ on an album track starts playback (sound is audible).
- [ ] Player bar shows the track title and artist.
- [ ] Play/pause toggles playback; the icon updates.
- [ ] Next/prev move through the album's tracks.
- [ ] The seek slider moves; the elapsed time updates.
- [ ] The volume slider changes loudness.
- [ ] Clicking Queue shows the queue with the current track.
- [ ] Queue reorder (drag), remove, and clear work.
- [ ] Settings lists output devices and changing quality persists.
- [ ] A track at Max (Hi-Res) plays (24-bit/192 kHz source is decoded).
- [ ] Logging out stops playback and clears the queue.
```

- [ ] **Step 3: Run the smoke test**

Run: `npm --prefix ui run app:dev`
Manually execute each checklist item and tick the boxes in the file. Record any failures inline.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "test: add plan 2 smoke checklist and verify"
```

---

## Self-Review (completed by author)

- **Spec coverage:** Plan 2 scope §2 — `oxitide-tidal::stream` (Task 2), `oxitide-audio` modules `source`/`fetch`/`demux`/`decode`/`output`/`queue`/`engine` (Tasks 3–11), FLAC playback (Tasks 7, 11), WASAPI shared + software volume (Tasks 9, 14), queue state machine (Task 10), IPC commands/events (Task 12), UI PlayerBar/QueueRail/play actions/settings slice (Tasks 13–15), fixture pipeline test (Tasks 7, 11). Deferred items (exclusive/bit-perfect, gapless, prefetch/disk cache, quality fallback, AAC, full Settings) are intentionally absent.
- **Placeholder scan:** no TBD/TODO in the deliverable code paths; every code step contains runnable code or an exact command. Two implementation notes (symphonia channel-count helper, wasapi method signatures) name the exact fallback and the doc URL to consult, rather than leaving the behavior undefined.
- **Type consistency:** `QueueItem`, `RepeatMode`, `PlaybackState`, `EngineEvent`, `EngineCmd`, `StreamPlan`, `AudioFormat`, `AudioOutput`, `Fetcher`, `StreamResolver`, `PcmSink`, and `OutputDevice` are each defined once and reused with identical field/method names across tasks. The TS mirror (`QueueItem`, `PlaybackState`, `EngineEvent`, `OutputDevice`, `RepeatMode`) matches the Rust serde shape (snake_case fields, tagged `EngineEventDto` via `type`).
- **Review Focus tests:** mid-stream fetch failure (Task 5 `fetch_failure_is_reported_to_readers`), empty/edge queue bounds (Task 10 `next_at_end_stops_when_repeat_off`, `prev_at_start_is_a_noop`, `remove_current_advances_to_next`), manifest shape errors (Tasks 3, 4), output open failure path (Task 9 typed `Error::Output`), missing metadata defaults (Task 10 `QueueItem` serde defaults; Task 13 `trackToQueueItem`).
- **Verified assumptions:** symphonia 0.6.1 decodes both the generated `tone.flac` and `tone.mp4` fixtures to 176,400 interleaved `f32` samples at 44.1 kHz stereo (spike run during planning); crate versions pinned from crates.io.
