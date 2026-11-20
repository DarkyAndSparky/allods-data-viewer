//! FMOD sound banks (`.fsb`, FSB5) and their zlib wrapper (`.bsb`).
//!
//! Only the container is read here: stream names, frequency, channels and length. Vorbis
//! decoding is done by an external program (see `docs/formats/media.md`).

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::pak::looks_like_zlib;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Header size of an FSB5 file.
const HEADER_LEN: usize = 60;

/// Frequencies by the 4 bit index of a sample header. Index 9 (48 000 Hz) was seen in
/// real files; the other entries follow the FMOD table and are not verified here.
const FREQUENCIES: [u32; 10] = [
    4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000,
];

/// One stream (sound) of a bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    /// Stream name, empty when the bank has no name table.
    pub name: String,
    /// Sample rate in Hz, `None` for an unknown frequency index.
    pub frequency: Option<u32>,
    /// Number of channels (from the sample header, or from chunk type 1).
    pub channels: u32,
    /// Length in samples per channel.
    pub samples: u64,
    /// Offset of the stream data from the start of the bank.
    pub data_offset: usize,
    /// Length of the stream data in bytes.
    pub data_len: usize,
    /// CRC32 naming the Vorbis setup packet (chunk type 11), if present.
    pub vorbis_crc: Option<u32>,
}

impl Stream {
    /// Duration in seconds, when the frequency is known.
    #[must_use]
    pub fn seconds(&self) -> Option<f64> {
        self.frequency
            .filter(|f| *f > 0)
            .map(|f| self.samples as f64 / f64::from(f))
    }
}

/// A parsed bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    /// FSB5 version field.
    pub version: u32,
    /// Codec mode field (15 = Vorbis in every file seen so far).
    pub mode: u32,
    /// Streams in file order.
    pub streams: Vec<Stream>,
}

fn u32_at(d: &[u8], pos: usize) -> Result<u32> {
    d.get(pos..pos + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| Error::format(format!("read past end at {pos}")))
}

fn u64_at(d: &[u8], pos: usize) -> Result<u64> {
    d.get(pos..pos + 8)
        .map(|b| {
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            u64::from_le_bytes(a)
        })
        .ok_or_else(|| Error::format(format!("read past end at {pos}")))
}

/// Returns the FSB5 bytes of a `.fsb` or `.bsb`: inflates the zlib layer and removes the
/// 4 byte length prefix of a `.bsb`.
///
/// # Errors
///
/// Fails on zlib errors, size limits, or when the result is not FSB5.
pub fn unwrap_bank(mut input: impl Read, limits: &Limits) -> Result<Vec<u8>> {
    let cap = limits.max_entry_size;
    let mut raw = Vec::new();
    (&mut input).take(cap + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > cap {
        return Err(Error::limit("sound bank too large"));
    }
    if raw.starts_with(b"FSB5") {
        return Ok(raw);
    }
    if raw.get(4..8) == Some(b"FSB5") {
        // Plain variant: a length prefix and the bank, without zlib (SFX_* banks).
        if u32_at(&raw, 0)? as usize != raw.len() - 4 {
            return Err(Error::format("length prefix does not match the data"));
        }
        raw.drain(..4);
        return Ok(raw);
    }
    if !looks_like_zlib(&raw) {
        return Err(Error::format("not an FSB5 bank or a zlib stream"));
    }
    let mut out = Vec::new();
    ZlibDecoder::new(raw.as_slice())
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::format(format!("zlib: {e}")))?;
    if out.len() as u64 > cap {
        return Err(Error::limit("inflated bank too large"));
    }
    let declared = u32_at(&out, 0)? as usize;
    if declared != out.len() - 4 {
        return Err(Error::format("length prefix does not match the data"));
    }
    out.drain(..4);
    if !out.starts_with(b"FSB5") {
        return Err(Error::format("inflated data is not FSB5"));
    }
    Ok(out)
}

/// A decoded sound: interleaved 16-bit PCM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm {
    /// Sample rate in Hz.
    pub rate: u32,
    /// Channel count.
    pub channels: u32,
    /// Interleaved samples.
    pub samples: Vec<i16>,
}

impl Pcm {
    /// The sound as a 16-bit PCM WAV file.
    #[must_use]
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let block = (self.channels * 2) as u16;
        let mut w = Vec::with_capacity(44 + self.samples.len() * 2);
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&(self.channels as u16).to_le_bytes());
        w.extend_from_slice(&self.rate.to_le_bytes());
        w.extend_from_slice(&(self.rate * u32::from(block)).to_le_bytes());
        w.extend_from_slice(&block.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for v in &self.samples {
            w.extend_from_slice(&v.to_le_bytes());
        }
        w
    }
}

impl Bank {
    /// Decodes stream `index` of this bank (`bank_bytes` is the unwrapped FSB5).
    ///
    /// # Errors
    /// Fails for an unsupported codec, an unknown Vorbis setup or damaged data.
    pub fn decode(&self, bank_bytes: &[u8], index: usize) -> Result<Pcm> {
        let s = self
            .streams
            .get(index)
            .ok_or_else(|| Error::format("no such stream"))?;
        if self.mode != 15 {
            return Err(Error::unsupported(format!(
                "codec mode {} cannot be decoded here",
                self.mode
            )));
        }
        let crc = s
            .vorbis_crc
            .ok_or_else(|| Error::format("stream has no Vorbis setup id"))?;
        let setup = crate::vorbis::setup_for(crc).ok_or_else(|| {
            Error::unsupported(format!(
                "Vorbis setup {crc:08x} is not in the built-in list"
            ))
        })?;
        let data = bank_bytes
            .get(s.data_offset..s.data_offset.saturating_add(s.data_len))
            .ok_or_else(|| Error::format("stream data is outside the bank"))?;
        let rate = s
            .frequency
            .ok_or_else(|| Error::unsupported("unknown sample rate"))?;
        let samples = crate::vorbis::decode_stream(setup, s.channels as usize, data, s.samples)?;
        Ok(Pcm {
            rate,
            channels: s.channels,
            samples,
        })
    }
}

/// Parses an FSB5 bank (already unwrapped).
///
/// # Errors
///
/// [`Error::Format`] for a bad header, tables running past the end, or inconsistent
/// sizes; [`Error::Limit`] for an absurd stream count.
pub fn parse_bank(d: &[u8], limits: &Limits) -> Result<Bank> {
    if d.len() < HEADER_LEN || &d[..4] != b"FSB5" {
        return Err(Error::format("missing FSB5 header"));
    }
    let version = u32_at(d, 4)?;
    let count = u32_at(d, 8)? as usize;
    let shdr = u32_at(d, 12)? as usize;
    let names = u32_at(d, 16)? as usize;
    let mode = u32_at(d, 24)?;
    if count as u64 > limits.max_entries || count > d.len() {
        return Err(Error::limit(format!(
            "{count} streams does not fit the file"
        )));
    }
    let shdr_start = HEADER_LEN;
    let names_start = shdr_start
        .checked_add(shdr)
        .ok_or_else(|| Error::format("sample header size overflow"))?;
    let data_start = names_start
        .checked_add(names)
        .ok_or_else(|| Error::format("name table size overflow"))?;
    if data_start > d.len() {
        return Err(Error::format("tables run past end of file"));
    }
    let table = &d[shdr_start..names_start];
    let mut pos = 0usize;
    let mut streams = Vec::with_capacity(count);
    for i in 0..count {
        let bits = u64_at(table, pos)?;
        pos += 8;
        let extra = bits & 1 != 0;
        let freq = ((bits >> 1) & 15) as usize;
        let mut channels = if (bits >> 5) & 1 != 0 { 2 } else { 1 };
        let samples = bits >> 34;
        let data_offset = data_start
            .checked_add((((bits >> 6) & 0x0FFF_FFFF) as usize) * 16)
            .ok_or_else(|| Error::format("data offset overflow"))?;
        let mut vorbis_crc = None;
        if extra {
            loop {
                let head = u32_at(table, pos)?;
                pos += 4;
                let size = ((head >> 1) & 0xFF_FFFF) as usize;
                let kind = head >> 25;
                let body = table
                    .get(pos..pos.saturating_add(size))
                    .ok_or_else(|| Error::format(format!("chunk of stream {i} runs past end")))?;
                if kind == 1 && size >= 1 {
                    channels = u32::from(body[0]).max(1);
                }
                if kind == 11 && size >= 4 {
                    vorbis_crc = Some(u32::from_le_bytes([body[0], body[1], body[2], body[3]]));
                }
                pos += size;
                if head & 1 == 0 {
                    break;
                }
            }
        }
        let name = if names >= count * 4 {
            let table = &d[names_start..data_start];
            let off = u32_at(table, i * 4)? as usize;
            let tail = table
                .get(off..)
                .ok_or_else(|| Error::format(format!("name of stream {i} out of range")))?;
            let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
            String::from_utf8_lossy(&tail[..end]).into_owned()
        } else {
            String::new()
        };
        streams.push(Stream {
            name,
            frequency: FREQUENCIES.get(freq).copied(),
            channels,
            samples,
            data_offset,
            data_len: 0,
            vorbis_crc,
        });
    }
    for i in 0..streams.len() {
        let end = streams
            .get(i + 1)
            .map_or(d.len(), |n| n.data_offset)
            .min(d.len());
        streams[i].data_len = end.saturating_sub(streams[i].data_offset);
    }
    Ok(Bank {
        version,
        mode,
        streams,
    })
}

#[cfg(test)]
pub(crate) mod testutil {
    /// Builds a minimal FSB5 with `(name, freq_index, stereo, samples)` streams.
    pub fn build(streams: &[(&str, u64, bool, u64)]) -> Vec<u8> {
        let mut shdr = Vec::new();
        for (_, f, st, n) in streams {
            let bits = (f << 1) | (u64::from(*st) << 5) | (n << 34);
            shdr.extend_from_slice(&bits.to_le_bytes());
        }
        let mut names = Vec::new();
        let mut body = Vec::new();
        for (name, ..) in streams {
            names.extend_from_slice(&((streams.len() * 4 + body.len()) as u32).to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.push(0);
        }
        names.extend_from_slice(&body);
        let mut out = b"FSB5".to_vec();
        for v in [
            1u32,
            streams.len() as u32,
            shdr.len() as u32,
            names.len() as u32,
            0,
            15,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(60, 0);
        out.extend_from_slice(&shdr);
        out.extend_from_slice(&names);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::build;
    use super::*;

    #[test]
    fn parses_names_frequency_channels_and_length() {
        let d = build(&[("Voice_01", 9, false, 96_000), ("Music", 8, true, 44_100)]);
        let b = parse_bank(&d, &Limits::default()).expect("parse");
        assert_eq!((b.version, b.mode, b.streams.len()), (1, 15, 2));
        assert_eq!(b.streams[0].name, "Voice_01");
        assert_eq!(b.streams[0].frequency, Some(48_000));
        assert_eq!(b.streams[0].seconds(), Some(2.0));
        assert_eq!(
            (b.streams[1].channels, b.streams[1].frequency),
            (2, Some(44_100))
        );
    }

    #[test]
    fn bsb_wrapper_is_removed() {
        let fsb = build(&[("A", 9, false, 10)]);
        let mut wrapped = (fsb.len() as u32).to_le_bytes().to_vec();
        wrapped.extend_from_slice(&fsb);
        let z = crate::pak::testutil::zlib(&wrapped);
        assert_eq!(
            unwrap_bank(z.as_slice(), &Limits::default()).expect("bsb"),
            fsb
        );
        assert_eq!(
            unwrap_bank(fsb.as_slice(), &Limits::default()).expect("fsb"),
            fsb
        );
        assert_eq!(
            unwrap_bank(wrapped.as_slice(), &Limits::default()).expect("plain bsb"),
            fsb
        );
        let mut wrong = wrapped.clone();
        wrong[0] ^= 1;
        assert!(unwrap_bank(wrong.as_slice(), &Limits::default()).is_err());
        let bad = crate::pak::testutil::zlib(&[9, 0, 0, 0, 1]);
        assert!(unwrap_bank(bad.as_slice(), &Limits::default()).is_err());
    }

    #[test]
    fn wav_header_and_decode_errors() {
        let pcm = Pcm {
            rate: 48_000,
            channels: 2,
            samples: vec![1, -1, 2, -2],
        };
        let w = pcm.to_wav();
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 48_000);
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 8);
        assert_eq!(w.len(), 44 + 8);

        let fsb = build(&[("A", 9, false, 10)]);
        let bank = parse_bank(&fsb, &Limits::default()).expect("bank");
        // No setup id in the sample header: not decodable.
        assert!(bank.decode(&fsb, 0).is_err());
        assert!(bank.decode(&fsb, 5).is_err());
    }

    #[test]
    fn garbage_never_panics() {
        let d = build(&[("A", 9, true, 5), ("B", 9, false, 7)]);
        for cut in 0..d.len() {
            let _ = parse_bank(&d[..cut], &Limits::default());
        }
        for i in 0..d.len() {
            let mut x = d.clone();
            x[i] ^= 0xFF;
            let _ = parse_bank(&x, &Limits::default());
        }
        assert!(parse_bank(b"not a bank at all", &Limits::default()).is_err());
    }
}
