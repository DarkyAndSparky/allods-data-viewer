//! CELT 0.11 decoder (FMOD FSB5 mode 12), ported from the BSD-licensed Xiph reference.

// The modules follow the C reference closely (index loops, constants copied digit by digit), so
// these style lints are allowed here.
#![allow(
    clippy::needless_range_loop,
    clippy::excessive_precision,
    clippy::collapsible_if
)]

mod bands;
mod cwrs;
mod decoder;
mod laplace;
mod mdct;
mod mode;
mod quant_bands;
mod rangedec;
mod rate;
mod vq;

pub use decoder::Decoder;

#[cfg(test)]
mod tests {
    use super::Decoder;

    fn noise(seed: &mut u32, len: usize) -> Vec<u8> {
        (0..len)
            .map(|_| {
                *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (*seed >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn zero_packet_decodes_to_a_full_frame() {
        let mut d = Decoder::new(44_100, 512, 2).expect("decoder");
        let mut out = Vec::new();
        d.decode(&[0u8; 40], &mut out).expect("decode");
        assert_eq!(out.len(), 1024);
    }

    #[test]
    fn mono_frame_has_one_value_per_sample() {
        let mut d = Decoder::new(48_000, 480, 1).expect("decoder");
        let mut out = Vec::new();
        d.decode(&[0x55u8; 60], &mut out).expect("decode");
        assert_eq!(out.len(), 480);
        assert_eq!(d.frame_size(), 480);
    }

    #[test]
    fn truncated_packet_is_an_error() {
        let mut d = Decoder::new(44_100, 512, 2).expect("decoder");
        let mut out = Vec::new();
        assert!(d.decode(&[], &mut out).is_err());
        assert!(d.decode(&[0x12], &mut out).is_err());
        assert!(out.is_empty());
    }

    #[test]
    fn unsupported_layouts_are_refused() {
        assert!(Decoder::new(44_100, 512, 0).is_err());
        assert!(Decoder::new(44_100, 512, 3).is_err());
        assert!(Decoder::new(44_100, 0, 2).is_err());
        assert!(Decoder::new(0, 512, 2).is_err());
    }

    #[test]
    fn damaged_packets_never_panic() {
        let mut seed = 7u32;
        for (rate, frame) in [(44_100, 512), (48_000, 960), (24_000, 480), (44_100, 1024)] {
            for channels in 1..=2usize {
                let Ok(mut d) = Decoder::new(rate, frame, channels) else {
                    continue;
                };
                for len in 2..200usize {
                    let packet = noise(&mut seed, len);
                    let mut out = Vec::new();
                    let _ = d.decode(&packet, &mut out);
                }
            }
        }
    }
}
