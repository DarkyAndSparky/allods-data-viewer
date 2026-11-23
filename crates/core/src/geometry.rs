//! Reader for `X.(Geometry).bin` model geometry (container only, see `docs/formats/geometry-bin.md`).
//!
//! The file is a zlib stream. Inflated layout (all little endian):
//! `u32 flags`, `u32 vertex_bytes`, vertices, `u32 ?`, `u32 index_bytes`, `u16` indices,
//! `u32 ?`, `u32 skeleton_bytes`, then more blocks that are not decoded yet.
//! The vertex stride is not stored; it is guessed (see [`guess_stride`]).

use crate::error::{Error, Result};
use crate::limits::Limits;
use std::io::Read;

/// Candidate vertex strides.
pub const STRIDES: [usize; 4] = [24, 28, 32, 36];

/// A parsed geometry file.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// Raw vertex bytes.
    pub vertex_data: Vec<u8>,
    /// Vertex stride in bytes.
    pub stride: usize,
    /// Triangle list indices (three per triangle).
    pub indices: Vec<u16>,
    /// Size of the block after the indices (skeleton data, not decoded).
    pub tail_block: u32,
    /// The tail block itself (`tail_block` bytes at most); see [`crate::skeleton`].
    pub tail: Vec<u8>,
}

/// One decoded vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position.
    pub pos: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
}

fn u32_at(d: &[u8], at: usize) -> Result<u32> {
    d.get(
        at..at
            .checked_add(4)
            .ok_or_else(|| Error::format("geometry offset overflow"))?,
    )
    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    .ok_or_else(|| Error::format("geometry is truncated"))
}

/// Inflates the zlib layer if there is one.
///
/// # Errors
/// Returns an error when inflation fails or exceeds the limits.
pub fn inflate(raw: &[u8], limits: &Limits) -> Result<Vec<u8>> {
    if raw.first() == Some(&0x78) {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(raw)
            .take(limits.max_inflated_size + 1)
            .read_to_end(&mut out)
            .map_err(|e| Error::format(format!("geometry zlib error: {e}")))?;
        if out.len() as u64 > limits.max_inflated_size {
            return Err(Error::limit(
                "geometry is larger than the inflated size limit",
            ));
        }
        Ok(out)
    } else {
        Ok(raw.to_vec())
    }
}

/// Picks the stride. Every candidate must divide the vertex block and keep all indices inside
/// it. Among those, the winner has the fewest non-finite positions (a wrong stride reads
/// garbage as floats), then a vertex tag `ff ff ff` at the end of the 5th or 7th vertex, then
/// the fewest vertices that no index points to.
///
/// # Errors
/// Returns an error when no candidate fits.
pub fn guess_stride(vertices: &[u8], indices: &[u16]) -> Result<usize> {
    let max = indices.iter().copied().max().map_or(0, usize::from);
    let tagged = |s: usize| {
        [5usize, 7].iter().any(|&k| {
            vertices
                .get(k * s - 3..k * s)
                .is_some_and(|t| t == [0xff, 0xff, 0xff])
        })
    };
    let non_finite = |s: usize| {
        vertices
            .chunks_exact(s)
            .filter(|v| {
                v.get(..12).is_none_or(|p| {
                    p.as_chunks::<4>()
                        .0
                        .iter()
                        .any(|c| !f32::from_le_bytes(*c).is_finite())
                })
            })
            .count()
    };
    STRIDES
        .iter()
        .copied()
        .filter(|&s| {
            !vertices.is_empty() && vertices.len().is_multiple_of(s) && max < vertices.len() / s
        })
        .min_by_key(|&s| (non_finite(s), !tagged(s), vertices.len() / s - max - 1))
        .ok_or_else(|| Error::format("cannot determine the vertex size"))
}

/// Reads the vertex stride from the first `<vertexDeclarations>` block of a Geometry `.xdb` descriptor.
///
/// The declaration stores the stride explicitly, so it is preferred over [`guess_stride`]. Returns
/// `None` when the block or a plausible `<stride>` value (16 to 256 bytes) is missing.
#[must_use]
pub fn declared_stride(xdb: &str) -> Option<usize> {
    let block = xdb.split_once("<vertexDeclarations>")?.1;
    let block = block
        .split_once("</vertexDeclarations>")
        .map_or(block, |b| b.0);
    let value = block.split_once("<stride>")?.1.split_once("</stride>")?.0;
    let stride: usize = value.trim().parse().ok()?;
    (16..=256).contains(&stride).then_some(stride)
}

/// Parses an inflated geometry file.
///
/// # Errors
/// Returns an error for truncated or inconsistent data.
pub fn parse(d: &[u8], stride: Option<usize>) -> Result<Geometry> {
    let vb = u32_at(d, 4)? as usize;
    let v_end = 8usize
        .checked_add(vb)
        .ok_or_else(|| Error::format("vertex block size overflows"))?;
    let vertex_data = d
        .get(8..v_end)
        .ok_or_else(|| Error::format("vertex block is outside the file"))?
        .to_vec();
    let ib = u32_at(d, v_end + 4)? as usize;
    if !ib.is_multiple_of(6) {
        return Err(Error::format(
            "index block is not a whole number of triangles",
        ));
    }
    let i_start = v_end + 8;
    let i_end = i_start
        .checked_add(ib)
        .ok_or_else(|| Error::format("index block size overflows"))?;
    let indices: Vec<u16> = d
        .get(i_start..i_end)
        .ok_or_else(|| Error::format("index block is outside the file"))?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    let tail_block = u32_at(d, i_end + 4).unwrap_or(0);
    let tail = d
        .get(i_end + 8..)
        .map(|t| t[..t.len().min(tail_block as usize)].to_vec())
        .unwrap_or_default();
    let stride = match stride {
        Some(s) if s >= 20 && vertex_data.len().is_multiple_of(s) => s,
        Some(_) => return Err(Error::format("stride does not divide the vertex block")),
        None => guess_stride(&vertex_data, &indices)?,
    };
    Ok(Geometry {
        vertex_data,
        stride,
        indices,
        tail_block,
        tail,
    })
}

/// Unit quaternion `[x, y, z, w]` of a proper rotation given as a row-major 3x3 matrix.
fn quaternion(m: &[f32; 9]) -> [f32; 4] {
    let t = m[0] + m[4] + m[8];
    let q = if t > 0.0 {
        let s = (t + 1.0).sqrt() * 2.0;
        [
            (m[7] - m[5]) / s,
            (m[2] - m[6]) / s,
            (m[3] - m[1]) / s,
            s / 4.0,
        ]
    } else if m[0] > m[4] && m[0] > m[8] {
        let s = (1.0 + m[0] - m[4] - m[8]).sqrt() * 2.0;
        [
            s / 4.0,
            (m[1] + m[3]) / s,
            (m[2] + m[6]) / s,
            (m[7] - m[5]) / s,
        ]
    } else if m[4] > m[8] {
        let s = (1.0 + m[4] - m[0] - m[8]).sqrt() * 2.0;
        [
            (m[1] + m[3]) / s,
            s / 4.0,
            (m[5] + m[7]) / s,
            (m[2] - m[6]) / s,
        ]
    } else {
        let s = (1.0 + m[8] - m[0] - m[4]).sqrt() * 2.0;
        [
            (m[2] + m[6]) / s,
            (m[5] + m[7]) / s,
            s / 4.0,
            (m[3] - m[1]) / s,
        ]
    };
    let l = q.iter().map(|c| c * c).sum::<f32>().sqrt();
    if l > 0.0 {
        q.map(|c| c / l)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// Skin data of one vertex: up to four bones and their weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skin {
    /// Bone indices into [`crate::skeleton::Skeleton`]; `None` where the slot is unused.
    pub bones: [Option<u8>; 4],
    /// Weights as stored (0..=255, the used ones add up to 255 or 256).
    pub weights: [u8; 4],
}

impl Geometry {
    /// Skin data per vertex. The last 8 bytes of a vertex hold four weights and then four bone
    /// indices (stored multiplied by 3, `0xFF` for an unused slot). Strides below 28 have none.
    #[must_use]
    pub fn skin(&self) -> Option<Vec<Skin>> {
        if self.stride < 28 {
            return None;
        }
        let w = self.stride - 8;
        Some(
            self.vertex_data
                .chunks_exact(self.stride)
                .map(|v| {
                    let mut bones = [None; 4];
                    for (k, b) in bones.iter_mut().enumerate() {
                        *b = (v[w + 4 + k] != 0xFF).then(|| v[w + 4 + k] / 3);
                    }
                    Skin {
                        bones,
                        weights: [v[w], v[w + 1], v[w + 2], v[w + 3]],
                    }
                })
                .collect(),
        )
    }

    /// The skeleton from the tail block, when it can be read.
    ///
    /// # Errors
    /// Returns an error when the tail block is missing or damaged.
    pub fn skeleton(&self) -> Result<crate::skeleton::Skeleton> {
        crate::skeleton::Skeleton::parse(&self.tail)
    }

    /// Decoded vertices (position and the first texture coordinate set).
    #[must_use]
    pub fn vertices(&self) -> Vec<Vertex> {
        let f = |c: &[u8]| f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        self.vertex_data
            .chunks_exact(self.stride)
            .map(|v| Vertex {
                pos: [f(&v[0..]), f(&v[4..]), f(&v[8..])],
                uv: [f(&v[12..]), f(&v[16..])],
            })
            .collect()
    }

    /// A binary glTF 2.0 file (`.glb`) with one untextured mesh: positions, texture coordinates
    /// (not flipped: glTF and the game both put the origin at the top left) and `u16` triangles.
    /// When the skeleton and the per-vertex weights can be read, the file also carries the bones
    /// as nodes and a skin (rest pose only, no animation).
    ///
    /// # Errors
    ///
    /// Fails when the mesh has no vertices or no triangles, or is too large for glTF.
    pub fn to_glb(&self) -> Result<Vec<u8>> {
        self.glb(None)
    }

    /// Like [`Geometry::to_glb`], and adds the clip as a glTF animation (translation and rotation
    /// of every bone; scale is not animated). The clip must animate exactly the model's bones, in
    /// the same order.
    ///
    /// # Errors
    ///
    /// Fails like [`Geometry::to_glb`], and when the model has no usable skeleton or the clip's
    /// bone count differs from it.
    pub fn to_glb_animated(&self, clip: &crate::animation::Clip, name: &str) -> Result<Vec<u8>> {
        self.glb(Some((clip, name)))
    }

    fn glb(&self, clip: Option<(&crate::animation::Clip, &str)>) -> Result<Vec<u8>> {
        let verts = self.vertices();
        if verts.is_empty() || self.indices.is_empty() {
            return Err(Error::format("the mesh has no vertices or no triangles"));
        }
        let (v_len, i_len) = (verts.len() * 12, self.indices.len() * 2);
        let uv_len = verts.len() * 8;
        let i_padded = i_len.div_ceil(4) * 4;
        if u32::try_from(v_len + uv_len + i_padded + verts.len() * 8 + 65_536).is_err() {
            return Err(Error::limit("the mesh is too large for glTF"));
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut bin = Vec::new();
        for v in &verts {
            for (k, c) in v.pos.iter().enumerate() {
                min[k] = min[k].min(*c);
                max[k] = max[k].max(*c);
                bin.extend(c.to_le_bytes());
            }
        }
        for v in &verts {
            for c in v.uv {
                bin.extend(c.to_le_bytes());
            }
        }
        for i in &self.indices {
            bin.extend(i.to_le_bytes());
        }
        bin.resize(v_len + uv_len + i_padded, 0);
        let rig = self.rig_json(&mut bin, verts.len());
        let (extra_attr, mut extra_access, mut extra_views, node_json, mut skin_json) =
            rig.unwrap_or_default();
        if let Some((clip, name)) = clip {
            let bones = self.skeleton().map_or(0, |s| s.bones.len());
            if node_json.is_empty() || clip.tracks.len() != bones {
                return Err(Error::format(
                    "the clip does not match the model's skeleton",
                ));
            }
            let (access, views, anim) = animation_json(&mut bin, clip, name);
            extra_access.push_str(&access);
            extra_views.push_str(&views);
            skin_json.push_str(&anim);
        }
        let json = format!(
            concat!(
                r#"{{"asset":{{"version":"2.0","generator":"Allods Data Viewer"}},"#,
                r#""scene":0,"scenes":[{{"nodes":[{scene_nodes}]}}],"nodes":[{nodes}],{skins}"#,
                r#""meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"TEXCOORD_0":1{attr}}},"indices":2}}]}}],"#,
                r#""accessors":[{{"bufferView":0,"componentType":5126,"count":{vc},"type":"VEC3","min":[{x0},{y0},{z0}],"max":[{x1},{y1},{z1}]}},"#,
                r#"{{"bufferView":1,"componentType":5126,"count":{vc},"type":"VEC2"}},"#,
                r#"{{"bufferView":2,"componentType":5123,"count":{ic},"type":"SCALAR"}}{access}],"#,
                r#""bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{v_len},"target":34962}},"#,
                r#"{{"buffer":0,"byteOffset":{v_len},"byteLength":{uv_len},"target":34962}},"#,
                r#"{{"buffer":0,"byteOffset":{i_off},"byteLength":{i_len},"target":34963}}{views}],"#,
                r#""buffers":[{{"byteLength":{bin_len}}}]}}"#,
            ),
            scene_nodes = if node_json.is_empty() {
                "0".to_owned()
            } else {
                "0,1".to_owned()
            },
            nodes = if node_json.is_empty() {
                r#"{"mesh":0}"#.to_owned()
            } else {
                format!(r#"{{"mesh":0,"skin":0}},{node_json}"#)
            },
            skins = skin_json,
            attr = extra_attr,
            access = extra_access,
            views = extra_views,
            vc = verts.len(),
            ic = self.indices.len(),
            x0 = min[0],
            y0 = min[1],
            z0 = min[2],
            x1 = max[0],
            y1 = max[1],
            z1 = max[2],
            i_off = v_len + uv_len,
            v_len = v_len,
            uv_len = uv_len,
            i_len = i_len,
            bin_len = bin.len(),
        );
        let mut json = json.into_bytes();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend((total as u32).to_le_bytes());
        out.extend((json.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(json);
        out.extend((bin.len() as u32).to_le_bytes());
        out.extend(b"BIN\0");
        out.extend(bin);
        Ok(out)
    }

    /// Appends joints, weights and inverse bind matrices to `bin` and returns the JSON pieces for
    /// them: extra attributes, accessors, buffer views, bone nodes and the skin. `None` when the
    /// model has no readable skeleton or no weights.
    #[allow(clippy::type_complexity)]
    fn rig_json(
        &self,
        bin: &mut Vec<u8>,
        vertex_count: usize,
    ) -> Option<(String, String, String, String, String)> {
        let skeleton = self.skeleton().ok()?;
        let skin = self.skin()?;
        let n = skeleton.bones.len();
        if n == 0 || n > 255 || skin.len() != vertex_count || !skeleton.is_forest() {
            return None;
        }
        // A bone matrix may carry a uniform scale `a` (18.0 models: 1.2); a node keeps the ratio
        // of its scale to its parent's, the rotation is normalised.
        let world = skeleton.world_pose();
        let scale_of = |m: &[f32; 9]| (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        let a: Vec<f32> = world.iter().map(|(m, _)| scale_of(m)).collect();
        let mut local = skeleton.local_pose();
        let mut node_scale = vec![1.0f32; n];
        for (i, b) in skeleton.bones.iter().enumerate() {
            let ap = b.parent.map_or(1.0, |p| a[p]);
            if a[i] < 1e-6 || ap < 1e-6 {
                return None;
            }
            let (r, t) = &mut local[i];
            for x in r.iter_mut() {
                *x /= a[i] * ap;
            }
            for x in t.iter_mut() {
                *x /= ap * ap;
            }
            node_scale[i] = a[i] / ap;
        }
        // Only proper rotations can become quaternions.
        let proper = |m: &[f32; 9]| {
            let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
                + m[2] * (m[3] * m[7] - m[4] * m[6]);
            (det - 1.0).abs() < 1e-2
        };
        if !local
            .iter()
            .all(|(r, t)| proper(r) && t.iter().all(|c| c.is_finite()))
        {
            return None;
        }
        let joints_at = bin.len();
        for s in &skin {
            for b in s.bones {
                bin.push(b.filter(|&b| usize::from(b) < n).unwrap_or(0));
            }
        }
        let weights_at = bin.len();
        for s in &skin {
            let mut w = s.weights;
            for (k, b) in s.bones.iter().enumerate() {
                if b.is_none_or(|b| usize::from(b) >= n) {
                    w[k] = 0;
                }
            }
            let sum: u32 = w.iter().map(|&x| u32::from(x)).sum();
            if sum == 0 {
                w = [255, 0, 0, 0];
            } else if let Some(big) = (0..4).max_by_key(|&k| w[k]) {
                // Stored weights add up to 255 or 256, sometimes a little less: put the rounding
                // difference on the biggest weight so that they add up to exactly 255.
                let fixed = i64::from(w[big]) + 255 - i64::from(sum);
                w[big] = u8::try_from(fixed.clamp(0, 255)).unwrap_or(255);
            }
            bin.extend(w);
        }
        let ibm_at = bin.len();
        // Inverse bind matrix, column-major: rotation = transpose of the stored matrix, translation =
        // the stored position (the file keeps the inverse bind transform).
        for b in &skeleton.bones {
            let m = &b.matrix;
            for j in 0..3 {
                for c in [m[j * 3], m[j * 3 + 1], m[j * 3 + 2], 0.0f32] {
                    bin.extend(c.to_le_bytes());
                }
            }
            for c in [b.position[0], b.position[1], b.position[2], 1.0f32] {
                bin.extend(c.to_le_bytes());
            }
        }
        let vc = vertex_count;
        let mut nodes = Vec::with_capacity(n);
        for (i, b) in skeleton.bones.iter().enumerate() {
            let (r, t) = &local[i];
            let q = quaternion(r);
            let children: Vec<String> = skeleton
                .bones
                .iter()
                .enumerate()
                .filter(|(_, c)| c.parent == Some(i))
                .map(|(k, _)| (k + 2).to_string())
                .collect();
            let name = b.name.replace(['\\', '"'], "_");
            let kids = if children.is_empty() {
                String::new()
            } else {
                format!(r#","children":[{}]"#, children.join(","))
            };
            let sc = node_scale[i];
            let scale = if (sc - 1.0).abs() > 1e-4 {
                format!(r#","scale":[{sc},{sc},{sc}]"#)
            } else {
                String::new()
            };
            nodes.push(format!(
                r#"{{"name":"{name}","translation":[{},{},{}],"rotation":[{},{},{},{}]{scale}{kids}}}"#,
                t[0], t[1], t[2], q[0], q[1], q[2], q[3]
            ));
        }
        let root = skeleton.bones.iter().position(|b| b.parent.is_none())? + 2;
        // The bones hang under one extra node (index 1) so that the scene lists two roots only.
        let node_json = format!(
            r#"{{"name":"Skeleton","children":[{root}]}},{}"#,
            nodes.join(",")
        );
        let joints: Vec<String> = (0..n).map(|k| (k + 2).to_string()).collect();
        let skin_json = format!(
            r#""skins":[{{"inverseBindMatrices":5,"joints":[{}],"skeleton":1}}],"#,
            joints.join(",")
        );
        let attr = r#","JOINTS_0":3,"WEIGHTS_0":4"#.to_owned();
        let access = format!(
            r#",{{"bufferView":3,"componentType":5121,"count":{vc},"type":"VEC4"}},{{"bufferView":4,"componentType":5121,"normalized":true,"count":{vc},"type":"VEC4"}},{{"bufferView":5,"componentType":5126,"count":{n},"type":"MAT4"}}"#
        );
        let views = format!(
            r#",{{"buffer":0,"byteOffset":{joints_at},"byteLength":{jl},"target":34962}},{{"buffer":0,"byteOffset":{weights_at},"byteLength":{jl},"target":34962}},{{"buffer":0,"byteOffset":{ibm_at},"byteLength":{il}}}"#,
            jl = vc * 4,
            il = n * 64
        );
        Some((attr, access, views, node_json, skin_json))
    }

    /// Wavefront OBJ text (positions, UVs with V flipped, triangles).
    #[must_use]
    pub fn to_obj(&self, name: &str) -> String {
        use std::fmt::Write as _;
        let mut s = format!("# Allods Data Viewer\no {name}\n");
        for v in self.vertices() {
            let _ = writeln!(s, "v {} {} {}", v.pos[0], v.pos[1], v.pos[2]);
        }
        for v in self.vertices() {
            let _ = writeln!(s, "vt {} {}", v.uv[0], 1.0 - v.uv[1]);
        }
        for t in self.indices.as_chunks::<3>().0 {
            let (a, b, c) = (t[0] + 1, t[1] + 1, t[2] + 1);
            let _ = writeln!(s, "f {a}/{a} {b}/{b} {c}/{c}");
        }
        s
    }
}

/// Appends animation samplers to `bin` and returns accessors, buffer views and the `animations`
/// JSON member. Accessor 6 and buffer view 6 are the key times, then a translation and a rotation
/// accessor per bone (bone `b` animates node `b + 2`).
fn animation_json(
    bin: &mut Vec<u8>,
    clip: &crate::animation::Clip,
    name: &str,
) -> (String, String, String) {
    let frames = usize::from(clip.frames);
    let fps = f32::from(clip.fps.max(1));
    let times_at = bin.len();
    for f in 0..frames {
        bin.extend((f as f32 / fps).to_le_bytes());
    }
    let last = frames.saturating_sub(1) as f32 / fps;
    let mut access = format!(
        r#",{{"bufferView":6,"componentType":5126,"count":{frames},"type":"SCALAR","min":[0],"max":[{last}]}}"#
    );
    let mut views = format!(
        r#",{{"buffer":0,"byteOffset":{times_at},"byteLength":{}}}"#,
        frames * 4
    );
    let mut channels = Vec::new();
    let mut samplers = Vec::new();
    for (b, poses) in clip.tracks.iter().enumerate() {
        for (k, (path, comps)) in [("translation", 3usize), ("rotation", 4)]
            .into_iter()
            .enumerate()
        {
            let at = bin.len();
            for p in poses {
                if k == 0 {
                    for c in p.position {
                        bin.extend(c.to_le_bytes());
                    }
                } else {
                    for c in p.rotation {
                        bin.extend(c.to_le_bytes());
                    }
                }
            }
            let view = 7 + 2 * b + k;
            let kind = if comps == 3 { "VEC3" } else { "VEC4" };
            views.push_str(&format!(
                r#",{{"buffer":0,"byteOffset":{at},"byteLength":{}}}"#,
                frames * comps * 4
            ));
            access.push_str(&format!(
                r#",{{"bufferView":{view},"componentType":5126,"count":{frames},"type":"{kind}"}}"#
            ));
            let sampler = samplers.len();
            samplers.push(format!(
                r#"{{"input":6,"output":{view},"interpolation":"LINEAR"}}"#
            ));
            channels.push(format!(
                r#"{{"sampler":{sampler},"target":{{"node":{},"path":"{path}"}}}}"#,
                b + 2
            ));
        }
    }
    let name = name.replace(['\\', '"'], "_");
    let anim = format!(
        r#""animations":[{{"name":"{name}","samplers":[{}],"channels":[{}]}}],"#,
        samplers.join(","),
        channels.join(",")
    );
    (access, views, anim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_stride_is_read_from_the_declaration() {
        let xdb = "<root><vertexDeclarations><Item><stride>36</stride></Item></vertexDeclarations></root>";
        assert_eq!(declared_stride(xdb), Some(36));
    }

    #[test]
    fn declared_stride_rejects_truncated_descriptors() {
        assert_eq!(declared_stride("<vertexDeclarations><stride>36"), None);
        assert_eq!(declared_stride("<root><stride>36</stride></root>"), None);
    }

    #[test]
    fn declared_stride_rejects_corrupted_values() {
        let bad = "<vertexDeclarations><stride>{}</stride></vertexDeclarations>";
        assert_eq!(declared_stride(&bad.replace("{}", "abc")), None);
        assert_eq!(declared_stride(&bad.replace("{}", "4")), None);
        assert_eq!(declared_stride(&bad.replace("{}", "99999")), None);
    }

    #[test]
    fn stride_with_finite_positions_wins_over_a_smaller_one() {
        // 8 vertices of 36 bytes also divide by 24 and keep all indices inside (12 vertices),
        // but read at 24 bytes the filler becomes NaN positions.
        let mut v = Vec::new();
        for i in 0..8u32 {
            for c in [i as f32, 1.0, 2.0] {
                v.extend(c.to_le_bytes());
            }
            v.extend([0xffu8; 24]);
        }
        assert_eq!(guess_stride(&v, &[0, 1, 2, 5, 6, 7]).unwrap(), 36);
    }

    #[test]
    fn glb_has_a_consistent_container() {
        let mut vb = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 1.0]] {
            for c in p {
                vb.extend(c.to_le_bytes());
            }
            vb.extend(0.5f32.to_le_bytes());
            vb.extend(0.25f32.to_le_bytes());
            vb.resize(vb.len() + 4, 0);
        }
        let g = Geometry {
            vertex_data: vb,
            stride: 24,
            indices: vec![0, 1, 2],
            tail_block: 0,
            tail: Vec::new(),
        };
        let glb = g.to_glb().expect("glb");
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize,
            glb.len()
        );
        assert_eq!(glb.len() % 4, 0);
        let jl = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        let json = std::str::from_utf8(&glb[20..20 + jl]).expect("utf8");
        assert!(
            json.contains(r#""count":3,"type":"VEC3","min":[0,0,0],"max":[1,2,1]"#),
            "{json}"
        );
        assert!(json.contains(r#""byteLength":6,"target":34963"#), "{json}");
        let bl = 20 + jl;
        assert_eq!(&glb[bl + 4..bl + 8], b"BIN\0");
        let binl = u32::from_le_bytes([glb[bl], glb[bl + 1], glb[bl + 2], glb[bl + 3]]) as usize;
        assert_eq!(binl, 3 * 12 + 3 * 8 + 8); // indices padded from 6 to 8
        assert_eq!(glb.len(), bl + 8 + binl);
        let empty = Geometry {
            vertex_data: Vec::new(),
            stride: 24,
            indices: Vec::new(),
            tail_block: 0,
            tail: Vec::new(),
        };
        assert!(empty.to_glb().is_err());
    }

    fn build(vertices: &[u8], indices: &[u16]) -> Vec<u8> {
        let mut d = vec![0u8; 4];
        d.extend((vertices.len() as u32).to_le_bytes());
        d.extend(vertices);
        d.extend(1u32.to_le_bytes());
        d.extend(((indices.len() * 2) as u32).to_le_bytes());
        for i in indices {
            d.extend(i.to_le_bytes());
        }
        d.extend(2u32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d
    }

    #[test]
    fn parses_blocks_and_decodes_vertices() {
        let mut v = Vec::new();
        for i in 0..3u32 {
            for f in [i as f32, 0.0, 0.0, 0.25, 0.75] {
                v.extend(f.to_le_bytes());
            }
            v.extend([0u8; 4]); // stride 24
        }
        let g = parse(&build(&v, &[0, 1, 2]), None).expect("parse");
        assert_eq!((g.stride, g.indices.len()), (24, 3));
        let vs = g.vertices();
        assert_eq!(vs[2].pos, [2.0, 0.0, 0.0]);
        assert!(g.to_obj("t").contains("f 1/1 2/2 3/3"));
    }

    #[test]
    fn skin_reads_weights_and_bone_indices_from_the_vertex_end() {
        let mut v = vec![0u8; 36 * 2];
        v[28..36].copy_from_slice(&[255, 0, 0, 0, 9, 255, 255, 255]);
        v[36 + 28..72].copy_from_slice(&[128, 127, 0, 0, 3, 84, 255, 255]);
        let g = Geometry {
            vertex_data: v,
            stride: 36,
            indices: vec![0, 1, 0],
            tail_block: 0,
            tail: Vec::new(),
        };
        let skin = g.skin().expect("skin");
        assert_eq!(skin[0].bones, [Some(3), None, None, None]);
        assert_eq!(skin[0].weights, [255, 0, 0, 0]);
        assert_eq!(skin[1].bones, [Some(1), Some(28), None, None]);
        let small = Geometry {
            stride: 24,
            vertex_data: vec![0; 24],
            ..g
        };
        assert!(small.skin().is_none());
    }

    #[test]
    fn glb_carries_the_skeleton_and_normalised_weights() {
        let mut vb = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 1.0]] {
            for c in p {
                vb.extend(c.to_le_bytes());
            }
            vb.extend([0u8; 8]); // uv
            vb.extend([0u8; 8]); // normal, spare
            // weights 100 + 154 = 254 (one short), bones 1 and 2 stored times three
            vb.extend([100, 154, 0, 0, 3, 6, 255, 255]);
        }
        let g = Geometry {
            vertex_data: vb,
            stride: 36,
            indices: vec![0, 1, 2],
            tail_block: 0,
            tail: crate::skeleton::tests::sample(),
        };
        let glb = g.to_glb().expect("glb");
        let jl = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json = std::str::from_utf8(&glb[20..20 + jl]).expect("utf8");
        assert!(
            json.contains(r#""skins":[{"inverseBindMatrices":5,"joints":[2,3,4],"skeleton":1}]"#),
            "{json}"
        );
        assert!(json.contains(r#""JOINTS_0":3,"WEIGHTS_0":4"#), "{json}");
        assert!(json.contains(r#""name":"Hand""#), "{json}");
        let bl = 20 + jl + 8;
        let bin = &glb[bl..];
        let weights_at = 3 * 12 + 3 * 8 + 8 + 12;
        assert_eq!(&bin[weights_at..weights_at + 4], &[100, 155, 0, 0]);
        assert_eq!(&bin[weights_at - 12..weights_at - 8], &[1, 2, 0, 0]);
    }

    #[test]
    fn glb_can_carry_an_animation_for_a_matching_skeleton() {
        let mut vb = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 1.0]] {
            for c in p {
                vb.extend(c.to_le_bytes());
            }
            vb.extend([0u8; 16]);
            vb.extend([100, 155, 0, 0, 3, 6, 255, 255]);
        }
        let g = Geometry {
            vertex_data: vb,
            stride: 36,
            indices: vec![0, 1, 2],
            tail_block: 0,
            tail: crate::skeleton::tests::sample(),
        };
        let n = g.skeleton().expect("skeleton").bones.len();
        let pose = crate::animation::Pose {
            position: [1.0, 2.0, 3.0],
            scale: 1.0,
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let clip = crate::animation::Clip {
            fps: 30,
            frames: 2,
            tracks: vec![vec![pose; 2]; n],
        };
        let glb = g.to_glb_animated(&clip, "Idle").expect("glb");
        let jl = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json = std::str::from_utf8(&glb[20..20 + jl]).expect("utf8");
        assert!(json.contains(r#""animations":[{"name":"Idle""#), "{json}");
        assert_eq!(json.matches(r#""path":"rotation""#).count(), n);
        assert!(json.contains(r#""type":"SCALAR","min":[0]"#), "{json}");
        let short = crate::animation::Clip {
            tracks: vec![vec![pose; 2]; n + 1],
            ..clip
        };
        assert!(g.to_glb_animated(&short, "x").is_err());
    }

    #[test]
    fn the_tail_is_cut_to_the_declared_size() {
        let mut d = build(&[0u8; 24 * 3], &[0, 1, 2]);
        let n = d.len();
        d[n - 4..].copy_from_slice(&5u32.to_le_bytes());
        d.extend([7u8; 9]);
        let g = parse(&d, Some(24)).expect("parse");
        assert_eq!(g.tail, vec![7u8; 5]);
    }

    #[test]
    fn garbage_never_panics() {
        for n in 0..64 {
            let d: Vec<u8> = (0..n).map(|i| (i * 37) as u8).collect();
            let _ = parse(&d, None);
        }
        let mut d = build(&[0u8; 48], &[0, 1, 2]);
        d[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&d, None).is_err());
    }

    #[test]
    fn indices_outside_vertices_reject_a_stride() {
        let v = vec![0u8; 24 * 2];
        assert!(guess_stride(&v, &[0, 1, 5]).is_err());
    }
}
