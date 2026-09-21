use oxitide_tidal::stream::PlaybackInfo;

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

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_tidal::stream::Quality;

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
