//! FMOD sound banks, FSB4 variant (Cloud Pirates `.bsb`/`.fsb` from the first game generation).
//!
//! Layout (see `docs/formats/fsb4.md`): a 48 byte header, one sample header per stream
//! (normally 80 bytes), then the stream data one after another. Streams decode to 16-bit PCM
//! from IMA ADPCM (verified on a real mono bank) or raw PCM (draft: no sample seen).

use crate::error::{Error, Result};
use crate::fsb::{Bank, Pcm, Stream};
use crate::limits::Limits;

const HEADER_LEN: usize = 48;
/// Smallest sample header that holds every field read here.
const MIN_SAMPLE_HEADER: usize = 64;
const MODE_8BIT: u32 = 0x8;
const MODE_16BIT: u32 = 0x10;
const MODE_UNSIGNED: u32 = 0x80;
const MODE_IMA_ADPCM: u32 = 0x0040_0000;
/// Mode bits that select a compressed or console codec (IMA ADPCM is handled separately).
const MODE_OTHER_CODEC: u32 = 0xFFA0_0000;

const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const INDEX_DELTA: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
/// One IMA block of a channel: 4 header bytes and 32 bytes of nibbles.
const IMA_BLOCK: usize = 36;
const IMA_SAMPLES: usize = 64;

fn u16_at(d: &[u8], pos: usize) -> Option<u16> {
    d.get(pos..pos.checked_add(2)?)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], pos: usize) -> Option<u32> {
    d.get(pos..pos.checked_add(4)?)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parses an FSB4 bank (already unwrapped).
///
/// # Errors
/// [`Error::Format`] for a bad header or tables and data running past the end;
/// [`Error::Limit`] for an absurd stream count.
pub fn parse(d: &[u8], limits: &Limits) -> Result<Bank> {
    if d.len() < HEADER_LEN || &d[..4] != b"FSB4" {
        return Err(Error::format("missing FSB4 header"));
    }
    let bad = || Error::format("truncated FSB4 header");
    let count = u32_at(d, 4).ok_or_else(bad)? as usize;
    let shdr = u32_at(d, 8).ok_or_else(bad)? as usize;
    let version = u32_at(d, 16).ok_or_else(bad)?;
    let bank_mode = u32_at(d, 20).ok_or_else(bad)?;
    if count as u64 > limits.max_entries || count > d.len() {
        return Err(Error::limit(format!(
            "{count} streams does not fit the file"
        )));
    }
    let data_start = HEADER_LEN
        .checked_add(shdr)
        .filter(|&e| e <= d.len())
        .ok_or_else(|| Error::format("sample headers run past end of file"))?;
    let mut streams = Vec::with_capacity(count);
    let mut pos = HEADER_LEN;
    let mut data_pos = data_start;
    for i in 0..count {
        let size = usize::from(u16_at(d, pos).ok_or_else(bad)?);
        if size < MIN_SAMPLE_HEADER || pos + size > data_start {
            return Err(Error::format(format!("sample header {i} is malformed")));
        }
        let h = &d[pos..pos + size];
        let name_bytes = h.get(2..32).ok_or_else(bad)?;
        let end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(name_bytes.len());
        let samples = u64::from(u32_at(h, 32).ok_or_else(bad)?);
        let data_len = u32_at(h, 36).ok_or_else(bad)? as usize;
        let mode = u32_at(h, 48).ok_or_else(bad)?;
        let freq = u32_at(h, 52).ok_or_else(bad)?;
        let channels = u32::from(u16_at(h, 62).ok_or_else(bad)?).max(1);
        let next = data_pos
            .checked_add(data_len)
            .filter(|&e| e <= d.len())
            .ok_or_else(|| Error::format(format!("data of stream {i} runs past end of file")))?;
        streams.push(Stream {
            name: String::from_utf8_lossy(&name_bytes[..end]).into_owned(),
            frequency: Some(freq).filter(|&f| f > 0),
            channels,
            samples,
            data_offset: data_pos,
            data_len,
            vorbis_crc: None,
            mode,
        });
        data_pos = next;
        pos += size;
    }
    Ok(Bank {
        version,
        mode: bank_mode,
        streams,
        fsb4: true,
    })
}

struct ImaChannel {
    predictor: i32,
    index: i32,
}

impl ImaChannel {
    fn step(&mut self, nibble: u8) -> i16 {
        let step = STEPS[self.index as usize];
        let mut diff = step >> 3;
        if nibble & 1 != 0 {
            diff += step >> 2;
        }
        if nibble & 2 != 0 {
            diff += step >> 1;
        }
        if nibble & 4 != 0 {
            diff += step;
        }
        if nibble & 8 != 0 {
            diff = -diff;
        }
        self.predictor = (self.predictor + diff).clamp(-32768, 32767);
        self.index = (self.index + INDEX_DELTA[usize::from(nibble & 7)]).clamp(0, 88);
        self.predictor as i16
    }
}

/// Decodes one stream of an FSB4 bank to PCM.
///
/// # Errors
/// [`Error::Unsupported`] for codecs other than IMA ADPCM and raw PCM, [`Error::Format`] for
/// data outside the bank.
pub fn decode(bank_bytes: &[u8], s: &Stream) -> Result<Pcm> {
    let data = bank_bytes
        .get(s.data_offset..s.data_offset.saturating_add(s.data_len))
        .ok_or_else(|| Error::format("stream data is outside the bank"))?;
    let rate = s
        .frequency
        .ok_or_else(|| Error::unsupported("unknown sample rate"))?;
    let channels = s.channels as usize;
    let samples = if s.mode & MODE_IMA_ADPCM != 0 {
        decode_ima(data, channels, s.samples)
    } else if s.mode & MODE_OTHER_CODEC == 0 && s.mode & (MODE_8BIT | MODE_16BIT) != 0 {
        decode_pcm(data, channels, s.samples, s.mode)
    } else {
        return Err(Error::unsupported(format!(
            "FSB4 stream mode {:#010x} cannot be decoded here",
            s.mode
        )));
    };
    Ok(Pcm {
        rate,
        channels: s.channels,
        samples,
    })
}

/// Blocks of every channel follow each other (channel 0, channel 1, ... per 64 frames).
/// Mono is verified on a real bank; the stereo order is an assumption.
fn decode_ima(data: &[u8], channels: usize, frames: u64) -> Vec<i16> {
    let group = IMA_BLOCK.saturating_mul(channels);
    let groups = data.len().checked_div(group).unwrap_or(0);
    let wanted = usize::try_from(frames).unwrap_or(usize::MAX);
    let total = groups.saturating_mul(IMA_SAMPLES).min(wanted);
    let mut out = vec![0i16; total.saturating_mul(channels)];
    for g in 0..groups {
        for c in 0..channels {
            let Some(block) =
                data.get(g * group + c * IMA_BLOCK..(g * group + (c + 1) * IMA_BLOCK))
            else {
                return out;
            };
            let mut st = ImaChannel {
                predictor: i32::from(i16::from_le_bytes([block[0], block[1]])),
                index: i32::from(block[2]).min(88),
            };
            for k in 0..IMA_SAMPLES {
                let byte = block[4 + k / 2];
                let nibble = if k % 2 == 0 { byte & 15 } else { byte >> 4 };
                let v = st.step(nibble);
                let frame = g * IMA_SAMPLES + k;
                if frame < total {
                    out[frame * channels + c] = v;
                }
            }
        }
    }
    out
}

/// Raw interleaved PCM, 16-bit little endian or 8-bit (unsigned when flagged).
/// Draft: no real bank with this mode has been seen.
fn decode_pcm(data: &[u8], channels: usize, frames: u64, mode: u32) -> Vec<i16> {
    let wide = mode & MODE_16BIT != 0;
    let width = if wide { 2 } else { 1 };
    let available = data.len() / width / channels.max(1);
    let total = available.min(usize::try_from(frames).unwrap_or(usize::MAX));
    let count = total.saturating_mul(channels);
    if wide {
        data.as_chunks::<2>()
            .0
            .iter()
            .take(count)
            .map(|b| i16::from_le_bytes(*b))
            .collect()
    } else {
        data.iter()
            .take(count)
            .map(|&b| {
                let v = if mode & MODE_UNSIGNED != 0 {
                    i16::from(b) - 128
                } else {
                    i16::from(b as i8)
                };
                v << 8
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    /// One test stream: name, mode flags, frequency, channels, samples, data.
    pub type TestStream<'a> = (&'a str, u32, u32, u16, u32, Vec<u8>);

    /// Builds an FSB4 from test streams.
    pub fn build(streams: &[TestStream<'_>]) -> Vec<u8> {
        let mut headers = Vec::new();
        let mut data = Vec::new();
        for (name, mode, freq, ch, samples, bytes) in streams {
            let mut h = vec![0u8; 80];
            h[..2].copy_from_slice(&80u16.to_le_bytes());
            h[2..2 + name.len()].copy_from_slice(name.as_bytes());
            h[32..36].copy_from_slice(&samples.to_le_bytes());
            h[36..40].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            h[48..52].copy_from_slice(&mode.to_le_bytes());
            h[52..56].copy_from_slice(&freq.to_le_bytes());
            h[62..64].copy_from_slice(&ch.to_le_bytes());
            headers.extend_from_slice(&h);
            data.extend_from_slice(bytes);
        }
        let mut out = b"FSB4".to_vec();
        for v in [
            streams.len() as u32,
            headers.len() as u32,
            data.len() as u32,
            0x0004_0000,
            0x20,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(48, 0);
        out.extend_from_slice(&headers);
        out.extend_from_slice(&data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::build;
    use super::*;

    fn ima_block(predictor: i16, index: u8, nibbles: u8) -> Vec<u8> {
        let mut b = predictor.to_le_bytes().to_vec();
        b.extend_from_slice(&[index, 0]);
        b.extend(std::iter::repeat_n(nibbles, 32));
        b
    }

    #[test]
    fn ima_block_follows_the_reference_step_rule() {
        // Index 0 (step 7), nibble 0b0111: diff = 0 + 1 + 3 + 7 = 11; the index rises by 8.
        let d = build(&[("a", 0x0050_0020, 44_100, 1, 64, ima_block(100, 0, 0x77))]);
        let b = parse(&d, &Limits::default()).expect("parse");
        assert_eq!(b.streams[0].name, "a");
        let pcm = decode(&d, &b.streams[0]).expect("decode");
        assert_eq!((pcm.rate, pcm.channels, pcm.samples.len()), (44_100, 1, 64));
        assert_eq!(pcm.samples[0], 111);
        // Index is 8 now (step 16): diff = 2 + 4 + 8 + 16 = 30.
        assert_eq!(pcm.samples[1], 141);
    }

    #[test]
    fn length_is_cut_to_the_declared_samples() {
        let d = build(&[("a", MODE_IMA_ADPCM, 22_050, 1, 10, ima_block(0, 0, 0))]);
        let b = parse(&d, &Limits::default()).expect("parse");
        assert_eq!(decode(&d, &b.streams[0]).expect("decode").samples.len(), 10);
    }

    #[test]
    fn stereo_blocks_interleave_per_frame() {
        let mut data = ima_block(1000, 0, 0x99);
        data.extend(ima_block(-1000, 0, 0x11));
        let d = build(&[("s", MODE_IMA_ADPCM, 44_100, 2, 64, data)]);
        let b = parse(&d, &Limits::default()).expect("parse");
        let pcm = decode(&d, &b.streams[0]).expect("decode");
        assert_eq!(pcm.samples.len(), 128);
        assert_eq!((pcm.samples[0], pcm.samples[1]), (999, -999));
    }

    #[test]
    fn raw_pcm16_is_copied() {
        let bytes: Vec<u8> = [1i16, -2, 3].iter().flat_map(|v| v.to_le_bytes()).collect();
        let d = build(&[("p", MODE_16BIT | 0x20, 8000, 1, 3, bytes)]);
        let b = parse(&d, &Limits::default()).expect("parse");
        assert_eq!(
            decode(&d, &b.streams[0]).expect("decode").samples,
            [1, -2, 3]
        );
    }

    #[test]
    fn other_codecs_are_refused_by_name() {
        let d = build(&[("m", 0x0020_0020, 8000, 1, 3, vec![0; 4])]);
        let b = parse(&d, &Limits::default()).expect("parse");
        let e = decode(&d, &b.streams[0]).expect_err("unsupported");
        assert!(e.to_string().contains("0x00200020"), "{e}");
    }

    #[test]
    fn truncated_and_corrupted_banks_are_errors() {
        let d = build(&[("a", MODE_IMA_ADPCM, 44_100, 1, 64, ima_block(0, 0, 0))]);
        for cut in [3, 47, 60, d.len() - 1] {
            assert!(parse(&d[..cut], &Limits::default()).is_err(), "cut {cut}");
        }
        let mut bad = d.clone();
        bad[48..50].copy_from_slice(&10u16.to_le_bytes());
        assert!(parse(&bad, &Limits::default()).is_err());
        let mut huge = d;
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge, &Limits::default()).is_err());
    }

    #[test]
    fn damaged_index_byte_is_clamped_not_a_panic() {
        let d = build(&[("a", MODE_IMA_ADPCM, 44_100, 1, 64, ima_block(0, 255, 0xFF))]);
        let b = parse(&d, &Limits::default()).expect("parse");
        assert_eq!(decode(&d, &b.streams[0]).expect("decode").samples.len(), 64);
    }
}
