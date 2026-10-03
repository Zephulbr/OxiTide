use crate::decode::{PcmFormat, StreamInfo};
use crate::output::exclusive_formats;

/// The ALSA sample formats OxiTide writes, best quality first. All are
/// little-endian and use a 32-, 24-, or 16-bit container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaSampleFormat {
    S32Le,
    S24_3Le,
    S16Le,
}

impl AlsaSampleFormat {
    pub fn store_bits(self) -> u16 {
        match self {
            AlsaSampleFormat::S32Le => 32,
            AlsaSampleFormat::S24_3Le => 24,
            AlsaSampleFormat::S16Le => 16,
        }
    }
}

/// Formats to try, in order, for an exclusive open. Empty when the source is
/// not an integer 16/24/32-bit bit-perfect candidate. The 32-bit container is
/// preferred (symphonia left-aligns integer samples to 32 bits); packed 24-bit
/// and 16-bit containers are the step-down options for DACs without S32.
pub fn alsa_exclusive_formats(info: &StreamInfo) -> Vec<AlsaSampleFormat> {
    let PcmFormat::I32 { bits } = info.format else {
        return Vec::new();
    };
    // `exclusive_formats` gates the engine's 16/24-bit window; a 32-bit I32
    // source still has a valid S32_LE container for ALSA.
    if bits != 32 && exclusive_formats(info).is_empty() {
        return Vec::new();
    }
    let mut out = vec![AlsaSampleFormat::S32Le];
    if bits <= 24 {
        out.push(AlsaSampleFormat::S24_3Le);
    }
    if bits <= 16 {
        out.push(AlsaSampleFormat::S16Le);
    }
    out
}

/// Pack left-aligned i32 samples into interleaved little-endian bytes for
/// `format`. `out` is cleared first. S24_3Le keeps bytes 1..4 of the LE word
/// and S16Le bytes 2..4, which drops the low padding bits symphonia leaves in
/// the 32-bit container.
pub fn pack_i32(format: AlsaSampleFormat, samples: &[i32], out: &mut Vec<u8>) {
    out.clear();
    match format {
        AlsaSampleFormat::S32Le => {
            for &sample in samples {
                out.extend_from_slice(&sample.to_le_bytes());
            }
        }
        AlsaSampleFormat::S24_3Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[1..4]);
            }
        }
        AlsaSampleFormat::S16Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[2..4]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(bits: u16, format: PcmFormat) -> StreamInfo {
        StreamInfo { sample_rate: 44_100, channels: 2, bits_per_sample: bits, format }
    }

    #[test]
    fn alsa_formats_step_down_for_narrow_sources() {
        assert_eq!(
            alsa_exclusive_formats(&info(16, PcmFormat::I32 { bits: 16 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le, AlsaSampleFormat::S16Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(24, PcmFormat::I32 { bits: 24 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(32, PcmFormat::I32 { bits: 32 })),
            vec![AlsaSampleFormat::S32Le]
        );
    }

    #[test]
    fn alsa_formats_are_empty_for_non_candidates() {
        assert!(alsa_exclusive_formats(&info(16, PcmFormat::F32)).is_empty());
    }

    #[test]
    fn store_bits_matches_the_container_or_packed_width() {
        assert_eq!(AlsaSampleFormat::S32Le.store_bits(), 32);
        assert_eq!(AlsaSampleFormat::S24_3Le.store_bits(), 24);
        assert_eq!(AlsaSampleFormat::S16Le.store_bits(), 16);
    }

    #[test]
    fn pack_s32le_preserves_left_aligned_words() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S32Le, &[0x7fff_ff00u32 as i32, -256], &mut out);
        assert_eq!(out, vec![0x00, 0xff, 0xff, 0x7f, 0x00, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn pack_s24_3le_keeps_the_top_three_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S24_3Le, &[0x7fff_ff00u32 as i32], &mut out);
        assert_eq!(out, vec![0xff, 0xff, 0x7f]);
    }

    #[test]
    fn pack_s16le_keeps_the_top_two_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S16Le, &[0x7fff_0000u32 as i32, 0], &mut out);
        assert_eq!(out, vec![0xff, 0x7f, 0x00, 0x00]);
    }

    #[test]
    fn pack_clears_previous_contents() {
        let mut out = vec![0xaa, 0xbb];
        pack_i32(AlsaSampleFormat::S32Le, &[], &mut out);
        assert!(out.is_empty());
    }
}
