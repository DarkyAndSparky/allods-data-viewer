//! Skeleton stored in the tail block of `X.(Geometry).bin` (see `docs/formats/geometry-bin.md`).
//!
//! The tail starts with four `(offset, count)` pairs of `u32`. Each offset is relative to the
//! address of its own pair, so the pair at byte `8 * k` points to `8 * k + offset`:
//!
//! 1. `count` bone entries of 52 bytes: 3x3 matrix (9 `f32`), position (3 `f32`), parent index
//!    (`u32`, `0xFFFF` and above mean no parent);
//! 2. `count` name references of 8 bytes: `u32` offset relative to the reference, `u32` length of
//!    the name including its terminating zero;
//! 3. a table of floats that is not decoded;
//! 4. `count` `u16` values (a permutation of `0..count`, meaning unknown).

use crate::error::{Error, Result};

const BONE_SIZE: usize = 52;
const MAX_BONES: usize = 4096;

/// One bone of the rest pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    /// Name from the tail, such as `Spine1` or `L_finger_2`.
    pub name: String,
    /// Index of the parent bone, `None` for the root.
    pub parent: Option<usize>,
    /// The nine matrix values as stored (row or column order is not verified).
    pub matrix: [f32; 9],
    /// The position as stored (its frame of reference is not verified).
    pub position: [f32; 3],
}

/// The bones of a model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Skeleton {
    /// Bones in file order; vertex bone indices and parent indices refer to this order.
    pub bones: Vec<Bone>,
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn f32_at(d: &[u8], at: usize) -> Option<f32> {
    u32_at(d, at).map(f32::from_bits)
}

/// Where the data of the pair at byte `8 * k` starts and how many entries it has.
fn pair(d: &[u8], k: usize) -> Result<(usize, usize)> {
    let at = k * 8;
    let off = u32_at(d, at).ok_or_else(|| Error::format("skeleton header is truncated"))?;
    let count = u32_at(d, at + 4).ok_or_else(|| Error::format("skeleton header is truncated"))?;
    let start = at
        .checked_add(off as usize)
        .ok_or_else(|| Error::format("skeleton offset overflows"))?;
    Ok((start, count as usize))
}

impl Skeleton {
    /// Parses the tail block (the bytes right after the block size).
    ///
    /// # Errors
    /// Returns an error when the header, a table or a name lies outside the block, or when the
    /// two tables disagree on the number of bones.
    pub fn parse(tail: &[u8]) -> Result<Self> {
        let (bones_at, n) = pair(tail, 0)?;
        let (names_at, n_names) = pair(tail, 1)?;
        if n != n_names || n > MAX_BONES {
            return Err(Error::format("skeleton tables disagree on the bone count"));
        }
        let mut bones = Vec::with_capacity(n);
        for i in 0..n {
            let at = bones_at.saturating_add(i * BONE_SIZE);
            let mut matrix = [0.0; 9];
            for (k, m) in matrix.iter_mut().enumerate() {
                *m = f32_at(tail, at.saturating_add(k * 4))
                    .ok_or_else(|| Error::format("bone is outside the block"))?;
            }
            let mut position = [0.0; 3];
            for (k, p) in position.iter_mut().enumerate() {
                *p = f32_at(tail, at.saturating_add(36 + k * 4))
                    .ok_or_else(|| Error::format("bone is outside the block"))?;
            }
            let parent = u32_at(tail, at.saturating_add(48))
                .ok_or_else(|| Error::format("bone is outside the block"))?;
            let parent = usize::try_from(parent).ok().filter(|&p| p < n);
            let r = names_at.saturating_add(i * 8);
            let rel = u32_at(tail, r)
                .ok_or_else(|| Error::format("name reference is outside the block"))?;
            let len = u32_at(tail, r.saturating_add(4))
                .ok_or_else(|| Error::format("name reference is outside the block"))?
                as usize;
            let start = r
                .checked_add(rel as usize)
                .ok_or_else(|| Error::format("name offset overflows"))?;
            let raw = tail
                .get(
                    start
                        ..start
                            .checked_add(len)
                            .ok_or_else(|| Error::format("name length overflows"))?,
                )
                .ok_or_else(|| Error::format("name is outside the block"))?;
            let name =
                String::from_utf8_lossy(raw.split(|&b| b == 0).next().unwrap_or(&[])).into_owned();
            bones.push(Bone {
                name,
                parent,
                matrix,
                position,
            });
        }
        Ok(Self { bones })
    }

    /// Rest pose of every bone in the model's own space: `(rotation, position)` with the rotation
    /// as a row-major 3x3 matrix (the stored matrix is the bone's rotation in model space) and the
    /// position of the bone origin. The file stores the inverse bind transform, so the position is
    /// `-(matrix * stored_position)`. Checked on three models (`docs/formats/geometry-bin.md`).
    #[must_use]
    pub fn world_pose(&self) -> Vec<([f32; 9], [f32; 3])> {
        self.bones
            .iter()
            .map(|b| {
                let m = &b.matrix;
                let p = &b.position;
                let w = [
                    -(m[0] * p[0] + m[1] * p[1] + m[2] * p[2]),
                    -(m[3] * p[0] + m[4] * p[1] + m[5] * p[2]),
                    -(m[6] * p[0] + m[7] * p[1] + m[8] * p[2]),
                ];
                (*m, w)
            })
            .collect()
    }

    /// Rest pose of every bone relative to its parent: `(rotation, translation)`. The translation
    /// of a bone equals the first three parameters of the matching translation channel in an
    /// animation clip (checked on `Spine1`, `R_forearm` and `Head` of AirElemental).
    #[must_use]
    pub fn local_pose(&self) -> Vec<([f32; 9], [f32; 3])> {
        let world = self.world_pose();
        self.bones
            .iter()
            .enumerate()
            .map(|(i, b)| match b.parent {
                None => world[i],
                Some(p) => {
                    let (rp, wp) = &world[p];
                    let (r, w) = &world[i];
                    let d = [w[0] - wp[0], w[1] - wp[1], w[2] - wp[2]];
                    // rp^T * r  and  rp^T * d (rp is row-major, so rp^T[i][j] = rp[j][i])
                    let mut lr = [0.0; 9];
                    for i3 in 0..3 {
                        for j in 0..3 {
                            lr[i3 * 3 + j] = (0..3).map(|k| rp[k * 3 + i3] * r[k * 3 + j]).sum();
                        }
                    }
                    let lt = [0, 1, 2].map(|i3| (0..3).map(|k| rp[k * 3 + i3] * d[k]).sum());
                    (lr, lt)
                }
            })
            .collect()
    }

    /// True when following parents from every bone ends at a root without a loop.
    #[must_use]
    pub fn is_forest(&self) -> bool {
        let n = self.bones.len();
        (0..n).all(|start| {
            let mut at = start;
            for _ in 0..=n {
                match self.bones[at].parent {
                    None => return true,
                    Some(p) => at = p,
                }
            }
            false
        })
    }

    /// Index of the bone with this name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tail with three bones: `Root`, `Arm` (child of 0) and `Hand` (child of 1).
    pub(crate) fn sample() -> Vec<u8> {
        let names = ["Root", "Arm", "Hand"];
        let n = names.len();
        let bones_at = 32;
        let names_at = bones_at + n * BONE_SIZE;
        let pool_at = names_at + n * 8;
        let mut t = vec![0u8; pool_at];
        let put =
            |t: &mut Vec<u8>, at: usize, v: u32| t[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut t, 0, bones_at as u32);
        put(&mut t, 4, n as u32);
        put(&mut t, 8, (names_at - 8) as u32);
        put(&mut t, 12, n as u32);
        let mut pool = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let at = bones_at + i * BONE_SIZE;
            for k in 0..9 {
                let v = if k % 4 == 0 { 1.0f32 } else { 0.0 };
                t[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            t[at + 36..at + 40].copy_from_slice(&(i as f32).to_le_bytes());
            let parent = if i == 0 { 0xFFFF } else { (i - 1) as u32 };
            put(&mut t, at + 48, parent);
            let r = names_at + i * 8;
            let target = pool_at + pool.len();
            put(&mut t, r, (target - r) as u32);
            put(&mut t, r + 4, (name.len() + 1) as u32);
            pool.extend(name.as_bytes());
            pool.push(0);
        }
        t.extend(pool);
        t
    }

    #[test]
    fn reads_names_parents_and_positions() {
        let s = Skeleton::parse(&sample()).expect("parse");
        let names: Vec<&str> = s.bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Root", "Arm", "Hand"]);
        assert_eq!(
            s.bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
            [None, Some(0), Some(1)]
        );
        assert_eq!(s.bones[2].position, [2.0, 0.0, 0.0]);
        assert!(s.is_forest());
        assert_eq!(s.find("Hand"), Some(2));
    }

    #[test]
    fn poses_follow_the_inverse_bind_convention() {
        let mut t = sample();
        // Bone 1: identity rotation, stored position (0, 0, -3) means the bone sits at (0, 0, 3).
        let at = 32 + BONE_SIZE;
        t[at + 36..at + 40].copy_from_slice(&0f32.to_le_bytes());
        t[at + 44..at + 48].copy_from_slice(&(-3f32).to_le_bytes());
        let s = Skeleton::parse(&t).expect("parse");
        let w = s.world_pose();
        assert_eq!(w[1].1, [-0.0, -0.0, 3.0]);
        let l = s.local_pose();
        // Bone 0 sits at the origin (stored position 0), so bone 1 is 3 up from it.
        assert_eq!(l[1].1, [0.0, 0.0, 3.0]);
        assert_eq!(l[1].0, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_parent_loop_is_not_a_forest() {
        let mut s = Skeleton::parse(&sample()).expect("parse");
        s.bones[0].parent = Some(2);
        assert!(!s.is_forest());
    }

    #[test]
    fn truncated_or_hostile_tails_fail_cleanly() {
        let t = sample();
        for cut in 0..t.len() {
            let _ = Skeleton::parse(&t[..cut]);
        }
        let mut bad = t.clone();
        bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Skeleton::parse(&bad).is_err());
        let mut far = t;
        far[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Skeleton::parse(&far).is_err());
    }
}
