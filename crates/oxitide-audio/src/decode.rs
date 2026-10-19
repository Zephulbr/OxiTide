use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{well_known::CODEC_ID_AAC, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
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
    I32 {
        bits: u16,
    },
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
    /// Takes the decoded buffer by value, so a sink that forwards it (the
    /// playback channel) does not copy every packet (Track 13, EF-D5).
    fn on_samples(&mut self, buf: PcmBuffer) -> Result<()>;
}

/// Collects the source info and every sample as `f32`.
pub struct CollectSink {
    pub info: Option<StreamInfo>,
    pub samples: Vec<f32>,
}

impl CollectSink {
    pub fn new() -> Self {
        Self {
            info: None,
            samples: Vec::new(),
        }
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
    fn on_samples(&mut self, buf: PcmBuffer) -> Result<()> {
        match &buf {
            PcmBuffer::F32(v) => {
                self.samples.extend_from_slice(v);
                Ok(())
            }
            PcmBuffer::I32(v) => {
                self.samples
                    .extend(v.iter().map(|&s| s as f32 / 2_147_483_648.0));
                Ok(())
            }
        }
    }
}

/// Format details read from a media file's headers (no decoding).
#[derive(Debug, Clone, PartialEq)]
pub struct ProbedFormat {
    /// The manifest-style codec name, when known. Bare `mp4a` does not
    /// claim an AAC profile that may only be discoverable in-band.
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
}

/// Read the default audio track's format from an MP4 file's headers.
pub fn probe(source: impl MediaSource + 'static) -> Result<ProbedFormat> {
    use symphonia::core::codecs::audio::well_known::{CODEC_ID_AAC, CODEC_ID_ALAC, CODEC_ID_FLAC};
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let reader = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| Error::Decode(e.to_string()))?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::Decode("no audio track".into()))?;
    let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
        return Err(Error::Decode("unsupported codec parameters".into()));
    };
    let codec = match params.codec {
        id if id == CODEC_ID_FLAC => Some("flac"),
        id if id == CODEC_ID_ALAC => Some("alac"),
        id if id == CODEC_ID_AAC => Some(
            match params
                .extra_data
                .as_deref()
                .and_then(|asc| rusty_aac::sbr::parse_sbr_config(asc).ok())
            {
                Some(config) if config.ps_present => "mp4a.40.29",
                Some(config) if config.sbr_present => "mp4a.40.5",
                _ => "mp4a",
            },
        ),
        _ => None,
    };
    Ok(ProbedFormat {
        codec: codec.map(str::to_string),
        sample_rate: params.sample_rate,
        bit_depth: params.bits_per_sample,
    })
}

pub fn run(source: impl MediaSource + 'static, ext: &str, sink: &mut dyn PcmSink) -> Result<()> {
    run_with_aac_hint(source, ext, false, sink)
}

/// Keep AAC-LC on Symphonia unless the plan indicates HE-AAC. The raw ASC
/// can also select the HE decoder; implicit SBR/PS needs the plan's hint.
pub fn run_with_aac_hint(
    source: impl MediaSource + 'static,
    ext: &str,
    he_aac: bool,
    sink: &mut dyn PcmSink,
) -> Result<()> {
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);

    let mut reader = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| Error::Decode(e.to_string()))?;

    let (track_id, mut decoder, bits_hint) = {
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Decode("no audio track".into()))?;
        let params = match track.codec_params.as_ref() {
            Some(CodecParameters::Audio(audio)) => audio,
            _ => return Err(Error::Decode("unsupported codec parameters".into())),
        };
        if params.codec == CODEC_ID_AAC && use_he_aac(he_aac, params.extra_data.as_deref()) {
            let asc = params
                .extra_data
                .as_deref()
                .ok_or_else(|| Error::Decode("AAC stream has no AudioSpecificConfig".into()))?;
            let decoder = rusty_aac::AacDecoder::with_config_bytes(asc)
                .map_err(|e| Error::Decode(e.to_string()))?;
            let track_id = track.id;
            return run_aac(reader.as_mut(), track_id, decoder, sink);
        }
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| Error::Decode(e.to_string()))?;
        (track.id, decoder, params.bits_per_sample)
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
                        format: source_format(&decoded, bits_hint),
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
                        sink.on_samples(PcmBuffer::F32(buf))?;
                    }
                    PcmFormat::I32 { .. } => {
                        let mut buf = vec![0i32; n];
                        decoded.copy_to_slice_interleaved(&mut buf);
                        sink.on_samples(PcmBuffer::I32(buf))?;
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

fn use_he_aac(hint: bool, asc: Option<&[u8]>) -> bool {
    hint || asc
        .and_then(|asc| rusty_aac::sbr::parse_sbr_config(asc).ok())
        .is_some_and(|config| config.sbr_present || config.ps_present)
}

fn run_aac(
    reader: &mut dyn FormatReader,
    track_id: u32,
    mut decoder: rusty_aac::AacDecoder,
    sink: &mut dyn PcmSink,
) -> Result<()> {
    let mut stream_info = None;
    let mut out_format = PcmFormat::F32;
    while let Some(packet) = reader
        .next_packet()
        .map_err(|e| Error::Decode(e.to_string()))?
    {
        if packet.track_id != track_id {
            continue;
        }
        // A rejected packet can leave this decoder's channel layout poisoned.
        // Terminate the track; a new run always constructs a fresh decoder.
        let decoded = decoder
            .decode(&packet.data, Some(packet.pts.get()))
            .map_err(|e| Error::Decode(e.to_string()))?;
        if decoded.sample_rate == 0
            || decoded.channels == 0
            || decoded.samples.len() % decoded.channels as usize != 0
            || decoded.samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(Error::Decode("AAC decoder returned invalid PCM".into()));
        }
        if decoded.samples.is_empty() {
            continue;
        }
        let info = StreamInfo {
            sample_rate: decoded.sample_rate,
            channels: decoded.channels,
            bits_per_sample: 16,
            format: PcmFormat::F32,
        };
        match stream_info {
            None => {
                sink.on_stream_info(&info)?;
                out_format = sink.preferred_format();
                stream_info = Some(info);
            }
            Some(previous) if previous != info => {
                return Err(Error::Decode("AAC format changed during playback".into()));
            }
            _ => {}
        }
        let samples = match out_format {
            PcmFormat::F32 => PcmBuffer::F32(decoded.samples),
            PcmFormat::I32 { .. } => PcmBuffer::I32(
                decoded
                    .samples
                    .iter()
                    .map(|sample| (sample * 2_147_483_648.0) as i32)
                    .collect(),
            ),
        };
        sink.on_samples(samples)?;
    }
    if stream_info.is_none() {
        return Err(Error::Decode("AAC stream contains no audio".into()));
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

/// The representation of the *decoded* samples. The decoded buffer is
/// authoritative: AAC-in-MP4 reports no bit depth and no sample format, yet its
/// decoder produces f32. Integer decoders always produce an integer buffer, and
/// FLAC always reports the bit depth needed to size the container.
fn source_format(decoded: &GenericAudioBufferRef<'_>, bits: Option<u32>) -> PcmFormat {
    match decoded {
        GenericAudioBufferRef::F32(_) | GenericAudioBufferRef::F64(_) => PcmFormat::F32,
        _ => PcmFormat::I32 {
            bits: normalize_bits(bits),
        },
    }
}

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
            Self {
                info: None,
                samples: Vec::new(),
            }
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
        fn on_samples(&mut self, buf: PcmBuffer) -> Result<()> {
            match &buf {
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
    fn decodes_fragmented_mp4_aac() {
        let sink = decode_fixture_f32("tone_aac.mp4", "mp4");
        let info = sink.info.unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.format, PcmFormat::F32);
        assert!(
            !info.is_bit_perfect_candidate(),
            "AAC must not be treated as a bit-perfect candidate"
        );
        // AAC-LC adds up to one frame of encoder delay/padding (1024 samples
        // per channel) around the 2 s tone.
        assert!(
            sink.samples.len() >= 176_400 && sink.samples.len() <= 176_400 + 2 * 2 * 1024,
            "unexpected AAC sample count: {}",
            sink.samples.len()
        );
        assert!(sink.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn he_aac_reconstructs_sbr_and_stereo_like_the_independent_decoder() {
        // FFmpeg 9.0.2 native AAC, untrimmed f32le; see fixtures/README.md.
        // Check PCM, not just plausible metadata or an upsampled LC core.
        let references = [
            (
                1,
                149504,
                [
                    (8191, [-0.33381212, 0.06831797]),
                    (24000, [-0.16039744, -0.08140720]),
                    (72000, [-0.16802303, -0.07372804]),
                    (140000, [-0.12361196, -0.01172292]),
                ],
            ),
            (
                2,
                151552,
                [
                    (8191, [-0.20273273, -0.12402325]),
                    (24000, [-0.16285948, 0.09551252]),
                    (72000, [-0.30055341, 0.07540117]),
                    (140000, [0.09424540, 0.11844872]),
                ],
            ),
        ];
        for (version, frames, checkpoints) in references {
            for signaling in ["hierarchical", "sync", "implicit"] {
                let name = format!("he_aac_v{version}_96k_{signaling}.mp4");
                let mut sink = CollectSink::new();
                let bytes = fs::read(format!("tests/fixtures/{name}")).unwrap();
                run_with_aac_hint(BytesSource::new(bytes), "mp4", true, &mut sink).unwrap();
                let info = sink.info.unwrap();
                assert_eq!(info.sample_rate, 48000, "{name}");
                assert_eq!(info.channels, 2, "{name}");
                assert_eq!(info.format, PcmFormat::F32);
                assert!(!info.is_bit_perfect_candidate());
                assert_eq!(sink.samples.len(), frames * 2, "{name}");
                assert!(sink.samples.iter().all(|s| s.is_finite()));
                for (frame, expected) in checkpoints {
                    for (channel, expected) in expected.into_iter().enumerate() {
                        let actual = sink.samples[frame * 2 + channel];
                        assert!(
                            (f64::from(actual) - expected).abs() < 0.0001,
                            "{name}, frame {frame}, channel {channel}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }

    fn fixture_reader(bytes: Vec<u8>) -> Box<dyn FormatReader> {
        let mss = MediaSourceStream::new(Box::new(BytesSource::new(bytes)), Default::default());
        let mut hint = Hint::new();
        hint.with_extension("mp4");
        symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .unwrap()
    }

    #[test]
    fn he_aac_decoder_is_selected_only_by_the_hint_or_explicit_config() {
        let mut names = vec![("tone_aac.mp4".to_string(), false)];
        for version in [1, 2] {
            for signaling in ["hierarchical", "sync", "implicit"] {
                names.push((
                    format!("he_aac_v{version}_96k_{signaling}.mp4"),
                    signaling != "implicit",
                ));
            }
        }
        for (name, explicit_he) in names {
            let reader = fixture_reader(fs::read(format!("tests/fixtures/{name}")).unwrap());
            let track = reader.default_track(TrackType::Audio).unwrap();
            let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
                panic!("no audio parameters for {name}");
            };
            assert_eq!(params.codec, CODEC_ID_AAC);
            assert_eq!(
                use_he_aac(false, params.extra_data.as_deref()),
                explicit_he,
                "{name}"
            );
            assert!(use_he_aac(true, params.extra_data.as_deref()), "{name}");
            if explicit_he {
                let sink = decode_fixture_f32(&name, "mp4");
                assert_eq!(sink.info.unwrap().sample_rate, 48000, "{name}");
                assert_eq!(sink.info.unwrap().channels, 2, "{name}");
            }
        }
        assert!(!use_he_aac(false, None));
        assert!(!use_he_aac(false, Some(&[0xff])));
    }

    #[test]
    fn aac_lc_keeps_symphonia_packet_error_recovery() {
        let mut bytes = fs::read("tests/fixtures/tone_aac.mp4").unwrap();
        let mut reader = fixture_reader(bytes.clone());
        let mut packet = reader.next_packet().unwrap().unwrap();
        for _ in 0..8 {
            packet = reader.next_packet().unwrap().unwrap();
        }
        let offset = bytes
            .windows(packet.data.len())
            .position(|b| b == packet.data.as_ref())
            .unwrap();
        // CPE with a common window and the reserved ICS bit set: a packet
        // DecodeError (not an unsupported-feature error) in Symphonia.
        bytes[offset..offset + 6].copy_from_slice(&[0x21, 0x80, 0, 0, 0, 0]);
        let mut recovered = CollectSink::new();
        run(BytesSource::new(bytes.clone()), "mp4", &mut recovered).unwrap();
        let clean = decode_fixture_f32("tone_aac.mp4", "mp4");
        assert_eq!(recovered.samples.len() + 1024 * 2, clean.samples.len());
        assert!(recovered.samples.iter().all(|s| s.is_finite()));
        assert_eq!(recovered.info.unwrap().sample_rate, 44100);
        assert!(!recovered.info.unwrap().is_bit_perfect_candidate());

        let mut terminal = CollectSink::new();
        assert!(run_with_aac_hint(BytesSource::new(bytes), "mp4", true, &mut terminal).is_err());
        assert_eq!(terminal.samples.len(), 8 * 1024 * 2);
        assert!(recovered.samples.len() > terminal.samples.len());
    }

    #[test]
    fn probe_does_not_claim_implicit_he_aac_is_lc() {
        for version in [1, 2] {
            for signaling in ["hierarchical", "sync", "implicit"] {
                let name = format!("tests/fixtures/he_aac_v{version}_96k_{signaling}.mp4");
                let format = probe(BytesSource::new(fs::read(name).unwrap())).unwrap();
                let expected = match (version, signaling) {
                    (_, "implicit") => "mp4a",
                    (1, _) => "mp4a.40.5",
                    _ => "mp4a.40.29",
                };
                assert_eq!(format.codec.as_deref(), Some(expected));
            }
        }
    }

    #[test]
    fn aac_integer_sink_conversion_does_not_make_the_source_bit_perfect() {
        let float = decode_fixture_f32("tone_aac.mp4", "mp4");
        let integer = decode_fixture_i32("tone_aac.mp4", "mp4");
        assert!(!integer.info.unwrap().is_bit_perfect_candidate());
        assert_eq!(float.samples.len(), integer.samples.len());
        for (float, integer) in float.samples.iter().zip(integer.samples) {
            assert_eq!((float * 2_147_483_648.0) as i32, integer);
        }
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
        for (index, (&int, &float)) in i32_sink
            .samples
            .iter()
            .zip(f32_sink.samples.iter())
            .enumerate()
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
        for (index, (&int, &float)) in i32_sink
            .samples
            .iter()
            .zip(f32_sink.samples.iter())
            .enumerate()
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
