//! Header of `X.<Clip>.(SkeletalAnimation).bin` (see `docs/formats/skeletal-animation.md`).
//!
//! [`parse`] reads the header: frame rate, frame count and bone count. [`decode`] reads the
//! per-bone tracks into one pose (position, scale, rotation quaternion) per frame,
//! [`bone_names`] reads the track names and [`material_tracks`] the texture scroll and
//! transparency tracks.

use crate::error::{Error, Result};

/// What the header of an animation clip says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationInfo {
    /// Frames per second.
    pub fps: u16,
    /// Number of frames (the first and the last one are both counted).
    pub frames: u16,
    /// Number of bones the clip animates, as the table at the start of the file counts them.
    pub bones: u32,
}

impl AnimationInfo {
    /// Length of the clip in seconds: `(frames - 1) / fps`.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        f64::from(self.frames.saturating_sub(1)) / f64::from(self.fps.max(1))
    }
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

/// Reads the header of an inflated clip.
///
/// # Errors
///
/// Fails when the data is too short or the header fields are not plausible: the size field must
/// equal the data length minus 8, the frame rate must be 1 to 240, there must be at least one
/// frame and the bone count must be 1 to 1024.
pub fn parse(d: &[u8]) -> Result<AnimationInfo> {
    let bad = |m: &str| Error::format(format!("not a skeletal animation: {m}"));
    let size = u32_at(d, 4).ok_or_else(|| bad("too short"))?;
    if u32_at(d, 0) != Some(0) || u64::from(size) + 8 != d.len() as u64 {
        return Err(bad("size field does not match"));
    }
    let fps = u16_at(d, 8).ok_or_else(|| bad("too short"))?;
    let frames = u16_at(d, 10).ok_or_else(|| bad("too short"))?;
    let bones = u32_at(d, 16).ok_or_else(|| bad("too short"))?;
    if !(1..=240).contains(&fps) || frames == 0 || !(1..=1024).contains(&bones) {
        return Err(bad("implausible frame rate, frame count or bone count"));
    }
    Ok(AnimationInfo { fps, frames, bones })
}

/// Most bones and total poses [`decode`] accepts, so a hostile header cannot ask for gigabytes.
const MAX_BONES: u32 = 1024;
const MAX_POSES: u64 = 4_000_000;
const RECORD_SIZE: usize = 20;

/// One bone pose in one frame, in the bone's local space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Local translation.
    pub position: [f32; 3],
    /// Uniform scale.
    pub scale: f32,
    /// Rotation quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

/// A decoded clip: `tracks[track][frame]`.
///
/// A track belongs to the bone of the model with the same name (see `names`). Do not take the
/// track number for the bone number: a clip may animate fewer bones than the skeleton has, and
/// the order can differ.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Frames per second.
    pub fps: u16,
    /// Number of frames.
    pub frames: u16,
    /// One list of poses per track.
    pub tracks: Vec<Vec<Pose>>,
    /// The bone name of every track, or empty when the clip has no usable name table (then the
    /// tracks are taken to be in the skeleton's order).
    pub names: Vec<String>,
}

fn f32_at(d: &[u8], at: usize) -> Option<f32> {
    u32_at(d, at).map(f32::from_bits)
}

/// Quaternion `[x, y, z, w]` for three angle words (yaw, pitch, roll): the product of rotations
/// about z, y and x whose half angle is `2π · word / 65536`.
#[must_use]
pub fn rotation_from_words(words: [u16; 3]) -> [f32; 4] {
    fn mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    }
    let half = |w: u16| f64::from(w) * std::f64::consts::TAU / 65536.0;
    let axis = |i: usize, w: u16| {
        let mut q = [0.0, 0.0, 0.0, half(w).cos()];
        q[i] = half(w).sin();
        q
    };
    let q = mul(mul(axis(2, words[0]), axis(1, words[1])), axis(0, words[2]));
    #[allow(clippy::cast_possible_truncation)]
    [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32]
}

/// Decodes all bone tracks of an inflated clip.
///
/// Each 20-byte record holds a type word (low 16 bits: which of the seven channels position
/// x, y, z, scale, yaw, pitch, roll are constants; high 16 bits: how many are animated), a
/// `u16` track and a float list. See `docs/formats/skeletal-animation.md`.
///
/// # Errors
///
/// Fails when the header is implausible, a record points outside the data, the animated channel
/// count or the float count does not fit the type mask, or the clip is larger than the limits.
pub fn decode(d: &[u8]) -> Result<Clip> {
    let info = parse(d)?;
    let bad = |m: &str| Error::format(format!("skeletal animation: {m}"));
    if info.bones > MAX_BONES || u64::from(info.bones) * u64::from(info.frames) > MAX_POSES {
        return Err(bad("clip too large"));
    }
    let table = 12usize
        .checked_add(u32_at(d, 12).ok_or_else(|| bad("too short"))? as usize)
        .ok_or_else(|| bad("bad record table"))?;
    let frames = usize::from(info.frames);
    let mut tracks = Vec::new();
    for bone in 0..info.bones as usize {
        let rec = bone
            .checked_mul(RECORD_SIZE)
            .and_then(|o| o.checked_add(table))
            .ok_or_else(|| bad("bad record table"))?;
        let word = |k: usize| u32_at(d, rec + 4 * k).ok_or_else(|| bad("record out of range"));
        let tag = word(0)?;
        let (mask, animated) = ((tag & 0x7f) as usize, (tag >> 16) as usize);
        if animated != 7 - (tag & 0x7f).count_ones() as usize {
            return Err(bad("animated channel count does not match the type"));
        }
        let rel = |from: usize, off: u32| from.checked_add(off as usize);
        let data_at = rel(rec + 4, word(1)?).ok_or_else(|| bad("bad data offset"))?;
        let data_n = word(2)? as usize;
        let par_at = rel(rec + 12, word(3)?).ok_or_else(|| bad("bad parameter offset"))?;
        let par_n = word(4)? as usize;
        if animated.checked_mul(frames) != Some(data_n) {
            return Err(bad("track length does not match frames"));
        }
        let data_end = data_n
            .checked_mul(2)
            .and_then(|n| n.checked_add(data_at))
            .ok_or_else(|| bad("bad data offset"))?;
        let par_end = par_n
            .checked_mul(4)
            .and_then(|n| n.checked_add(par_at))
            .ok_or_else(|| bad("bad parameter offset"))?;
        if data_end > d.len() || par_end > d.len() {
            return Err(bad("record points outside the data"));
        }
        // Float channels first: a constant is one float, an animated one is (min, step).
        let mut cursor = par_at;
        let mut float_ch: [(bool, f32, f32); 4] = [(true, 0.0, 0.0); 4];
        for (i, ch) in float_ch.iter_mut().enumerate() {
            let a = f32_at(d, cursor).ok_or_else(|| bad("parameters too short"))?;
            if mask & (1 << i) != 0 {
                *ch = (true, a, 0.0);
                cursor += 4;
            } else {
                let b = f32_at(d, cursor + 4).ok_or_else(|| bad("parameters too short"))?;
                *ch = (false, a, b);
                cursor += 8;
            }
        }
        let mut angle_const = [0u16; 3];
        for (i, w) in angle_const.iter_mut().enumerate() {
            if mask & (0x10 << i) != 0 {
                *w = u16_at(d, cursor).ok_or_else(|| bad("parameters too short"))?;
                cursor += 4;
            }
        }
        if cursor != par_end {
            return Err(bad("parameter count does not match the type"));
        }
        let mut poses = Vec::with_capacity(frames);
        for f in 0..frames {
            let mut k = f * animated;
            let mut next = || -> Option<u16> {
                let v = u16_at(d, data_at + 2 * k)?;
                k += 1;
                Some(v)
            };
            let mut fl = [0f32; 4];
            for (v, &(constant, a, step)) in fl.iter_mut().zip(&float_ch) {
                *v = if constant {
                    a
                } else {
                    a + f32::from(next().ok_or_else(|| bad("track too short"))?) * step
                };
            }
            let mut words = angle_const;
            for (i, w) in words.iter_mut().enumerate() {
                if mask & (0x10 << i) == 0 {
                    *w = next().ok_or_else(|| bad("track too short"))?;
                }
            }
            poses.push(Pose {
                position: [fl[0], fl[1], fl[2]],
                scale: fl[3],
                rotation: rotation_from_words(words),
            });
        }
        tracks.push(poses);
    }
    let names = bone_names(d)
        .ok()
        .filter(|n| n.len() == tracks.len())
        .unwrap_or_default();
    Ok(Clip {
        fps: info.fps,
        frames: info.frames,
        tracks,
        names,
    })
}

/// Most material tracks a clip may hold.
const MAX_MATERIALS: usize = 1024;

/// The part of a clip that names things: header position of each array triple (relative offset,
/// item count) inside an inflated clip.
const NAMES_AT: usize = 20;
const MATERIALS_AT: usize = 36;
const MATERIAL_NAMES_AT: usize = 44;
/// Size of one material record.
const MATERIAL_RECORD_SIZE: usize = 20;

/// Position and item count of the array triple whose header lies at `header`: the first word is
/// the offset from the header, the second the item count.
fn array_at(d: &[u8], header: usize, stride: usize, what: &str) -> Result<(usize, usize)> {
    let bad = || Error::format(format!("skeletal animation: bad {what} array"));
    let rel = u32_at(d, header).ok_or_else(bad)? as usize;
    let count = u32_at(d, header + 4).ok_or_else(bad)? as usize;
    let start = header.checked_add(rel).ok_or_else(bad)?;
    let bytes = count.checked_mul(stride).ok_or_else(bad)?;
    if start.checked_add(bytes).is_none_or(|end| end > d.len()) {
        return Err(bad());
    }
    Ok((start, count))
}

/// Reads a name table: `count` entries of (offset from the entry, byte length), the text is
/// padded with zero bytes.
fn name_table(d: &[u8], header: usize, what: &str) -> Result<Vec<String>> {
    let bad = || Error::format(format!("skeletal animation: bad {what} name"));
    let (start, count) = array_at(d, header, 8, what)?;
    if count > MAX_MATERIALS {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = start + i * 8;
        let rel = u32_at(d, at).ok_or_else(bad)? as usize;
        let len = u32_at(d, at + 4).ok_or_else(bad)? as usize;
        let from = at.checked_add(rel).ok_or_else(bad)?;
        let to = from.checked_add(len).ok_or_else(bad)?;
        let bytes = d.get(from..to).ok_or_else(bad)?;
        let keep = bytes.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        out.push(String::from_utf8_lossy(&bytes[..keep]).into_owned());
    }
    Ok(out)
}

/// Names of the bone tracks of an inflated clip, in track order (the order of [`Clip::tracks`]).
///
/// # Errors
///
/// Fails when the header is implausible, the name table points outside the data or does not
/// have one name per bone track.
pub fn bone_names(d: &[u8]) -> Result<Vec<String>> {
    let info = parse(d)?;
    let names = name_table(d, NAMES_AT, "bone")?;
    if names.len() != info.bones as usize {
        return Err(Error::format(
            "skeletal animation: name count differs from the bone count",
        ));
    }
    Ok(names)
}

/// One sample of a material track. A channel the track does not carry is `0.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSample {
    /// Transparency as the amount *removed* (`0.0` leaves the material as it is, `1.0` removes
    /// all of it); the opacity is `1 - transparency`.
    pub transparency: f32,
    /// Texture shift along U as a fraction of one turn. The stored value wraps around, so
    /// interpolate on the shortest way round.
    pub translate_u: f32,
    /// Texture shift along V, same meaning as `translate_u`.
    pub translate_v: f32,
}

/// The animation of one material of the model, one sample per frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialTrack {
    /// Material name, as in the `ModelElement` of the geometry.
    pub name: String,
    /// The track carries the transparency channel.
    pub has_transparency: bool,
    /// The track carries the U shift channel.
    pub has_translate_u: bool,
    /// The track carries the V shift channel.
    pub has_translate_v: bool,
    /// One sample per frame of the clip.
    pub samples: Vec<MaterialSample>,
}

/// Reads the material tracks of an inflated clip (texture scrolling and transparency).
///
/// A clip without material animation gives an empty list.
///
/// # Errors
///
/// Fails when the header is implausible, a table points outside the data, the channel flags are
/// unknown, or the sample size does not fit the frame count.
pub fn material_tracks(d: &[u8]) -> Result<Vec<MaterialTrack>> {
    let info = parse(d)?;
    let bad = |m: &str| Error::format(format!("skeletal animation: material track: {m}"));
    let (start, count) = array_at(d, MATERIALS_AT, MATERIAL_RECORD_SIZE, "material")?;
    let names = name_table(d, MATERIAL_NAMES_AT, "material")?;
    if count > MAX_MATERIALS || names.len() != count {
        return Err(bad("count does not match the names"));
    }
    let frames = usize::from(info.frames);
    let mut out = Vec::with_capacity(count);
    for (i, name) in names.into_iter().enumerate() {
        let rec = start + i * MATERIAL_RECORD_SIZE;
        let flags = u16_at(d, rec).ok_or_else(|| bad("too short"))?;
        let stride = usize::from(u16_at(d, rec + 2).ok_or_else(|| bad("too short"))?);
        let size = usize::from(flags & 1)
            + 2 * usize::from((flags >> 1) & 1)
            + 2 * usize::from((flags >> 2) & 1);
        if flags >= 8 || stride != size {
            return Err(bad("unknown channel flags"));
        }
        let (data, bytes) = array_at(d, rec + 4, 1, "material samples")?;
        let (_, floats) = array_at(d, rec + 12, 4, "material floats")?;
        if floats != 0 || bytes != stride * frames {
            return Err(bad("sample size does not fit the frame count"));
        }
        let mut samples = Vec::with_capacity(frames);
        for f in 0..frames {
            let mut at = data + f * stride;
            let mut sample = MaterialSample {
                transparency: 0.0,
                translate_u: 0.0,
                translate_v: 0.0,
            };
            if flags & 1 != 0 {
                sample.transparency = f32::from(d[at]) / 255.0;
                at += 1;
            }
            if flags & 2 != 0 {
                sample.translate_u =
                    f32::from(u16_at(d, at).ok_or_else(|| bad("too short"))?) / 65535.0;
                at += 2;
            }
            if flags & 4 != 0 {
                sample.translate_v =
                    f32::from(u16_at(d, at).ok_or_else(|| bad("too short"))?) / 65535.0;
            }
            samples.push(sample);
        }
        out.push(MaterialTrack {
            name,
            has_transparency: flags & 1 != 0,
            has_translate_u: flags & 2 != 0,
            has_translate_v: flags & 4 != 0,
            samples,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(fps: u16, frames: u16, bones: u32) -> Vec<u8> {
        let mut d = vec![0u8; 8];
        d.extend(fps.to_le_bytes());
        d.extend(frames.to_le_bytes());
        d.extend(408u32.to_le_bytes());
        d.extend(bones.to_le_bytes());
        d.resize(64, 0);
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
        d
    }

    #[test]
    fn header_is_read() {
        let info = parse(&clip(30, 91, 46)).expect("parse");
        assert_eq!((info.fps, info.frames, info.bones), (30, 91, 46));
        assert!((info.seconds() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn implausible_or_short_input_is_rejected() {
        assert!(parse(&[]).is_err());
        assert!(parse(&clip(30, 91, 46)[..19]).is_err());
        assert!(parse(&clip(0, 91, 46)).is_err());
        assert!(parse(&clip(30, 0, 46)).is_err());
        assert!(parse(&clip(30, 10, 0)).is_err());
        assert!(parse(&clip(30, 10, 5000)).is_err());
        let mut wrong_size = clip(30, 10, 46);
        wrong_size[4] ^= 1;
        assert!(parse(&wrong_size).is_err());
    }

    /// Builds a clip with the given records: (type word, track words, floats, constant angle
    /// words stored in 4-byte float slots are passed as raw `u32`s inside `floats`).
    fn clip_with(frames: u16, recs: &[(u32, Vec<u16>, Vec<u32>)]) -> Vec<u8> {
        let mut d = vec![0u8; 8];
        d.extend(30u16.to_le_bytes());
        d.extend(frames.to_le_bytes());
        d.extend(408u32.to_le_bytes());
        d.extend((recs.len() as u32).to_le_bytes());
        d.resize(420, 0);
        let table = d.len();
        d.resize(table + recs.len() * RECORD_SIZE, 0);
        for (i, (tag, track, floats)) in recs.iter().enumerate() {
            let rec = table + i * RECORD_SIZE;
            let data_at = d.len();
            for w in track {
                d.extend(w.to_le_bytes());
            }
            while !d.len().is_multiple_of(4) {
                d.push(0);
            }
            let par_at = d.len();
            for f in floats {
                d.extend(f.to_le_bytes());
            }
            let put = |d: &mut Vec<u8>, at: usize, v: u32| {
                d[at..at + 4].copy_from_slice(&v.to_le_bytes())
            };
            put(&mut d, rec, *tag);
            put(&mut d, rec + 4, (data_at - (rec + 4)) as u32);
            put(&mut d, rec + 8, track.len() as u32);
            put(&mut d, rec + 12, (par_at - (rec + 12)) as u32);
            put(&mut d, rec + 16, floats.len() as u32);
        }
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
        d
    }

    fn fb(v: f32) -> u32 {
        v.to_bits()
    }

    #[test]
    fn constant_and_animated_channels_are_decoded() {
        // Bone 0: everything constant. Bone 1: position x animated, the rest constant.
        let still = (
            0x7f,
            vec![],
            vec![fb(1.0), fb(2.0), fb(3.0), fb(0.5), 0, 0, 0],
        );
        let moving = (
            0x0001_007e,
            vec![0, 2, 4],
            vec![fb(10.0), fb(0.5), fb(0.0), fb(0.0), fb(1.0), 0, 0, 0],
        );
        // mask 0x7e: only position x is animated.
        let clip = decode(&clip_with(3, &[still, moving])).expect("decode");
        assert_eq!(clip.tracks.len(), 2);
        assert_eq!(clip.tracks[0][1].position, [1.0, 2.0, 3.0]);
        assert!((clip.tracks[0][2].scale - 0.5).abs() < 1e-6);
        assert_eq!(clip.tracks[0][0].rotation, [0.0, 0.0, 0.0, 1.0]);
        let xs: Vec<f32> = clip.tracks[1].iter().map(|p| p.position[0]).collect();
        assert_eq!(xs, vec![10.0, 11.0, 12.0]);
    }

    #[test]
    fn angles_become_a_quaternion() {
        let q = rotation_from_words([16384, 0, 0]);
        assert!((q[2] - 1.0).abs() < 1e-6 && q[0].abs() < 1e-6 && q[3].abs() < 1e-6);
        let q = rotation_from_words([0, 0, 16384]);
        assert!((q[0] - 1.0).abs() < 1e-6);
        assert_eq!(rotation_from_words([0, 0, 0]), [0.0, 0.0, 0.0, 1.0]);
        // Animated angles: tag 0x3000f (position and scale constants, three animated angles).
        let rec = (
            0x0003_000f,
            vec![16384, 0, 0, 0, 0, 0],
            vec![fb(0.0), fb(0.0), fb(0.0), fb(1.0)],
        );
        let clip = decode(&clip_with(2, &[rec])).expect("decode");
        assert!((clip.tracks[0][0].rotation[2] - 1.0).abs() < 1e-6);
        assert_eq!(clip.tracks[0][1].rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn damaged_clips_are_rejected() {
        let good = clip_with(2, &[(0x7f, vec![], vec![0; 7])]);
        assert!(decode(&good).is_ok());
        // Truncated inside the record table.
        let mut cut = good[..430].to_vec();
        let size = (cut.len() - 8) as u32;
        cut[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(decode(&cut).is_err());
        // Wrong float count for the type.
        assert!(decode(&clip_with(2, &[(0x7f, vec![], vec![0; 6])])).is_err());
        // Animated count disagrees with the mask.
        assert!(decode(&clip_with(2, &[(0x0002_007f, vec![0; 4], vec![0; 7])])).is_err());
        // Track length disagrees with the frame count.
        assert!(decode(&clip_with(2, &[(0x0001_007e, vec![0; 3], vec![0; 7])])).is_err());
        // Data offset far outside the file.
        let mut far = good.clone();
        far[424..428].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&far).is_err());
    }

    /// Adds a name table (array triple at `header` of the clip) with the given names.
    fn put_names(d: &mut Vec<u8>, header: usize, names: &[&str]) {
        let table = d.len();
        d.resize(table + names.len() * 8, 0);
        for (i, n) in names.iter().enumerate() {
            let entry = table + i * 8;
            let text = d.len();
            d.extend(n.as_bytes());
            d.extend([0u8; 2]);
            d[entry..entry + 4].copy_from_slice(&((text - entry) as u32).to_le_bytes());
            d[entry + 4..entry + 8].copy_from_slice(&((n.len() + 2) as u32).to_le_bytes());
        }
        d[header..header + 4].copy_from_slice(&((table - header) as u32).to_le_bytes());
        d[header + 4..header + 8].copy_from_slice(&(names.len() as u32).to_le_bytes());
    }

    fn fix_size(d: &mut [u8]) {
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
    }

    #[test]
    fn bone_names_are_read_and_checked_against_the_bone_count() {
        let mut d = clip_with(2, &[(0x7f, vec![], vec![0; 7]), (0x7f, vec![], vec![0; 7])]);
        put_names(&mut d, NAMES_AT, &["Bip01", "Bip01 Pelvis"]);
        fix_size(&mut d);
        assert_eq!(bone_names(&d).unwrap(), ["Bip01", "Bip01 Pelvis"]);
        let mut one = clip_with(2, &[(0x7f, vec![], vec![0; 7]), (0x7f, vec![], vec![0; 7])]);
        put_names(&mut one, NAMES_AT, &["Bip01"]);
        fix_size(&mut one);
        assert!(bone_names(&one).is_err());
        // No name table at all.
        assert!(bone_names(&clip_with(2, &[(0x7f, vec![], vec![0; 7])])).is_err());
    }

    /// A 3 frame clip with one material track named `glow`: channel flags, sample size, the
    /// sample bytes and the float count of the record.
    fn clip_with_material(flags: u16, stride: u16, samples: &[u8], floats: u32) -> Vec<u8> {
        let mut d = clip_with(3, &[(0x7f, vec![], vec![0; 7])]);
        let rec = d.len();
        d.resize(rec + MATERIAL_RECORD_SIZE, 0);
        let sample_at = d.len();
        d.extend(samples);
        let put =
            |d: &mut Vec<u8>, at: usize, v: u32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        d[rec..rec + 2].copy_from_slice(&flags.to_le_bytes());
        d[rec + 2..rec + 4].copy_from_slice(&stride.to_le_bytes());
        put(&mut d, rec + 4, (sample_at - (rec + 4)) as u32);
        put(&mut d, rec + 8, samples.len() as u32);
        put(&mut d, rec + 12, (sample_at - (rec + 12)) as u32);
        put(&mut d, rec + 16, floats);
        put(&mut d, MATERIALS_AT, (rec - MATERIALS_AT) as u32);
        put(&mut d, MATERIALS_AT + 4, 1);
        put_names(&mut d, MATERIAL_NAMES_AT, &["glow"]);
        fix_size(&mut d);
        d
    }

    #[test]
    fn material_track_with_all_channels_is_decoded() {
        // 3 frames of (transparency byte, u word, v word) = 5 bytes each.
        let mut s = Vec::new();
        for (t, u, v) in [(0u8, 0u16, 65535u16), (255, 65535, 0), (51, 32768, 13107)] {
            s.push(t);
            s.extend(u.to_le_bytes());
            s.extend(v.to_le_bytes());
        }
        let tracks = material_tracks(&clip_with_material(7, 5, &s, 0)).unwrap();
        assert_eq!(tracks.len(), 1);
        let t = &tracks[0];
        assert_eq!(t.name, "glow");
        assert!(t.has_transparency && t.has_translate_u && t.has_translate_v);
        assert_eq!(t.samples.len(), 3);
        assert!((t.samples[1].transparency - 1.0).abs() < 1e-6);
        assert!((t.samples[1].translate_u - 1.0).abs() < 1e-6);
        assert!((t.samples[2].transparency - 0.2).abs() < 1e-6);
        assert!((t.samples[2].translate_v - 0.2).abs() < 1e-6);
    }

    #[test]
    fn material_channels_that_are_not_flagged_stay_zero() {
        // Only the V shift: 2 bytes per frame.
        let s: Vec<u8> = [100u16, 200, 300]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let t = &material_tracks(&clip_with_material(4, 2, &s, 0)).unwrap()[0];
        assert!(!t.has_transparency && !t.has_translate_u && t.has_translate_v);
        assert!(
            t.samples
                .iter()
                .all(|x| x.transparency == 0.0 && x.translate_u == 0.0)
        );
        assert!((t.samples[2].translate_v - 300.0 / 65535.0).abs() < 1e-7);
    }

    #[test]
    fn clip_without_materials_gives_an_empty_list_and_bad_ones_fail() {
        assert!(
            material_tracks(&clip_with(3, &[(0x7f, vec![], vec![0; 7])]))
                .unwrap()
                .is_empty()
        );
        let s = [0u8; 9];
        // Unknown flag bit, wrong stride, wrong sample count and a float payload are all errors.
        assert!(material_tracks(&clip_with_material(8, 0, &[], 0)).is_err());
        assert!(material_tracks(&clip_with_material(7, 4, &s, 0)).is_err());
        assert!(material_tracks(&clip_with_material(7, 5, &s, 0)).is_err());
        assert!(material_tracks(&clip_with_material(1, 1, &[0, 0, 0], 1)).is_err());
    }

    #[test]
    fn truncated_clips_never_panic_in_the_name_and_material_readers() {
        let s: Vec<u8> = [1u16, 2, 3].iter().flat_map(|w| w.to_le_bytes()).collect();
        let d = clip_with_material(4, 2, &s, 0);
        for cut in 0..d.len() {
            let _ = bone_names(&d[..cut]);
            let _ = material_tracks(&d[..cut]);
        }
    }
}
