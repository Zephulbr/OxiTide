use symphonia::core::audio::AudioSpec;
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

#[cfg(test)]
mod tests {
    use std::fs;

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
        assert_eq!(spec.rate(), 44100);
        assert_eq!(sink.samples.len(), 176_400); // 2s * 44100 * 2ch
        assert!(sink.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn decodes_bare_flac() {
        let sink = decode_fixture("tone.flac", "flac");
        assert_eq!(sink.samples.len(), 176_400);
    }
}
