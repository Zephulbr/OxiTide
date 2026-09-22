# OxiTide Plan 3 — Bit-perfect, Gapless & Seek Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the working Plan 2 player into a bit-perfect daily driver: typed integer PCM with no float round-trip, WASAPI exclusive output at the source's native rate/bit depth, gapless queue chaining, a working seek bar, device hot-plug recovery, and a persisted minimal `config.toml` with a bit-perfect Settings toggle.

**Architecture:** `oxitide-audio` gains typed PCM (`PcmFormat`/`StreamInfo`/`PcmBuffer`), a reworked output trait (`AudioOutputSpec`, volume on the output, endpoint volume), a bounded download buffer, and a single long-lived playback worker that owns one persistent `AudioOutput` while per-track producer threads (resolve → fetch → decode) feed it a bounded PCM channel. Gapless chaining happens on `StreamInfo` equality with a ~10 s PCM lookahead; seek restarts at the DASH segment covering the target and discards to the exact sample. `oxitide-app` gains `config.toml` persistence and new IPC commands; the Svelte UI gains the bit-perfect toggle, badge/notice, and an interactive seek slider.

**Tech Stack:** Rust 2021, stable toolchain, Tokio, reqwest (+ `blocking`), quick-xml, symphonia 0.6, rtrb 0.4, wasapi 0.24 + `windows = "0.62"`, `toml = "1"`, `directories = "6"`; Tauri 2; Svelte 5 + Vite + TypeScript; Vitest.

**Spec:** `docs/superpowers/specs/2026-09-21-oxitide-plan3-bitperfect-gapless-design.md`

**Approved refinements to the spec (recorded here so executors know the wider contract):**

1. `AudioOutput` gains `fn status(&self) -> OutputStatus` so the engine can report the actual `mode`, `bit_perfect`, and `notice`. The spec's trait has no status channel.
2. A new `get_config` Tauri command returns the persisted config so Settings can hydrate; `set_output_device` and `set_preferred_quality` now persist. Settings reconciles a stored device id that no longer exists back to "System default".
3. Changing output device/mode/fallback/quality while playing restarts the current track **at its current position** (implemented with the seek path in Task 8, on the worker delivered by Task 6).
4. Device loss pauses (`playing = false`, queue and current item kept), stores a notice, and recovers on the next `PlayPause`; a stale device id falls back to the system default.
5. `Engine::new` takes `Box<dyn AudioOutput>` instead of `Arc<dyn OutputFactory>`; the worker owns the output. Gapless/prefetch requires decoding the next track while the current one plays, so decode runs on per-track producer threads feeding a bounded `std::sync::mpsc` channel that the worker drains.

## Global Constraints

- Rust edition 2021; toolchain pinned by `rust-toolchain.toml` (`stable`, clippy + rustfmt installed). Node 20+. Tauri 2.x. Svelte 5 runes only (`$state`/`$props`/`$effect`; no stores, no `export let`).
- Windows-first. `keyring` uses the `windows-native` feature. Config/app dir: `%APPDATA%\OxiTide`; file `config.toml`. Keyring service name: `oxitide`.
- Verified dependency floors: `wasapi = "0.24"`, `windows = "0.62"` (must match wasapi's transitive `windows`), `symphonia = "0.6"` (`flac`, `isomp4`, `pcm`), `rtrb = "0.4"`, `quick-xml = "0.42"`, `tokio = "1"`, `reqwest = "0.12"` (`blocking`), `serde = "1"`, `toml = "1"`, `directories = "6"`.
- `oxitide-audio` must not depend on Tauri or UI code, and must never read `config.toml` itself (the engine receives audio settings through setters/commands only).
- Every new `#[tauri::command]` must also be added to `generate_handler![...]` in `crates/oxitide-app/src/main.rs`.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`redirect_url` ↔ `redirectUrl`). Rust DTOs are the source of truth; update `ui/src/lib/api.ts` by hand.
- No DRM, no key extraction, no credential/token logging. Tokens stay in the OS keyring; unit tests use `MemoryStore`.
- No live network in tests: HTTP tests use `wiremock`; parser tests use inline fixtures; audio tests use committed tone fixtures and `NullOutput`.
- CI gates (all must pass at the end of every task that touches Rust/UI): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
- Conventional commits (`feat:`, `fix:`, `test:`, `chore:`, `docs:`). Commit at the end of every task. Never commit generated installers or secrets.

## Review Focus

Uncovered input classes / failure modes most likely to bite a real user; each is pinned to a test in its owning task:

1. **Exclusive format unsupported on a real DAC** — the user expects either playback to continue (configured fallback) or a clear, non-crashing error. Owning task: 3 (`exclusive_formats_chain_valid_bits`) and 6 (`output_open_failure_pauses_with_notice`).
2. **Endpoint volume a no-op on an exclusive driver** — the slider must still change loudness and the UI must say bit-perfect is compromised. Owning task: 3 (`null_output_records_i32_with_software_volume`) and 6 (`output_status_notice_flows_to_playback_state`).
3. **Device unplugged mid-track** — pause with a notice, keep the queue, recover on the next play press, never panic. Owning task: 9 (`output_failure_pauses_keeps_queue_and_recovers`).
4. **Seek beyond track length / timeline MPDs** — clamp to the last segment, no error, no panic. Owning task: 8 (`target_segment_clamps_beyond_the_end`).
5. **Malformed or partial `config.toml`** — back up to `config.toml.bak`, fall back to defaults, never crash on startup. Owning task: 10 (`malformed_file_is_backed_up_and_defaults_used`).

---

### Task 1: Plan 2 smoke verification

**Files:**
- Modify: `docs/superpowers/plans/notes/plan2-smoke.md`
- Modify (only if the smoke run surfaces a real bug): whatever file the bug lives in

**Interfaces:**
- Consumes: the shipped Plan 2 build.
- Produces: a recorded, checked Plan 2 smoke checklist; any bug fixes land before Plan 3 edits the same code.

- [ ] **Step 1: Run the automated gates and record exact results**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```
Expected: all green. Note the counts (e.g. "47 Rust tests, 5 UI tests"). If anything fails, fix it first via `superpowers:systematic-debugging` (failing test first), and commit the fix separately.

- [ ] **Step 2: Run the Plan 2 human checklist**

Start the app with a real account and a real audio device:
```powershell
npm --prefix ui run app:dev
```
Have the human run every item below and report pass/fail:

```
- [ ] Clicking ▶ on an album track starts playback (sound is audible).
- [ ] Player bar shows the track title and artist.
- [ ] Play/pause toggles playback; the icon updates.
- [ ] Next/prev move through the album's tracks.
- [ ] The progress bar and elapsed time advance during playback (seeking itself is not implemented yet).
- [ ] The volume slider changes loudness.
- [ ] Clicking Queue shows the queue with the current track.
- [ ] Queue reorder (drag), remove, and clear work.
- [ ] Settings lists output devices and changing quality persists (for the session).
- [ ] A track at Max (Hi-Res) plays (24-bit/192 kHz source is decoded).
- [ ] Logging out stops playback and clears the queue.
```

- [ ] **Step 3: Record the results**

Update `docs/superpowers/plans/notes/plan2-smoke.md`: check every passing item (`- [x]`) and append a `## Results (2026-09-22)` section with the exact automated-check counts from Step 1 and a one-line note per failure. Keep unchecked items unchecked.

- [ ] **Step 4: Fix any real playback bugs**

For each failure, use `superpowers:systematic-debugging`: capture the symptom, write a failing test (a Rust unit test where possible, otherwise a documented manual repro), fix, re-run the gate(s), and commit each fix as its own `fix(audio): ...` commit. Do not paper over failures.

- [ ] **Step 5: Commit**

```powershell
git add docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "test: record plan 2 smoke results"
```
If Step 4 produced fixes, commit those first (one commit per fix).

---

### Task 2: Typed PCM path

**Files:**
- Modify: `crates/oxitide-audio/src/decode.rs`
- Modify: `crates/oxitide-audio/src/engine.rs` (only the `OutputSink` bridge)
- Create: `crates/oxitide-audio/tests/fixtures/tone24.flac`
- Create: `crates/oxitide-audio/tests/fixtures/tone48.flac`
- Create: `crates/oxitide-audio/tests/fixtures/tone48.mp4`

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 3, 6, 7, 8):
  - `decode::PcmFormat` — `F32 | I32 { bits: u16 }`
  - `decode::StreamInfo { sample_rate: u32, channels: u16, bits_per_sample: u16, format: PcmFormat }` with `is_bit_perfect_candidate()`
  - `decode::PcmBuffer` — `F32(Vec<f32>) | I32(Vec<i32>)` with `len()`, `is_empty()`, `frames(channels)`, `as_f32()`, `as_i32()`
  - `decode::PcmSink { preferred_format() -> PcmFormat, on_stream_info(&StreamInfo), on_samples(&PcmBuffer) }`
  - `decode::CollectSink { info: Option<StreamInfo>, samples: Vec<f32> }`

- [ ] **Step 1: Generate the three new fixtures**

`tone24.flac` and `tone48.flac` are bare FLAC; `tone48.mp4` is the same 48 kHz tone in the FLAC-in-fMP4 container used by `tone.mp4`, so the gapless format-change test (Task 7) exercises an identical container with a different sample rate. All commands verified with ffmpeg:

```powershell
ffmpeg -y -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=2" -ac 2 -sample_fmt s32 -c:a flac crates/oxitide-audio/tests/fixtures/tone24.flac
ffmpeg -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" -ac 2 -sample_fmt s16 -c:a flac crates/oxitide-audio/tests/fixtures/tone48.flac
ffmpeg -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" -ac 2 -c:a flac -movflags frag_keyframe+empty_moov+default_base_moof crates/oxitide-audio/tests/fixtures/tone48.mp4
ffprobe -v error -show_entries stream=codec_name,bits_per_raw_sample,sample_rate,channels,duration -of default=noprint_wrappers=1 crates/oxitide-audio/tests/fixtures/tone24.flac
ffprobe -v error -show_entries stream=codec_name,bits_per_raw_sample,sample_rate,channels,duration -of default=noprint_wrappers=1 crates/oxitide-audio/tests/fixtures/tone48.flac
ffprobe -v error -show_entries stream=codec_name,bits_per_raw_sample,sample_rate,channels,duration -of default=noprint_wrappers=1 crates/oxitide-audio/tests/fixtures/tone48.mp4
```
Expected: `tone24.flac` → flac, 24 bits, 44100, 2 ch, 2 s. `tone48.flac` → flac, 16 bits, 48000, 2 ch, 2 s. `tone48.mp4` → flac, 16 bits, 48000, 2 ch, 2 s.

- [ ] **Step 2: Write the failing decode tests**

Replace the `#[cfg(test)] mod tests` block in `crates/oxitide-audio/src/decode.rs` with (this also updates the two existing tests to the new `CollectSink.info` field):

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::demux::BytesSource;

    /// Collects every sample as `i32`, exactly as a bit-perfect sink would.
    struct I32Sink {
        pub info: Option<StreamInfo>,
        pub samples: Vec<i32>,
    }

    impl I32Sink {
        fn new() -> Self {
            Self { info: None, samples: Vec::new() }
        }
    }

    impl PcmSink for I32Sink {
        fn preferred_format(&self) -> PcmFormat {
            PcmFormat::I32 { bits: 16 }
        }
        fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
            self.info = Some(*info);
            Ok(())
        }
        fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
            match buf {
                PcmBuffer::I32(v) => {
                    self.samples.extend_from_slice(v);
                    Ok(())
                }
                PcmBuffer::F32(_) => Err(Error::Decode("expected i32 samples".into())),
            }
        }
    }

    fn run_fixture<S: PcmSink>(name: &str, ext: &str, sink: &mut S) {
        let bytes = fs::read(format!("tests/fixtures/{name}")).unwrap();
        run(BytesSource::new(bytes), ext, sink).unwrap();
    }

    fn decode_fixture_f32(name: &str, ext: &str) -> CollectSink {
        let mut sink = CollectSink::new();
        run_fixture(name, ext, &mut sink);
        sink
    }

    fn decode_fixture_i32(name: &str, ext: &str) -> I32Sink {
        let mut sink = I32Sink::new();
        run_fixture(name, ext, &mut sink);
        sink
    }

    #[test]
    fn decodes_fragmented_mp4_flac() {
        let sink = decode_fixture_f32("tone.mp4", "mp4");
        let info = sink.info.unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(sink.samples.len(), 176_400); // 2s * 44100 * 2ch
        assert!(sink.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn decodes_bare_flac() {
        let sink = decode_fixture_f32("tone.flac", "flac");
        assert_eq!(sink.samples.len(), 176_400);
    }

    #[test]
    fn i32_path_preserves_16_bit_samples_exactly() {
        let f32_sink = decode_fixture_f32("tone.flac", "flac");
        let i32_sink = decode_fixture_i32("tone.flac", "flac");
        let info = i32_sink.info.unwrap();
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(info.format, PcmFormat::I32 { bits: 16 });
        assert_eq!(i32_sink.samples.len(), 176_400);
        for (index, (&int, &float)) in
            i32_sink.samples.iter().zip(f32_sink.samples.iter()).enumerate()
        {
            // Symphonia left-aligns integer samples to 32 bits, and i32 -> f32
            // divides by 2^31, so this round-trip must be exact for every sample.
            assert_eq!(
                int,
                (float * 2_147_483_648.0) as i32,
                "sample {index} lost precision"
            );
        }
    }

    #[test]
    fn i32_path_preserves_24_bit_samples_exactly() {
        let f32_sink = decode_fixture_f32("tone24.flac", "flac");
        let i32_sink = decode_fixture_i32("tone24.flac", "flac");
        let info = i32_sink.info.unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.bits_per_sample, 24);
        assert_eq!(info.format, PcmFormat::I32 { bits: 24 });
        assert_eq!(i32_sink.samples.len(), 176_400);
        for (index, (&int, &float)) in
            i32_sink.samples.iter().zip(f32_sink.samples.iter()).enumerate()
        {
            assert_eq!(
                int,
                (float * 2_147_483_648.0) as i32,
                "sample {index} lost precision"
            );
        }
    }

    #[test]
    fn reads_48khz_fixture_at_native_rate() {
        let sink = decode_fixture_f32("tone48.flac", "flac");
        let info = sink.info.unwrap();
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(sink.samples.len(), 192_000); // 2s * 48000 * 2ch
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio decode::`
Expected: compile errors (`PcmBuffer`, `StreamInfo`, `PcmFormat`, `CollectSink.info`, and the new `PcmSink` methods do not exist).

- [ ] **Step 4: Implement the typed PCM core in `decode.rs`**

Replace the top of `crates/oxitide-audio/src/decode.rs` (everything above `pub fn run`) with:

```rust
use symphonia::core::audio::sample::SampleFormat;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::error::{Error, Result};

/// The sample representation a sink wants to receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFormat {
    F32,
    /// Signed integers in a 32-bit container, left-aligned to the container.
    /// This is the layout WASAPI expects for 16/24 valid bits in a 32-bit
    /// container, and symphonia's FLAC decoder already produces it.
    I32 { bits: u16 },
}

/// Format of a decoded audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub format: PcmFormat,
}

impl StreamInfo {
    /// True when the source can be sent to an exclusive integer output without
    /// resampling or a float round-trip.
    pub fn is_bit_perfect_candidate(&self) -> bool {
        matches!(self.format, PcmFormat::I32 { bits } if bits == 16 || bits == 24)
    }
}

/// One interleaved block of PCM samples.
#[derive(Debug, Clone, PartialEq)]
pub enum PcmBuffer {
    F32(Vec<f32>),
    I32(Vec<i32>),
}

impl PcmBuffer {
    pub fn len(&self) -> usize {
        match self {
            PcmBuffer::F32(v) => v.len(),
            PcmBuffer::I32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of interleaved frames in this buffer.
    pub fn frames(&self, channels: u16) -> u64 {
        (self.len() / channels.max(1) as usize) as u64
    }

    pub fn as_f32(&self) -> Option<&[f32]> {
        match self {
            PcmBuffer::F32(v) => Some(v),
            PcmBuffer::I32(_) => None,
        }
    }

    pub fn as_i32(&self) -> Option<&[i32]> {
        match self {
            PcmBuffer::I32(v) => Some(v),
            PcmBuffer::F32(_) => None,
        }
    }
}

pub trait PcmSink {
    /// The representation `on_samples` will receive. Decode calls this once,
    /// after `on_stream_info`, so a sink can decide from the source info.
    fn preferred_format(&self) -> PcmFormat;
    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()>;
    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()>;
}

/// Collects the source info and every sample as `f32`.
pub struct CollectSink {
    pub info: Option<StreamInfo>,
    pub samples: Vec<f32>,
}

impl CollectSink {
    pub fn new() -> Self {
        Self { info: None, samples: Vec::new() }
    }
}

impl Default for CollectSink {
    fn default() -> Self {
        Self::new()
    }
}

impl PcmSink for CollectSink {
    fn preferred_format(&self) -> PcmFormat {
        PcmFormat::F32
    }
    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.info = Some(*info);
        Ok(())
    }
    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
        match buf {
            PcmBuffer::F32(v) => {
                self.samples.extend_from_slice(v);
                Ok(())
            }
            PcmBuffer::I32(v) => {
                self.samples.extend(v.iter().map(|&s| s as f32 / 2_147_483_648.0));
                Ok(())
            }
        }
    }
}
```

Replace `pub fn run` (lines 51–102 of the current file) with:

```rust
pub fn run(source: impl MediaSource + 'static, ext: &str, sink: &mut dyn PcmSink) -> Result<()> {
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);

    let mut reader = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::Decode(e.to_string()))?;

    let (track_id, mut decoder, bits_hint, sample_format) = {
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
        (track.id, decoder, params.bits_per_sample, params.sample_format)
    };

    let mut info_sent = false;
    let mut out_format = PcmFormat::F32;
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
                if !info_sent {
                    let spec = decoded.spec();
                    let info = StreamInfo {
                        sample_rate: spec.rate(),
                        channels: spec.channels().count().max(1) as u16,
                        bits_per_sample: normalize_bits(bits_hint),
                        format: source_format(sample_format, bits_hint),
                    };
                    sink.on_stream_info(&info)?;
                    out_format = sink.preferred_format();
                    info_sent = true;
                }
                let n = decoded.samples_interleaved();
                match out_format {
                    PcmFormat::F32 => {
                        let mut buf = vec![0f32; n];
                        decoded.copy_to_slice_interleaved(&mut buf);
                        sink.on_samples(&PcmBuffer::F32(buf))?;
                    }
                    PcmFormat::I32 { .. } => {
                        let mut buf = vec![0i32; n];
                        decoded.copy_to_slice_interleaved(&mut buf);
                        sink.on_samples(&PcmBuffer::I32(buf))?;
                    }
                }
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

/// Clamp a source bit depth to what the exclusive container supports.
fn normalize_bits(bits: Option<u32>) -> u16 {
    match bits {
        Some(b) if b >= 17 => 24,
        _ => 16,
    }
}

/// The representation of the *source* samples. FLAC always reports a bit depth
/// and no sample format, so `None` with a bit depth is an integer source.
fn source_format(sample_format: Option<SampleFormat>, bits: Option<u32>) -> PcmFormat {
    match sample_format {
        Some(SampleFormat::F32) | Some(SampleFormat::F64) => PcmFormat::F32,
        _ => PcmFormat::I32 { bits: normalize_bits(bits) },
    }
}
```

- [ ] **Step 5: Bridge the engine's `OutputSink` to the new trait**

In `crates/oxitide-audio/src/engine.rs`, update the import (line 3) to:

```rust
use crate::decode::{AudioFormat, PcmBuffer, PcmFormat, PcmSink, StreamInfo};
```

and replace `impl PcmSink for OutputSink<'_>` (lines 434–468) with:

```rust
impl PcmSink for OutputSink<'_> {
    fn preferred_format(&self) -> PcmFormat {
        PcmFormat::F32
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.rate = info.sample_rate;
        self.channels = info.channels;
        self.output.open(
            self.device_id.as_deref(),
            AudioFormat { sample_rate: info.sample_rate, channels: info.channels },
        )
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
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
        // This sink always prefers F32; an I32 buffer would be a bug.
        let PcmBuffer::F32(samples) = buf else {
            return Err(Error::Output("output sink received non-f32 samples".into()));
        };
        let volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        if (volume - 1.0).abs() < f32::EPSILON {
            self.output.write(samples)?;
        } else {
            let scaled: Vec<f32> = samples.iter().map(|s| s * volume).collect();
            self.output.write(&scaled)?;
        }
        if self.rate > 0 {
            self.frames += (samples.len() / self.channels as usize) as u64;
            self.position_ms
                .store(self.frames.saturating_mul(1000) / self.rate as u64, Ordering::SeqCst);
        }
        Ok(())
    }
}
```

`AudioFormat` stays defined in `decode.rs` for this task; Task 3 replaces it with `StreamInfo` in the output trait.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: PASS — all decode tests (including the 24-bit exactness test and the 48 kHz fixture) plus the existing engine/queue/fetch/mpd tests.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p oxitide-audio -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```powershell
git add crates/oxitide-audio/src/decode.rs crates/oxitide-audio/src/engine.rs crates/oxitide-audio/tests/fixtures/tone24.flac crates/oxitide-audio/tests/fixtures/tone48.flac crates/oxitide-audio/tests/fixtures/tone48.mp4
git commit -m "feat(audio): add typed PCM path and integer-exact decode"
```

---

### Task 3: Output trait and NullOutput recording

**Files:**
- Modify: `crates/oxitide-audio/src/output.rs`
- Modify: `crates/oxitide-audio/src/wasapi_output.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`
- Modify: `crates/oxitide-audio/src/decode.rs` (remove the now-unused `AudioFormat`)

**Interfaces:**
- Consumes: `decode::{PcmBuffer, PcmFormat, StreamInfo}` from Task 2.
- Produces (used by Tasks 4, 6, 7, 8, 9):
  - `output::OutputMode` — `Shared | Exclusive` (serializes lowercase)
  - `output::ExclusiveFallback` — `Shared | WarnOnly` (serializes `shared`/`warn_only`)
  - `output::AudioOutputSpec { device_id: Option<String>, info: StreamInfo, mode: OutputMode, exclusive_fallback: ExclusiveFallback }`
  - `output::OutputStatus { mode: OutputMode, bit_perfect: bool, notice: Option<String> }`
  - `output::AudioOutput { open(&AudioOutputSpec), start(), write(&PcmBuffer), set_volume(f32), flush(), stop(), status() -> OutputStatus }`
  - `output::exclusive_formats(&StreamInfo) -> Vec<ExclusiveFormat>`
  - `NullOutput` accessors: `samples()`, `i32_samples()`, `opens()`, `stops()`, `last_spec()`, `volumes()`
  - `Engine::audio_status() -> OutputStatus`, `Engine::set_output_mode(OutputMode)`, `Engine::set_exclusive_fallback(ExclusiveFallback)`

- [ ] **Step 1: Write the failing output tests**

Replace the `#[cfg(test)] mod tests` block in `crates/oxitide-audio/src/output.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::PcmFormat;

    fn info(bits: u16, format: PcmFormat) -> StreamInfo {
        StreamInfo { sample_rate: 44100, channels: 2, bits_per_sample: bits, format }
    }

    fn spec(mode: OutputMode) -> AudioOutputSpec {
        AudioOutputSpec {
            device_id: None,
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode,
            exclusive_fallback: ExclusiveFallback::Shared,
        }
    }

    #[test]
    fn null_output_records_lifecycle_and_volume() {
        let mut out = NullOutput::new();
        out.open(&spec(OutputMode::Shared)).unwrap();
        out.start().unwrap();
        out.set_volume(0.5).unwrap();
        out.write(&PcmBuffer::F32(vec![1.0, -1.0])).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();

        assert_eq!(out.opens(), 1);
        assert_eq!(out.stops(), 1);
        assert_eq!(out.last_spec(), Some(spec(OutputMode::Shared)));
        assert_eq!(out.volumes(), vec![0.5]);
        assert_eq!(out.samples(), vec![0.5, -0.5]);
        assert_eq!(out.status().mode, OutputMode::Shared);
        assert!(!out.status().bit_perfect);
    }

    #[test]
    fn null_output_records_i32_with_software_volume() {
        let mut out = NullOutput::new();
        out.open(&spec(OutputMode::Exclusive)).unwrap();
        out.set_volume(0.5).unwrap();
        out.write(&PcmBuffer::I32(vec![1000, -1000])).unwrap();
        assert_eq!(out.i32_samples(), vec![500, -500]);
        assert_eq!(out.status().mode, OutputMode::Exclusive);
        assert!(out.status().bit_perfect);
    }

    #[test]
    fn exclusive_formats_chain_valid_bits() {
        let sixteen = exclusive_formats(&info(16, PcmFormat::I32 { bits: 16 }));
        assert_eq!(
            sixteen.iter().map(|f| f.valid_bits).collect::<Vec<_>>(),
            vec![16, 24, 32]
        );
        let twenty_four = exclusive_formats(&info(24, PcmFormat::I32 { bits: 24 }));
        assert_eq!(
            twenty_four.iter().map(|f| f.valid_bits).collect::<Vec<_>>(),
            vec![24, 32]
        );
        assert!(exclusive_formats(&info(16, PcmFormat::F32)).is_empty());
        assert!(sixteen.iter().all(|f| f.store_bits == 32 && f.sample_rate == 44100));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio output::`
Expected: compile errors (`AudioOutputSpec`, `OutputStatus`, `exclusive_formats`, `PcmBuffer` import path, new trait methods missing).

- [ ] **Step 3: Implement the new `output.rs`**

Replace everything in `crates/oxitide-audio/src/output.rs` above the `#[cfg(windows)] pub fn enumerate_output_devices` block (keep the enumeration and `platform` module exactly as they are) with:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::decode::{PcmBuffer, PcmFormat, StreamInfo};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// How the output was asked to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    #[default]
    Shared,
    Exclusive,
}

/// What to do when the exclusive format is not supported by the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusiveFallback {
    #[default]
    Shared,
    WarnOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputSpec {
    pub device_id: Option<String>,
    pub info: StreamInfo,
    pub mode: OutputMode,
    pub exclusive_fallback: ExclusiveFallback,
}

/// What the output actually opened as, and whether the data path is still
/// bit-perfect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputStatus {
    pub mode: OutputMode,
    pub bit_perfect: bool,
    pub notice: Option<String>,
}

/// A 32-bit container with `valid_bits` significant bits. `valid_bits >= info
/// bit depth` is safe because symphonia left-aligns integer samples to 32
/// bits: the same buffer is valid for any larger valid-bits count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExclusiveFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub store_bits: u16,
    pub valid_bits: u16,
}

/// Formats to try, in order, for an exclusive open; empty when the source is
/// not an integer 16/24-bit stream.
pub fn exclusive_formats(info: &StreamInfo) -> Vec<ExclusiveFormat> {
    let PcmFormat::I32 { bits } = info.format else {
        return Vec::new();
    };
    if !info.is_bit_perfect_candidate() {
        return Vec::new();
    }
    let mut out: Vec<ExclusiveFormat> = Vec::new();
    for valid_bits in [bits, 24, 32] {
        if valid_bits < bits {
            continue;
        }
        let candidate = ExclusiveFormat {
            sample_rate: info.sample_rate,
            channels: info.channels,
            store_bits: 32,
            valid_bits,
        };
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

pub trait AudioOutput: Send {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn write(&mut self, buf: &PcmBuffer) -> Result<()>;
    fn set_volume(&mut self, volume: f32) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    /// The mode/bit-perfect state currently in effect.
    fn status(&self) -> OutputStatus;
}

/// Captures everything written, for tests and headless use. Clones share all
/// recorded state.
#[derive(Clone)]
pub struct NullOutput {
    volume: f32,
    samples: Arc<Mutex<Vec<f32>>>,
    i32_samples: Arc<Mutex<Vec<i32>>>,
    opens: Arc<AtomicUsize>,
    stops: Arc<AtomicUsize>,
    last_spec: Arc<Mutex<Option<AudioOutputSpec>>>,
    volumes: Arc<Mutex<Vec<f32>>>,
    status: Arc<Mutex<OutputStatus>>,
}

impl Default for NullOutput {
    fn default() -> Self {
        Self {
            volume: 1.0,
            samples: Arc::new(Mutex::new(Vec::new())),
            i32_samples: Arc::new(Mutex::new(Vec::new())),
            opens: Arc::new(AtomicUsize::new(0)),
            stops: Arc::new(AtomicUsize::new(0)),
            last_spec: Arc::new(Mutex::new(None)),
            volumes: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(OutputStatus::default())),
        }
    }
}

impl NullOutput {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn samples(&self) -> Vec<f32> {
        self.samples.lock().unwrap().clone()
    }
    pub fn i32_samples(&self) -> Vec<i32> {
        self.i32_samples.lock().unwrap().clone()
    }
    pub fn opens(&self) -> usize {
        self.opens.load(Ordering::SeqCst)
    }
    pub fn stops(&self) -> usize {
        self.stops.load(Ordering::SeqCst)
    }
    pub fn last_spec(&self) -> Option<AudioOutputSpec> {
        self.last_spec.lock().unwrap().clone()
    }
    pub fn volumes(&self) -> Vec<f32> {
        self.volumes.lock().unwrap().clone()
    }
}

impl AudioOutput for NullOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        *self.last_spec.lock().unwrap() = Some(spec.clone());
        *self.status.lock().unwrap() = OutputStatus {
            mode: spec.mode,
            bit_perfect: spec.mode == OutputMode::Exclusive,
            notice: None,
        };
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        Ok(())
    }
    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        match buf {
            PcmBuffer::F32(v) => {
                let mut out = self.samples.lock().unwrap();
                if (self.volume - 1.0).abs() < f32::EPSILON {
                    out.extend_from_slice(v);
                } else {
                    out.extend(v.iter().map(|s| s * self.volume));
                }
            }
            PcmBuffer::I32(v) => {
                let mut out = self.i32_samples.lock().unwrap();
                if (self.volume - 1.0).abs() < f32::EPSILON {
                    out.extend_from_slice(v);
                } else {
                    out.extend(v.iter().map(|&s| (s as f64 * self.volume as f64) as i32));
                }
            }
        }
        Ok(())
    }
    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
        self.volumes.lock().unwrap().push(self.volume);
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn status(&self) -> OutputStatus {
        self.status.lock().unwrap().clone()
    }
}
```

- [ ] **Step 4: Adapt `wasapi_output.rs` to the new trait (shared mode only)**

Replace lines 1–136 of `crates/oxitide-audio/src/wasapi_output.rs` (everything through the blank line before `fn run_device`) with:

```rust
#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::PcmBuffer;
use crate::error::{Error, Result};
use crate::output::{platform, AudioOutput, AudioOutputSpec, OutputMode, OutputStatus};

/// Floor for the ring so tiny formats still leave room for the device period.
const MIN_RING_SAMPLES: usize = 4096;

pub struct WasapiOutput {
    device_id: Option<String>,
    producer: Option<Producer<u32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    volume: f32,
    status: Arc<Mutex<OutputStatus>>,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            volume: 1.0,
            status: Arc::new(Mutex::new(OutputStatus::default())),
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
    }
}

impl Default for WasapiOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for WasapiOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.shutdown_worker();
        platform::ensure_com()?;
        self.device_id = spec.device_id.clone();
        let sample_rate = spec.info.sample_rate;
        let channels = spec.info.channels;

        // ~200ms of samples (u32 words: one 4-byte word per sample). A small
        // ring keeps pause/volume changes responsive.
        let frames = (sample_rate as usize * channels as usize) / 5;
        let (producer, mut consumer) = RingBuffer::<u32>::new(frames.max(MIN_RING_SAMPLES));

        let device_id = self.device_id.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || {
                if let Err(e) = run_device(device_id, sample_rate, channels, &mut consumer, &stop_thread)
                {
                    tracing::error!("wasapi output stopped: {e}");
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        self.producer = Some(producer);
        self.worker = Some(handle);
        self.stop = stop;
        // Task 4 opens exclusive; until then every spec opens shared.
        *self.status.lock().unwrap() =
            OutputStatus { mode: OutputMode::Shared, bit_perfect: false, notice: None };
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        let volume = self.volume;
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        match buf {
            PcmBuffer::F32(v) => {
                for &sample in v {
                    let scaled =
                        if (volume - 1.0).abs() < f32::EPSILON { sample } else { sample * volume };
                    push_word(producer, scaled.to_bits())?;
                }
            }
            PcmBuffer::I32(v) => {
                for &sample in v {
                    let scaled = if (volume - 1.0).abs() < f32::EPSILON {
                        sample
                    } else {
                        (sample as f64 * volume as f64) as i32
                    };
                    push_word(producer, scaled as u32)?;
                }
            }
        }
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.shutdown_worker();
        Ok(())
    }

    fn status(&self) -> OutputStatus {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

/// Push one sample word, waiting briefly if the ring is full but giving up if
/// the device stalled so playback cannot hang forever.
fn push_word(producer: &mut Producer<u32>, word: u32) -> Result<()> {
    let mut pending = word;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match producer.push(pending) {
            Ok(()) => return Ok(()),
            Err(rtrb::PushError::Full(sample)) => {
                if std::time::Instant::now() > deadline {
                    return Err(Error::Output("audio device stalled".into()));
                }
                pending = sample;
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
```

Then in `fn run_device` change the consumer parameter type to `&mut rtrb::Consumer<u32>` and replace `fn write_frames` with:

```rust
fn write_frames(
    client: &wasapi::AudioClient,
    render: &wasapi::AudioRenderClient,
    consumer: &mut rtrb::Consumer<u32>,
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
        let word = consumer.pop().unwrap_or(0); // silence on underrun (0.0f32 / i32 0)
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    render
        .write_to_device(frames, &buf, None)
        .map_err(|e| Error::Output(e.to_string()))
}
```

Finally replace the ignored device test at the bottom with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};
    use crate::output::ExclusiveFallback;

    #[test]
    #[ignore = "requires an audio device"]
    fn opens_default_device_and_plays_a_second_of_silence() {
        let mut out = WasapiOutput::new();
        out.open(&AudioOutputSpec {
            device_id: None,
            info: StreamInfo {
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: 16,
                format: PcmFormat::I32 { bits: 16 },
            },
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::F32(vec![0.0f32; 44100 * 2])).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        out.flush().unwrap();
        out.stop().unwrap();
    }
}
```

- [ ] **Step 5: Adapt `engine.rs` to the new output trait**

1. Change the import at line 3 to:

```rust
use crate::decode::{PcmBuffer, PcmFormat, PcmSink, StreamInfo};
```

2. Add to the `use crate::output::...` import (line 6):

```rust
use crate::output::{AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus};
```

3. Add three fields to `struct Inner` (after `output_status`-adjacent fields):

```rust
    output_mode: Mutex<OutputMode>,
    exclusive_fallback: Mutex<ExclusiveFallback>,
    output_status: Arc<Mutex<OutputStatus>>,
```

4. Initialize them in `Engine::new`:

```rust
                output_mode: Mutex::new(OutputMode::Shared),
                exclusive_fallback: Mutex::new(ExclusiveFallback::Shared),
                output_status: Arc::new(Mutex::new(OutputStatus::default())),
```

5. Add these methods after `set_output_device`:

```rust
    /// The mode/bit-perfect state the output last reported.
    pub fn audio_status(&self) -> OutputStatus {
        self.inner.output_status.lock().unwrap().clone()
    }

    /// Select shared or exclusive output. Applies to the next track opened;
    /// Task 6 makes this apply to a playing track.
    pub fn set_output_mode(&self, mode: OutputMode) {
        *self.inner.output_mode.lock().unwrap() = mode;
    }

    /// Select what to do when exclusive output is unavailable.
    pub fn set_exclusive_fallback(&self, fallback: ExclusiveFallback) {
        *self.inner.exclusive_fallback.lock().unwrap() = fallback;
    }
```

6. Add fields to `struct OutputSink<'a>`:

```rust
    mode: OutputMode,
    fallback: ExclusiveFallback,
    status: Arc<Mutex<OutputStatus>>,
    last_volume: f32,
```

7. Replace `impl PcmSink for OutputSink<'_>` with:

```rust
impl PcmSink for OutputSink<'_> {
    fn preferred_format(&self) -> PcmFormat {
        PcmFormat::F32
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.rate = info.sample_rate;
        self.channels = info.channels;
        let spec = AudioOutputSpec {
            device_id: self.device_id.clone(),
            info: *info,
            mode: self.mode,
            exclusive_fallback: self.fallback,
        };
        self.output.open(&spec)?;
        self.output.start()?;
        self.last_volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        self.output.set_volume(self.last_volume)?;
        *self.status.lock().unwrap() = self.output.status();
        Ok(())
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
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
        if (volume - self.last_volume).abs() >= f32::EPSILON {
            self.output.set_volume(volume)?;
            self.last_volume = volume;
        }
        self.output.write(buf)?;
        if self.rate > 0 {
            self.frames += buf.frames(self.channels);
            self.position_ms
                .store(self.frames.saturating_mul(1000) / self.rate as u64, Ordering::SeqCst);
        }
        Ok(())
    }
}
```

8. In both `play_track_into` and `run_track`, construct `OutputSink` with the new fields:

```rust
        let mut sink = OutputSink {
            output,
            device_id: self.output_device(),
            mode: *self.inner.output_mode.lock().unwrap(),
            fallback: *self.inner.exclusive_fallback.lock().unwrap(),
            status: self.inner.output_status.clone(),
            generation: None, // Some((self.inner.generation.clone(), generation)) in run_track
            paused: self.inner.paused.clone(),
            volume: self.inner.volume.clone(),
            position_ms: self.inner.position_ms.clone(),
            frames: 0,
            rate: 0,
            channels: 2,
            last_volume: 1.0,
        };
```

9. Replace the engine test helper `RecordingOutput` (lines ~830–852) with:

```rust
    /// An output that records the spec passed to `open`.
    struct RecordingOutput {
        specs: Arc<Mutex<Vec<AudioOutputSpec>>>,
    }

    impl AudioOutput for RecordingOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.specs.lock().unwrap().push(spec.clone());
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }
```

and the assertion in `set_output_device_is_passed_to_open` becomes:

```rust
        assert_eq!(opened.lock().unwrap()[0].device_id.as_deref(), Some("device-x"));
```

Renaming `opened` to `specs` in that test as needed:

```rust
    #[test]
    fn set_output_device_is_passed_to_open() {
        let (plan, files) = fixture_plan();
        let specs = Arc::new(Mutex::new(Vec::new()));
        let specs_ref = specs.clone();
        let factory: Arc<dyn OutputFactory> = Arc::new(move || -> Box<dyn AudioOutput> {
            Box::new(RecordingOutput { specs: specs_ref.clone() })
        });
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );
        engine.set_output_device(Some("device-x".into()));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while specs.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(specs.lock().unwrap()[0].device_id.as_deref(), Some("device-x"));
    }
```

- [ ] **Step 6: Remove the now-unused `AudioFormat`**

Delete the `pub struct AudioFormat` definition from `crates/oxitide-audio/src/decode.rs` (it has no remaining users).

- [ ] **Step 7: Run the workspace tests and clippy**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green. Any engine test that constructs an output must compile against the new trait (there are no other implementors: `grep -r "impl AudioOutput" crates/` should show only `NullOutput` and `WasapiOutput`).

- [ ] **Step 8: Commit**

```powershell
git add crates/oxitide-audio/src/output.rs crates/oxitide-audio/src/wasapi_output.rs crates/oxitide-audio/src/engine.rs crates/oxitide-audio/src/decode.rs
git commit -m "feat(audio): rework output trait with typed PCM, volume, and status"
```

---

### Task 4: WASAPI exclusive output and endpoint volume

**Files:**
- Create: `crates/oxitide-audio/src/endpoint_volume.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`
- Modify: `crates/oxitide-audio/Cargo.toml`
- Modify: `crates/oxitide-audio/src/wasapi_output.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/lib/player.test.ts`
- Modify: `ui/src/lib/components/PlayerBar.svelte`
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: `output::{exclusive_formats, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus}` from Task 3.
- Produces:
  - `endpoint_volume::set_master_volume(device_id: Option<&str>, volume: f32) -> Result<()>`
  - `PlaybackState` gains `output_mode: OutputMode`, `bit_perfect: bool`, `bit_perfect_notice: Option<String>`
  - IPC: `set_exclusive { enabled }`, `set_exclusive_fallback { mode }`, `get_audio_status -> { output_mode, bit_perfect, notice }`
  - TS: `PlaybackState.output_mode | bit_perfect | bit_perfect_notice`, `api.setExclusive`, `api.setExclusiveFallback`, `api.getAudioStatus`

- [ ] **Step 1: Add the `windows` dependency**

In the root `Cargo.toml` there is no `windows` entry; add one only to `crates/oxitide-audio/Cargo.toml` under the existing Windows target section, replacing:

```toml
[target.'cfg(windows)'.dependencies]
wasapi = "0.24"
```

with:

```toml
[target.'cfg(windows)'.dependencies]
wasapi = "0.24"
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_Media_Audio",
    "Win32_Media_Audio_Endpoints",
    "Win32_System_Com",
    "Win32_System_Com_StructuredStorage",
    "Win32_System_Variant",
] }
```

The feature set is what `IMMDevice::Activate` and `IAudioEndpointVolume` require; `0.62` matches wasapi's own `windows` dependency so Cargo unifies them.

- [ ] **Step 2: Write `endpoint_volume.rs`**

Create `crates/oxitide-audio/src/endpoint_volume.rs`:

```rust
use windows::core::HSTRING;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::error::{Error, Result};

/// Set the endpoint master volume scalar (0.0..=1.0) for one render device, or
/// the system default when `device_id` is `None`.
pub fn set_master_volume(device_id: Option<&str>, volume: f32) -> Result<()> {
    crate::output::platform::ensure_com()?;
    let volume = volume.clamp(0.0, 1.0);
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(to_error)?;
        let device: IMMDevice = match device_id {
            Some(id) => {
                let id = HSTRING::from(id);
                enumerator.GetDevice(&id).map_err(to_error)?
            }
            None => enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).map_err(to_error)?,
        };
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).map_err(to_error)?;
        endpoint
            .SetMasterVolumeLevelScalar(volume, std::ptr::null())
            .map_err(to_error)?;
    }
    Ok(())
}

fn to_error(e: windows::core::Error) -> Error {
    Error::Output(e.to_string())
}
```

Then in `crates/oxitide-audio/src/lib.rs` add (next to the wasapi module):

```rust
#[cfg(windows)]
pub mod endpoint_volume;
```

- [ ] **Step 3: Write the failing engine tests for status reporting**

Add these two tests plus two stub outputs to `crates/oxitide-audio/src/engine.rs`'s test module:

```rust
    /// An output that always reports the given status, so tests can pin how a
    /// notice reaches `PlaybackState`.
    struct FixedStatusOutput {
        status: OutputStatus,
    }

    impl AudioOutput for FixedStatusOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            self.status.clone()
        }
    }

    fn fixed_status_factory(status: OutputStatus) -> Arc<dyn OutputFactory> {
        Arc::new(move || -> Box<dyn AudioOutput> {
            Box::new(FixedStatusOutput { status: status.clone() })
        })
    }

    #[test]
    fn null_output_reports_exclusive_status() {
        let (plan, files) = fixture_plan();
        let (factory, _output) = null_factory();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            factory,
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.audio_status().mode != OutputMode::Exclusive
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(engine.audio_status().mode, OutputMode::Exclusive);
        assert!(engine.audio_status().bit_perfect);
    }

    #[test]
    fn output_status_notice_flows_to_playback_state() {
        let (plan, files) = fixture_plan();
        let status = OutputStatus {
            mode: OutputMode::Exclusive,
            bit_perfect: false,
            notice: Some("volume applied in software; bit-perfect compromised".into()),
        };
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            fixed_status_factory(status),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while engine.state().bit_perfect_notice.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let state = engine.state();
        assert_eq!(state.output_mode, OutputMode::Exclusive);
        assert!(!state.bit_perfect);
        assert_eq!(
            state.bit_perfect_notice.as_deref(),
            Some("volume applied in software; bit-perfect compromised")
        );
    }
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio engine::`
Expected: compile errors (no `output_mode`/`bit_perfect`/`bit_perfect_notice` fields, no `audio_status`, no `set_output_mode`).

- [ ] **Step 5: Add the fields to `PlaybackState` and `state()`**

In `crates/oxitide-audio/src/engine.rs`:

1. Add fields to `struct PlaybackState` (before `queue`):

```rust
    pub output_mode: OutputMode,
    pub bit_perfect: bool,
    pub bit_perfect_notice: Option<String>,
```

2. In `Engine::state()`, clone the status before locking the queue and fill the fields:

```rust
    pub fn state(&self) -> PlaybackState {
        let status = self.inner.output_status.lock().unwrap().clone();
        let q = self.inner.queue.lock().unwrap();
        PlaybackState {
            playing: q.is_playing(),
            buffering: false,
            current: q.current().cloned(),
            position_ms: self.inner.position_ms.load(Ordering::SeqCst),
            duration_ms: q.current().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0),
            volume: q.volume(),
            shuffle: q.shuffle(),
            repeat: q.repeat(),
            preferred_quality: *self.inner.quality.lock().unwrap(),
            output_mode: status.mode,
            bit_perfect: status.bit_perfect,
            bit_perfect_notice: status.notice,
            queue: q.items().to_vec(),
        }
    }
```

(`audio_status`, `set_output_mode`, and `set_exclusive_fallback` were added in Task 3.)

- [ ] **Step 6: Make the engine's sink produce integer samples in exclusive mode**

In `OutputSink`, add a field `exclusive_format: PcmFormat` (default `PcmFormat::F32`) and update the two trait methods:

```rust
    fn preferred_format(&self) -> PcmFormat {
        match self.mode {
            OutputMode::Shared => PcmFormat::F32,
            OutputMode::Exclusive => self.exclusive_format,
        }
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.rate = info.sample_rate;
        self.channels = info.channels;
        // An integer source can stay integer through an exclusive output; a
        // float source cannot, so request shared-style float buffers and let
        // the exclusive open fail into the configured fallback.
        self.exclusive_format = if info.is_bit_perfect_candidate() {
            info.format
        } else {
            PcmFormat::F32
        };
        let spec = AudioOutputSpec {
            device_id: self.device_id.clone(),
            info: *info,
            mode: self.mode,
            exclusive_fallback: self.fallback,
        };
        self.output.open(&spec)?;
        self.output.start()?;
        self.last_volume = f32::from_bits(self.volume.load(Ordering::SeqCst));
        self.output.set_volume(self.last_volume)?;
        *self.status.lock().unwrap() = self.output.status();
        // A fallback to shared changed the mode the output can accept.
        Ok(())
    }
```

Initialize `exclusive_format: PcmFormat::F32` in both sink constructions.

- [ ] **Step 7: Implement exclusive output in `wasapi_output.rs`**

Replace the whole `crates/oxitide-audio/src/wasapi_output.rs` with:

```rust
#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Producer, RingBuffer};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use crate::decode::PcmBuffer;
use crate::endpoint_volume;
use crate::error::{Error, Result};
use crate::output::{
    exclusive_formats, platform, AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode,
    OutputStatus,
};

/// Floor for the ring so tiny formats still leave room for the device period.
const MIN_RING_SAMPLES: usize = 4096;

pub struct WasapiOutput {
    device_id: Option<String>,
    resolved_device_id: Option<String>,
    producer: Option<Producer<u32>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    volume: f32,
    /// True when samples are scaled in software (shared mode, or an exclusive
    /// device whose endpoint volume is unavailable).
    software_volume: bool,
    device_lost: Arc<AtomicBool>,
    status: Arc<Mutex<OutputStatus>>,
}

impl WasapiOutput {
    pub fn new() -> Self {
        Self {
            device_id: None,
            resolved_device_id: None,
            producer: None,
            worker: None,
            stop: Arc::new(AtomicBool::new(false)),
            volume: 1.0,
            software_volume: true,
            device_lost: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(OutputStatus::default())),
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.producer = None;
    }
}

impl Default for WasapiOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for WasapiOutput {
    fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
        self.shutdown_worker();
        platform::ensure_com()?;
        self.device_id = spec.device_id.clone();
        self.resolved_device_id = None;
        self.device_lost.store(false, Ordering::SeqCst);
        self.software_volume = true;

        let sample_rate = spec.info.sample_rate;
        let channels = spec.info.channels;
        let frames = (sample_rate as usize * channels as usize) / 5;
        let (producer, mut consumer) = RingBuffer::<u32>::new(frames.max(MIN_RING_SAMPLES));

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let spec_thread = spec.clone();
        let lost = self.device_lost.clone();

        let handle = std::thread::Builder::new()
            .name("oxitide-wasapi".into())
            .spawn(move || match setup_device(&spec_thread, &lost) {
                Ok((mut session, ready)) => {
                    let _ = ready_tx.send(Ok(ready));
                    if let Err(e) = session.pump(&mut consumer, &stop_thread) {
                        tracing::error!("wasapi output stopped: {e}");
                    }
                }
                Err(e) => {
                    // Setup failed before any audio was written.
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| Error::Output(e.to_string()))?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(ready)) => {
                self.producer = Some(producer);
                self.worker = Some(handle);
                self.stop = stop;
                self.resolved_device_id = ready.device_id;
                *self.status.lock().unwrap() = ready.status;
            }
            Ok(Err(message)) => {
                stop.store(true, Ordering::SeqCst);
                let _ = handle.join();
                return Err(Error::Output(message));
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                let _ = handle.join();
                return Err(Error::Output("audio device did not initialize in time".into()));
            }
        }

        let volume = self.volume;
        self.set_volume(volume)
    }

    fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.device_lost.load(Ordering::SeqCst) {
            return Err(Error::Output("output device unavailable".into()));
        }
        let volume = self.volume;
        let software = self.software_volume && (volume - 1.0).abs() >= f32::EPSILON;
        // The requested mode can differ from the mode that actually opened
        // (exclusive -> shared fallback), so convert to what the device accepts.
        let actual_mode = self.status.lock().unwrap().mode;
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| Error::Output("output not open".into()))?;
        match buf {
            PcmBuffer::F32(v) => {
                for &sample in v {
                    let scaled = if software { sample * volume } else { sample };
                    push_word(producer, scaled.to_bits())?;
                }
            }
            PcmBuffer::I32(v) if actual_mode == OutputMode::Exclusive => {
                for &sample in v {
                    let scaled = if software {
                        (sample as f64 * volume as f64) as i32
                    } else {
                        sample
                    };
                    push_word(producer, scaled as u32)?;
                }
            }
            PcmBuffer::I32(v) => {
                // Shared output fell back from exclusive: the device wants
                // float samples. Left-aligned i32 -> f32 is a pure divide.
                for &sample in v {
                    let float = sample as f32 / 2_147_483_648.0;
                    let scaled = if software { float * volume } else { float };
                    push_word(producer, scaled.to_bits())?;
                }
            }
        }
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        self.volume = volume;
        let exclusive = self.status.lock().unwrap().mode == OutputMode::Exclusive;
        if exclusive {
            let target = self.resolved_device_id.as_deref().or(self.device_id.as_deref());
            if endpoint_volume::set_master_volume(target, volume).is_ok() {
                self.software_volume = false;
                return Ok(());
            }
            self.software_volume = true;
            let mut status = self.status.lock().unwrap();
            status.bit_perfect = false;
            status.notice = Some("volume applied in software; bit-perfect compromised".into());
        } else {
            self.software_volume = true;
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

    fn status(&self) -> OutputStatus {
        let mut status = self.status.lock().unwrap().clone();
        if self.device_lost.load(Ordering::SeqCst) {
            status.bit_perfect = false;
            status.notice = Some("output device disconnected".into());
        }
        status
    }
}

impl Drop for WasapiOutput {
    fn drop(&mut self) {
        self.shutdown_worker();
    }
}

struct DeviceReady {
    device_id: Option<String>,
    status: OutputStatus,
}

struct DeviceSession {
    client: wasapi::AudioClient,
    render: wasapi::AudioRenderClient,
    event: Option<wasapi::Handle>,
    channels: usize,
    // Kept alive so device-change notifications keep firing.
    _registration: Option<wasapi::DeviceEventRegistration>,
}

impl DeviceSession {
    fn pump(&mut self, consumer: &mut rtrb::Consumer<u32>, stop: &AtomicBool) -> Result<()> {
        // Prime the buffer before starting.
        write_frames(&self.client, &self.render, consumer, self.channels)?;
        self.client.start_stream().map_err(to_error)?;
        while !stop.load(Ordering::SeqCst) {
            match &self.event {
                Some(event) => {
                    if event.wait_for_event(200).is_err() {
                        continue; // timeout: re-check the stop flag
                    }
                }
                // Polling exclusive: no event handle, just poll.
                None => std::thread::sleep(Duration::from_millis(5)),
            }
            write_frames(&self.client, &self.render, consumer, self.channels)?;
        }
        let _ = self.client.stop_stream();
        Ok(())
    }
}

/// Resolve the requested device, falling back to the system default when the
/// id no longer exists. The bool is true when a fallback happened.
fn resolve_device(
    enumerator: &DeviceEnumerator,
    wanted: Option<&str>,
) -> Result<(wasapi::Device, bool)> {
    if let Some(id) = wanted {
        if let Ok(device) = enumerator.get_device(id) {
            return Ok((device, false));
        }
    }
    let device = enumerator.get_default_device(&Direction::Render).map_err(to_error)?;
    Ok((device, wanted.is_some()))
}

/// Try every negotiated exclusive format in order.
fn try_exclusive(
    device: &wasapi::Device,
    spec: &AudioOutputSpec,
    def_time: i64,
) -> Result<wasapi::AudioClient> {
    for format in exclusive_formats(&spec.info) {
        let wave = WaveFormat::new(
            32,
            format.valid_bits as usize,
            &SampleType::Int,
            format.sample_rate as usize,
            format.channels as usize,
            None,
        );
        let Ok(mut client) = device.get_iaudioclient() else {
            continue;
        };
        let Ok(accepted) = client.is_supported_exclusive_with_quirks(&wave) else {
            continue;
        };
        // Polling exclusive supports more devices and avoids the documented
        // USB-driver stutter of event-driven exclusive.
        let mode = StreamMode::PollingExclusive {
            buffer_duration_hns: def_time * 4,
            period_hns: def_time,
        };
        if client.initialize_client(&accepted, &Direction::Render, &mode).is_ok() {
            return Ok(client);
        }
    }
    Err(Error::Output("no supported exclusive format".into()))
}

fn open_shared(
    device: &wasapi::Device,
    spec: &AudioOutputSpec,
    def_time: i64,
) -> Result<(wasapi::AudioClient, Option<wasapi::Handle>)> {
    let mut client = device.get_iaudioclient().map_err(to_error)?;
    let wave = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        spec.info.sample_rate as usize,
        spec.info.channels as usize,
        None,
    );
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: def_time };
    client.initialize_client(&wave, &Direction::Render, &mode).map_err(to_error)?;
    let event = client.set_get_eventhandle().map_err(to_error)?;
    Ok((client, Some(event)))
}

fn setup_device(
    spec: &AudioOutputSpec,
    lost: &Arc<AtomicBool>,
) -> Result<(DeviceSession, DeviceReady)> {
    let enumerator = DeviceEnumerator::new().map_err(to_error)?;
    let (device, fell_back) = resolve_device(&enumerator, spec.device_id.as_deref())?;
    let resolved_id = device.get_id().ok();

    let (def_time, _min_time) = device
        .get_iaudioclient()
        .map_err(to_error)?
        .get_device_period()
        .map_err(to_error)?;

    let mut status = OutputStatus { mode: OutputMode::Shared, bit_perfect: false, notice: None };
    let (mut client, event) = match spec.mode {
        OutputMode::Exclusive => match try_exclusive(&device, spec, def_time) {
            Ok(client) => {
                status.mode = OutputMode::Exclusive;
                status.bit_perfect = true;
                (client, None)
            }
            Err(_) => match spec.exclusive_fallback {
                ExclusiveFallback::Shared => {
                    status.notice = Some("exclusive mode unavailable; using shared output".into());
                    open_shared(&device, spec, def_time)?
                }
                ExclusiveFallback::WarnOnly => {
                    return Err(Error::Output(
                        "exclusive mode is not supported by this output device".into(),
                    ));
                }
            },
        },
        OutputMode::Shared => open_shared(&device, spec, def_time)?,
    };
    if fell_back {
        status.notice = Some("selected output device unavailable; using system default".into());
    }

    let render = client.get_audiorenderclient().map_err(to_error)?;

    // Keep the device-change registration alive for the session.
    let mut registration = None;
    if let Ok(notify_enum) = DeviceEnumerator::new() {
        let mut callbacks = wasapi::DeviceEventCallbacks::new();
        {
            let lost = lost.clone();
            let our_id = resolved_id.clone();
            callbacks.set_device_removed_callback(move |id| {
                if our_id.as_deref() == Some(id.as_str()) {
                    lost.store(true, Ordering::SeqCst);
                }
            });
        }
        {
            let lost = lost.clone();
            let our_id = resolved_id.clone();
            let opened_default = spec.device_id.is_none();
            callbacks.set_default_device_callback(move |_direction, _role, id| {
                if opened_default && our_id.as_deref() != Some(id.as_str()) {
                    lost.store(true, Ordering::SeqCst);
                }
            });
        }
        registration = notify_enum.register_notification_callback(callbacks).ok();
    }

    let session = DeviceSession {
        client,
        render,
        event,
        channels: spec.info.channels as usize,
        _registration: registration,
    };
    Ok((session, DeviceReady { device_id: resolved_id, status }))
}

/// Push one sample word, waiting briefly if the ring is full but giving up if
/// the device stalled so playback cannot hang forever.
fn push_word(producer: &mut Producer<u32>, word: u32) -> Result<()> {
    let mut pending = word;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match producer.push(pending) {
            Ok(()) => return Ok(()),
            Err(rtrb::PushError::Full(sample)) => {
                if std::time::Instant::now() > deadline {
                    return Err(Error::Output("audio device stalled".into()));
                }
                pending = sample;
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn write_frames(
    client: &wasapi::AudioClient,
    render: &wasapi::AudioRenderClient,
    consumer: &mut rtrb::Consumer<u32>,
    channels: usize,
) -> Result<()> {
    let frames = client
        .get_available_space_in_frames()
        .map_err(to_error)? as usize;
    if frames == 0 {
        return Ok(());
    }
    let mut buf = vec![0u8; frames * channels * 4];
    for chunk in buf.chunks_mut(4) {
        let word = consumer.pop().unwrap_or(0); // silence on underrun (0.0f32 / i32 0)
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    render
        .write_to_device(frames, &buf, None)
        .map_err(to_error)
}

fn to_error(e: wasapi::WasapiError) -> Error {
    Error::Output(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{PcmFormat, StreamInfo};

    #[test]
    #[ignore = "requires an audio device"]
    fn opens_default_device_and_plays_a_second_of_silence() {
        let mut out = WasapiOutput::new();
        out.open(&AudioOutputSpec {
            device_id: None,
            info: StreamInfo {
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: 16,
                format: PcmFormat::I32 { bits: 16 },
            },
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::F32(vec![0.0f32; 44100 * 2])).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        out.flush().unwrap();
        out.stop().unwrap();
    }
}
```

Note: `WasapiOutput::write` handles an `I32` buffer on a shared device by converting to float, because the requested mode can be exclusive while the actual open fell back to shared.

- [ ] **Step 8: Add the Tauri commands**

In `crates/oxitide-app/src/commands.rs`:

1. Add to the imports:

```rust
use oxitide_audio::output::{ExclusiveFallback, OutputMode};
```

2. Add the DTO next to `SessionInfoDto`:

```rust
#[derive(Serialize)]
pub struct AudioStatusDto {
    pub output_mode: OutputMode,
    pub bit_perfect: bool,
    pub notice: Option<String>,
}
```

3. Add the three commands after `set_preferred_quality`:

```rust
#[tauri::command]
pub fn set_exclusive(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_output_mode(if enabled {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    });
    Ok(())
}

#[tauri::command]
pub fn set_exclusive_fallback(
    state: State<'_, AppState>,
    mode: ExclusiveFallback,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_exclusive_fallback(mode);
    Ok(())
}

#[tauri::command]
pub fn get_audio_status(state: State<'_, AppState>) -> Result<AudioStatusDto, String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let status = player.engine.audio_status();
    Ok(AudioStatusDto {
        output_mode: status.mode,
        bit_perfect: status.bit_perfect,
        notice: status.notice,
    })
}
```

4. Add all three to `generate_handler![...]` in `crates/oxitide-app/src/main.rs`, next to `commands::set_preferred_quality`:

```rust
        commands::set_exclusive,
        commands::set_exclusive_fallback,
        commands::get_audio_status,
```

- [ ] **Step 9: Update the UI types and store**

In `ui/src/lib/api.ts`:

1. Extend `PlaybackState` with:

```ts
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  bit_perfect_notice?: string | null;
```

2. Add after `OutputDevice`:

```ts
export interface AudioStatus {
  output_mode: "shared" | "exclusive";
  bit_perfect: boolean;
  notice?: string | null;
}
```

3. Add to the `api` object:

```ts
  setExclusive: (enabled: boolean) => invoke<void>("set_exclusive", { enabled }),
  setExclusiveFallback: (mode: "shared" | "warn_only") => invoke<void>("set_exclusive_fallback", { mode }),
  getAudioStatus: () => invoke<AudioStatus>("get_audio_status"),
```

In `ui/src/lib/player.svelte.ts`, extend `PlayerState` with the same three fields and `emptyState()` with:

```ts
    output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
```

In `ui/src/lib/player.test.ts`, add the three fields to the `"state"` event literal in `applies a state event`:

```ts
      repeat: "off", preferred_quality: "lossless",
      output_mode: "shared", bit_perfect: false, bit_perfect_notice: null,
      queue: [],
```

- [ ] **Step 10: Add the PlayerBar badge/notice and Settings toggles**

In `ui/src/lib/components/PlayerBar.svelte`, add after the quality badge:

```svelte
      {#if player.state.bit_perfect}
        <span class="badge" title="Exclusive output at the source's native format">Bit-perfect</span>
      {/if}
```

and after the buffering indicator (inside the `{#if player.state.current}` block):

```svelte
    {#if player.state.bit_perfect_notice}
      <span style="color:#e0a030">{player.state.bit_perfect_notice}</span>
    {/if}
```

In `ui/src/views/Settings.svelte`, extend the script with:

```ts
  let exclusive = $state(false);
  let fallback = $state<"shared" | "warn_only">("shared");

  $effect(() => {
    exclusive = player.state.output_mode === "exclusive";
  });
```

and append after the quality select:

```svelte
<h3>Bit-perfect output</h3>
<label>
  <input
    type="checkbox"
    bind:checked={exclusive}
    onchange={() => api.setExclusive(exclusive).catch((e) => (error = String(e)))}
  />
  Bit-perfect (exclusive mode)
</label>
<p>
  <label>
    If unsupported:
    <select
      bind:value={fallback}
      onchange={() => api.setExclusiveFallback(fallback).catch((e) => (error = String(e)))}
    >
      <option value="shared">Fall back to shared output</option>
      <option value="warn_only">Stop with a message</option>
    </select>
  </label>
</p>
{#if player.state.bit_perfect_notice}
  <p style="color:#e0a030">{player.state.bit_perfect_notice}</p>
{/if}
```

Note: the `$effect` tracks the *actual* output mode. Task 10 replaces this with the persisted requested setting.

- [ ] **Step 11: Run all gates**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```
Expected: all green.

- [ ] **Step 12: Manual check on real hardware**

Run `npm --prefix ui run app:dev`, enable the bit-perfect toggle in Settings, and confirm:
- the DAC/device shows the source sample rate (Windows Sound settings → device properties → Advanced, or a DAC display);
- the PlayerBar shows the "Bit-perfect" badge;
- changing the volume still changes loudness;
- unplugging tests come in Task 9 — a clean open/close is enough here.

Record the result in `docs/superpowers/plans/notes/plan2-smoke.md` under a `## Plan 3 exclusive check` heading.

- [ ] **Step 13: Commit**

```powershell
git add crates/oxitide-audio ui/src crates/oxitide-app docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "feat(audio): add WASAPI exclusive output, endpoint volume, and bit-perfect status"
```

---

### Task 5: Bounded download buffer

**Files:**
- Modify: `crates/oxitide-audio/src/fetch.rs`
- Modify: `crates/oxitide-audio/src/demux.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 6, 7, 8):
  - `fetch::MAX_BUFFERED_BYTES: usize` (2 MiB)
  - `StreamBuffer::consume_to(pos)`, `StreamBuffer::wait_room(cap)`, `StreamBuffer::cancel()`, `StreamBuffer::is_cancelled()`, `StreamBuffer::is_done()`, `StreamBuffer::buffered_bytes()`
  - `StreamBuffer` offsets become absolute and independent of released bytes: `wait_len`/`read_at` take absolute positions, and bytes before the consumer's position are freed.
  - `fetch::download(buffer, fetcher, plan)` keeps its signature but applies backpressure and stops early when cancelled.

- [ ] **Step 1: Write the failing buffer tests**

Append to the test module of `crates/oxitide-audio/src/fetch.rs`:

```rust
    #[test]
    fn consume_releases_bytes_and_keeps_absolute_offsets() {
        let buf = StreamBuffer::new();
        buf.push(b"AAAA");
        buf.push(b"BBBB");
        assert_eq!(buf.buffered_bytes(), 8);
        buf.consume_to(4);
        assert_eq!(buf.buffered_bytes(), 4);
        let mut out = [0u8; 4];
        let n = buf.read_at(4, &mut out).unwrap();
        assert_eq!(&out[..n], b"BBBB");
        buf.consume_to(8);
        assert_eq!(buf.buffered_bytes(), 0);
        assert_eq!(buf.read_at(8, &mut out).unwrap(), 0);
    }

    #[test]
    fn cancel_wakes_waiting_readers() {
        let buf = StreamBuffer::new();
        let waiter = {
            let buf = buf.clone();
            std::thread::spawn(move || buf.wait_len(0))
        };
        std::thread::sleep(Duration::from_millis(50));
        buf.cancel();
        assert!(matches!(waiter.join().unwrap(), Err(Error::Fetch(_))));
        assert!(buf.is_cancelled());
    }

    struct ChunkFetcher {
        chunk: Vec<u8>,
        calls: Mutex<usize>,
    }

    impl ChunkFetcher {
        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl Fetcher for ChunkFetcher {
        fn get(&self, _url: &str) -> Result<Vec<u8>> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.chunk.clone())
        }
    }

    /// Review Focus: a long stream must not grow the buffer without bound.
    #[test]
    fn download_is_backpressured_and_bounded() {
        const CHUNK: usize = 64 * 1024;
        const SEGMENTS: usize = 64; // 4 MiB total, twice the cap
        let fetcher = Arc::new(ChunkFetcher { chunk: vec![7u8; CHUNK], calls: Mutex::new(0) });
        let plan = StreamPlan {
            init_url: None,
            media_urls: (0..SEGMENTS).map(|i| format!("s{i}")).collect(),
            codec: None,
            sample_rate: None,
            bit_depth: None,
        };
        let buf = StreamBuffer::new();
        let handle = {
            let buf = buf.clone();
            let fetcher = fetcher.clone();
            std::thread::spawn(move || download(buf, fetcher, plan))
        };

        // With no consumer, the downloader must stop fetching once the cap is
        // reached (plus the in-flight segment).
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            buf.buffered_bytes() <= MAX_BUFFERED_BYTES + CHUNK,
            "buffered {} bytes",
            buf.buffered_bytes()
        );
        assert!(
            fetcher.calls() < SEGMENTS,
            "downloader never hit backpressure ({} calls)",
            fetcher.calls()
        );

        // Draining releases consumed bytes and lets the downloader finish.
        let mut pos = 0usize;
        let mut out = [0u8; CHUNK];
        loop {
            let n = buf.read_at(pos, &mut out).unwrap();
            if n == 0 && buf.is_done() {
                break;
            }
            pos += n;
            buf.consume_to(pos);
        }
        handle.join().unwrap();
        assert_eq!(pos, CHUNK * SEGMENTS);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio fetch::`
Expected: compile errors (`consume_to`, `buffered_bytes`, `cancel`, `is_cancelled`, `is_done`, `MAX_BUFFERED_BYTES` do not exist).

- [ ] **Step 3: Implement the bounded/cancellable `StreamBuffer`**

In `crates/oxitide-audio/src/fetch.rs`, update the imports:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
```

Add the constant next to `MAX_RETRY_WAIT`:

```rust
/// Cap on unconsumed buffered bytes; the downloader waits for the decoder to
/// catch up before fetching another segment.
pub const MAX_BUFFERED_BYTES: usize = 2 * 1024 * 1024;
```

Replace `struct State` and the whole `impl StreamBuffer` block (current lines 113–174) with:

```rust
struct State {
    data: Vec<u8>,
    /// Absolute offset of `data[0]`; grows as consumed bytes are released.
    start: usize,
    done: bool,
    error: Option<String>,
}

pub struct StreamBuffer {
    state: Mutex<State>,
    cond: Condvar,
    cancelled: AtomicBool,
}

impl StreamBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State { data: Vec::new(), start: 0, done: false, error: None }),
            cond: Condvar::new(),
            cancelled: AtomicBool::new(false),
        })
    }

    pub fn push(&self, bytes: &[u8]) {
        if self.cancelled.load(Ordering::SeqCst) {
            return;
        }
        let mut s = self.state.lock().unwrap();
        if s.error.is_some() {
            return;
        }
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
        if s.error.is_none() {
            s.error = Some(message);
        }
        s.done = true;
        self.cond.notify_all();
    }

    /// Cancel the stream: blocked readers/downloaders wake and exit. Used when
    /// a track is superseded (next/prev/seek/clear).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let mut s = self.state.lock().unwrap();
        if s.error.is_none() {
            s.error = Some("cancelled".into());
        }
        s.done = true;
        self.cond.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn is_done(&self) -> bool {
        self.state.lock().unwrap().done
    }

    /// Number of unconsumed bytes currently held in memory.
    pub fn buffered_bytes(&self) -> usize {
        self.state.lock().unwrap().data.len()
    }

    /// Release everything before absolute offset `pos`. The demuxer calls this
    /// after every read so a long stream never grows without bound.
    pub fn consume_to(&self, pos: usize) {
        let mut s = self.state.lock().unwrap();
        if pos <= s.start {
            return;
        }
        let drop = (pos - s.start).min(s.data.len());
        s.data.drain(..drop);
        s.start += drop;
        self.cond.notify_all();
    }

    /// Block until fewer than `cap` unconsumed bytes are buffered, or the
    /// stream is done/cancelled. Downloader backpressure.
    pub fn wait_room(&self, cap: usize) {
        let mut s = self.state.lock().unwrap();
        while s.data.len() >= cap && !s.done {
            s = self.cond.wait(s).unwrap();
        }
    }

    /// Block until `pos` is buffered or the stream ends. Returns the absolute
    /// end offset of the buffered range, or a typed error if the download
    /// failed or was cancelled.
    pub fn wait_len(&self, pos: usize) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        loop {
            if let Some(err) = &s.error {
                return Err(Error::Fetch(err.clone()));
            }
            let end = s.start + s.data.len();
            if pos < end || s.done {
                return Ok(end);
            }
            s = self.cond.wait(s).unwrap();
        }
    }

    pub fn read_at(&self, pos: usize, out: &mut [u8]) -> Result<usize> {
        let end = self.wait_len(pos)?;
        if pos >= end {
            return Ok(0);
        }
        let s = self.state.lock().unwrap();
        if pos < s.start {
            return Err(Error::Fetch("requested bytes were already released".into()));
        }
        let n = out.len().min(s.start + s.data.len() - pos);
        out[..n].copy_from_slice(&s.data[pos - s.start..pos - s.start + n]);
        Ok(n)
    }
}
```

Replace `pub fn download` (current lines 196–212) with:

```rust
/// Download every part of `plan` in order into `buffer`, waiting for the
/// consumer when the buffer is full. Runs on the calling thread; intended to
/// be spawned via `std::thread::spawn`.
pub fn download(buffer: Arc<StreamBuffer>, fetcher: Arc<dyn Fetcher>, plan: StreamPlan) {
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.media_urls.iter().cloned());
    for url in urls {
        if buffer.is_cancelled() {
            return;
        }
        buffer.wait_room(MAX_BUFFERED_BYTES);
        if buffer.is_cancelled() {
            return;
        }
        match get_with_retry(fetcher.as_ref(), &url) {
            Ok(bytes) => buffer.push(&bytes),
            Err(e) => {
                buffer.fail(e.to_string());
                return;
            }
        }
    }
    buffer.finish();
}
```

- [ ] **Step 4: Release consumed bytes in the demuxer**

In `crates/oxitide-audio/src/demux.rs`, update `impl Read for StreamBufferSource`'s `read`:

```rust
impl Read for StreamBufferSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self
            .buffer
            .read_at(self.pos as usize, out)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.pos += n as u64;
        // Free decoded bytes so a long stream stays bounded.
        self.buffer.consume_to(self.pos as usize);
        Ok(n)
    }
}
```

- [ ] **Step 5: Run the tests and clippy**

Run:
```powershell
cargo test -p oxitide-audio
cargo clippy --workspace -- -D warnings
```
Expected: all green; the existing `download_pushes_all_parts_in_order`, `fetch_failure_is_reported_to_readers`, `download_retries_transient_fetch_failures`, and `download_fails_after_bounded_attempts` tests still pass unchanged.

- [ ] **Step 6: Commit**

```powershell
git add crates/oxitide-audio/src/fetch.rs crates/oxitide-audio/src/demux.rs
git commit -m "feat(audio): bound the download buffer and release consumed bytes"
```

---

### Task 6: Long-lived playback worker

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs` (replace the non-test code and the test module)
- Modify: `crates/oxitide-app/src/player.rs`
- Modify: `crates/oxitide-app/src/commands.rs`

**Interfaces:**
- Consumes: `output::{AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus}` (Tasks 3–4), `fetch::StreamBuffer` with `cancel` (Task 5), `decode::{PcmBuffer, PcmFormat, PcmSink, StreamInfo}` (Task 2).
- Produces (used by Tasks 7, 8, 9):
  - `Engine::new(resolver, fetcher, output: Box<dyn AudioOutput>, quality) -> Engine`
  - `Engine::shutdown()` (signals the worker to stop; called on logout)
  - `Engine::play_track_into(&self, item: &QueueItem, output: &mut dyn AudioOutput) -> Result<()>`
  - `worker_busy` semantics: `PlayPause` bumps the generation when resuming an idle worker
  - `engine` internals: `SessionOutcome`, `start_track`, `wait_for_info`, `run_session`, `emit_position_if_due`
- Removed: `engine::OutputFactory`, per-track `start_playback`/`run_track`, the `oxitide-position` ticker thread (the worker emits `Position`).

Behavior is **identical** to Plan 2 after this task: no gapless yet (the worker stops and reopens the output between tracks), and `EngineCmd::Seek` stays a no-op until Task 8.

- [ ] **Step 1: Replace the non-test code in `engine.rs`**

Replace everything in `crates/oxitide-audio/src/engine.rs` above `#[cfg(test)] mod tests` with:

```rust
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::decode::{PcmBuffer, PcmFormat, PcmSink, StreamInfo};
use crate::error::{Error, Result};
use crate::fetch::{self, Fetcher, StreamBuffer};
use crate::output::{AudioOutput, AudioOutputSpec, ExclusiveFallback, OutputMode, OutputStatus};
use crate::queue::{Queue, QueueItem, RepeatMode};
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
    pub repeat: RepeatMode,
    pub preferred_quality: Quality,
    pub output_mode: OutputMode,
    pub bit_perfect: bool,
    pub bit_perfect_notice: Option<String>,
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
    SetRepeat(RepeatMode),
    Reorder { from: usize, to: usize },
    Remove(usize),
    Clear,
}

type Listener = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// Number of decoded PCM buffers a track's producer may run ahead of the
/// worker. Bounds the lookahead (typically a few seconds of audio) so a long
/// track never buffers in memory without limit.
const PREFETCH_BUFFERS: usize = 96;

struct Inner {
    queue: Mutex<Queue>,
    resolver: Arc<dyn StreamResolver>,
    fetcher: Arc<dyn Fetcher>,
    quality: Mutex<Quality>,
    device_id: Mutex<Option<String>>,
    output_mode: Mutex<OutputMode>,
    exclusive_fallback: Mutex<ExclusiveFallback>,
    output_status: Mutex<OutputStatus>,
    listeners: Mutex<Vec<Listener>>,
    /// Serializes queue mutation against the worker's advance decision.
    /// Lock ordering: always take `playback_lock` before `queue`.
    playback_lock: Mutex<()>,
    /// Incremented to invalidate the running session; the worker restarts.
    generation: AtomicU64,
    paused: AtomicBool,
    volume: AtomicU32, // f32 bits
    position_ms: AtomicU64,
    /// Pending seek target in milliseconds; consumed by the worker.
    seek_target: Mutex<Option<u64>>,
    /// True while the worker is running a session (used by `PlayPause` to know
    /// whether a resume needs a fresh generation).
    worker_busy: AtomicBool,
    wake_lock: Mutex<()>,
    wake: Condvar,
    shutdown: AtomicBool,
}

impl Inner {
    fn notify_worker(&self) {
        self.wake.notify_all();
    }
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

fn build_state(inner: &Inner) -> PlaybackState {
    let status = inner.output_status.lock().unwrap().clone();
    let q = inner.queue.lock().unwrap();
    PlaybackState {
        playing: q.is_playing(),
        buffering: false,
        current: q.current().cloned(),
        position_ms: inner.position_ms.load(Ordering::SeqCst),
        duration_ms: q.current().map(|i| i.duration_secs as u64 * 1000).unwrap_or(0),
        volume: q.volume(),
        shuffle: q.shuffle(),
        repeat: q.repeat(),
        preferred_quality: *inner.quality.lock().unwrap(),
        output_mode: status.mode,
        bit_perfect: status.bit_perfect,
        bit_perfect_notice: status.notice,
        queue: q.items().to_vec(),
    }
}

fn emit(inner: &Arc<Inner>, event: EngineEvent) {
    for listener in inner.listeners.lock().unwrap().iter() {
        listener(event.clone());
    }
}

impl Engine {
    pub fn new(
        resolver: Arc<dyn StreamResolver>,
        fetcher: Arc<dyn Fetcher>,
        output: Box<dyn AudioOutput>,
        quality: Quality,
    ) -> Self {
        let inner = Arc::new(Inner {
            queue: Mutex::new(Queue::default()),
            resolver,
            fetcher,
            quality: Mutex::new(quality),
            device_id: Mutex::new(None),
            output_mode: Mutex::new(OutputMode::Shared),
            exclusive_fallback: Mutex::new(ExclusiveFallback::Shared),
            output_status: Mutex::new(OutputStatus::default()),
            listeners: Mutex::new(Vec::new()),
            playback_lock: Mutex::new(()),
            generation: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            position_ms: AtomicU64::new(0),
            seek_target: Mutex::new(None),
            worker_busy: AtomicBool::new(false),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });
        let worker_inner = inner.clone();
        std::thread::Builder::new()
            .name("oxitide-worker".into())
            .spawn(move || worker_main(worker_inner, output))
            .expect("spawn playback worker");
        Self { inner }
    }

    pub fn subscribe(&self, listener: Listener) {
        self.inner.listeners.lock().unwrap().push(listener);
    }

    /// Stop the playback worker and release the output. Called on logout.
    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.notify_worker();
    }

    pub fn set_quality(&self, quality: Quality) {
        *self.inner.quality.lock().unwrap() = quality;
    }

    /// Select the output device for future sessions. `None` means the system
    /// default. Takes effect on the next track (Task 8 makes it immediate).
    pub fn set_output_device(&self, device: Option<String>) {
        *self.inner.device_id.lock().unwrap() = device;
    }

    fn output_device(&self) -> Option<String> {
        self.inner.device_id.lock().unwrap().clone()
    }

    /// The mode/bit-perfect state the output last reported.
    pub fn audio_status(&self) -> OutputStatus {
        self.inner.output_status.lock().unwrap().clone()
    }

    /// Select shared or exclusive output. Applies to the next track opened;
    /// Task 8 makes this apply immediately.
    pub fn set_output_mode(&self, mode: OutputMode) {
        *self.inner.output_mode.lock().unwrap() = mode;
    }

    /// Select what to do when exclusive output is unavailable.
    pub fn set_exclusive_fallback(&self, fallback: ExclusiveFallback) {
        *self.inner.exclusive_fallback.lock().unwrap() = fallback;
    }

    pub fn state(&self) -> PlaybackState {
        build_state(&self.inner)
    }

    fn emit(&self, event: EngineEvent) {
        emit(&self.inner, event);
    }

    /// Run a queue-mutating playback command under `playback_lock`, bumping the
    /// generation first so the running worker session is invalidated.
    fn mutate_playing_queue<T>(&self, mutate: impl FnOnce(&mut Queue) -> T) -> T {
        let _guard = self.inner.playback_lock.lock().unwrap();
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        let mut queue = self.inner.queue.lock().unwrap();
        mutate(&mut queue)
    }

    /// Like `mutate_playing_queue`, but only bumps the generation when `mutate`
    /// actually moved the current index or stopped playback (a no-op `Prev` at
    /// the first track must leave the running session untouched).
    fn mutate_queue_if_changed(&self, mutate: impl FnOnce(&mut Queue)) -> (Option<QueueItem>, bool) {
        let _guard = self.inner.playback_lock.lock().unwrap();
        let mut queue = self.inner.queue.lock().unwrap();
        let before = queue.current_index();
        let was_playing = queue.is_playing();
        mutate(&mut queue);
        let changed = queue.current_index() != before;
        if changed || (was_playing && !queue.is_playing()) {
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
        }
        (queue.current().cloned(), changed)
    }

    pub fn handle(&self, cmd: EngineCmd) -> Result<()> {
        match cmd {
            EngineCmd::PlayTracks { items, start } => {
                let item = self.mutate_playing_queue(|q| {
                    q.play_context(items, start);
                    q.current().cloned()
                });
                self.inner.notify_worker();
                self.emit(EngineEvent::NowPlaying(item.clone()));
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::PlayPause => {
                // Only toggle when a track is current. On an empty queue
                // `toggle_play_pause` is a no-op, so flipping `paused` here
                // would latch it true and silence the next `PlayTracks`.
                let paused = {
                    let mut q = self.inner.queue.lock().unwrap();
                    if q.current().is_some() {
                        q.toggle_play_pause();
                        Some(!q.is_playing())
                    } else {
                        None
                    }
                };
                if let Some(paused) = paused {
                    self.inner.paused.store(paused, Ordering::SeqCst);
                    // Resuming while the worker is idle (queue ended, output
                    // error, device loss) needs a fresh generation to start a
                    // session; while a session is live, pause is an atomic
                    // check and must NOT invalidate the running session.
                    if !paused && !self.inner.worker_busy.load(Ordering::SeqCst) {
                        self.inner.generation.fetch_add(1, Ordering::SeqCst);
                    }
                }
                self.inner.notify_worker();
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Next => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.next();
                });
                self.inner.notify_worker();
                if changed {
                    self.emit(EngineEvent::NowPlaying(item.clone()));
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Prev => {
                let (item, changed) = self.mutate_queue_if_changed(|q| {
                    q.prev();
                });
                self.inner.notify_worker();
                if changed {
                    self.emit(EngineEvent::NowPlaying(item.clone()));
                }
                self.emit(EngineEvent::State(self.state()));
            }
            EngineCmd::Seek(_) => { /* real seek lands in Task 8 */ }
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
                self.mutate_playing_queue(|q| q.clear());
                self.inner.paused.store(false, Ordering::SeqCst);
                self.inner.notify_worker();
                self.emit(EngineEvent::NowPlaying(None));
                self.emit(EngineEvent::State(self.state()));
            }
        }
        Ok(())
    }

    /// Synchronous resolve/download/decode into `output`; used by tests.
    /// Ignores the queue and cancellation.
    pub fn play_track_into(&self, item: &QueueItem, output: &mut dyn AudioOutput) -> Result<()> {
        let mode = *self.inner.output_mode.lock().unwrap();
        let fallback = *self.inner.exclusive_fallback.lock().unwrap();
        let generation = self.inner.generation.load(Ordering::SeqCst);
        let mut stream = start_track(&self.inner, item, mode, generation);
        let info = wait_for_info(&self.inner, &mut stream, generation)?;
        let spec = AudioOutputSpec {
            device_id: self.output_device(),
            info,
            mode,
            exclusive_fallback: fallback,
        };
        output.open(&spec)?;
        output.start()?;
        output.set_volume(f32::from_bits(self.inner.volume.load(Ordering::SeqCst)))?;
        loop {
            match stream.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(StreamEvent::Samples(buf))) => output.write(&buf)?,
                Ok(Ok(StreamEvent::Info(_))) => {}
                Ok(Err(e)) => {
                    let _ = output.stop();
                    return Err(e);
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        output.stop()
    }
}

/// One decoded chunk handed from a track's producer to the worker.
enum StreamEvent {
    Info(StreamInfo),
    Samples(PcmBuffer),
}

/// A running resolve/fetch/decode pipeline for one track.
struct TrackStream {
    rx: Receiver<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
}

impl Drop for TrackStream {
    fn drop(&mut self) {
        // Stop the producer's fetch/decode promptly when the stream is
        // abandoned (next/prev/seek/clear or track change).
        self.buffer.cancel();
    }
}

fn start_track(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
) -> TrackStream {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<StreamEvent>>(PREFETCH_BUFFERS);
    let buffer = StreamBuffer::new();
    let stream = TrackStream { rx, buffer: buffer.clone() };
    let inner = inner.clone();
    let item = item.clone();
    let spawned = std::thread::Builder::new()
        .name("oxitide-decode".into())
        .spawn(move || decode_track(inner, item, mode, generation, tx, buffer));
    if let Err(e) = spawned {
        // Dropping `tx` disconnects the channel; the worker treats it as an
        // ended track and advances.
        tracing::error!("failed to spawn decode thread: {e}");
    }
    stream
}

fn decode_track(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
) {
    let outcome: Result<()> = (|| {
        let quality = *inner.quality.lock().unwrap();
        let plan = inner.resolver.resolve(item.track_id, quality)?;
        let fetch_buffer = buffer.clone();
        let fetcher = inner.fetcher.clone();
        let downloader = std::thread::spawn(move || fetch::download(fetch_buffer, fetcher, plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut sink = ChannelSink {
            inner,
            generation,
            tx: tx.clone(),
            mode,
            out_format: PcmFormat::F32,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = downloader.join();
        result
    })();
    if let Err(e) = outcome {
        let _ = tx.send(Err(e));
    }
}

struct ChannelSink {
    inner: Arc<Inner>,
    generation: u64,
    tx: SyncSender<Result<StreamEvent>>,
    mode: OutputMode,
    out_format: PcmFormat,
}

impl ChannelSink {
    fn send(&self, event: Result<StreamEvent>) -> Result<()> {
        self.tx.send(event).map_err(|_| Error::Output("playback stream closed".into()))
    }
}

impl PcmSink for ChannelSink {
    fn preferred_format(&self) -> PcmFormat {
        self.out_format
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.out_format = match self.mode {
            OutputMode::Exclusive if info.is_bit_perfect_candidate() => info.format,
            _ => PcmFormat::F32,
        };
        self.send(Ok(StreamEvent::Info(*info)))
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.inner.generation.load(Ordering::SeqCst) != self.generation {
            return Err(Error::Output("playback superseded".into()));
        }
        self.send(Ok(StreamEvent::Samples(buf.clone())))
    }
}

fn wait_for_info(inner: &Arc<Inner>, stream: &mut TrackStream, gen: u64) -> Result<StreamInfo> {
    loop {
        // A supersede while a track is still resolving/fetching (e.g. Next
        // pressed during a slow network resolve) must exit promptly.
        if inner.generation.load(Ordering::SeqCst) != gen {
            return Err(Error::Output("playback superseded".into()));
        }
        match stream.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(StreamEvent::Info(info))) => return Ok(info),
            Ok(Ok(StreamEvent::Samples(_))) => continue,
            Ok(Err(e)) => return Err(e),
            Err(RecvTimeoutError::Timeout) => {
                if inner.shutdown.load(Ordering::SeqCst) {
                    return Err(Error::Output("engine shut down".into()));
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Decode("stream ended before any audio".into()));
            }
        }
    }
}

fn emit_position_if_due(inner: &Arc<Inner>, frames: u64, rate: u32, last: &mut Instant) {
    if last.elapsed() < Duration::from_millis(500) {
        return;
    }
    *last = Instant::now();
    let position_ms = if rate == 0 { 0 } else { frames.saturating_mul(1000) / rate as u64 };
    let duration_ms = inner
        .queue
        .lock()
        .unwrap()
        .current()
        .map(|i| i.duration_secs as u64 * 1000)
        .unwrap_or(0);
    emit(inner, EngineEvent::Position { position_ms, duration_ms });
}

enum SessionOutcome {
    Superseded,
    QueueEnded,
    OutputError(String),
    StreamError(Error),
}

/// Play `first_item` and keep advancing through the queue until it ends, the
/// generation changes, or a device/stream error occurs.
fn run_session(
    inner: &Arc<Inner>,
    output: &mut dyn AudioOutput,
    open_spec: &mut Option<AudioOutputSpec>,
    first_item: &QueueItem,
    mode: OutputMode,
    fallback: ExclusiveFallback,
    gen: u64,
) -> SessionOutcome {
    let mut stream = start_track(inner, first_item, mode, gen);
    let mut frames: u64 = 0;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    loop {
        if inner.generation.load(Ordering::SeqCst) != gen {
            return SessionOutcome::Superseded;
        }
        let info = match wait_for_info(inner, &mut stream, gen) {
            Ok(info) => info,
            Err(_) if inner.generation.load(Ordering::SeqCst) != gen => {
                return SessionOutcome::Superseded;
            }
            Err(e) => return SessionOutcome::StreamError(e),
        };
        let want = AudioOutputSpec {
            device_id: inner.device_id.lock().unwrap().clone(),
            info,
            mode,
            exclusive_fallback: fallback,
        };
        if open_spec.as_ref() != Some(&want) {
            let _ = output.stop();
            if let Err(e) = output.open(&want) {
                return SessionOutcome::OutputError(e.to_string());
            }
            if let Err(e) = output.start() {
                return SessionOutcome::OutputError(e.to_string());
            }
            let volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
            if let Err(e) = output.set_volume(volume) {
                return SessionOutcome::OutputError(e.to_string());
            }
            *inner.output_status.lock().unwrap() = output.status();
            *open_spec = Some(want);
        }
        let channels = info.channels;
        let rate = info.sample_rate;
        let mut last_volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
        loop {
            if inner.generation.load(Ordering::SeqCst) != gen {
                return SessionOutcome::Superseded;
            }
            if inner.paused.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
                emit_position_if_due(inner, frames, rate, &mut last_emit);
                continue;
            }
            let volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
            if (volume - last_volume).abs() >= f32::EPSILON {
                if let Err(e) = output.set_volume(volume) {
                    return SessionOutcome::OutputError(e.to_string());
                }
                last_volume = volume;
            }
            match stream.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(StreamEvent::Samples(buf))) => {
                    if let Err(e) = output.write(&buf) {
                        return SessionOutcome::OutputError(e.to_string());
                    }
                    frames += buf.frames(channels);
                    if rate > 0 {
                        inner
                            .position_ms
                            .store(frames.saturating_mul(1000) / rate as u64, Ordering::SeqCst);
                    }
                    emit_position_if_due(inner, frames, rate, &mut last_emit);
                }
                Ok(Ok(StreamEvent::Info(_))) => {}
                Ok(Err(e)) => return SessionOutcome::StreamError(e),
                Err(RecvTimeoutError::Timeout) => {
                    emit_position_if_due(inner, frames, rate, &mut last_emit);
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        // Track ended: advance the queue under the playback lock, the same way
        // the old per-track thread did.
        let next = {
            let _guard = inner.playback_lock.lock().unwrap();
            if inner.generation.load(Ordering::SeqCst) != gen {
                return SessionOutcome::Superseded;
            }
            let next = {
                let mut q = inner.queue.lock().unwrap();
                q.on_track_ended().and_then(|i| q.items().get(i).cloned())
            };
            emit(inner, EngineEvent::NowPlaying(next.clone()));
            emit(inner, EngineEvent::State(build_state(inner)));
            next
        };
        match next {
            Some(item) => {
                // Task 7 keeps the output open when the next track's format
                // matches; for now every track reopens.
                let _ = output.stop();
                *open_spec = None;
                inner.position_ms.store(0, Ordering::SeqCst);
                frames = 0;
                stream = start_track(inner, &item, mode, gen);
            }
            None => return SessionOutcome::QueueEnded,
        }
    }
}

fn worker_main(inner: Arc<Inner>, mut output: Box<dyn AudioOutput>) {
    let mut open_spec: Option<AudioOutputSpec> = None;
    let mut handled_gen: u64 = 0;
    loop {
        if inner.shutdown.load(Ordering::SeqCst) {
            let _ = output.stop();
            return;
        }
        let gen_now = inner.generation.load(Ordering::SeqCst);
        let ready = {
            let q = inner.queue.lock().unwrap();
            gen_now != handled_gen && q.current().is_some() && q.is_playing()
        };
        if !ready {
            if open_spec.is_some() {
                let _ = output.stop();
                open_spec = None;
            }
            inner.worker_busy.store(false, Ordering::SeqCst);
            let guard = inner.wake_lock.lock().unwrap();
            let _ = inner.wake.wait_timeout(guard, Duration::from_millis(200)).unwrap();
            continue;
        }
        handled_gen = gen_now;
        inner.worker_busy.store(true, Ordering::SeqCst);

        let item = {
            let _guard = inner.playback_lock.lock().unwrap();
            let q = inner.queue.lock().unwrap();
            q.current().cloned()
        };
        let Some(item) = item else {
            continue;
        };
        inner.paused.store(false, Ordering::SeqCst);
        inner.position_ms.store(0, Ordering::SeqCst);
        let mode = *inner.output_mode.lock().unwrap();
        let fallback = *inner.exclusive_fallback.lock().unwrap();

        match run_session(&inner, output.as_mut(), &mut open_spec, &item, mode, fallback, handled_gen)
        {
            SessionOutcome::Superseded => {}
            SessionOutcome::QueueEnded => {
                let _ = output.stop();
                open_spec = None;
            }
            SessionOutcome::OutputError(message) => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                {
                    let mut status = inner.output_status.lock().unwrap();
                    status.bit_perfect = false;
                    status.notice = Some(message.clone());
                }
                emit(&inner, EngineEvent::Error { code: "output".into(), message });
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
            SessionOutcome::StreamError(e) => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                emit(&inner, EngineEvent::Error { code: "playback".into(), message: e.to_string() });
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
        }
    }
}
```

- [ ] **Step 2: Replace the engine test module**

Replace the whole `#[cfg(test)] mod tests` block at the bottom of `crates/oxitide-audio/src/engine.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::PcmFormat;
    use crate::output::NullOutput;
    use std::collections::HashMap;

    struct FakeResolver {
        plan: StreamPlan,
    }

    impl StreamResolver for FakeResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Ok(self.plan.clone())
        }
    }

    struct FailingResolver;

    impl StreamResolver for FailingResolver {
        fn resolve(&self, _track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            Err(Error::Manifest("boom".into()))
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
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 2,
            cover: None,
        }
    }

    fn engine_with(
        plan: StreamPlan,
        files: HashMap<String, Vec<u8>>,
    ) -> (Engine, NullOutput) {
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        (engine, output)
    }

    fn wait_for(mut ready: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
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
        assert!(matches!(events[0], EngineEvent::NowPlaying(Some(_))));
        assert!(events.iter().take(4).any(|e| matches!(e, EngineEvent::State(_))));
    }

    #[test]
    fn play_track_into_decodes_fixture() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let mut out = NullOutput::new();
        engine.play_track_into(&item(1), &mut out).unwrap();
        assert_eq!(out.samples().len(), 176_400);
        assert_eq!(out.opens(), 1);
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
    fn worker_writes_samples_and_advances() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() >= 176_400);
        assert!(output.samples().len() >= 176_400);
    }

    /// Regression: `PlayPause` on an empty queue must not latch `paused` true
    /// and silence the next `PlayTracks`.
    #[test]
    fn play_pause_on_empty_queue_does_not_silence_later_playback() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayPause).unwrap();
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| output.samples().len() >= 176_400);
        assert!(output.samples().len() >= 176_400);
    }

    /// Regression: `Position` events must stop once the queue ends instead of
    /// ticking forever.
    #[test]
    fn position_events_stop_when_queue_ends() {
        let (plan, files) = fixture_plan();
        let (engine, _output) = engine_with(plan, files);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| !engine.state().playing);
        assert!(!engine.state().playing, "single track did not finish");

        let positions = || {
            seen.lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, EngineEvent::Position { .. }))
                .count()
        };
        std::thread::sleep(Duration::from_millis(600));
        let before = positions();
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(positions(), before, "position events kept coming after the queue ended");
    }

    struct Gate {
        released: Mutex<std::collections::HashSet<u64>>,
        cond: std::sync::Condvar,
    }

    impl Gate {
        fn new() -> Self {
            Self { released: Mutex::new(std::collections::HashSet::new()), cond: std::sync::Condvar::new() }
        }
        fn wait(&self, id: u64) {
            let mut released = self.released.lock().unwrap();
            while !released.contains(&id) {
                released = self.cond.wait(released).unwrap();
            }
        }
        fn release(&self, id: u64) {
            self.released.lock().unwrap().insert(id);
            self.cond.notify_all();
        }
    }

    struct GatedResolver {
        plan: StreamPlan,
        gate: Arc<Gate>,
        order: Arc<Mutex<Vec<u64>>>,
    }

    impl StreamResolver for GatedResolver {
        fn resolve(&self, track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            self.order.lock().unwrap().push(track_id);
            self.gate.wait(track_id);
            Ok(self.plan.clone())
        }
    }

    fn wait_for_order(order: &Arc<Mutex<Vec<u64>>>, id: u64) {
        wait_for(|| order.lock().unwrap().contains(&id));
        assert!(order.lock().unwrap().contains(&id), "track {id} was never resolved");
    }

    /// Regression: a `Next` issued while a track is playing must not cause a
    /// double advance (duplicate or skipped track).
    #[test]
    fn next_while_playing_does_not_duplicate_or_skip() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(GatedResolver { plan, gate: gate.clone(), order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
        );

        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2), item(3)], start: 0 })
            .unwrap();
        wait_for_order(&order, 1);
        // Let track 1 complete and auto-advance to track 2, then press Next.
        gate.release(1);
        wait_for_order(&order, 2);
        engine.handle(EngineCmd::Next).unwrap();
        wait_for_order(&order, 3);
        gate.release(2);
        gate.release(3);

        wait_for(|| !engine.state().playing);
        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(3));
    }

    /// Regression: a failed resolve/decode must stop the queue and report.
    #[test]
    fn playback_error_stops_and_reports() {
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FailingResolver),
            Arc::new(crate::fetch::MapFetcher::default()),
            Box::new(output),
            Quality::Lossless,
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();
        engine.subscribe(Arc::new(move |e| seen_ref.lock().unwrap().push(e)));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| {
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. }))
        });
        assert!(
            seen.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::Error { .. })),
            "no error event emitted"
        );
        assert!(!engine.state().playing, "queue still reports playing after an error");
    }

    /// Regression: `Next` at the end of the queue with repeat off must not
    /// restart the last track.
    #[test]
    fn next_at_end_with_repeat_off_does_not_restart() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| !engine.state().playing);
        assert!(!engine.state().playing, "single track did not finish");

        let before = output.samples().len();
        assert!(before >= 176_400);
        engine.handle(EngineCmd::Next).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(output.samples().len(), before, "Next at the end restarted playback");
        assert!(!engine.state().playing, "Next at the end left the queue playing");
    }

    struct GatedFetcher {
        files: HashMap<String, Vec<u8>>,
        gate: Arc<Gate>,
        id: u64,
    }

    impl Fetcher for GatedFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>> {
            self.gate.wait(self.id);
            self.files
                .get(url)
                .cloned()
                .ok_or_else(|| Error::Fetch(format!("no fixture for {url}")))
        }
    }

    /// Regression: `Prev` at the first track is a no-op and must leave the
    /// running session (and its generation) untouched.
    #[test]
    fn prev_at_first_track_does_not_restart() {
        let (plan, files) = fixture_plan();
        let gate = Arc::new(Gate::new());
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(GatedFetcher { files, gate: gate.clone(), id: 1 }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();
        // Playback is parked in the fetcher, so nothing has been written yet.
        std::thread::sleep(Duration::from_millis(200));
        let before_gen = engine.inner.generation.load(Ordering::SeqCst);
        let before_samples = output.samples().len();

        engine.handle(EngineCmd::Prev).unwrap();

        assert_eq!(
            engine.inner.generation.load(Ordering::SeqCst),
            before_gen,
            "no-op Prev bumped the generation"
        );
        assert!(engine.state().playing, "no-op Prev stopped playback");
        assert_eq!(engine.state().current.map(|i| i.track_id), Some(1));
        assert_eq!(output.samples().len(), before_samples);
        gate.release(1);
        gate.release(2);
    }

    /// An output that records the spec passed to `open`.
    struct RecordingOutput {
        specs: Arc<Mutex<Vec<AudioOutputSpec>>>,
    }

    impl AudioOutput for RecordingOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.specs.lock().unwrap().push(spec.clone());
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }

    #[test]
    fn set_output_device_is_passed_to_open() {
        let (plan, files) = fixture_plan();
        let specs = Arc::new(Mutex::new(Vec::new()));
        let specs_ref = specs.clone();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(RecordingOutput { specs: specs_ref }),
            Quality::Lossless,
        );
        engine.set_output_device(Some("device-x".into()));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| !specs.lock().unwrap().is_empty());
        assert_eq!(specs.lock().unwrap()[0].device_id.as_deref(), Some("device-x"));
    }

    /// An output that always reports the given status.
    struct FixedStatusOutput {
        status: OutputStatus,
    }

    impl AudioOutput for FixedStatusOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            self.status.clone()
        }
    }

    #[test]
    fn null_output_reports_exclusive_status() {
        let (plan, files) = fixture_plan();
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.audio_status().mode == OutputMode::Exclusive);
        assert_eq!(engine.audio_status().mode, OutputMode::Exclusive);
        assert!(engine.audio_status().bit_perfect);
    }

    #[test]
    fn output_status_notice_flows_to_playback_state() {
        let (plan, files) = fixture_plan();
        let status = OutputStatus {
            mode: OutputMode::Exclusive,
            bit_perfect: false,
            notice: Some("volume applied in software; bit-perfect compromised".into()),
        };
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FixedStatusOutput { status }),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert_eq!(state.output_mode, OutputMode::Exclusive);
        assert!(!state.bit_perfect);
        assert_eq!(
            state.bit_perfect_notice.as_deref(),
            Some("volume applied in software; bit-perfect compromised")
        );
    }

    /// Review Focus: an output that cannot open (e.g. exclusive unsupported
    /// with `warn_only`) must pause with a notice, keep the queue, and never
    /// panic.
    struct FailingOpenOutput;

    impl AudioOutput for FailingOpenOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            Err(Error::Output("exclusive mode is not supported by this output device".into()))
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, _buf: &PcmBuffer) -> Result<()> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            OutputStatus::default()
        }
    }

    #[test]
    fn output_open_failure_pauses_with_notice() {
        let (plan, files) = fixture_plan();
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(FailingOpenOutput),
            Quality::Lossless,
        );
        engine.set_output_mode(OutputMode::Exclusive);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert!(!state.playing, "open failure must pause, not stop the queue");
        assert_eq!(state.current.map(|i| i.track_id), Some(1));
        assert_eq!(state.queue.len(), 1);
        assert!(state.bit_perfect_notice.is_some());
    }
}
```

- [ ] **Step 3: Update the app to own a single output**

In `crates/oxitide-app/src/player.rs`, replace the `OutputFactory` import and `RealOutputs` (lines 1 and 39–54) with:

```rust
use oxitide_audio::engine::{EngineEvent, StreamResolver};
use oxitide_audio::output::{self, AudioOutput, OutputDevice};
```

```rust
/// The real output: a WASAPI device on Windows, silent `NullOutput` elsewhere
/// (keeps the crate building on non-Windows dev machines).
pub fn real_output() -> Box<dyn AudioOutput> {
    #[cfg(windows)]
    {
        Box::new(oxitide_audio::wasapi_output::WasapiOutput::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(oxitide_audio::output::NullOutput::new())
    }
}
```

In `crates/oxitide-app/src/commands.rs`:
1. Replace the `crate::player::{devices, OutputDeviceDto, QualityDto, RealOutputs}` import with:

```rust
use crate::player::{devices, OutputDeviceDto, QualityDto};
```

2. In `build_player`, replace `Arc::new(RealOutputs)` with `crate::player::real_output()`.

3. In `auth_logout`, after the `Clear` command, release the worker:

```rust
    if let Some(player) = state.player() {
        let _ = player.engine.handle(oxitide_audio::engine::EngineCmd::Clear);
        player.engine.shutdown();
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-audio`
Expected: PASS. The engine tests are timing-based; if `next_while_playing_does_not_duplicate_or_skip` is flaky, check that `wait_for_order` waits on the worker's producer thread (it does) and that `Gate` blocks inside `resolve`.

- [ ] **Step 5: Run the full gates**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green. `grep -r "OutputFactory" crates/` must return nothing.

- [ ] **Step 6: Manual check**

Run `npm --prefix ui run app:dev` with a real account: play an album, skip next/prev during playback, pause/resume, change volume, clear the queue, and log out. Behavior must match Plan 2. Record the result in `docs/superpowers/plans/notes/plan2-smoke.md` under `## Plan 3 worker check`.

- [ ] **Step 7: Commit**

```powershell
git add crates/oxitide-audio/src/engine.rs crates/oxitide-app/src/player.rs crates/oxitide-app/src/commands.rs docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "feat(audio): replace per-track threads with a long-lived playback worker"
```

---

### Task 7: Gapless chaining and lookahead

**Files:**
- Modify: `crates/oxitide-audio/src/queue.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`

**Interfaces:**
- Consumes: `TrackStream`, `start_track`, `wait_for_info`, `run_session`, `PREFETCH_BUFFERS` from Task 6; `StreamInfo: PartialEq` from Task 2.
- Produces:
  - `Queue::peek_next_index() -> Option<usize>` (what `on_track_ended` would select, without mutating)
  - `engine::start_prefetch(inner, mode, gen) -> Option<TrackStream>`
  - Gapless rule: the output is reopened only when the next track's `StreamInfo` differs from the one the output was opened with.

- [ ] **Step 1: Write the failing queue tests**

Add to the test module of `crates/oxitide-audio/src/queue.rs`:

```rust
    #[test]
    fn peek_next_index_matches_on_track_ended() {
        for repeat in [RepeatMode::Off, RepeatMode::All, RepeatMode::One] {
            for start in 0..3 {
                let mut q = Queue::default();
                q.play_context((1..=3).map(item).collect(), start);
                q.set_repeat(repeat);
                let mut expected = q.clone();
                assert_eq!(
                    q.peek_next_index(),
                    expected.on_track_ended(),
                    "repeat {repeat:?}, start {start}"
                );
            }
        }
    }

    #[test]
    fn peek_next_index_is_none_when_stopped_at_end() {
        let mut q = queue(2);
        q.next(); // last item
        assert_eq!(q.peek_next_index(), None);
        // Peeks never mutate.
        assert!(q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio queue::`
Expected: compile error (`peek_next_index` does not exist).

- [ ] **Step 3: Refactor `on_track_ended` around `peek_next_index`**

In `crates/oxitide-audio/src/queue.rs`, replace `on_track_ended` (current lines 122–145) with:

```rust
    /// What `on_track_ended` would select, without mutating. Used to prefetch
    /// the next track while the current one plays.
    pub fn peek_next_index(&self) -> Option<usize> {
        let cur = self.current?;
        if self.items.is_empty() {
            return None;
        }
        if self.repeat == RepeatMode::One {
            return Some(cur);
        }
        if cur + 1 < self.items.len() {
            return Some(cur + 1);
        }
        if self.repeat == RepeatMode::All {
            return Some(0);
        }
        None
    }

    pub fn on_track_ended(&mut self) -> Option<usize> {
        match self.peek_next_index() {
            Some(index) => {
                self.current = Some(index);
                self.playing = true;
                Some(index)
            }
            // End of queue with repeat off: stop, keep the last item current.
            None => {
                self.playing = false;
                None
            }
        }
    }
```

- [ ] **Step 4: Run the queue tests**

Run: `cargo test -p oxitide-audio queue::`
Expected: PASS (all existing queue tests plus the two new ones).

- [ ] **Step 5: Write the failing gapless engine tests**

Add to the engine test module:

```rust
    /// A resolver over per-track plans that records resolve order.
    struct PerTrackResolver {
        plans: HashMap<u64, StreamPlan>,
        order: Arc<Mutex<Vec<u64>>>,
    }

    impl StreamResolver for PerTrackResolver {
        fn resolve(&self, track_id: u64, _quality: Quality) -> Result<StreamPlan> {
            self.order.lock().unwrap().push(track_id);
            self.plans
                .get(&track_id)
                .cloned()
                .ok_or_else(|| Error::Manifest(format!("no plan for {track_id}")))
        }
    }

    fn plan_for(url: &str, sample_rate: u32, bit_depth: u32) -> StreamPlan {
        StreamPlan {
            init_url: None,
            media_urls: vec![url.into()],
            codec: Some("flac".into()),
            sample_rate: Some(sample_rate),
            bit_depth: Some(bit_depth),
        }
    }

    #[test]
    fn matching_formats_chain_without_reopening() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();

        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert_eq!(output.samples().len(), 176_400 * 2);
        // One open for both tracks; a reopen between tracks would make this 2.
        assert_eq!(output.opens(), 1, "gapless playback must not reopen between matching tracks");
        assert_eq!(output.stops(), 1, "gapless playback must not stop between matching tracks");
    }

    #[test]
    fn format_change_reopens_at_the_boundary() {
        let tone16 = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let tone48 = std::fs::read("tests/fixtures/tone48.mp4").unwrap();
        let mut plans = HashMap::new();
        plans.insert(1, plan_for("mem://t16", 44100, 16));
        plans.insert(2, plan_for("mem://t48", 48000, 16));
        let mut files = HashMap::new();
        files.insert("mem://t16".to_string(), tone16);
        files.insert("mem://t48".to_string(), tone48);
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(PerTrackResolver { plans, order }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );

        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();

        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 + 192_000);
        // 2s @ 44.1k stereo + 2s @ 48k stereo.
        assert_eq!(output.samples().len(), 176_400 + 192_000);
        assert_eq!(output.opens(), 2, "a format change must reopen the output");
        assert_eq!(output.stops(), 2, "a format change must stop and reopen");
        assert_eq!(output.last_spec().map(|s| s.info.sample_rate), Some(48000));
    }

    #[test]
    fn prefetched_next_track_is_not_resolved_twice() {
        let (plan, files) = fixture_plan();
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut plans = HashMap::new();
        plans.insert(1, plan.clone());
        plans.insert(2, plan);
        let output = NullOutput::new();
        let engine = Engine::new(
            Arc::new(PerTrackResolver { plans, order: order.clone() }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(output.clone()),
            Quality::Lossless,
        );
        engine
            .handle(EngineCmd::PlayTracks { items: vec![item(1), item(2)], start: 0 })
            .unwrap();
        wait_for(|| !engine.state().playing && output.samples().len() >= 176_400 * 2);
        assert_eq!(*order.lock().unwrap(), vec![1, 2], "a prefetched track must not be resolved again");
    }
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio engine::`
Expected: `matching_formats_chain_without_reopening` FAILS with `opens == 2` (each track reopens), `format_change_reopens_at_the_boundary` passes already (Task 6 behavior), `prefetched_next_track_is_not_resolved_twice` FAILS (no prefetch).

- [ ] **Step 7: Implement prefetch and gapless chaining**

In `crates/oxitide-audio/src/engine.rs`:

1. Add the track item to `TrackStream` so the boundary can verify a prefetched stream is the right one:

```rust
/// A running resolve/fetch/decode pipeline for one track.
struct TrackStream {
    item: QueueItem,
    rx: Receiver<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
}
```

and in `start_track`, set the field:

```rust
    let stream = TrackStream { item: item.clone(), rx, buffer: buffer.clone() };
```

2. Add the prefetch helper next to `start_track`:

```rust
/// Start decoding the queue item that `on_track_ended` would select next, so
/// the worker can chain into it without a gap.
fn start_prefetch(inner: &Arc<Inner>, mode: OutputMode, gen: u64) -> Option<TrackStream> {
    let next = {
        let q = inner.queue.lock().unwrap();
        q.peek_next_index().and_then(|i| q.items().get(i).cloned())
    };
    next.map(|item| start_track(inner, &item, mode, gen))
}
```

3. In `run_session`, start the prefetch right after the first stream:

```rust
    let mut stream = start_track(inner, first_item, mode, gen);
    let mut prefetch = start_prefetch(inner, mode, gen);
```

4. Replace the boundary block in `run_session` (the `match next { Some(item) => { ... } None => ... }` at the end of the loop) with:

```rust
        match next {
            Some(item) => {
                // Use the prefetched stream when it is the right track; the
                // top of the loop decides whether the output can be reused
                // (matching StreamInfo) or must reopen (format change).
                stream = match prefetch.take() {
                    Some(prefetched) if prefetched.item.track_id == item.track_id => prefetched,
                    _ => start_track(inner, &item, mode, gen),
                };
                prefetch = start_prefetch(inner, mode, gen);
                inner.position_ms.store(0, Ordering::SeqCst);
                frames = 0;
            }
            None => return SessionOutcome::QueueEnded,
        }
```

Note that this removes the `output.stop()`/`*open_spec = None` calls that Task 6 had at the boundary. The top-of-loop `if open_spec.as_ref() != Some(&want)` now only reopens on a real `StreamInfo` (or device) change, which is the gapless behavior.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p oxitide-audio`
Expected: PASS — matching formats chain with `opens == 1`, `stops == 1`; a 48 kHz track reopens (`opens == 2`, `stops == 2`); the prefetched track is resolved exactly once.

- [ ] **Step 9: Run clippy and the UI gates**

Run:
```powershell
cargo clippy --workspace -- -D warnings
npm --prefix ui test
```
Expected: clean.

- [ ] **Step 10: Manual check**

With a real account, play an album whose tracks share a format (most albums): tracks must run into each other with no device click/re-open, and the position counter must reset at each boundary. Record in `docs/superpowers/plans/notes/plan2-smoke.md` under `## Plan 3 gapless check`.

- [ ] **Step 11: Commit**

```powershell
git add crates/oxitide-audio/src/queue.rs crates/oxitide-audio/src/engine.rs docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "feat(audio): chain matching-format tracks gaplessly with lookahead"
```

---

### Task 8: Seek

**Files:**
- Create: `crates/oxitide-audio/src/seek.rs`
- Modify: `crates/oxitide-audio/src/lib.rs`
- Modify: `crates/oxitide-audio/src/mpd.rs`
- Modify: `crates/oxitide-audio/src/source.rs`
- Modify: `crates/oxitide-audio/src/fetch.rs`
- Modify: `crates/oxitide-audio/src/engine.rs`
- Modify: `ui/src/lib/player.svelte.ts`
- Modify: `ui/src/lib/player.test.ts`
- Modify: `ui/src/lib/components/PlayerBar.svelte`

**Interfaces:**
- Consumes: prefetch/gapless session from Task 7; bounded `StreamBuffer` from Task 5.
- Produces:
  - `source::Segment { url: String, start_ticks: u64, duration_ticks: u64 }`
  - `StreamPlan { init_url, segments, timescale, codec, sample_rate, bit_depth }`, `StreamPlan::media_urls() -> Vec<&str>`, `StreamPlan::single(url)`, `StreamPlan::from_segment(index)`
  - `mpd::SegmentTiming { start_ticks, duration_ticks }`, `Representation::segment_timings(mpd_duration)`
  - `seek::{SeekTarget, target_frames, target_segment, Discarder}`
  - `engine::StreamEvent::Info(StreamStart { info, start_frames })`
  - `Engine::restart_current_at_position()` used by the settings setters
  - UI: `player.pendingSeek` + `reconcileSeek(pending, event)`; interactive PlayerBar slider

- [ ] **Step 1: Write the failing seek tests**

Create `crates/oxitide-audio/src/seek.rs` with only the tests first (the implementation lands in Step 3):

```rust
use crate::source::{Segment, StreamPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekTarget {
    pub segment_index: usize,
    pub discard_frames: u64,
    pub position_frames: u64,
}

pub fn target_frames(position_ms: u64, sample_rate: u32) -> u64 {
    unimplemented!()
}

pub fn target_segment(plan: &StreamPlan, position_ms: u64, sample_rate: u32) -> SeekTarget {
    unimplemented!()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Discarder {
    remaining_frames: u64,
}

impl Discarder {
    pub fn new(frames: u64) -> Self {
        unimplemented!()
    }

    pub fn apply(&mut self, total_samples: usize, channels: u16) -> (usize, usize) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_with_segments(timescale: u64, segments: &[(u64, u64)]) -> StreamPlan {
        StreamPlan {
            init_url: Some("init".into()),
            segments: segments
                .iter()
                .enumerate()
                .map(|(i, &(start_ticks, duration_ticks))| Segment {
                    url: format!("seg-{i}"),
                    start_ticks,
                    duration_ticks,
                })
                .collect(),
            timescale,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
        }
    }

    #[test]
    fn target_frames_rounds_down() {
        assert_eq!(target_frames(0, 44100), 0);
        assert_eq!(target_frames(1000, 44100), 44_100);
        assert_eq!(target_frames(1500, 44100), 66_150);
    }

    /// Duration-based MPD: timescale 1, 4-second segments; sample rate 1000 so
    /// frames == milliseconds.
    #[test]
    fn duration_based_target_uses_segment_math() {
        let plan = plan_with_segments(1, &[(0, 4), (4, 4), (8, 4)]);
        let target = target_segment(&plan, 5000, 1000);
        assert_eq!(target.segment_index, 1);
        assert_eq!(target.discard_frames, 1000);
        assert_eq!(target.position_frames, 5000);

        let start = target_segment(&plan, 4000, 1000);
        assert_eq!(start.segment_index, 1);
        assert_eq!(start.discard_frames, 0);

        let first = target_segment(&plan, 0, 1000);
        assert_eq!(first.segment_index, 0);
        assert_eq!(first.discard_frames, 0);
    }

    /// Timeline MPD: timescale 44100, 176400-tick segments.
    #[test]
    fn timeline_target_uses_timescale() {
        let plan = plan_with_segments(44100, &[(0, 176400), (176400, 176400), (352800, 176400)]);
        let target = target_segment(&plan, 2000, 44100);
        assert_eq!(target.segment_index, 0);
        assert_eq!(target.discard_frames, 88_200);
        assert_eq!(target.position_frames, 88_200);

        let second = target_segment(&plan, 5000, 44100);
        assert_eq!(second.segment_index, 1);
        assert_eq!(second.discard_frames, 44_100);
    }

    /// Review Focus: seeking beyond the track clamps to the last segment.
    #[test]
    fn target_segment_clamps_beyond_the_end() {
        let plan = plan_with_segments(1, &[(0, 4), (4, 4)]);
        let target = target_segment(&plan, 999_999, 1000);
        assert_eq!(target.segment_index, 1);
        assert_eq!(target.discard_frames, 999_999 - 4000);
        assert_eq!(target.position_frames, 999_999);
    }

    #[test]
    fn single_file_source_discards_from_the_start() {
        let plan = StreamPlan::single("file".into());
        let target = target_segment(&plan, 1000, 44100);
        assert_eq!(target.segment_index, 0);
        assert_eq!(target.discard_frames, 44_100);
    }

    #[test]
    fn discarder_drops_partial_buffers_across_calls() {
        let mut discarder = Discarder::new(5);
        // 8 samples = 4 stereo frames: all dropped, one frame still pending.
        assert_eq!(discarder.apply(8, 2), (8, 0));
        // Next buffer: 1 frame dropped, the remaining 3 frames are used.
        assert_eq!(discarder.apply(8, 2), (2, 6));
        // Once drained, buffers pass through untouched.
        assert_eq!(discarder.apply(8, 2), (0, 8));
    }
}
```

Add `pub mod seek;` to `crates/oxitide-audio/src/lib.rs` (alphabetical, after `queue`). This step does not compile yet because `StreamPlan::single` does not exist — that is expected; Step 3 creates it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio seek::`
Expected: compile error (`StreamPlan::single` missing; `unimplemented!` bodies would panic once it compiles).

- [ ] **Step 3: Implement the seek math**

Replace the placeholder bodies in `crates/oxitide-audio/src/seek.rs` with:

```rust
/// Frames for a millisecond position at `sample_rate`.
pub fn target_frames(position_ms: u64, sample_rate: u32) -> u64 {
    position_ms.saturating_mul(sample_rate as u64) / 1000
}

/// Pick the segment covering `position_ms` and how much of it to discard.
/// Positions beyond the last segment clamp to that segment.
pub fn target_segment(plan: &StreamPlan, position_ms: u64, sample_rate: u32) -> SeekTarget {
    let frames = target_frames(position_ms, sample_rate);
    if plan.segments.is_empty() || plan.timescale == 0 {
        // A single-file source or unknown timing: decode from the start and
        // discard everything before the target.
        return SeekTarget { segment_index: 0, discard_frames: frames, position_frames: frames };
    }
    let mut index = 0;
    let mut start = 0u64;
    for (i, segment) in plan.segments.iter().enumerate() {
        let segment_start =
            segment.start_ticks.saturating_mul(sample_rate as u64) / plan.timescale;
        if segment_start <= frames {
            index = i;
            start = segment_start;
        } else {
            break;
        }
    }
    SeekTarget { segment_index: index, discard_frames: frames - start, position_frames: frames }
}
```

and for `Discarder`:

```rust
impl Discarder {
    pub fn new(frames: u64) -> Self {
        Self { remaining_frames: frames }
    }

    /// Returns `(offset, len)` for the part of an interleaved buffer that
    /// should be used after dropping the pending frames. Operates on sample
    /// counts so it does not care about the sample type.
    pub fn apply(&mut self, total_samples: usize, channels: u16) -> (usize, usize) {
        let channels = channels.max(1) as usize;
        let available_frames = (total_samples / channels) as u64;
        if self.remaining_frames >= available_frames {
            self.remaining_frames -= available_frames;
            return (total_samples, 0);
        }
        let skip = self.remaining_frames as usize * channels;
        self.remaining_frames = 0;
        (skip, total_samples - skip)
    }
}
```

- [ ] **Step 4: Add segment timing to `mpd.rs`**

In `crates/oxitide-audio/src/mpd.rs`, add above `impl Representation`:

```rust
/// Timing of one media segment in SegmentTemplate ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentTiming {
    pub start_ticks: u64,
    pub duration_ticks: u64,
}
```

Add `segment_timings` to `impl Representation`:

```rust
    /// Start/duration ticks for every media segment, in the same order as
    /// `media_urls`.
    pub fn segment_timings(&self, mpd_duration: Option<&str>) -> Result<Vec<SegmentTiming>> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("no SegmentTemplate".into()))?;
        if !t.timeline.is_empty() {
            let ts = if t.timescale == 0 { 1 } else { t.timescale };
            let total_secs = mpd_duration.and_then(parse_iso8601_duration);
            let mut timings = Vec::new();
            let mut time = t.timeline[0].t.unwrap_or(0);
            for e in &t.timeline {
                if let Some(start) = e.t {
                    time = start;
                }
                let reps = repeat_count(e, time, ts, total_secs);
                for _ in 0..reps {
                    timings.push(SegmentTiming { start_ticks: time, duration_ticks: e.d });
                    time += e.d;
                }
            }
            return Ok(timings);
        }
        let n = self.segment_count(mpd_duration)?;
        let duration_ticks = t
            .duration
            .ok_or_else(|| Error::Manifest("no segment duration".into()))?;
        Ok((0..n)
            .map(|i| SegmentTiming { start_ticks: duration_ticks * i, duration_ticks })
            .collect())
    }
```

Replace `media_urls` (current lines 111–144) with a version built on `segment_timings` so the two can never disagree:

```rust
    pub fn media_urls(
        &self,
        mpd_duration: Option<&str>,
        mpd_base: &[String],
    ) -> Result<Vec<String>> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("no SegmentTemplate".into()))?;
        let base = self.base_url(mpd_base);
        let repr_id = self.id.clone().unwrap_or_default();
        let bw = self.bandwidth.unwrap_or(0);
        let timings = self.segment_timings(mpd_duration)?;
        Ok(timings
            .iter()
            .enumerate()
            .map(|(i, timing)| {
                let (number, time) = if t.timeline.is_empty() {
                    (t.start_number + i as u64, 0)
                } else {
                    (0, timing.start_ticks)
                };
                resolve_template(&base, &t.media, &repr_id, bw, number, time)
            })
            .collect())
    }
```

Add these tests to `mpd.rs`'s test module:

```rust
    #[test]
    fn segment_timings_match_number_template_durations() {
        let mpd = parse(NUMBER_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        let timings = r.segment_timings(mpd.media_presentation_duration.as_deref()).unwrap();
        assert_eq!(
            timings,
            vec![
                SegmentTiming { start_ticks: 0, duration_ticks: 4 },
                SegmentTiming { start_ticks: 4, duration_ticks: 4 },
            ]
        );
    }

    #[test]
    fn segment_timings_match_timeline_offsets() {
        let mpd = parse(TIMELINE_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        let timings = r.segment_timings(None).unwrap();
        assert_eq!(
            timings,
            vec![
                SegmentTiming { start_ticks: 0, duration_ticks: 176_400 },
                SegmentTiming { start_ticks: 176_400, duration_ticks: 176_400 },
                SegmentTiming { start_ticks: 352_800, duration_ticks: 176_400 },
            ]
        );
    }
```

- [ ] **Step 5: Convert `StreamPlan` to timed segments**

In `crates/oxitide-audio/src/source.rs`, replace `pub struct StreamPlan` and add the helper methods:

```rust
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
```

In `plan_from_info`, replace the `DASH_MIME` arm's tail and the `BTS_MIME` arm:

```rust
            let duration = doc.media_presentation_duration.as_deref();
            let media_urls = repr.media_urls(duration, &doc.base_urls)?;
            let timings = repr.segment_timings(duration)?;
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
                .collect();
            Ok(StreamPlan {
                init_url,
                segments,
                timescale,
                codec: repr.codecs.clone(),
                sample_rate: repr.sample_rate,
                bit_depth: repr.bit_depth,
            })
```

```rust
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
```

Update the source tests: `assert_eq!(plan.media_urls, vec![...])` becomes `assert_eq!(plan.media_urls(), vec![...])`, and add:

```rust
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
```

- [ ] **Step 6: Update `fetch.rs` to iterate segments**

In `download`, replace the URL assembly:

```rust
    let mut urls = Vec::new();
    if let Some(init) = &plan.init_url {
        urls.push(init.clone());
    }
    urls.extend(plan.segments.iter().map(|s| s.url.clone()));
```

Update the fetch tests to build plans with segments:

```rust
        let plan = StreamPlan {
            init_url: Some("init".into()),
            segments: vec![
                Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 },
                Segment { url: "b".into(), start_ticks: 0, duration_ticks: 0 },
            ],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
        };
```

and for the other three tests (`fetch_failure_is_reported_to_readers`, `download_retries_transient_fetch_failures`, `download_fails_after_bounded_attempts`) replace the plan literals with:

```rust
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "a".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
        };
```

(`fetch_failure_is_reported_to_readers` uses `"missing"` instead of `"a"`.) For `download_is_backpressured_and_bounded`, build the segments:

```rust
        let plan = StreamPlan {
            init_url: None,
            segments: (0..SEGMENTS)
                .map(|i| Segment { url: format!("s{i}"), start_ticks: 0, duration_ticks: 0 })
                .collect(),
            timescale: 0,
            codec: None,
            sample_rate: None,
            bit_depth: None,
        };
```

Add `use crate::source::Segment;` to the fetch test module.

- [ ] **Step 7: Wire seek into the engine**

In `crates/oxitide-audio/src/engine.rs`:

1. Add `use crate::seek::{self, Discarder};` to the imports.

2. Change `StreamEvent` and add `StreamStart`:

```rust
struct StreamStart {
    info: StreamInfo,
    start_frames: u64,
}

/// One decoded chunk handed from a track's producer to the worker.
enum StreamEvent {
    Info(StreamStart),
    Samples(PcmBuffer),
}
```

3. Update `ChannelSink`:

```rust
struct ChannelSink {
    inner: Arc<Inner>,
    generation: u64,
    tx: SyncSender<Result<StreamEvent>>,
    mode: OutputMode,
    out_format: PcmFormat,
    channels: u16,
    /// The original (untruncated) plan, for seek discard math.
    plan: StreamPlan,
    seek_ms: Option<u64>,
    discarder: Discarder,
    start_frames: u64,
}
```

```rust
impl PcmSink for ChannelSink {
    fn preferred_format(&self) -> PcmFormat {
        self.out_format
    }

    fn on_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.out_format = match self.mode {
            OutputMode::Exclusive if info.is_bit_perfect_candidate() => info.format,
            _ => PcmFormat::F32,
        };
        self.channels = info.channels;
        if let Some(ms) = self.seek_ms {
            let target = seek::target_segment(&self.plan, ms, info.sample_rate);
            self.discarder = Discarder::new(target.discard_frames);
            self.start_frames = target.position_frames;
        }
        self.send(Ok(StreamEvent::Info(StreamStart {
            info: *info,
            start_frames: self.start_frames,
        })))
    }

    fn on_samples(&mut self, buf: &PcmBuffer) -> Result<()> {
        if self.inner.generation.load(Ordering::SeqCst) != self.generation {
            return Err(Error::Output("playback superseded".into()));
        }
        let buf = match buf {
            PcmBuffer::F32(v) => {
                let (offset, len) = self.discarder.apply(v.len(), self.channels);
                if len == 0 {
                    return Ok(());
                }
                PcmBuffer::F32(v[offset..offset + len].to_vec())
            }
            PcmBuffer::I32(v) => {
                let (offset, len) = self.discarder.apply(v.len(), self.channels);
                if len == 0 {
                    return Ok(());
                }
                PcmBuffer::I32(v[offset..offset + len].to_vec())
            }
        };
        self.send(Ok(StreamEvent::Samples(buf)))
    }
}
```

4. Update `start_track` and `decode_track` to take and apply the seek:

```rust
fn start_track(
    inner: &Arc<Inner>,
    item: &QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
) -> TrackStream {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<StreamEvent>>(PREFETCH_BUFFERS);
    let buffer = StreamBuffer::new();
    let stream = TrackStream { item: item.clone(), rx, buffer: buffer.clone() };
    let inner = inner.clone();
    let item = item.clone();
    let spawned = std::thread::Builder::new()
        .name("oxitide-decode".into())
        .spawn(move || decode_track(inner, item, mode, generation, seek_ms, tx, buffer));
    if let Err(e) = spawned {
        tracing::error!("failed to spawn decode thread: {e}");
    }
    stream
}

fn decode_track(
    inner: Arc<Inner>,
    item: QueueItem,
    mode: OutputMode,
    generation: u64,
    seek_ms: Option<u64>,
    tx: SyncSender<Result<StreamEvent>>,
    buffer: Arc<StreamBuffer>,
) {
    let outcome: Result<()> = (|| {
        let quality = *inner.quality.lock().unwrap();
        let plan = inner.resolver.resolve(item.track_id, quality)?;
        // Seek by segment index: media URLs are an ordered list, so restarting
        // at the segment covering the target needs no HTTP Range.
        let download_plan = match seek_ms {
            Some(ms) if plan.sample_rate.is_some() => {
                let target = seek::target_segment(&plan, ms, plan.sample_rate.unwrap());
                plan.from_segment(target.segment_index)
            }
            _ => plan.clone(),
        };
        let fetch_buffer = buffer.clone();
        let fetcher = inner.fetcher.clone();
        let downloader = std::thread::spawn(move || fetch::download(fetch_buffer, fetcher, download_plan));
        let source = crate::demux::StreamBufferSource::new(buffer);
        let mut sink = ChannelSink {
            inner,
            generation,
            tx: tx.clone(),
            mode,
            out_format: PcmFormat::F32,
            channels: 2,
            plan,
            seek_ms,
            discarder: Discarder::default(),
            start_frames: 0,
        };
        let result = crate::decode::run(source, "mp4", &mut sink);
        let _ = downloader.join();
        result
    })();
    if let Err(e) = outcome {
        let _ = tx.send(Err(e));
    }
}
```

5. Update `wait_for_info` to return `StreamStart` (keeping the generation check added in Task 6):

```rust
fn wait_for_info(inner: &Arc<Inner>, stream: &mut TrackStream, gen: u64) -> Result<StreamStart> {
    loop {
        // A supersede while a track is still resolving/fetching (e.g. Next
        // pressed during a slow network resolve) must exit promptly.
        if inner.generation.load(Ordering::SeqCst) != gen {
            return Err(Error::Output("playback superseded".into()));
        }
        match stream.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(StreamEvent::Info(start))) => return Ok(start),
            Ok(Ok(StreamEvent::Samples(_))) => continue,
            Ok(Err(e)) => return Err(e),
            Err(RecvTimeoutError::Timeout) => {
                if inner.shutdown.load(Ordering::SeqCst) {
                    return Err(Error::Output("engine shut down".into()));
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Decode("stream ended before any audio".into()));
            }
        }
    }
}
```

6. Update `play_track_into` to the new `start_track` signature and `StreamStart`:

```rust
        let mut stream = start_track(&self.inner, item, mode, generation, None);
        let start = wait_for_info(&self.inner, &mut stream, generation)?;
        let spec = AudioOutputSpec {
            device_id: self.output_device(),
            info: start.info,
            mode,
            exclusive_fallback: fallback,
        };
```

7. Give `run_session` a `seek_ms` parameter and apply the start frames (the `#[allow]` keeps `clippy -D warnings` happy at 8 parameters):

```rust
#[allow(clippy::too_many_arguments)]
fn run_session(
    inner: &Arc<Inner>,
    output: &mut dyn AudioOutput,
    open_spec: &mut Option<AudioOutputSpec>,
    first_item: &QueueItem,
    mode: OutputMode,
    fallback: ExclusiveFallback,
    gen: u64,
    seek_ms: Option<u64>,
) -> SessionOutcome {
    // A seek must not play the audio still buffered in the device ring, so
    // force a reopen on the first iteration.
    if seek_ms.is_some() {
        let _ = output.stop();
        *open_spec = None;
    }
    let mut stream = start_track(inner, first_item, mode, gen, seek_ms);
    let mut prefetch = start_prefetch(inner, mode, gen);
    let mut frames: u64 = 0;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    loop {
        if inner.generation.load(Ordering::SeqCst) != gen {
            return SessionOutcome::Superseded;
        }
        let start = match wait_for_info(inner, &mut stream, gen) {
            Ok(start) => start,
            Err(_) if inner.generation.load(Ordering::SeqCst) != gen => {
                return SessionOutcome::Superseded;
            }
            Err(e) => return SessionOutcome::StreamError(e),
        };
        let info = start.info;
        // The stream's absolute starting frame (non-zero only for a seek).
        frames = start.start_frames;
        if info.sample_rate > 0 {
            inner
                .position_ms
                .store(frames.saturating_mul(1000) / info.sample_rate as u64, Ordering::SeqCst);
        }
        ...
```

and at the boundary replace `start_track(inner, &item, mode, gen)` with `start_track(inner, &item, mode, gen, None)`, and remove the `inner.position_ms.store(0, ...)`/`frames = 0` lines (the top of the loop now sets them from `StreamStart`). Also update `start_prefetch` to pass `None`:

```rust
    next.map(|item| start_track(inner, &item, mode, gen, None))
```

In `worker_main`, pass the seek target:

```rust
        let (item, seek_ms) = {
            let _guard = inner.playback_lock.lock().unwrap();
            let q = inner.queue.lock().unwrap();
            (q.current().cloned(), inner.seek_target.lock().unwrap().take())
        };
        let Some(item) = item else {
            continue;
        };
        if seek_ms.is_none() {
            inner.paused.store(false, Ordering::SeqCst);
        }
        inner.position_ms.store(seek_ms.unwrap_or(0), Ordering::SeqCst);
        ...
        match run_session(
            &inner,
            output.as_mut(),
            &mut open_spec,
            &item,
            mode,
            fallback,
            handled_gen,
            seek_ms,
        ) {
```

8. Implement `EngineCmd::Seek`:

```rust
            EngineCmd::Seek(ms) => {
                let playing = {
                    let q = self.inner.queue.lock().unwrap();
                    q.current().is_some() && q.is_playing()
                };
                if playing {
                    *self.inner.seek_target.lock().unwrap() = Some(ms);
                    self.inner.generation.fetch_add(1, Ordering::SeqCst);
                    self.inner.notify_worker();
                }
                self.emit(EngineEvent::State(self.state()));
            }
```

9. Add `restart_current_at_position` and call it from the settings setters:

```rust
    /// Restart the current track at its position so an output setting change
    /// applies immediately without losing the listener's place.
    fn restart_current_at_position(&self) {
        let playing = {
            let q = self.inner.queue.lock().unwrap();
            q.current().is_some() && q.is_playing()
        };
        if playing {
            let position = self.inner.position_ms.load(Ordering::SeqCst);
            *self.inner.seek_target.lock().unwrap() = Some(position);
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
            self.inner.notify_worker();
        }
    }
```

Add `self.restart_current_at_position();` to the end of `set_quality`, `set_output_device`, `set_output_mode`, and `set_exclusive_fallback`, and update the "applies to the next track" comments to "applies immediately (restarts the current track at its position)".

10. Update the engine test helpers for the new `StreamPlan`:

```rust
    fn fixture_plan() -> (StreamPlan, HashMap<String, Vec<u8>>) {
        let bytes = std::fs::read("tests/fixtures/tone.mp4").unwrap();
        let plan = StreamPlan {
            init_url: None,
            segments: vec![Segment { url: "mem://tone".into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
        };
        let mut files = HashMap::new();
        files.insert("mem://tone".to_string(), bytes);
        (plan, files)
    }

    fn plan_for(url: &str, sample_rate: u32, bit_depth: u32) -> StreamPlan {
        StreamPlan {
            init_url: None,
            segments: vec![Segment { url: url.into(), start_ticks: 0, duration_ticks: 0 }],
            timescale: 0,
            codec: Some("flac".into()),
            sample_rate: Some(sample_rate),
            bit_depth: Some(bit_depth),
        }
    }
```

with `use crate::source::Segment;` added to the test module imports.

11. Add the engine seek test:

```rust
    #[test]
    fn seek_restarts_at_the_requested_offset() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        wait_for(|| engine.state().position_ms > 100);

        engine.handle(EngineCmd::Seek(1000)).unwrap();
        wait_for(|| engine.state().position_ms >= 1000);
        let state = engine.state();
        assert!(state.position_ms >= 1000, "seek did not land at 1000ms: {}", state.position_ms);
        assert!(state.playing);
        assert!(output.opens() >= 2, "seek must reopen the output to drop buffered audio");
    }
```

- [ ] **Step 8: Run the Rust tests and clippy**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
Expected: all green. Every `StreamPlan` construction site must now use `segments` (`grep -rn "media_urls:" crates/` should return nothing).

- [ ] **Step 9: Add the store seek reconciliation**

In `ui/src/lib/player.svelte.ts`, add the pure helper above `createPlayer`:

```ts
/** Keeps the slider pinned to the requested position until the engine catches up. */
export function reconcileSeek(pending: number | null, event: EngineEvent): number | null {
  if (pending === null) return null;
  if (event.type === "position" || event.type === "state") {
    return Math.abs(event.position_ms - pending) <= 1500 ? null : pending;
  }
  if (event.type === "error" || event.type === "now_playing") return null;
  return pending;
}
```

In `createPlayer`, add `let pendingSeek = $state<number | null>(null);`, update the listener:

```ts
    await listen<EngineEvent>("playback_event", (event) => {
      state = reduce(state, event.payload);
      pendingSeek = reconcileSeek(pendingSeek, event.payload);
    });
```

replace the `seek` pass-through with:

```ts
    function seek(ms: number) {
      pendingSeek = ms;
      return api.seek(ms);
    }
```

and expose it:

```ts
    get state() { return state; },
    get pendingSeek() { return pendingSeek; },
    init,
    ...
    seek,
```

Add to `ui/src/lib/player.test.ts`:

```ts
import { reduce, emptyState, reconcileSeek, type PlayerState } from "./player.svelte";
```

```ts
describe("seek reconciliation", () => {
  it("releases the pending position when the engine catches up", () => {
    expect(
      reconcileSeek(1000, { type: "position", position_ms: 100, duration_ms: 2000 }),
    ).toBe(1000);
    expect(
      reconcileSeek(1000, { type: "position", position_ms: 1100, duration_ms: 2000 }),
    ).toBeNull();
  });

  it("clears the pending position on errors and track changes", () => {
    expect(reconcileSeek(1000, { type: "error", code: "x", message: "y" })).toBeNull();
    expect(reconcileSeek(1000, { type: "now_playing", item: null })).toBeNull();
  });
});
```

- [ ] **Step 10: Make the PlayerBar slider interactive**

In `ui/src/lib/components/PlayerBar.svelte`, replace the derived values and the slider:

```svelte
  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const quality = $derived(player.state.preferred_quality);
```

```svelte
    <input
      type="range" min="0" max={Math.max(duration, 1)} value={position}
      oninput={(e) => {
        dragging = true;
        dragValue = Number((e.target as HTMLInputElement).value);
      }}
      onchange={(e) => {
        dragging = false;
        player.seek(Number((e.target as HTMLInputElement).value));
      }}
      aria-label="Playback position"
    />
```

- [ ] **Step 11: Run all gates**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```
Expected: all green.

- [ ] **Step 12: Manual check**

With a real account: drag the seek bar to the middle of a track and release — audio restarts at that point, the time label does not jump back, and seeking near the end clamps without an error. Record in `docs/superpowers/plans/notes/plan2-smoke.md` under `## Plan 3 seek check`.

- [ ] **Step 13: Commit**

```powershell
git add crates/oxitide-audio ui/src docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "feat(audio): implement segment-accurate seek with an interactive seek bar"
```

---

### Task 9: Device hot-plug recovery

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs`
- Modify: `docs/superpowers/plans/notes/plan2-smoke.md`

**Interfaces:**
- Consumes: `SessionOutcome::OutputError` handling and the worker from Task 6; seek restart from Task 8; `WasapiOutput`'s `device_lost` guard and notification registration from Task 4.
- Produces: device-loss recovery at the last known position on the next `PlayPause`; engine-level failure-path test.

Task 4 already detects removal (`DeviceEventCallbacks` → `device_lost`), returns a prompt `Error::Output("output device unavailable")` from `write`, reports a notice through `status()`, and falls back to the system default when the stored device id is gone. Task 6's worker already pauses (`q.stop()`), keeps the queue, stores the notice, and `PlayPause` already restarts when the worker is idle. This task makes recovery resume **at the position where the device failed** and pins the whole path with a test.

- [ ] **Step 1: Write the failing recovery test**

Add to the engine test module:

```rust
    /// An output that fails every write until `failing` is cleared, then
    /// records samples. Simulates an unplugged device.
    struct DeviceLostOutput {
        failing: Arc<AtomicBool>,
        samples: Arc<Mutex<Vec<f32>>>,
        opens: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl AudioOutput for DeviceLostOutput {
        fn open(&mut self, _spec: &AudioOutputSpec) -> Result<()> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            Ok(())
        }
        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            if self.failing.load(Ordering::SeqCst) {
                return Err(Error::Output("output device unavailable".into()));
            }
            if let PcmBuffer::F32(v) = buf {
                self.samples.lock().unwrap().extend_from_slice(v);
            }
            Ok(())
        }
        fn set_volume(&mut self, _volume: f32) -> Result<()> {
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn status(&self) -> OutputStatus {
            let mut status = OutputStatus::default();
            if self.failing.load(Ordering::SeqCst) {
                status.notice = Some("output device disconnected".into());
            }
            status
        }
    }

    /// Review Focus: losing the device mid-track pauses with a notice, keeps
    /// the queue, and recovers on the next play press.
    #[test]
    fn output_failure_pauses_keeps_queue_and_recovers() {
        let (plan, files) = fixture_plan();
        let failing = Arc::new(AtomicBool::new(true));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let engine = Engine::new(
            Arc::new(FakeResolver { plan }),
            Arc::new(crate::fetch::MapFetcher { files }),
            Box::new(DeviceLostOutput {
                failing: failing.clone(),
                samples: samples.clone(),
                opens: opens.clone(),
            }),
            Quality::Lossless,
        );
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();

        wait_for(|| engine.state().bit_perfect_notice.is_some());
        let state = engine.state();
        assert!(!state.playing, "device loss must pause playback");
        assert_eq!(state.current.map(|i| i.track_id), Some(1));
        assert_eq!(state.queue.len(), 1, "device loss must keep the queue");
        assert!(state.bit_perfect_notice.is_some());

        // The device comes back; the next user action recovers the track.
        failing.store(false, Ordering::SeqCst);
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| samples.lock().unwrap().len() > 0);
        assert!(engine.state().playing, "recovery did not resume playback");
        wait_for(|| !engine.state().playing);
        assert_eq!(samples.lock().unwrap().len(), 176_400);
        assert!(opens.load(Ordering::SeqCst) >= 2, "recovery must reopen the output");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-audio engine::output_failure`
Expected: FAIL at `wait_for(|| !engine.state().playing)`/sample count if recovery restarts at 0 is fine — actually the test should pass already except the position-resume assertion is absent. To make the resume-at-position behavior testable, the next step changes the error path; the current test may pass. Run it and note: if it passes, keep it as the regression test; Step 3 still adds position preservation, verified by the manual check.

- [ ] **Step 3: Preserve the position on output failure**

In `crates/oxitide-audio/src/engine.rs`, in `worker_main`'s `SessionOutcome::OutputError(message)` arm, add the seek target after the `q.stop()` block and before the generation bump:

```rust
            SessionOutcome::OutputError(message) => {
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                }
                // Remember where the device failed so the next play press
                // resumes there.
                *inner.seek_target.lock().unwrap() =
                    Some(inner.position_ms.load(Ordering::SeqCst));
                inner.generation.fetch_add(1, Ordering::SeqCst);
                let _ = output.stop();
                open_spec = None;
                {
                    let mut status = inner.output_status.lock().unwrap();
                    status.bit_perfect = false;
                    status.notice = Some(message.clone());
                }
                emit(&inner, EngineEvent::Error { code: "output".into(), message });
                emit(&inner, EngineEvent::State(build_state(&inner)));
            }
```

- [ ] **Step 4: Run the tests and clippy**

Run:
```powershell
cargo test -p oxitide-audio
cargo clippy --workspace -- -D warnings
```
Expected: all green.

- [ ] **Step 5: Manual hot-plug check**

Run `npm --prefix ui run app:dev` with a USB DAC and a real account:
1. Start an album in bit-perfect mode.
2. Unplug the DAC mid-track: playback must pause, the PlayerBar must show the amber "output device disconnected" notice, and the queue must remain.
3. Replug the DAC and press play: playback must resume (at approximately the failure point) and the notice must clear on the next successful open.
4. With a *different* stored device that no longer exists (rename/disable it in Windows sound settings before starting), playback must fall back to the system default with the "selected output device unavailable; using system default" notice instead of failing.

Record the results in `docs/superpowers/plans/notes/plan2-smoke.md` under `## Plan 3 hot-plug check`.

- [ ] **Step 6: Commit**

```powershell
git add crates/oxitide-audio/src/engine.rs docs/superpowers/plans/notes/plan2-smoke.md
git commit -m "feat(audio): recover from device loss at the failure position"
```

---

### Task 10: Config file and Settings persistence

**Files:**
- Create: `crates/oxitide-app/src/config.rs`
- Modify: `crates/oxitide-app/Cargo.toml`
- Modify: `Cargo.toml` (workspace dependencies)
- Modify: `crates/oxitide-app/src/main.rs`
- Modify: `crates/oxitide-app/src/state.rs`
- Modify: `crates/oxitide-app/src/commands.rs`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: `engine::{set_output_mode, set_exclusive_fallback, set_output_device, set_quality}` from Tasks 3–8; `player::QualityDto`; `output::ExclusiveFallback`.
- Produces:
  - `app::config::{Config, AudioConfig, StreamingConfig, config_path}` with `load()`, `load_from(path)`, `save()`, `save_to(path)`
  - `AppState::config() -> Config`, `AppState::set_config(Config)`
  - `get_config` command; `set_exclusive`, `set_exclusive_fallback`, `set_output_device`, `set_preferred_quality` now persist
  - TS `Config` interface + `api.getConfig()`

- [ ] **Step 1: Add the dependencies**

Root `Cargo.toml` `[workspace.dependencies]`, add:

```toml
toml = "1"
directories = "6"
```

`crates/oxitide-app/Cargo.toml` `[dependencies]`, add:

```toml
toml.workspace = true
directories.workspace = true
```

- [ ] **Step 2: Write the failing config tests**

Create `crates/oxitide-app/src/config.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use oxitide_audio::output::ExclusiveFallback;
use serde::{Deserialize, Serialize};

use crate::player::QualityDto;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub audio: AudioConfig,
    pub streaming: StreamingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self { audio: AudioConfig::default(), streaming: StreamingConfig::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Empty string means the system default device.
    pub output_device: String,
    pub exclusive: bool,
    pub exclusive_fallback: ExclusiveFallback,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            exclusive: false,
            exclusive_fallback: ExclusiveFallback::Shared,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingConfig {
    pub quality: QualityDto,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self { quality: QualityDto::Lossless }
    }
}

impl Config {
    /// Load `%APPDATA%\OxiTide\config.toml`; a missing file yields defaults and
    /// a malformed file is backed up to `config.toml.bak` and replaced with
    /// defaults. Never fails.
    pub fn load() -> Self {
        match config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        match toml::from_str::<Config>(&text) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!("malformed config at {}: {e}", path.display());
                let backup = path.with_extension("toml.bak");
                let _ = fs::rename(path, &backup);
                Self::default()
            }
        }
    }

    /// Atomic write (temp file + rename).
    pub fn save(&self) -> Result<(), String> {
        match config_path() {
            Some(path) => self.save_to(&path),
            None => Err("no config directory available".to_string()),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// `%APPDATA%\OxiTide\config.toml`.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "OxiTide")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxitide-config-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = temp_dir();
        let config = Config::load_from(&dir.join("config.toml"));
        assert_eq!(config, Config::default());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let config = Config {
            audio: AudioConfig {
                output_device: "dev-1".into(),
                exclusive: true,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
            },
            streaming: StreamingConfig { quality: QualityDto::Max },
        };
        config.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), config);
        assert!(!dir.join("config.toml.tmp").exists(), "temp file left behind");
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a malformed config must never crash startup.
    #[test]
    fn malformed_file_is_backed_up_and_defaults_used() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "not = [valid").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config, Config::default());
        assert!(!path.exists());
        assert!(dir.join("config.toml.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[audio]\nexclusive = true\n").unwrap();
        let config = Config::load_from(&path);
        assert!(config.audio.exclusive);
        assert_eq!(config.audio.output_device, "");
        assert_eq!(config.audio.exclusive_fallback, ExclusiveFallback::Shared);
        assert_eq!(config.streaming.quality, QualityDto::Lossless);
        let _ = fs::remove_dir_all(dir);
    }
}
```

Add `mod config;` to `crates/oxitide-app/src/main.rs` (next to `mod commands;`).

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app config::`
Expected: compile errors (`toml`, `directories`, `mod config` missing).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p oxitide-app config::`
Expected: PASS (needs network the first time to fetch `toml`/`directories`).

- [ ] **Step 5: Store the config in `AppState`**

In `crates/oxitide-app/src/state.rs`:

1. Add `use crate::config::Config;`.
2. Remove the dead `output_device` field from `Player`:

```rust
pub struct Player {
    pub engine: Engine,
}
```

3. Add to `AppState`:

```rust
    pub config: Mutex<Config>,
```

4. Add methods:

```rust
    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn set_config(&self, config: Config) {
        *self.config.lock().unwrap() = config;
    }
```

- [ ] **Step 6: Load, apply, and persist config in the app commands**

In `crates/oxitide-app/src/commands.rs`:

1. Add imports:

```rust
use crate::config::Config;
use crate::player::QualityDto; // already imported via the player re-export list
```

2. In `auth_complete`, before `state.set_player(...)`:

```rust
    let config = Config::load();
    state.set_config(config.clone());
    state.set_player(build_player(&app, client, &config)?);
```

3. In `auth_restore`, inside the `Some((session, tokens))` arm, do the same.

4. Change `build_player` to accept and apply the config, and return the slimmed `Player`:

```rust
fn build_player(
    app: &tauri::AppHandle,
    client: TidalClient,
    config: &Config,
) -> Result<Player, String> {
    let resolver = crate::player::TidalResolver::new(client).map_err(|e| e.to_string())?;
    let engine = Engine::new(
        Arc::new(resolver),
        Arc::new(HttpFetcher::default()),
        crate::player::real_output(),
        config.streaming.quality.to_quality(),
    );
    engine.set_output_device(if config.audio.output_device.is_empty() {
        None
    } else {
        Some(config.audio.output_device.clone())
    });
    engine.set_output_mode(if config.audio.exclusive {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    });
    engine.set_exclusive_fallback(config.audio.exclusive_fallback);
    let handle = app.clone();
    engine.subscribe(Arc::new(move |event| {
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
    Ok(Player { engine })
}
```

5. Replace `set_exclusive` and `set_exclusive_fallback` with persisting versions:

```rust
#[tauri::command]
pub fn set_exclusive(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_output_mode(if enabled {
        OutputMode::Exclusive
    } else {
        OutputMode::Shared
    });
    let mut config = state.config();
    config.audio.exclusive = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_exclusive_fallback(
    state: State<'_, AppState>,
    mode: ExclusiveFallback,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_exclusive_fallback(mode);
    let mut config = state.config();
    config.audio.exclusive_fallback = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

6. Replace `set_output_device` and `set_preferred_quality` with persisting versions:

```rust
#[tauri::command]
pub fn set_output_device(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    let device = if device_id.is_empty() { None } else { Some(device_id.clone()) };
    player.engine.set_output_device(device);
    let mut config = state.config();
    config.audio.output_device = device_id;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_preferred_quality(
    state: State<'_, AppState>,
    quality: QualityDto,
) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player.engine.set_quality(quality.to_quality());
    let mut config = state.config();
    config.streaming.quality = quality;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

7. Add the config getter:

```rust
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config())
}
```

8. Add `commands::get_config` to `generate_handler![...]` in `crates/oxitide-app/src/main.rs` (next to `commands::get_playback_state`).

- [ ] **Step 7: Update the UI types**

In `ui/src/lib/api.ts`, add:

```ts
export interface Config {
  audio: {
    output_device: string;
    exclusive: boolean;
    exclusive_fallback: "shared" | "warn_only";
  };
  streaming: { quality: "low" | "high" | "lossless" | "max" };
}
```

and to the `api` object:

```ts
  getConfig: () => invoke<Config>("get_config"),
```

- [ ] **Step 8: Hydrate Settings from the config**

Replace the whole `<script>` block and body of `ui/src/views/Settings.svelte` with:

```svelte
<script lang="ts">
  import { api, type Config, type OutputDevice } from "../lib/api";
  import type { Player } from "../lib/player.svelte";
  let { player }: { player: Player } = $props();
  let devices = $state<OutputDevice[]>([]);
  let config = $state<Config | null>(null);
  let error = $state("");

  function apply(failure: unknown) {
    error = String(failure);
  }

  function saveAudio(patch: Partial<Config["audio"]>) {
    if (!config) return;
    config = { ...config, audio: { ...config.audio, ...patch } };
  }

  $effect(() => {
    Promise.all([api.getConfig(), api.getOutputDevices()])
      .then(([loaded, list]) => {
        devices = list;
        // The stored device may be gone; fall back to the system default and
        // persist that so the next start does not retry it.
        let next = loaded;
        if (next.audio.output_device && !list.some((d) => d.id === next.audio.output_device)) {
          next = { ...next, audio: { ...next.audio, output_device: "" } };
          api.setOutputDevice("").catch(apply);
        }
        config = next;
      })
      .catch(apply);
  });

  function setDevice(deviceId: string) {
    saveAudio({ output_device: deviceId });
    api.setOutputDevice(deviceId).catch(apply);
  }

  function setExclusive(enabled: boolean) {
    saveAudio({ exclusive: enabled });
    api.setExclusive(enabled).catch(apply);
  }

  function setFallback(mode: "shared" | "warn_only") {
    saveAudio({ exclusive_fallback: mode });
    api.setExclusiveFallback(mode).catch(apply);
  }

  function setQuality(quality: Config["streaming"]["quality"]) {
    if (!config) return;
    config = { ...config, streaming: { ...config.streaming, quality } };
    api.setPreferredQuality(quality).catch(apply);
  }
</script>

<h2>Settings</h2>
{#if error}<p style="color:#ff6b6b">{error}</p>{/if}
{#if config}
  <h3>Audio output</h3>
  <select
    value={config.audio.output_device}
    onchange={(e) => setDevice((e.target as HTMLSelectElement).value)}
  >
    <option value="">System default</option>
    {#each devices as d (d.id)}
      <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
    {/each}
  </select>

  <h3>Bit-perfect output</h3>
  <label>
    <input
      type="checkbox"
      checked={config.audio.exclusive}
      onchange={(e) => setExclusive((e.target as HTMLInputElement).checked)}
    />
    Bit-perfect (exclusive mode)
  </label>
  <p>
    <label>
      If unsupported:
      <select
        value={config.audio.exclusive_fallback}
        onchange={(e) =>
          setFallback((e.target as HTMLSelectElement).value as "shared" | "warn_only")}
      >
        <option value="shared">Fall back to shared output</option>
        <option value="warn_only">Stop with a message</option>
      </select>
    </label>
  </p>

  <h3>Streaming quality</h3>
  <select
    value={config.streaming.quality}
    onchange={(e) =>
      setQuality((e.target as HTMLSelectElement).value as Config["streaming"]["quality"])}
  >
    <option value="low">Low</option>
    <option value="high">High</option>
    <option value="lossless">Lossless</option>
    <option value="max">Max (Hi-Res)</option>
  </select>
{/if}
{#if player.state.bit_perfect_notice}
  <p style="color:#e0a030">{player.state.bit_perfect_notice}</p>
{/if}
```

This removes the earlier `$effect`s that mirrored `player.state` into the selects; the config file is now the single source of truth for the *requested* settings, while `PlaybackState` remains the source of *actual* output mode and notices.

- [ ] **Step 9: Run all gates**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```
Expected: all green.

- [ ] **Step 10: Manual check**

Run `npm --prefix ui run app:dev`:
1. Toggle bit-perfect on, pick a non-default output device and Max quality, then restart the app. All three must come back selected.
2. Quit, corrupt `%APPDATA%\OxiTide\config.toml` (write `not = [valid`), start again: the app must start with defaults and a `config.toml.bak` must exist.
3. Disable/rename the saved device, restart: the Settings select must show System default and the config file must have been updated.

Record in `docs/superpowers/plans/notes/plan2-smoke.md` under `## Plan 3 config check`.

- [ ] **Step 11: Commit**

```powershell
git add Cargo.toml crates/oxitide-app ui/src docs/superpowers/plans/notes/plan2-smoke.md Cargo.lock
git commit -m "feat(app): persist audio settings in %APPDATA% config.toml"
```

---

### Task 11: Verification

**Files:**
- Create: `docs/superpowers/plans/notes/plan3-smoke.md`
- Modify (only if a gate or the smoke run surfaces an issue): whatever file the issue lives in

**Interfaces:**
- Consumes: everything shipped in Tasks 1–10.
- Produces: a recorded Plan 3 verification (automated counts + human checklist) and any fixes.

- [ ] **Step 1: Run every CI gate and record exact counts**

Run:
```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui test
npm --prefix ui run check
```
Expected: all green. Record the exact test counts per crate/UI file, e.g. `oxitide-audio: N tests`, `oxitide-app: N tests`, `oxitide-tidal: N tests`, `ui: N tests`.

- [ ] **Step 2: Create the Plan 3 smoke checklist**

Create `docs/superpowers/plans/notes/plan3-smoke.md`:

```markdown
# Plan 3 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and an audio
> device (ideally a USB DAC). Run with `npm --prefix ui run app:dev`.

## Automated checks
- cargo test --workspace: <fill in>
- cargo clippy --workspace -- -D warnings: <fill in>
- npm --prefix ui test: <fill in>
- npm --prefix ui run check: <fill in>

## Manual checks
- [ ] Bit-perfect toggle is on; the PlayerBar shows the "Bit-perfect" badge.
- [ ] A 44.1 kHz track opens the DAC at 44.1 kHz (DAC display or Windows device properties → Advanced).
- [ ] A 96 kHz or 192 kHz track opens the DAC at that rate — no resampling.
- [ ] Shared mode still plays when the bit-perfect toggle is off.
- [ ] The volume slider changes loudness in exclusive mode (endpoint volume); if it shows the amber
      software-volume notice, loudness still changes.
- [ ] Setting the fallback to "Stop with a message" and disabling the DAC in Windows sound settings
      produces an error/paused state with a notice, not a crash or silence that pretends to play.
- [ ] Two tracks from the same album run into each other with no gap and a single device open.
- [ ] A track at a different sample rate correctly reopens the device at the boundary.
- [ ] The seek bar drags, restarts audio at the requested position, and the time label does not snap back.
- [ ] Seeking near the end of a track clamps without an error and the queue advances.
- [ ] Unplugging the DAC mid-track pauses playback with an amber notice and keeps the queue.
- [ ] Replugging and pressing play resumes at approximately the failure point.
- [ ] Choosing a device id that no longer exists falls back to the system default with a notice.
- [ ] Settings: bit-perfect, fallback, device, and quality survive an app restart.
- [ ] Corrupting `%APPDATA%\OxiTide\config.toml` starts with defaults and leaves `config.toml.bak`.
- [ ] Logging out stops playback and releases the exclusive device.
```

- [ ] **Step 3: Run the checklist with the human**

Start the app, run every item, and mark the checklist: `- [x]` for pass, leave `- [ ]` and append a note for any failure.

- [ ] **Step 4: Fix anything the gates or the checklist surfaces**

Use `superpowers:systematic-debugging` for each failure: reproduce, write a failing test where possible, fix, re-run the gate(s), and commit each fix as its own `fix:` commit before proceeding. Do not mark a failing item as passing.

- [ ] **Step 5: Commit**

```powershell
git add docs/superpowers/plans/notes/plan3-smoke.md
git commit -m "test: record plan 3 verification results"
```

---

## Self-Review (completed by author)

**1. Spec coverage** (spec sections → tasks):

| Spec | Task |
|---|---|
| P3-1 Plan 2 smoke | 1 |
| §5.1 typed PCM | 2 |
| §5.2 output trait + volume + NullOutput recording | 3 |
| §5.4 exclusive polling + negotiation + endpoint volume | 4 |
| §2 bounded lookahead (fetch side) | 5 |
| §5.3 long-lived worker | 6 |
| §2/§5.3 gapless + next-track PCM lookahead | 7 |
| §5.5 seek + §8 seek bar | 8 |
| §9/§2 device hot-plug | 9 |
| §6 config + §7 `get_config` refinement + §8 Settings | 10 |
| §10/§11 manual checklist + CI gates | 11 |
| §7 `seek`, `set_exclusive`, `set_exclusive_fallback`, `get_audio_status`, `PlaybackState` fields | 4, 8 |
| §13 P3-D1..D8 | 1–10 (D4: PollingExclusive in Task 4; D6: no HTTP Range in Task 8; D7: bounded in 5+7) |

All §9 error-handling rows are implemented: exclusive unsupported (Tasks 4/6), endpoint volume unavailable (Task 4), device removed (Tasks 4/9), stored device gone (Tasks 4/10), seek clamp (Task 8), malformed config (Task 10), open failure on format change (Tasks 4/7).

**2. Placeholder scan:** no `TODO`/`TBD`/"implement later"/"similar to Task N" remain; every code step carries the actual code or an exact command. The `unimplemented!()` bodies in Task 8 Step 1 are deliberate TDD scaffolding, replaced in Step 3.

**3. Type consistency:** `PcmFormat`/`PcmBuffer`/`StreamInfo`/`PcmSink` (Tasks 2→8), `AudioOutputSpec`/`OutputMode`/`ExclusiveFallback`/`OutputStatus`/`AudioOutput` (Tasks 3→9), `NullOutput` accessors (Tasks 3→9), `StreamPlan`/`Segment` (Tasks 8; helper shapes updated in fetch/engine tests in the same task), `seek::{target_frames, target_segment, Discarder}` (Tasks 8), `StreamStart`/`StreamEvent` (Tasks 6, 8), `start_track(inner, item, mode, gen, seek_ms)` / `start_prefetch(inner, mode, gen)` / `wait_for_info(inner, stream, gen)` / `run_session(..., gen, seek_ms)` (Tasks 6→9), `Config`/`AudioConfig`/`StreamingConfig` (Task 10), `reconcileSeek`/`pendingSeek` (Task 8, used by PlayerBar). Every call site of a signature that changes is updated in the same task that changes it.

**4. Review Focus tests:** (1) exclusive-unsupported → `exclusive_formats` mapping (Task 3) + `output_open_failure_pauses_with_notice` (Task 6); (2) endpoint-volume fallback → NullOutput software scaling (Task 3) + `output_status_notice_flows_to_playback_state` (Task 6); (3) device unplug → `output_failure_pauses_keeps_queue_and_recovers` (Task 9); (4) seek past end → `target_segment_clamps_beyond_the_end` (Task 8); (5) malformed config → `malformed_file_is_backed_up_and_defaults_used` (Task 10).
