//! Header reader for `.ogv` videos (an Ogg container with one Theora stream, see
//! `docs/formats/media.md`). Only the identification header and the position of the last page are
//! read; no frame is decoded.

use crate::error::{Error, Result};

/// What the file says about its video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoInfo {
    /// Width of the picture in pixels.
    pub width: u32,
    /// Height of the picture in pixels.
    pub height: u32,
    /// Frame rate numerator.
    pub fps_num: u32,
    /// Frame rate denominator.
    pub fps_den: u32,
    /// Number of frames, when the last page of the stream carries a position.
    pub frames: Option<u64>,
    /// Theora version `(major, minor, revision)`.
    pub version: (u8, u8, u8),
}

impl VideoInfo {
    /// Frames per second.
    #[must_use]
    pub fn fps(&self) -> f64 {
        if self.fps_den == 0 {
            0.0
        } else {
            f64::from(self.fps_num) / f64::from(self.fps_den)
        }
    }

    /// Length in seconds, when the number of frames is known.
    #[must_use]
    pub fn seconds(&self) -> Option<f64> {
        let fps = self.fps();
        self.frames.filter(|_| fps > 0.0).map(|f| f as f64 / fps)
    }
}

struct Page<'a> {
    granule: i64,
    serial: u32,
    body: &'a [u8],
    first_packet_starts: bool,
}

/// Reads one Ogg page at `at`; returns it and the offset of the next page.
fn page(d: &[u8], at: usize) -> Option<(Page<'_>, usize)> {
    let h = d.get(at..at.checked_add(27)?)?;
    if &h[0..4] != b"OggS" || h[4] != 0 {
        return None;
    }
    let granule = i64::from_le_bytes(h[6..14].try_into().ok()?);
    let serial = u32::from_le_bytes(h[14..18].try_into().ok()?);
    let nseg = usize::from(h[26]);
    let table = d.get(at + 27..at + 27 + nseg)?;
    let len: usize = table.iter().map(|&b| usize::from(b)).sum();
    let start = at + 27 + nseg;
    let end = start.checked_add(len)?;
    Some((
        Page {
            granule,
            serial,
            body: d.get(start..end)?,
            first_packet_starts: h[5] & 1 == 0,
        },
        end,
    ))
}

fn be(d: &[u8], at: usize, n: usize) -> Option<u32> {
    d.get(at..at + n)
        .map(|b| b.iter().fold(0u32, |a, &x| (a << 8) | u32::from(x)))
}

/// Reads the Theora identification header and the length of the video.
///
/// # Errors
/// Returns an error when the data is not an Ogg file with a Theora stream.
pub fn parse(d: &[u8]) -> Result<VideoInfo> {
    let (first, mut next) = page(d, 0).ok_or_else(|| Error::format("not an Ogg file"))?;
    let h = first.body;
    if !first.first_packet_starts || h.get(..7) != Some(&[0x80, b't', b'h', b'e', b'o', b'r', b'a'])
    {
        return Err(Error::format("the first Ogg stream is not Theora"));
    }
    let bad = || Error::format("Theora header is truncated");
    let (width, height) = (be(h, 14, 3).ok_or_else(bad)?, be(h, 17, 3).ok_or_else(bad)?);
    let (fps_num, fps_den) = (be(h, 22, 4).ok_or_else(bad)?, be(h, 26, 4).ok_or_else(bad)?);
    let packed = be(h, 40, 2).ok_or_else(bad)?;
    let shift = (packed >> 5) & 0x1f; // KFGSHIFT
    let version = (h[7], h[8], h[9]);
    if width == 0 || height == 0 {
        return Err(Error::format("Theora picture size is zero"));
    }
    // Walk the pages: the last page of this stream tells how many frames there are.
    let mut last = None;
    let mut guard = 0usize;
    while let Some((p, n)) = page(d, next) {
        if p.serial == first.serial && p.granule >= 0 {
            last = Some(p.granule);
        }
        next = n;
        guard += 1;
        if guard > d.len() / 27 + 1 {
            break;
        }
    }
    let frames = last.and_then(|g| {
        let g = u64::try_from(g).ok()?;
        let key = g >> shift;
        let off = g & ((1u64 << shift) - 1);
        // From version 3.2.1 the position counts the frame that ends at the page, so it is
        // one more than the index of the last frame.
        let plus = u64::from(version >= (3, 2, 1));
        Some(key + off + if plus == 1 { 0 } else { 1 })
    });
    Ok(VideoInfo {
        width,
        height,
        fps_num,
        fps_den,
        frames,
        version,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny Theora file: 640 x 480, 25 frames per second, 12 frames.
    pub(crate) fn sample() -> Vec<u8> {
        let mut f = page_bytes(2, 0, 7, &ident(640, 480, 25, 1, 6));
        f.extend(page_bytes(4, (10 << 6) | 2, 7, b"y"));
        f
    }

    fn page_bytes(header_type: u8, granule: i64, serial: u32, body: &[u8]) -> Vec<u8> {
        assert!(body.len() < 255);
        let mut p = b"OggS".to_vec();
        p.extend([0, header_type]);
        p.extend(granule.to_le_bytes());
        p.extend(serial.to_le_bytes());
        p.extend([0u8; 8]); // sequence number and CRC are not checked
        p.push(1);
        p.push(body.len() as u8);
        p.extend(body);
        p
    }

    fn ident(w: u32, h: u32, num: u32, den: u32, shift: u32) -> Vec<u8> {
        let mut b = vec![0x80];
        b.extend(b"theora");
        b.extend([3, 2, 1]);
        b.extend(((w / 16) as u16).to_be_bytes());
        b.extend(((h / 16) as u16).to_be_bytes());
        b.extend(&w.to_be_bytes()[1..]);
        b.extend(&h.to_be_bytes()[1..]);
        b.extend([0, 0]);
        b.extend(num.to_be_bytes());
        b.extend(den.to_be_bytes());
        b.extend([0; 6]); // pixel aspect
        b.push(0);
        b.extend([0; 3]);
        b.extend((((10u16) << 10) | ((shift as u16) << 5)).to_be_bytes());
        b
    }

    #[test]
    fn reads_size_rate_and_length() {
        let mut f = page_bytes(2, 0, 7, &ident(1280, 720, 30000, 1001, 6));
        f.extend(page_bytes(0, (3 << 6) | 4, 7, b"x"));
        f.extend(page_bytes(4, (9 << 6) | 2, 7, b"y"));
        let v = parse(&f).expect("parse");
        assert_eq!((v.width, v.height, v.version), (1280, 720, (3, 2, 1)));
        assert_eq!((v.fps_num, v.fps_den), (30000, 1001));
        assert_eq!(v.frames, Some(11));
        let s = v.seconds().expect("seconds");
        assert!((s - 11.0 * 1001.0 / 30000.0).abs() < 1e-9);
    }

    #[test]
    fn other_codecs_and_damage_are_rejected() {
        assert!(parse(b"").is_err());
        assert!(parse(b"RIFF....").is_err());
        let f = page_bytes(2, 0, 1, b"\x01vorbis....");
        assert!(parse(&f).is_err());
        let good = page_bytes(2, 0, 7, &ident(640, 480, 25, 1, 6));
        for cut in 0..good.len() {
            let _ = parse(&good[..cut]);
        }
        assert!(parse(&good[..good.len() - 10]).is_err());
        let zero = page_bytes(2, 0, 7, &ident(0, 480, 25, 1, 6));
        assert!(parse(&zero).is_err());
    }

    #[test]
    fn a_header_without_a_length_still_parses() {
        let f = page_bytes(2, 0, 7, &ident(640, 480, 25, 1, 6));
        let v = parse(&f).expect("parse");
        assert_eq!(v.frames, None);
        assert_eq!(v.seconds(), None);
    }
}
