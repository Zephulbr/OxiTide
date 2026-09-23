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
    };

    let plan = source::plan_from_info(&info).unwrap();
    assert_eq!(plan.codec.as_deref(), Some("flac"));
    assert_eq!(plan.media_urls(), vec![format!("{}/tone.mp4", server.uri())]);

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
