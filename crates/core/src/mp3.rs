//! MP3 export of decoded sounds, on top of the `rusty_mp3` encoder (pure Rust, Apache-2.0).
//!
//! The encoder runs at the highest constant bitrate the format allows for the sample rate
//! (320 kbps for 32, 44.1 and 48 kHz; 160 kbps for the MPEG-2 rates). MP3 is lossy: the WAV
//! export stays the lossless route (see `docs/formats/mp3-export.md`).

use crate::error::{Error, Result};
use crate::fsb::Pcm;
use rusty_mp3::{Error as Mp3Error, Mp3Encoder, Mp3EncoderConfig};

/// Sample rates Layer III can carry (MPEG-1, MPEG-2 and MPEG-2.5).
const RATES: [u32; 9] = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];
/// Requested bitrate; the encoder snaps it to the highest legal value of the MPEG version.
const MAX_KBPS: u32 = 320;

fn mp3_err(e: &Mp3Error) -> Error {
    Error::format(format!("MP3 encoder: {e}"))
}

impl Pcm {
    /// Mixes a multichannel sound down to stereo. 6 and 8 channels follow the usual layout
    /// (FL, FR, FC, LFE, BL, BR, [SL, SR]) and drop the low-frequency channel; any other
    /// count (3..=16) averages the even channels into left and the odd ones into right.
    fn downmix_stereo(&self) -> Vec<i16> {
        let n = self.channels as usize;
        if !(3..=16).contains(&n) {
            return Vec::new();
        }
        if n != 6 && n != 8 {
            let mut out = Vec::with_capacity(self.samples.len() / n * 2);
            for f in self.samples.chunks_exact(n) {
                let (mut l, mut r) = (0i32, 0i32);
                for (i, v) in f.iter().enumerate() {
                    if i % 2 == 0 {
                        l += i32::from(*v);
                    } else {
                        r += i32::from(*v);
                    }
                }
                let (nl, nr) = (n.div_ceil(2) as i32, (n / 2) as i32);
                out.push((l / nl) as i16);
                out.push((r / nr) as i16);
            }
            return out;
        }
        const K: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let norm = 1.0 / (1.0 + K + K + if n == 8 { K } else { 0.0 });
        let mut out = Vec::with_capacity(self.samples.len() / n * 2);
        for f in self.samples.chunks_exact(n) {
            let g = |i: usize| f32::from(f[i]);
            let side = |i: usize| if n == 8 { g(i) } else { 0.0 };
            let l = (g(0) + K * g(2) + K * g(4) + K * side(6)) * norm;
            let r = (g(1) + K * g(2) + K * g(5) + K * side(7)) * norm;
            out.push(l.round().clamp(-32768.0, 32767.0) as i16);
            out.push(r.round().clamp(-32768.0, 32767.0) as i16);
        }
        out
    }

    /// The sound as an MP3 file at the highest constant bitrate for its sample rate.
    ///
    /// # Errors
    /// [`Error::Unsupported`] for a sample rate or channel count MP3 cannot carry,
    /// [`Error::Format`] for an empty sound or an encoder failure.
    pub fn to_mp3(&self) -> Result<Vec<u8>> {
        if !RATES.contains(&self.rate) {
            return Err(Error::unsupported(format!(
                "{} Hz cannot be stored as MP3 (use WAV)",
                self.rate
            )));
        }
        if self.samples.is_empty() {
            return Err(Error::format("the sound is empty"));
        }
        // MP3 carries one or two channels; 5.1 and 7.1 sounds are mixed down to stereo.
        let mixed;
        let (samples, channels): (&[i16], u16) = match self.channels {
            1 => (&self.samples, 1),
            2 => (&self.samples, 2),
            3..=16 => {
                mixed = self.downmix_stereo();
                (&mixed, 2)
            }
            n => {
                return Err(Error::unsupported(format!(
                    "{n} channels cannot be stored as MP3 (use WAV)"
                )));
            }
        };
        let mut enc = Mp3Encoder::new(Mp3EncoderConfig {
            bitrate_kbps: MAX_KBPS,
            vbr_quality: None,
        });
        enc.push_pcm_s16(samples, channels, self.rate)
            .map_err(|e| mp3_err(&e))?;
        enc.finish();
        let mut out = Vec::new();
        loop {
            match enc.next_packet() {
                Ok(p) => out.extend_from_slice(&p),
                Err(Mp3Error::Again | Mp3Error::Eof) => break,
                Err(e) => return Err(mp3_err(&e)),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_mp3::Mp3Decoder;

    fn tone(rate: u32, channels: u32, seconds: u32) -> Pcm {
        let frames = rate * seconds;
        let mut samples = Vec::new();
        for i in 0..frames {
            let t = i as f32 / rate as f32;
            let v = (0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 32767.0) as i16;
            for _ in 0..channels {
                samples.push(v);
            }
        }
        Pcm {
            rate,
            channels,
            samples,
        }
    }

    fn decode(mp3: &[u8]) -> (u32, u16, Vec<f32>) {
        let mut d = Mp3Decoder::new();
        d.push(mp3);
        d.flush();
        let (mut rate, mut ch, mut pcm) = (0, 0, Vec::new());
        while let Ok(f) = d.next_frame() {
            rate = f.sample_rate;
            ch = f.channels;
            pcm.extend_from_slice(&f.samples);
        }
        (rate, ch, pcm)
    }

    #[test]
    fn stereo_tone_round_trips_with_length_rate_and_level() {
        let src = tone(44_100, 2, 1);
        let mp3 = src.to_mp3().expect("encode");
        assert!(mp3.starts_with(&[0xFF]) || mp3.starts_with(b"ID3"));
        let (rate, ch, pcm) = decode(&mp3);
        assert_eq!((rate, ch), (44_100, 2));
        // The decoder keeps the encoder delay and the padding: the result is longer by less
        // than three frames. Find the delay, then compare the middle of the signal.
        let extra = pcm.len() - src.samples.len().min(pcm.len());
        assert!(
            pcm.len() >= src.samples.len() && extra < 3 * 1152 * 2,
            "extra {extra}"
        );
        let mut best = f64::MAX;
        let mut snr = 0.0;
        for lag in 0..=extra / 2 {
            let (mut err, mut energy) = (0f64, 0f64);
            for i in 20_000..60_000 {
                let a = f64::from(src.samples[i]) / 32768.0;
                let b = f64::from(pcm[i + lag * 2]);
                err += (a - b) * (a - b);
                energy += a * a;
            }
            if err < best {
                best = err;
                snr = 10.0 * (energy / err.max(1e-12)).log10();
            }
        }
        assert!(snr > 25.0, "signal to noise ratio only {snr:.1} dB");
    }

    #[test]
    fn mono_mpeg2_rate_is_encoded() {
        let mp3 = tone(22_050, 1, 1).to_mp3().expect("encode");
        let (rate, ch, _) = decode(&mp3);
        assert_eq!((rate, ch), (22_050, 1));
    }

    #[test]
    fn unsupported_rate_channels_and_empty_sound_are_errors() {
        assert!(tone(4000, 1, 1).to_mp3().is_err());
        assert!(tone(44_100, 3, 1).to_mp3().is_ok());
        assert!(tone(44_100, 10, 1).to_mp3().is_ok());
        assert!(tone(44_100, 17, 1).to_mp3().is_err());
        assert!(tone(44_100, 6, 1).to_mp3().is_ok());
        assert!(tone(44_100, 8, 1).to_mp3().is_ok());
        let empty = Pcm {
            rate: 44_100,
            channels: 1,
            samples: Vec::new(),
        };
        assert!(empty.to_mp3().is_err());
    }

    #[test]
    fn very_short_sound_does_not_panic() {
        let p = Pcm {
            rate: 48_000,
            channels: 1,
            samples: vec![1000; 10],
        };
        assert!(p.to_mp3().is_ok());
    }
}
