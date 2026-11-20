//! Guessing the size and format of a texture `.bin` that has no `.xdb` or database record.
//!
//! The `.bin` stores neither width, height nor pixel format. This module repeats the
//! approach of the community tool "Allods Online Texture Viewer" (Yuri Grachev / Altair,
//! used with the author's permission; see `docs/REFERENCES.md`): the number of pixels follows
//! from the byte size of a mip level, every power-of-two pair with that product is a
//! candidate, and the block format is guessed from typical alpha blocks. The code here is
//! written from the described behaviour, not copied. A guess can be wrong: a picture
//! 256x1024 has the same byte size as 512x512 and 1024x256.

use crate::texture::{Dxt, Level};

/// `(width, height)` pair.
pub type Size = (u32, u32);

/// 8-byte alpha blocks that suggest DXT5 (compared against bytes 0..8 of a 16-byte block).
const DXT5_ALPHA: [&[u8]; 5] = [
    &[0x04, 0x29, 0x03, 0x29, 0xaa, 0xaa, 0xaa, 0xaa],
    &[0xf3, 0x9c, 0xd3, 0x9c, 0xaa, 0xaa, 0xaa, 0xaa],
    &[0xff, 0xff, 0, 0, 0, 0, 0, 0],
    &[0x00, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    &[0x00, 0x05, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
];
/// A 6-byte pattern compared against bytes 2..8 of the alpha block.
const DXT5_ALPHA_TAIL: [u8; 6] = [0x49, 0x92, 0x24, 0x49, 0x92, 0x24];
/// Fully opaque DXT3 alpha block.
const DXT3_ALPHA: [u8; 8] = [0xff; 8];

fn hits(data: &[u8], pat: &[u8]) -> usize {
    let start = 8 - pat.len();
    (start..data.len())
        .step_by(16)
        .filter(|&j| data.get(j..j + pat.len()) == Some(pat))
        .count()
}

/// Guesses DXT1, DXT3 or DXT5 from the alpha blocks of all levels.
#[must_use]
pub fn deduce_format(levels: &[Level]) -> Dxt {
    let (mut d3, mut d5) = (0usize, 0usize);
    for l in levels {
        d3 += hits(&l.data, &DXT3_ALPHA);
        d5 += DXT5_ALPHA.iter().map(|p| hits(&l.data, p)).sum::<usize>()
            + hits(&l.data, &DXT5_ALPHA_TAIL);
    }
    if d3 > 0 {
        if d3 > d5 { Dxt::Dxt3 } else { Dxt::Dxt5 }
    } else if d5 > 0 {
        Dxt::Dxt5
    } else {
        Dxt::Dxt1
    }
}

fn pow2(v: u64) -> bool {
    v != 0 && v & (v - 1) == 0
}

/// Candidate sizes of mip level 0, ordered by width (a pair may appear twice).
///
/// `len` is the byte size of the level with index `id`.
#[must_use]
pub fn candidate_sizes(len: usize, id: i32, format: Dxt) -> Vec<Size> {
    let per = if format == Dxt::Dxt1 { 2 } else { 1 };
    sizes_for_pixels((len as u64 * per) << (2 * id.clamp(0, 15) as u32))
}

/// Candidate sizes of mip level 0 for a pixel count of level 0 (power-of-two pairs).
#[must_use]
pub fn sizes_for_pixels(px: u64) -> Vec<Size> {
    if px == 0 {
        return Vec::new();
    }
    let root = (px as f64).sqrt() as u64;
    let (w0, h0) = if pow2(root) {
        (root, root)
    } else {
        let p = root.next_power_of_two();
        let h = px / p;
        if !pow2(h) {
            return Vec::new();
        }
        (p, h)
    };
    let mut out: Vec<(u64, u64)> = Vec::new();
    let (mut w, mut h) = (w0, h0);
    loop {
        if pow2(w) && pow2(h) {
            out.push((w, h));
        }
        w /= 2;
        h *= 2;
        if !(w * h == px && w >= 4 && h <= 4096) {
            break;
        }
    }
    let (mut w, mut h) = (w0, h0);
    loop {
        if pow2(w) && pow2(h) {
            out.push((w, h));
        }
        w *= 2;
        h /= 2;
        if !(w * h == px && w <= 4096 && h >= 4) {
            break;
        }
    }
    out.sort_by_key(|&(w, _)| w);
    out.into_iter().map(|(w, h)| (w as u32, h as u32)).collect()
}

/// Index of the most likely candidate, given the number of mip slots (highest index + 1).
#[must_use]
pub fn default_index(list: &[Size], mip_slots: usize) -> usize {
    if mip_slots > 1 {
        let want = 1u64 << (mip_slots + 1).min(40);
        if let Some(i) = list.iter().position(|&(w, h)| u64::from(w.min(h)) == want) {
            return i;
        }
    }
    let mut i = list.len() / 2;
    if list.len().is_multiple_of(2) {
        i = i.saturating_sub(1);
    }
    i
}

/// A complete guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guess {
    /// Block format.
    pub format: Dxt,
    /// All candidate sizes.
    pub candidates: Vec<Size>,
    /// Index of the chosen candidate.
    pub index: usize,
}

impl Guess {
    /// The chosen size.
    #[must_use]
    pub fn size(&self) -> Size {
        self.candidates[self.index.min(self.candidates.len() - 1)]
    }
}

/// Guesses format and size from the levels of a texture. `format` overrides the format guess.
#[must_use]
pub fn guess(levels: &[Level], format: Option<Dxt>) -> Option<Guess> {
    let first = levels.iter().min_by_key(|l| l.id)?;
    let format = format.unwrap_or_else(|| deduce_format(levels));
    let candidates = candidate_sizes(first.data.len(), first.id, format);
    if candidates.is_empty() {
        return None;
    }
    let slots = levels.iter().map(|l| l.id.max(0) as usize + 1).max()?;
    let index = default_index(&candidates, slots);
    Some(Guess {
        format,
        candidates,
        index,
    })
}

/// Counts fully transparent columns on the right and rows at the bottom of an RGBA picture.
#[must_use]
pub fn detect_padding(rgba: &[u8], width: u32, height: u32) -> (u32, u32) {
    let (w, h) = (width as usize, height as usize);
    let (mut max_x, mut max_y, mut any) = (0usize, 0usize, false);
    for y in 0..h {
        for x in 0..w {
            if rgba.get((y * w + x) * 4 + 3).copied().unwrap_or(0) > 0 {
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                any = true;
            }
        }
    }
    if !any {
        return (0, 0);
    }
    ((w - 1 - max_x) as u32, (h - 1 - max_y) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_for_512x256_dxt5() {
        let c = candidate_sizes(131_072, 0, Dxt::Dxt5);
        assert!(c.contains(&(512, 256)) && c.contains(&(256, 512)));
        assert!(c.windows(2).all(|p| p[0].0 <= p[1].0));
    }

    #[test]
    fn square_texture_and_dxt1_factor() {
        let c = candidate_sizes(512 * 512 / 2, 0, Dxt::Dxt1);
        assert!(c.contains(&(512, 512)));
    }

    #[test]
    fn non_power_of_two_product_has_no_candidates() {
        assert!(candidate_sizes(3 * 1024, 0, Dxt::Dxt5).is_empty());
    }

    #[test]
    fn opaque_dxt3_blocks_are_detected() {
        let mut d = vec![0u8; 64];
        for b in d.chunks_mut(16) {
            b[..8].fill(0xff);
        }
        let l = Level { id: 0, data: d };
        assert_eq!(deduce_format(&[l]), Dxt::Dxt3);
    }

    #[test]
    fn plain_data_is_dxt1() {
        let l = Level {
            id: 0,
            data: vec![1u8; 64],
        };
        assert_eq!(deduce_format(&[l]), Dxt::Dxt1);
    }

    #[test]
    fn padding_counts_transparent_edges() {
        let mut p = vec![0u8; 4 * 4 * 4];
        p[3] = 255; // pixel (0,0)
        assert_eq!(detect_padding(&p, 4, 4), (3, 3));
    }

    #[test]
    fn default_index_uses_mip_count() {
        let list = vec![(128, 512), (256, 256), (512, 128)];
        assert_eq!(default_index(&list, 7), 1);
    }
}
