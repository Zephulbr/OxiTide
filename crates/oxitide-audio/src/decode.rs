use symphonia::core::audio::sample::SampleFormat;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

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
