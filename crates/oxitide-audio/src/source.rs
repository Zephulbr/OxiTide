use oxitide_tidal::stream::{PlaybackInfo, Quality, ReplayGain};

use crate::error::{Error, Result};
use crate::mpd;

pub const DASH_MIME: &str = "application/dash+xml";
pub const BTS_MIME: &str = "application/vnd.tidal.bts";

/// One media segment of a stream plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    pub url: String,
    pub start_ticks: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamPlan {
    pub init_url: Option<String>,
    pub segments: Vec<Segment>,
    /// Ticks per second for `Segment.start_ticks`/`duration_ticks`; 0 for a
    /// single-file source with no timing information.
    pub timescale: u64,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    /// Manifest bandwidth in kbps, when the manifest declares one.
    pub bitrate_kbps: Option<u32>,
    /// The quality the API says it served, or a value derived from the manifest.
    pub quality: Option<Quality>,
    /// FNV-1a 64 over the query-stripped URLs and format fields. `None` for
    /// plans built directly (tests, `StreamPlan::single`) — such plans bypass
    /// the cache.
    pub manifest_hash: Option<u64>,
    /// Loudness metadata from `playbackinfo` (all `None` when absent).
    pub replay_gain: ReplayGain,
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
            bitrate_kbps: None,
            quality: None,
            manifest_hash: None,
            replay_gain: ReplayGain::default(),
        }
    }

    /// The plan with every URL removed: what a cached file needs to be
    /// decoded and described without the network. Signed CDN URLs must not
    /// be written to disk.
    pub fn without_urls(&self) -> StreamPlan {
        let mut plan = self.clone();
        plan.init_url = None;
        for segment in &mut plan.segments {
            segment.url.clear();
        }
        plan
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

/// FNV-1a 64 over `bytes`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Signed CDN tokens live in the query string and rotate between
/// `playbackinfo` calls; hash only the stable part of a URL.
fn strip_query(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

fn manifest_hash(
    init_url: Option<&str>,
    segments: &[Segment],
    codec: Option<&str>,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
    timescale: u64,
) -> u64 {
    let mut canonical = String::new();
    if let Some(init) = init_url {
        canonical.push_str(strip_query(init));
    }
    canonical.push('\n');
    for segment in segments {
        canonical.push_str(strip_query(&segment.url));
        canonical.push('\n');
    }
    canonical.push_str(&format!(
        "{codec:?}|{sample_rate:?}|{bit_depth:?}|{timescale}"
    ));
    fnv1a64(canonical.as_bytes())
}

/// Best-effort quality for a manifest that does not declare `audioQuality`.
fn derive_quality(
    codec: Option<&str>,
    bit_depth: Option<u32>,
    sample_rate: Option<u32>,
    bandwidth: Option<u64>,
) -> Option<Quality> {
    let codec = codec?.to_ascii_lowercase();
    if codec.contains("flac") || codec.contains("alac") {
        let hi_res = bit_depth.unwrap_or(0) >= 24 || sample_rate.unwrap_or(0) > 48_000;
        Some(if hi_res { Quality::Max } else { Quality::Lossless })
    } else if is_he_aac(&codec) {
        Some(Quality::Low)
    } else if codec.contains("mp4a") || codec.contains("aac") {
        Some(if bandwidth.is_some_and(|b| b < 192_000) { Quality::Low } else { Quality::High })
    } else {
        None
    }
}

/// True for explicit HE-AAC codec strings (AAC + SBR/PS). Implicit SBR/PS
/// cannot be identified from an AAC-LC manifest codec string alone.
pub fn is_he_aac(codec: &str) -> bool {
    let codec = codec.to_ascii_lowercase();
    codec.contains("mp4a.40.5")
        || codec.contains("mp4a.40.05")
        || codec.contains("mp4a.40.29")
        || codec.contains("mp4a.40.029")
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
            let bitrate_kbps = repr.bandwidth.map(|b| (b / 1000) as u32);
            let quality = info.actual_quality.or_else(|| {
                derive_quality(repr.codecs.as_deref(), repr.bit_depth, repr.sample_rate, repr.bandwidth)
            });
            let hash = manifest_hash(
                init_url.as_deref(),
                &segments,
                repr.codecs.as_deref(),
                repr.sample_rate,
                repr.bit_depth,
                timescale,
            );
            Ok(StreamPlan {
                init_url,
                segments,
                timescale,
                codec: repr.codecs.clone(),
                sample_rate: repr.sample_rate,
                bit_depth: repr.bit_depth,
                bitrate_kbps,
                quality,
                manifest_hash: Some(hash),
                replay_gain: info.replay_gain,
            })
        }
        BTS_MIME => {
            let parsed: BtsManifest = serde_json::from_slice(&info.manifest)
                .map_err(|e| Error::Manifest(format!("bad bts manifest: {e}")))?;
            if parsed.urls.is_empty() {
                return Err(Error::Manifest("bts manifest has no urls".into()));
            }
            let segments: Vec<Segment> = parsed
                .urls
                .into_iter()
                .map(|url| Segment { url, start_ticks: 0, duration_ticks: 0 })
                .collect();
            let quality = info
                .actual_quality
                .or_else(|| derive_quality(parsed.codecs.as_deref(), None, None, None));
            let hash = manifest_hash(None, &segments, parsed.codecs.as_deref(), None, None, 0);
            Ok(StreamPlan {
                init_url: None,
                segments,
                timescale: 0,
                codec: parsed.codecs,
                sample_rate: None,
                bit_depth: None,
                bitrate_kbps: None,
                quality,
                manifest_hash: Some(hash),
                replay_gain: info.replay_gain,
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
        PlaybackInfo {
            quality: Quality::Max,
            actual_quality: None,
            manifest_mime: mime.into(),
            manifest: body.to_vec(),
            replay_gain: Default::default(),
        }
    }

    fn info_with_quality(mime: &str, body: &[u8], actual: Option<Quality>) -> PlaybackInfo {
        PlaybackInfo {
            quality: Quality::Max,
            actual_quality: actual,
            manifest_mime: mime.into(),
            manifest: body.to_vec(),
            replay_gain: Default::default(),
        }
    }

    fn flac_mpd(codec: &str, rate: u32, bits: u32) -> String {
        format!(
            r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
              <Representation id="audio" codecs="{codec}" audioSamplingRate="{rate}" bitDepth="{bits}">
                <SegmentTemplate initialization="init.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
              </Representation></AdaptationSet></Period></MPD>"#
        )
    }

    fn mpd_with_token(token: &str) -> String {
        format!(
            r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
              <Representation id="audio" codecs="flac" audioSamplingRate="48000" bitDepth="24">
                <SegmentTemplate initialization="init.mp4?token={token}" media="seg-$Number$.mp4?token={token}" startNumber="1" duration="4" timescale="1"/>
              </Representation></AdaptationSet></Period></MPD>"#
        )
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
    fn plans_carry_the_replay_gain_metadata() {
        let gain = ReplayGain {
            track_gain_db: Some(-7.5),
            track_peak: Some(0.9),
            album_gain_db: Some(-6.0),
            album_peak: Some(1.0),
        };
        let mut dash = info(DASH_MIME, flac_mpd("flac", 44100, 16).as_bytes());
        dash.replay_gain = gain;
        assert_eq!(plan_from_info(&dash).unwrap().replay_gain, gain);
        let mut bts = info(BTS_MIME, br#"{"codecs":"flac","urls":["https://cdn/a.flac"]}"#);
        bts.replay_gain = gain;
        assert_eq!(plan_from_info(&bts).unwrap().replay_gain, gain);
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

    #[test]
    fn explicit_audio_quality_wins_over_derivation() {
        let mpd = flac_mpd("flac", 44100, 16);
        let plan =
            plan_from_info(&info_with_quality(DASH_MIME, mpd.as_bytes(), Some(Quality::High))).unwrap();
        assert_eq!(plan.quality, Some(Quality::High));
    }

    #[test]
    fn quality_is_derived_when_audio_quality_is_absent() {
        let hi_res = flac_mpd("flac", 48000, 24);
        let lossless = flac_mpd("flac", 44100, 16);
        let aac_low = flac_mpd("mp4a.40.2", 44100, 16).replace("bitDepth=\"16\"", "bandwidth=\"128000\"");
        let aac_high = flac_mpd("mp4a.40.2", 44100, 16).replace("bitDepth=\"16\"", "bandwidth=\"320000\"");

        assert_eq!(
            plan_from_info(&info(DASH_MIME, hi_res.as_bytes())).unwrap().quality,
            Some(Quality::Max)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, lossless.as_bytes())).unwrap().quality,
            Some(Quality::Lossless)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, aac_low.as_bytes())).unwrap().quality,
            Some(Quality::Low)
        );
        assert_eq!(
            plan_from_info(&info(DASH_MIME, aac_high.as_bytes())).unwrap().quality,
            Some(Quality::High)
        );
    }

    #[test]
    fn he_aac_derives_low_without_overriding_api_quality() {
        for codec in ["mp4a.40.5", "mp4a.40.05", "mp4a.40.29", "mp4a.40.029"] {
            let dash = flac_mpd(codec, 24000, 16);
            let bts =
                serde_json::json!({ "codecs": codec, "urls": ["https://cdn/a.mp4"] }).to_string();
            for (mime, body) in [(DASH_MIME, dash.as_bytes()), (BTS_MIME, bts.as_bytes())] {
                assert_eq!(
                    plan_from_info(&info(mime, body)).unwrap().quality,
                    Some(Quality::Low)
                );
                let explicit = info_with_quality(mime, body, Some(Quality::High));
                assert_eq!(
                    plan_from_info(&explicit).unwrap().quality,
                    Some(Quality::High)
                );
            }
        }
    }

    #[test]
    fn requested_low_is_not_evidence_of_served_quality() {
        for (codec, expected) in [("mp4a.40.2", Some(Quality::High)), ("unknown", None)] {
            let bts =
                serde_json::json!({ "codecs": codec, "urls": ["https://cdn/a.mp4"] }).to_string();
            let mut requested_low = info(BTS_MIME, bts.as_bytes());
            requested_low.quality = Quality::Low;
            assert_eq!(plan_from_info(&requested_low).unwrap().quality, expected);
        }
    }

    #[test]
    fn unknown_codec_derives_no_quality() {
        let mpd = flac_mpd("opus", 44100, 16);
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(plan.quality, None);
    }

    #[test]
    fn bitrate_comes_from_dash_bandwidth() {
        let mpd = flac_mpd("flac", 44100, 16).replace("bitDepth=\"16\"", "bitDepth=\"16\" bandwidth=\"828801\"");
        let plan = plan_from_info(&info(DASH_MIME, mpd.as_bytes())).unwrap();
        assert_eq!(plan.bitrate_kbps, Some(828));
    }

    #[test]
    fn bts_plan_has_no_bitrate() {
        let json = r#"{"mimeType":"audio/flac","codecs":"flac","urls":["https://cdn/a.flac?token=A"]}"#;
        let plan = plan_from_info(&info(BTS_MIME, json.as_bytes())).unwrap();
        assert_eq!(plan.bitrate_kbps, None);
        assert_eq!(plan.quality, Some(Quality::Lossless));
    }

    #[test]
    fn manifest_hash_ignores_signed_tokens() {
        let a = plan_from_info(&info(DASH_MIME, mpd_with_token("A").as_bytes())).unwrap();
        let b = plan_from_info(&info(DASH_MIME, mpd_with_token("B").as_bytes())).unwrap();
        assert!(a.manifest_hash.is_some());
        assert_eq!(a.manifest_hash, b.manifest_hash, "token rotation changed the cache key");
    }

    #[test]
    fn manifest_hash_detects_a_different_stream() {
        let a = plan_from_info(&info(DASH_MIME, mpd_with_token("A").as_bytes())).unwrap();
        let other = mpd_with_token("A").replace("bitDepth=\"24\"", "bitDepth=\"16\"");
        let b = plan_from_info(&info(DASH_MIME, other.as_bytes())).unwrap();
        assert_ne!(a.manifest_hash, b.manifest_hash);
    }

    #[test]
    fn fnv1a64_is_deterministic() {
        assert_eq!(fnv1a64(b"abc"), fnv1a64(b"abc"));
        assert_ne!(fnv1a64(b"abc"), fnv1a64(b"abd"));
    }

    #[test]
    fn he_aac_codec_strings_are_detected() {
        assert!(is_he_aac("mp4a.40.5"));
        assert!(is_he_aac("mp4a.40.05"));
        assert!(is_he_aac("mp4a.40.29"));
        assert!(is_he_aac("mp4a.40.029"));
        assert!(is_he_aac("MP4A.40.5"));
        assert!(!is_he_aac("mp4a.40.2"));
        assert!(!is_he_aac("mp4a.40.02"));
        assert!(!is_he_aac("flac"));
    }
}
