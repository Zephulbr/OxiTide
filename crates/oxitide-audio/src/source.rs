use oxitide_tidal::stream::PlaybackInfo;

use crate::error::{Error, Result};
use crate::mpd;

pub const DASH_MIME: &str = "application/dash+xml";
pub const BTS_MIME: &str = "application/vnd.tidal.bts";

/// One media segment of a stream plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub url: String,
    pub start_ticks: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamPlan {
    pub init_url: Option<String>,
    pub segments: Vec<Segment>,
    /// Ticks per second for `Segment.start_ticks`/`duration_ticks`; 0 for a
    /// single-file source with no timing information.
    pub timescale: u64,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
}

impl StreamPlan {
    /// Media URLs in order.
    pub fn media_urls(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.url.as_str()).collect()
    }

    /// A single-file source with no segment timing (BTS manifests, tests).
    pub fn single(url: String) -> Self {
        Self {
            init_url: None,
            segments: vec![Segment { url, start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
        }
    }

    /// The same plan starting at media segment `index`; the init segment is
    /// kept. Used to restart playback at a seek target.
    pub fn from_segment(&self, index: usize) -> StreamPlan {
        let mut plan = self.clone();
        plan.segments = self.segments.iter().skip(index).cloned().collect();
        plan
    }
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
            let timings = repr.segment_timings(duration)?;
            let init_url = repr.init_url(&doc.base_urls);
            debug_assert_eq!(media_urls.len(), timings.len());
            let timescale = repr
                .template
                .as_ref()
                .map(|t| if t.timescale == 0 { 1 } else { t.timescale })
                .unwrap_or(0);
            let segments = media_urls
                .into_iter()
                .zip(timings)
                .map(|(url, timing)| Segment {
                    url,
                    start_ticks: timing.start_ticks,
                    duration_ticks: timing.duration_ticks,
                })
                .collect::<Vec<_>>();
            if segments.is_empty() {
                return Err(Error::Manifest("dash manifest has no media segments".into()));
            }
            Ok(StreamPlan {
                init_url,
                segments,
                timescale,
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
                segments: parsed
                    .urls
                    .into_iter()
                    .map(|url| Segment { url, start_ticks: 0, duration_ticks: 0 })
                    .collect(),
                timescale: 0,
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
        assert_eq!(plan.media_urls(), vec!["https://cdn/x/seg-1.mp4", "https://cdn/x/seg-2.mp4"]);
        assert_eq!(plan.sample_rate, Some(48000));
        assert_eq!(plan.bit_depth, Some(24));
    }

    #[test]
    fn tidal_dash_manifest_with_numbered_timeline_builds_all_segments() {
        let mpd = r#"<?xml version='1.0' encoding='UTF-8'?><MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT8M2.83S"><Period id="0"><AdaptationSet id="0" contentType="audio" mimeType="audio/mp4" group="main" segmentAlignment="true"><Representation id="FLAC,44100,16" codecs="flac" bandwidth="828801" audioSamplingRate="44100"><SegmentTemplate timescale="44100" initialization="https://cdn.example/mediatracks/TRACK/0.mp4?token=T" media="https://cdn.example/mediatracks/TRACK/$Number$.mp4?token=T" startNumber="1"><SegmentTimeline><S d="176128" r="119"/><S d="157448"/></SegmentTimeline></SegmentTemplate></Representation></AdaptationSet></Period></MPD>"#;
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(
            plan.init_url.as_deref(),
            Some("https://cdn.example/mediatracks/TRACK/0.mp4?token=T")
        );
        assert_eq!(plan.media_urls().len(), 121);
        assert_eq!(plan.media_urls()[0], "https://cdn.example/mediatracks/TRACK/1.mp4?token=T");
        assert_eq!(plan.media_urls()[120], "https://cdn.example/mediatracks/TRACK/121.mp4?token=T");
        assert_eq!(plan.codec.as_deref(), Some("flac"));
        assert_eq!(plan.sample_rate, Some(44100));
        assert_eq!(plan.timescale, 44100);
    }

    #[test]
    fn dash_manifest_without_media_segments_is_a_typed_error() {
        let mpd = r#"<MPD mediaPresentationDuration="PT0S"><Period><AdaptationSet>
          <Representation id="audio" codecs="flac">
            <SegmentTemplate initialization="init.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
          </Representation></AdaptationSet></Period></MPD>"#;
        assert!(matches!(
            plan_from_info(&info(DASH_MIME, mpd.as_bytes())),
            Err(Error::Manifest(_))
        ));
    }

    #[test]
    fn bts_plan_uses_direct_urls() {
        let json = r#"{"mimeType":"audio/flac","codecs":"flac","urls":["https://cdn/a.flac"]}"#;
        let plan = plan_from_info(&info(BTS_MIME, json.as_bytes())).unwrap();
        assert_eq!(plan.init_url, None);
        assert_eq!(plan.media_urls(), vec!["https://cdn/a.flac"]);
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

    #[test]
    fn from_segment_keeps_init_and_truncates() {
        let mpd = r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
          <BaseURL>https://cdn/x/</BaseURL>
          <Representation id="audio" codecs="flac" audioSamplingRate="48000" bitDepth="24">
            <SegmentTemplate initialization="init.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
          </Representation></AdaptationSet></Period></MPD>"#;
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        let tail = plan.from_segment(1);
        assert_eq!(tail.init_url.as_deref(), Some("https://cdn/x/init.mp4"));
        assert_eq!(tail.media_urls(), vec!["https://cdn/x/seg-2.mp4"]);
        assert_eq!(tail.timescale, plan.timescale);
        assert_eq!(tail.segments[0].start_ticks, 4);
    }
}
