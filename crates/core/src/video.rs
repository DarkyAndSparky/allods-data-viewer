//! Decoding of `.ogv` videos (an Ogg container with one Theora stream) for the in-window player.
//!
//! The container is read here (pages and packets, bounds-checked, only the first Theora stream
//! is used, page checksums are not verified); the pictures come from the `theora-rs` decoder.
//! Frames are returned as planar YCbCr, cropped to the picture region; the colour conversion is
//! done by the player. See `docs/formats/media.md`.

use crate::error::{Error, Result};
use crate::fsb::Pcm;
use crate::limits::Limits;
use theora_rs::{
    DecoderContext, HeaderParser, OggPacket, PixelFmt, th_decode_packetin, th_decode_ycbcr_out,
};

/// Largest picture side accepted (pixels); the area is limited too.
const MAX_SIDE: u32 = 8192;
const MAX_AREA: u64 = 33_554_432;

/// One decoded picture: three planes without row padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Picture width in pixels.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// Luma plane (`width` x `height`), then Cb and Cr planes (`chroma_width` x `chroma_height`).
    pub planes: [Vec<u8>; 3],
}

/// What the player needs to know to size and convert the frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoMeta {
    /// Picture width in pixels.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// Horizontal chroma subsampling shift (1 for 4:2:0 and 4:2:2, 0 for 4:4:4).
    pub chroma_shift_x: u32,
    /// Vertical chroma subsampling shift (1 for 4:2:0, 0 otherwise).
    pub chroma_shift_y: u32,
    /// Frame rate numerator.
    pub fps_num: u32,
    /// Frame rate denominator.
    pub fps_den: u32,
    /// Number of frames in the file.
    pub frames: u64,
}

impl VideoMeta {
    /// Width of a chroma plane.
    #[must_use]
    pub fn chroma_width(&self) -> u32 {
        (self.width + (1 << self.chroma_shift_x) - 1) >> self.chroma_shift_x
    }

    /// Height of a chroma plane.
    #[must_use]
    pub fn chroma_height(&self) -> u32 {
        (self.height + (1 << self.chroma_shift_y) - 1) >> self.chroma_shift_y
    }

    /// Size of one frame in bytes (the three planes one after another).
    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        let luma = self.width as usize * self.height as usize;
        let chroma = self.chroma_width() as usize * self.chroma_height() as usize;
        luma + 2 * chroma
    }
}

/// An opened video that decodes its frames one after another.
pub struct VideoStream {
    meta: VideoMeta,
    pic_x: u32,
    pic_y: u32,
    parser: HeaderParser,
    packets: Vec<Vec<u8>>,
    decoder: Option<DecoderContext>,
    next: usize,
}

/// Splits the Ogg pages of `d` into the packets of the first Theora stream (with the granule
/// position of the page for the last packet that ends on it, `-1` for the others).
/// Packets of one Ogg stream with the granule position of the page that ends each (else -1).
type Packets = Vec<(Vec<u8>, i64)>;

fn theora_packets(d: &[u8], limits: &Limits) -> Result<Packets> {
    let out = ogg_packets(d, limits, |p| {
        p.first() == Some(&0x80) && p.get(1..7) == Some(b"theora".as_slice())
    })?;
    out.ok_or_else(|| Error::format("no Theora stream in the file"))
}

/// Packets (with the granule position of the page they end a page on, else -1) of the first
/// logical Ogg stream whose first packet satisfies `starts`; `None` when there is none.
fn ogg_packets(
    d: &[u8],
    limits: &Limits,
    starts: impl Fn(&[u8]) -> bool,
) -> Result<Option<Packets>> {
    let cap = usize::try_from(limits.max_entry_size).unwrap_or(usize::MAX);
    let mut out: Vec<(Vec<u8>, i64)> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut serial: Option<u32> = None;
    let mut pos = 0usize;
    while let Some(head) = d.get(pos..pos.saturating_add(27)) {
        if &head[..4] != b"OggS" {
            return Err(Error::format(format!("no Ogg page at offset {pos}")));
        }
        let granule = i64::from_le_bytes([
            head[6], head[7], head[8], head[9], head[10], head[11], head[12], head[13],
        ]);
        let page_serial = u32::from_le_bytes([head[14], head[15], head[16], head[17]]);
        let segment_count = usize::from(head[26]);
        let table_end = pos + 27 + segment_count;
        let segments = d
            .get(pos + 27..table_end)
            .ok_or_else(|| Error::format("Ogg segment table runs past the end"))?;
        let body: usize = segments.iter().map(|&s| usize::from(s)).sum();
        let page_end = table_end
            .checked_add(body)
            .filter(|&e| e <= d.len())
            .ok_or_else(|| Error::format("Ogg page runs past the end"))?;
        let mut at = table_end;
        let mine = match serial {
            Some(s) => s == page_serial,
            None => {
                let starts_mine = head[5] & 2 != 0 && d.get(at..at + 7).is_some_and(&starts);
                if starts_mine {
                    serial = Some(page_serial);
                }
                starts_mine
            }
        };
        if mine {
            for (i, &seg) in segments.iter().enumerate() {
                let len = usize::from(seg);
                if cur.len() + len > cap {
                    return Err(Error::limit("video packet too large"));
                }
                cur.extend_from_slice(&d[at..at + len]);
                at += len;
                if seg < 255 {
                    if out.len() as u64 >= limits.max_entries {
                        return Err(Error::limit("too many video packets"));
                    }
                    let last_of_page = i + 1 == segments.len();
                    out.push((
                        std::mem::take(&mut cur),
                        if last_of_page { granule } else { -1 },
                    ));
                }
            }
        }
        pos = page_end;
    }
    Ok(serial.map(|_| out))
}

/// The Vorbis soundtrack of an `.ogv` file as 16-bit PCM, or `None` when it has none.
///
/// # Errors
/// [`Error::Format`] for damaged containers or Vorbis headers, [`Error::Unsupported`] for
/// Vorbis features the decoder lacks, [`Error::Limit`] for oversize packets or sound.
pub fn ogg_audio(data: &[u8], limits: &Limits) -> Result<Option<Pcm>> {
    let Some(packets) = ogg_packets(data, limits, |p| {
        p.first() == Some(&1) && p.get(1..7) == Some(b"vorbis".as_slice())
    })?
    else {
        return Ok(None);
    };
    let bad = |m: &str| Error::format(format!("Vorbis in Ogg: {m}"));
    let id = packets.first().map(|p| p.0.as_slice()).unwrap_or_default();
    if id.len() < 30 {
        return Err(bad("identification header is too short"));
    }
    let channels = usize::from(id[11]);
    let rate = u32::from_le_bytes([id[12], id[13], id[14], id[15]]);
    let (e0, e1) = (u32::from(id[28] & 15), u32::from(id[28] >> 4));
    if rate == 0 || e0 > 13 || e1 > 13 {
        return Err(bad("bad identification header"));
    }
    let setup = packets
        .get(2)
        .map(|p| p.0.as_slice())
        .ok_or_else(|| bad("no setup header"))?;
    let mut dec = crate::vorbis::Decoder::with_blocks(setup, channels, 1 << e0, 1 << e1)?;
    let cap = usize::try_from(limits.max_entry_size / 2).unwrap_or(usize::MAX);
    let mut samples: Vec<i16> = Vec::new();
    for (pkt, _) in packets.iter().skip(3) {
        let Ok(out) = dec.packet(pkt) else { continue };
        let n = out.first().map_or(0, Vec::len);
        if samples.len().saturating_add(n * channels) > cap {
            return Err(Error::limit("the soundtrack is too long"));
        }
        for i in 0..n {
            for ch in &out {
                samples.push((ch[i] * 32768.0).round().clamp(-32768.0, 32767.0) as i16);
            }
        }
    }
    if samples.is_empty() {
        return Ok(None);
    }
    Ok(Some(Pcm {
        rate,
        channels: channels as u32,
        samples,
    }))
}

fn theora_err(what: &str, e: theora_rs::TheoraError) -> Error {
    Error::format(format!("Theora {what}: {e}"))
}

impl VideoStream {
    /// Reads the container and the Theora headers of `.ogv` file bytes.
    ///
    /// # Errors
    /// [`Error::Format`] for damaged containers or headers, [`Error::Limit`] for oversize
    /// packets or pictures, [`Error::Unsupported`] for a pixel format Theora reserves.
    pub fn open(data: &[u8], limits: &Limits) -> Result<Self> {
        let all = theora_packets(data, limits)?;
        let mut parser = HeaderParser::new();
        let mut used = 0usize;
        for (i, (bytes, granule)) in all.iter().enumerate() {
            used = i + 1;
            let op = OggPacket {
                packet: bytes.clone(),
                b_o_s: i == 0,
                e_o_s: false,
                granulepos: *granule,
                packetno: i as i64,
            };
            parser.push(&op).map_err(|e| theora_err("header", e))?;
            if parser.is_ready() {
                break;
            }
        }
        if !parser.is_ready() {
            return Err(Error::format("the Theora headers are incomplete"));
        }
        let info = &parser.info;
        let (w, h) = (info.pic_width, info.pic_height);
        if w == 0
            || h == 0
            || w > MAX_SIDE
            || h > MAX_SIDE
            || u64::from(w) * u64::from(h) > MAX_AREA
        {
            return Err(Error::limit(format!(
                "picture size {w}x{h} is out of range"
            )));
        }
        if info.fps_denominator == 0 || info.fps_numerator == 0 {
            return Err(Error::format("the frame rate is zero"));
        }
        let (sx, sy) = match info.pixel_fmt {
            PixelFmt::Pf420 => (1, 1),
            PixelFmt::Pf422 => (1, 0),
            PixelFmt::Pf444 => (0, 0),
            PixelFmt::Reserved => return Err(Error::unsupported("reserved Theora pixel format")),
        };
        let meta_base = (w, h, sx, sy, info.fps_numerator, info.fps_denominator);
        let (pic_x, pic_y) = (info.pic_x, info.pic_y);
        let packets: Vec<Vec<u8>> = all.into_iter().skip(used).map(|(b, _)| b).collect();
        let meta = VideoMeta {
            width: meta_base.0,
            height: meta_base.1,
            chroma_shift_x: meta_base.2,
            chroma_shift_y: meta_base.3,
            fps_num: meta_base.4,
            fps_den: meta_base.5,
            frames: packets.len() as u64,
        };
        let mut stream = Self {
            meta,
            pic_x,
            pic_y,
            parser,
            packets,
            decoder: None,
            next: 0,
        };
        stream.rewind()?;
        Ok(stream)
    }

    /// Size, subsampling, frame rate and frame count.
    #[must_use]
    pub fn meta(&self) -> VideoMeta {
        self.meta
    }

    /// Index of the frame the next call of [`next_frame`](Self::next_frame) returns.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.next as u64
    }

    /// Starts again from the first frame.
    ///
    /// # Errors
    /// Fails when the decoder cannot be allocated.
    pub fn rewind(&mut self) -> Result<()> {
        self.decoder = Some(
            self.parser
                .decoder()
                .map_err(|e| theora_err("decoder", e))?,
        );
        self.next = 0;
        Ok(())
    }

    /// Decodes the next frame, `None` after the last one.
    ///
    /// # Errors
    /// [`Error::Format`] when a packet cannot be decoded; the decoder state is dropped and the
    /// next call starts from the first frame again.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        if self.decoder.is_none() {
            self.rewind()?;
        }
        let Some(bytes) = self.packets.get(self.next) else {
            return Ok(None);
        };
        let Some(decoder) = self.decoder.as_mut() else {
            return Err(Error::format("the Theora decoder is not available"));
        };
        let op = OggPacket {
            packet: bytes.clone(),
            b_o_s: false,
            e_o_s: false,
            granulepos: -1,
            packetno: self.next as i64 + 3,
        };
        // The decoder is a translation of a C library and works on hostile data: a panic inside it
        // becomes an error and drops the decoder state.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            th_decode_packetin(decoder, &op)?;
            th_decode_ycbcr_out(decoder)
        }));
        let planes = match result {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                self.decoder = None;
                return Err(theora_err("frame", e));
            }
            Err(_) => {
                self.decoder = None;
                return Err(Error::format(
                    "the Theora decoder failed on a damaged frame",
                ));
            }
        };
        self.next += 1;
        let m = self.meta;
        let crops = [
            (self.pic_x, self.pic_y, m.width, m.height),
            (
                self.pic_x >> m.chroma_shift_x,
                self.pic_y >> m.chroma_shift_y,
                m.chroma_width(),
                m.chroma_height(),
            ),
        ];
        let mut out: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for (k, plane) in planes.iter().enumerate() {
            let (x0, y0, w, h) = crops[usize::from(k > 0)];
            let stride = usize::try_from(plane.stride).unwrap_or(0);
            let mut buf = Vec::with_capacity(w as usize * h as usize);
            for row in 0..h as usize {
                let start = plane
                    .data_offset
                    .saturating_add((y0 as usize + row).saturating_mul(stride))
                    .saturating_add(x0 as usize);
                let line = plane
                    .data
                    .get(start..start.saturating_add(w as usize))
                    .ok_or_else(|| Error::format("decoded plane is smaller than the picture"))?;
                buf.extend_from_slice(line);
            }
            out[k] = buf;
        }
        Ok(Some(Frame {
            width: m.width,
            height: m.height,
            planes: out,
        }))
    }
}

impl Frame {
    /// The planes one after another, as the player expects them.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.planes.iter().map(Vec::len).sum());
        for p in &self.planes {
            v.extend_from_slice(p);
        }
        v
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    /// The tiny test video of `video_fixture.rs` as bytes.
    pub fn tiny_ogv() -> Vec<u8> {
        unhex(crate::video_fixture::TINY_OGV_HEX)
    }

    /// The test video with a Vorbis soundtrack.
    pub fn audio_ogv() -> Vec<u8> {
        unhex(crate::video_fixture::AUDIO_OGV_HEX)
    }

    fn unhex(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .chunks(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap_or("0"), 16).unwrap_or(0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::{audio_ogv, tiny_ogv};
    use super::*;

    #[test]
    fn decodes_every_frame_with_the_declared_geometry() {
        let d = tiny_ogv();
        let mut v = VideoStream::open(&d, &Limits::default()).expect("open");
        let m = v.meta();
        assert_eq!((m.width, m.height, m.frames), (32, 32, 3));
        assert_eq!(
            (m.chroma_width(), m.chroma_height(), m.frame_bytes()),
            (16, 16, 32 * 32 + 2 * 256)
        );
        let mut first = None;
        for i in 0..3 {
            let f = v.next_frame().expect("frame").expect("some");
            assert_eq!(f.planes[0].len(), 32 * 32);
            assert_eq!(f.planes[1].len(), 256);
            if i == 0 {
                first = Some(f.planes[0].clone());
            }
        }
        assert!(v.next_frame().expect("end").is_none());
        // The test pattern is not flat: the first frame has bright and dark luma.
        let luma = first.expect("first");
        let (lo, hi) = (
            luma.iter().min().copied().unwrap_or(0),
            luma.iter().max().copied().unwrap_or(0),
        );
        assert!(hi - lo > 60, "luma range {lo}..{hi}");
    }

    #[test]
    fn rewind_replays_the_same_frames() {
        let d = tiny_ogv();
        let mut v = VideoStream::open(&d, &Limits::default()).expect("open");
        let a = v.next_frame().expect("frame").expect("some");
        v.next_frame().expect("frame");
        v.rewind().expect("rewind");
        assert_eq!(v.position(), 0);
        assert_eq!(v.next_frame().expect("frame").expect("some"), a);
    }

    #[test]
    fn damaged_files_are_errors_not_panics() {
        let d = tiny_ogv();
        for cut in [0, 10, 26, 40, 100] {
            assert!(
                VideoStream::open(&d[..cut.min(d.len())], &Limits::default()).is_err(),
                "cut {cut}"
            );
        }
        assert!(
            VideoStream::open(b"not an ogg file at all, just text", &Limits::default()).is_err()
        );
        // Every single flipped byte may fail or decode, but must never panic.
        for at in (0..d.len()).step_by(7) {
            let mut bad = d.clone();
            bad[at] ^= 0xA5;
            if let Ok(mut v) = VideoStream::open(&bad, &Limits::default()) {
                while let Ok(Some(_)) = v.next_frame() {}
            }
        }
    }

    #[test]
    fn oversize_picture_is_refused() {
        let mut d = tiny_ogv();
        // The identification header starts at offset 28 (page header 27 + 1 segment): width is a
        // 24-bit field at byte 14 of the packet, height follows.
        let w = 28 + 14;
        d[w..w + 3].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
        assert!(VideoStream::open(&d, &Limits::default()).is_err());
    }

    #[test]
    fn soundtrack_is_decoded_and_silent_videos_have_none() {
        let pcm = ogg_audio(&audio_ogv(), &Limits::default())
            .expect("audio")
            .expect("soundtrack");
        assert_eq!((pcm.rate, pcm.channels), (16_000, 1));
        let secs = pcm.samples.len() as f64 / 16_000.0;
        assert!((0.25..=0.45).contains(&secs), "{secs} s");
        // A 440 Hz tone crosses zero about 880 times a second.
        let crossings = pcm
            .samples
            .windows(2)
            .filter(|w| (w[0] < 0) != (w[1] < 0))
            .count() as f64;
        let hz = crossings / 2.0 / secs;
        assert!((380.0..=500.0).contains(&hz), "{hz} Hz");
        assert!(
            ogg_audio(&tiny_ogv(), &Limits::default())
                .expect("no audio")
                .is_none()
        );
        // The picture still opens from the file with a soundtrack.
        assert!(VideoStream::open(&audio_ogv(), &Limits::default()).is_ok());
    }

    #[test]
    fn damaged_soundtracks_are_errors_not_panics() {
        let whole = audio_ogv();
        for cut in [0, 30, 200, 1000, whole.len() / 2, whole.len() - 1] {
            let _ = ogg_audio(&whole[..cut], &Limits::default());
        }
        let mut bad = whole.clone();
        for b in bad.iter_mut().skip(300).step_by(7) {
            *b ^= 0x5A;
        }
        let _ = ogg_audio(&bad, &Limits::default());
    }
}
