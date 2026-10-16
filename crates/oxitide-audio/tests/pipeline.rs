//! M4 gate: resolve -> plan -> fetch -> demux -> decode against a wiremock CDN.

use std::sync::Arc;

use oxitide_audio::source::{self, BTS_MIME};
use oxitide_audio::{decode, demux, fetch};

use oxitide_tidal::stream::{PlaybackInfo, Quality};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn resolve_plan_fetch_demux_decode_pipeline() {
    let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tone.mp4"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;

    let manifest = serde_json::json!({
        "mimeType": "audio/flac",
        "codecs": "flac",
        "urls": [format!("{}/tone.mp4", server.uri())],
    })
    .to_string();
    let info = PlaybackInfo {
        quality: Quality::Lossless,
        actual_quality: None,
        manifest_mime: BTS_MIME.into(),
        manifest: manifest.into_bytes(),
        replay_gain: Default::default(),
    };

    let plan = source::plan_from_info(&info).unwrap();
    assert_eq!(plan.codec.as_deref(), Some("flac"));
    assert_eq!(
        plan.media_urls(),
        vec![format!("{}/tone.mp4", server.uri())]
    );

    // `HttpFetcher` wraps a blocking client with its own runtime; create and
    // drop it inside the blocking task so no runtime is dropped on the async
    // thread.
    let buffer = fetch::StreamBuffer::new();
    let download_buffer = buffer.clone();
    tokio::task::spawn_blocking(move || {
        let fetcher: Arc<dyn fetch::Fetcher> = Arc::new(fetch::HttpFetcher::new().unwrap());
        fetch::download(download_buffer, fetcher, plan, None);
    })
    .await
    .unwrap();

    let mut sink = decode::CollectSink::new();
    let source = demux::StreamBufferSource::new(buffer);
    decode::run(source, "mp4", &mut sink).unwrap();

    assert_eq!(sink.samples.len(), 176_400); // 2s * 44100 * 2ch
    assert!(sink.samples.iter().all(|s| s.is_finite()));
}

#[tokio::test(flavor = "multi_thread")]
async fn he_aac_http_pipeline_decodes_all_six_generated_fixtures() {
    let server = MockServer::start().await;
    for version in [1, 2] {
        for signaling in ["hierarchical", "implicit", "sync"] {
            let name = format!("he_aac_v{version}_96k_{signaling}.mp4");
            let bytes = std::fs::read(format!("tests/fixtures/{name}")).unwrap();
            Mock::given(method("GET"))
                .and(path(format!("/{name}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
                .expect(1)
                .mount(&server)
                .await;
            let codec = if signaling == "hierarchical" {
                if version == 1 {
                    "mp4a.40.5"
                } else {
                    "mp4a.40.29"
                }
            } else {
                "mp4a.40.2"
            };
            let manifest = serde_json::json!({
                "codecs": codec,
                "urls": [format!("{}/{name}", server.uri())],
            });
            let plan = source::plan_from_info(&PlaybackInfo {
                quality: Quality::Max,
                actual_quality: if signaling == "hierarchical" {
                    None
                } else {
                    Some(Quality::Low)
                },
                manifest_mime: BTS_MIME.into(),
                manifest: manifest.to_string().into_bytes(),
                replay_gain: Default::default(),
            })
            .unwrap();
            assert_eq!(plan.quality, Some(Quality::Low));
            let sink = tokio::task::spawn_blocking(move || {
                let buffer = fetch::StreamBuffer::new();
                let fetcher = Arc::new(fetch::HttpFetcher::new().unwrap());
                let he_aac = plan.needs_he_aac();
                fetch::download(buffer.clone(), fetcher, plan, None);
                let mut sink = decode::CollectSink::new();
                decode::run_with_aac_hint(
                    demux::StreamBufferSource::new(buffer),
                    "mp4",
                    he_aac,
                    &mut sink,
                )
                .unwrap();
                sink
            })
            .await
            .unwrap();
            let info = sink.info.unwrap();
            assert_eq!(info.sample_rate, 48000, "{name}");
            assert_eq!(info.channels, 2, "{name}");
            assert_eq!(info.format, decode::PcmFormat::F32);
            assert!(!info.is_bit_perfect_candidate());
            let frames = if version == 1 { 149504 } else { 151552 };
            assert_eq!(sink.samples.len(), frames * 2, "{name}");
            assert!(sink.samples.iter().all(|sample| sample.is_finite()));
        }
    }
}
