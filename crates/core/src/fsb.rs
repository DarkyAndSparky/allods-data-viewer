//! FMOD sound banks (`.fsb`, FSB5; FSB4 lives in `fsb4.rs`) and their zlib wrapper (`.bsb`).
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
    /// FSB4 sample mode flags (codec, channel layout); 0 for FSB5.
    pub mode: u32,
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
    /// True for an FSB4 bank (version and mode then come from the FSB4 header).
    pub fsb4: bool,
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

fn is_bank(d: &[u8]) -> bool {
    d.starts_with(b"FSB5") || d.starts_with(b"FSB4")
}

/// Returns the FSB5 (or FSB4) bytes of a `.fsb` or `.bsb`: inflates the zlib layer and removes the
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
    if is_bank(&raw) {
        return Ok(raw);
    }
    if raw.get(4..).is_some_and(is_bank) {
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
    if !is_bank(&out) {
        return Err(Error::format("inflated data is not FSB5 or FSB4"));
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
        // More than two channels need WAVE_FORMAT_EXTENSIBLE with a speaker mask.
        let extensible = self.channels > 2;
        let fmt_len: u32 = if extensible { 40 } else { 16 };
        let mut w = Vec::with_capacity(60 + self.samples.len() * 2);
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(20 + fmt_len + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&fmt_len.to_le_bytes());
        w.extend_from_slice(&(if extensible { 0xFFFEu16 } else { 1 }).to_le_bytes());
        w.extend_from_slice(&(self.channels as u16).to_le_bytes());
        w.extend_from_slice(&self.rate.to_le_bytes());
        w.extend_from_slice(&(self.rate * u32::from(block)).to_le_bytes());
        w.extend_from_slice(&block.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        if extensible {
            let mask: u32 = match self.channels {
                6 => 0x3F,
                8 => 0x63F,
                10 => 0x3FF,
                _ => 0,
            };
            w.extend_from_slice(&22u16.to_le_bytes());
            w.extend_from_slice(&16u16.to_le_bytes());
            w.extend_from_slice(&mask.to_le_bytes());
            // KSDATAFORMAT_SUBTYPE_PCM
            w.extend_from_slice(&[
                1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71,
            ]);
        }
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
        if self.fsb4 {
            return crate::fsb4::decode(bank_bytes, s);
        }
        if self.mode == 12 {
            let data = bank_bytes
                .get(s.data_offset..s.data_offset.saturating_add(s.data_len))
                .ok_or_else(|| Error::format("stream data is outside the bank"))?;
            let rate = s
                .frequency
                .ok_or_else(|| Error::unsupported("unknown sample rate"))?;
            let samples = decode_celt(data, rate, s.channels as usize, s.samples)?;
            return Ok(Pcm {
                rate,
                channels: s.channels,
                samples,
            });
        }
        if self.mode != 15 {
            return Err(Error::unsupported(format!(
                "the {} codec (FSB5 mode {}) is not supported yet; the bank can still be saved as a file",
                codec_name(self.mode),
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

/// Magic that starts every FMOD CELT packet.
const CELT_PACKET_MAGIC: u32 = 0xf30d_c317;

/// Splits FMOD's CELT stream (`magic`, `size`, packet) into packets.
fn celt_packets(data: &[u8]) -> Result<Vec<&[u8]>> {
    let mut packets = Vec::new();
    let mut p = 0usize;
    while p + 8 <= data.len() {
        if u32_at(data, p)? != CELT_PACKET_MAGIC {
            break;
        }
        let size = u32_at(data, p + 4)? as usize;
        let body = data
            .get(p + 8..p.saturating_add(8).saturating_add(size))
            .ok_or_else(|| Error::format("CELT packet is cut off"))?;
        packets.push(body);
        p += 8 + size;
    }
    if packets.is_empty() {
        return Err(Error::format("no CELT packets found"));
    }
    Ok(packets)
}

/// Decodes an FSB5 CELT stream. The frame size is not stored in the bank, so it is picked
/// from the usual sizes as the one that explains the number of packets and samples.
fn decode_celt(data: &[u8], rate: u32, channels: usize, samples: u64) -> Result<Vec<i16>> {
    let packets = celt_packets(data)?;
    let count = packets.len() as u64;
    let frame = [512u64, 1024, 256, 2048, 128]
        .into_iter()
        .find(|f| samples.div_ceil(*f) == count)
        .unwrap_or(512);
    let rate = i32::try_from(rate).map_err(|_| Error::unsupported("sample rate is too high"))?;
    let mut dec = crate::celt::Decoder::new(rate, frame as i32, channels)?;
    let want = usize::try_from(samples)
        .ok()
        .and_then(|n| n.checked_mul(channels))
        .ok_or_else(|| Error::limit("sound is too long"))?;
    let mut out: Vec<i16> = Vec::with_capacity(want.min(1 << 26));
    for packet in packets {
        dec.decode(packet, &mut out)?;
        if out.len() >= want {
            break;
        }
    }
    out.truncate(want);
    Ok(out)
}

/// Name of an FSB5 codec number (the bank-level mode of the header).
fn codec_name(mode: u32) -> &'static str {
    match mode {
        1 => "PCM 8-bit",
        2 => "PCM 16-bit",
        3 => "PCM 24-bit",
        4 => "PCM 32-bit",
        5 => "PCM float",
        6 => "GC ADPCM",
        7 => "IMA ADPCM",
        8 => "VAG",
        9 => "HE-VAG",
        10 => "XMA",
        11 => "MPEG",
        12 => "CELT",
        13 => "ATRAC9",
        14 => "XWMA",
        15 => "Vorbis",
        _ => "unknown",
    }
}

/// Parses an FSB5 bank (already unwrapped).
///
/// # Errors
///
/// [`Error::Format`] for a bad header, tables running past the end, or inconsistent
/// sizes; [`Error::Limit`] for an absurd stream count.
pub fn parse_bank(d: &[u8], limits: &Limits) -> Result<Bank> {
    if d.starts_with(b"FSB4") {
        return crate::fsb4::parse(d, limits);
    }
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
        // Bits 5..6: channel layout (0 = mono, 1 = stereo, 2 = 6 channels, 3 = 8 channels).
        let mut channels = [1, 2, 6, 8][((bits >> 5) & 3) as usize];
        let samples = bits >> 34;
        // Bits 7..33 hold the data offset in units of 32 bytes; bit 6 belongs to the channel
        // field and is not part of the offset.
        let data_offset = data_start
            .checked_add((((bits >> 7) & 0x07FF_FFFF) as usize) * 32)
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
            mode: 0,
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
        fsb4: false,
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
    fn data_offset_ignores_bit_6_of_the_mode_word() {
        // Real six-channel music banks set bit 6 with the data right after the tables
        // (offset 0); reading it as part of the offset skipped 16 bytes.
        let mut d = build(&[("a", 8, false, 1000), ("b", 8, false, 1000)]);
        let first = u64::from_le_bytes(d[60..68].try_into().expect("8 bytes")) | (1 << 6);
        let second = u64::from_le_bytes(d[68..76].try_into().expect("8 bytes")) | (3 << 7);
        d[60..68].copy_from_slice(&first.to_le_bytes());
        d[68..76].copy_from_slice(&second.to_le_bytes());
        d.extend_from_slice(&[0; 200]);
        let b = parse_bank(&d, &Limits::default()).expect("parse");
        let data_start = d.len() - 200;
        assert_eq!(b.streams[0].data_offset, data_start);
        assert_eq!(b.streams[1].data_offset, data_start + 96);
        assert_eq!(b.streams[0].channels, 6);
    }

    #[test]
    fn six_channel_wav_uses_the_extensible_header() {
        let pcm = Pcm {
            rate: 44_100,
            channels: 6,
            samples: vec![7; 12],
        };
        let w = pcm.to_wav();
        assert_eq!(&w[20..22], &0xFFFEu16.to_le_bytes());
        assert_eq!(&w[40..44], &0x3Fu32.to_le_bytes());
        assert_eq!(w.len(), 20 + 40 + 8 + 24);
        assert_eq!(
            u32::from_le_bytes(w[4..8].try_into().expect("4")) as usize,
            w.len() - 8
        );
    }

    #[test]
    fn celt_bank_without_packets_is_a_format_error() {
        let mut d = build(&[("v", 8, true, 1000)]);
        d[24..28].copy_from_slice(&12u32.to_le_bytes());
        let b = parse_bank(&d, &Limits::default()).expect("parse");
        let e = b.decode(&d, 0).expect_err("no packets");
        assert!(e.to_string().contains("CELT"), "{e}");
    }

    fn celt_stream(packets: usize, size: usize) -> Vec<u8> {
        let mut d = Vec::new();
        for _ in 0..packets {
            d.extend_from_slice(&CELT_PACKET_MAGIC.to_le_bytes());
            d.extend_from_slice(&(size as u32).to_le_bytes());
            d.extend(std::iter::repeat_n(0u8, size));
        }
        d
    }

    #[test]
    fn celt_stream_decodes_framed_packets() {
        let d = celt_stream(3, 40);
        let pcm = decode_celt(&d, 44_100, 2, 1536).expect("decode");
        assert_eq!(pcm.len(), 3072);
        // the length in the bank header wins over whole frames
        let pcm = decode_celt(&celt_stream(2, 40), 44_100, 2, 1000).expect("decode");
        assert_eq!(pcm.len(), 2000);
    }

    #[test]
    fn celt_stream_picks_the_frame_size_from_the_packet_count() {
        let pcm = decode_celt(&celt_stream(2, 60), 44_100, 1, 2048).expect("decode");
        assert_eq!(pcm.len(), 2048);
    }

    #[test]
    fn celt_stream_cut_off_or_unframed_is_an_error() {
        let d = celt_stream(2, 40);
        assert!(decode_celt(&d[..d.len() - 5], 44_100, 2, 1024).is_err());
        assert!(decode_celt(&[0u8; 64], 44_100, 2, 1024).is_err());
        assert!(decode_celt(&[], 44_100, 2, 1024).is_err());
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
