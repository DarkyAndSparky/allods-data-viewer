# Model geometry `X.(Geometry).bin`

**Status: container verified (2026-11-14): vertex block, index block and the next block size are read exactly; vertex stride is guessed, the tail is not decoded.** Samples: `AirElemental` and
`Anakonda` from the owner's `Desktop.zip` (2026-11-10). Analysed 2026-11-11.

## Container

- A zlib stream. Inflated sizes: AirElemental 214 606 bytes, Anakonda 97 766 bytes.
- `Kaa.(Geometry).bin` in the Anakonda folder inflates to a file that starts with the same
  bytes as `Anakonda.(Geometry).bin` (same vertex block header and size); we did not compare
  the rest.

## Layout seen

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | u32 | blob id 0 = vertex buffer (see "Blob list" below) |
| 4 | u32 | size in bytes of the vertex block |
| 8 | block size | vertices, little endian, fixed stride |
| 8 + block | u32 | blob id 1 = index buffer |
| 12 + block | u32 | 32 718 (AirElemental), 50 154 (Anakonda); meaning unknown, it is not a byte count of the index data |
| 16 + block | u16 each | triangle list: indices, three per triangle |
| after the indices | rest | unknown table, 81 104 bytes in AirElemental; looks like a list of small integers |

## Vertices

- First 20 bytes of a vertex: position as 3 x f32, then texture coordinates as 2 x f32.
  Checked on real data: positions are plausible (AirElemental spans about 6.8 x 5.7 x 5.5
  units; Anakonda is a long thin body) and the triangles drawn from the index list form
  a recognisable shape.
- **The stride is not constant.** AirElemental: 36 bytes (2799 vertices; the last 16 bytes
  look like packed normal/tangent bytes plus a byte pattern `ff 00 00 00` that repeats).
  Anakonda: 32 bytes (1290 vertices). Where the stride is stored is not known; the file
  may carry it in the tail or in the resource database (G-19).
- In AirElemental the first indices are `0 1 2 3 4 5 ...`, then ordinary triangles. All
  indices in the first 16 361 u16 values are below the vertex count (2799), then values
  jump above it, so about 16 360 u16 belong to the triangle list (the exact count is not
  known; 16 360 is not a multiple of 3, so some sub-meshes may share the stream).

## Not known

- Stride guess (2026-11-20): candidates 24, 28, 32, 36 that divide the vertex block and keep all indices inside; the winner has the fewest non-finite positions (first three floats of each vertex), then the `ff ff ff` tag, then the fewest vertices no index points to. Verified on client 2.0 models `Ship4_Hull` (28 bytes, 12 288 vertices), `ElfFemale` (36, 21 664), `ElfFemale_CutScene` (36, 21 734), `HadaganFemale` (36, 23 830): all four have only finite positions, which the old rule (first fit, picked 24) failed. The real stride is still not stored anywhere that is known.
- Vertex stride rule, vertex format flags, number of sub-meshes and their index ranges.
- Whether the bone index of a vertex is the skeleton index or a palette slot (see "Vertex skin data").
- Which texture goes on which sub-mesh (materials are a separate, unknown format).
- Whether the unit is metres or arbitrary.

## Next steps

1. Look for the stride and sub-mesh table in the tail of the inflated data by comparing
   several models with the same vertex count to stride ratio.
2. Get the resource database; it probably describes geometry resources (`types.xml` lists
   the types).
3. Only then write a reader and a glTF export (M7).

## Container, confirmed (2026-11-14)

Compared with the original AODataViewer (author's permission, no code copied) and checked on
AirElemental, Anakonda and Kaa:

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | u32 | blob id 0 (vertex buffer) |
| 4 | u32 `V` | vertex block size in bytes |
| 8 | `V` | vertices |
| 8 + V | u32 | blob id 1 (index buffer) |
| 12 + V | u32 `F` | **index block size in bytes**; always a multiple of 6 (three u16 per triangle) |
| 16 + V | `F` | u16 indices |
| 16 + V + F | u32 | blob id 2 (skeleton) |
| 20 + V + F | u32 `S` | size of the next block (5620 in AirElemental, 6308 in Anakonda/Kaa) |
| 24 + V + F | rest | in Anakonda exactly `S` bytes; in AirElemental 81100 bytes: `S` bytes and then more blocks (a count `3`, sizes 75472, 44912, ...), not decoded |

Counts: AirElemental 2799 vertices, 5453 triangles; Anakonda 1290 vertices, 8359 triangles.
This replaces the earlier note that the u32 at `12 + V` is not a byte count: it is.

The original viewer also guesses the stride: candidates 24, 28, 32 and 36 that divide `V`, and
a candidate is accepted when the last three bytes of the 5th or 7th vertex are `ff ff ff`.
We additionally require that every index is below the vertex count. Result: AirElemental 36,
Anakonda 32. Position at +0 and UV at +12 are the first 20 bytes of each vertex.
`adv geom info` and `adv geom obj` (0.1.11) use this. `adv geom glb` (0.1.21) writes a binary glTF 2.0
(positions, UVs not flipped, u16 triangles); a file made from AirElemental loads in Three.js `GLTFLoader` with
2799 vertices and 5453 triangles. The triangle winding is not verified against the game.

## Tail block: skeleton data (draft, 2026-11-15)

Resource file names carry their type: `<Name>.(Geometry).bin`, `<Name>.(Texture).bin` (with
`<Name>.hi.(Texture).bin` for the sharp levels), `<Name>.<Clip>.(SkeletalAnimation).bin`. The app
uses the tag to tell models from textures without opening the files.

## Skeleton in the tail block (2026-11-16, read by `adv_core::skeleton`)

The block after the index data (`S` bytes, see the table above) holds the skeleton. Checked on
AirElemental (46 bones), Anakonda and Kaa (52 bones). Status: the layout below reads all three files
into a consistent skeleton (one root, no loops, sensible names and parents); what the matrix and position
mean is not verified against the game.

- It starts with four `(offset, count)` pairs of `u32`: AirElemental `(32, 46) (2416, 46) (5512, 46)
  (2768, 46)`. **Each offset is relative to the address of its own pair**, so pair `k` (at byte `8 * k`)
  points to `8 * k + offset`: bone entries at 32, name references at 2424, a float table at 2792, a
  `u16` table at 5528. Everything fits the block exactly (32 + 46 x 52 = 2424).
- **Bone entry, 52 bytes**: 9 `f32` matrix, 3 `f32` position, `u32` **parent index** (`65535` = none).
  AirElemental has one root, `VisualSceneNode`; Anakonda's hierarchy reads `Tongue_03` under
  `Tongue_02`, `Jaw` under `Jaw_root`, `Head` under `HeadRoot`. Row or column order of the matrix and
  the frame of the position are not verified. Computed as the difference of positions, two rest
  positions match two animation channels exactly (`skeletal-animation.md`).
- **Name reference, 8 bytes**: `u32` offset relative to the address of the reference, `u32` length including
  the terminating zero. Names sit in a pool after the tables, each padded to 4 bytes; bone `i` has name `i`
  (`Spine1`, `L_arm`, `L_forearm`, ... for AirElemental).
- Float table (pair 3): not decoded; may hold inverse bind matrices.
- `u16` table (pair 4): a permutation of `0..count` that also ends every animation clip of the model;
  its meaning is unknown.
- AirElemental has more blocks after the first `S` bytes (a count 3, then sizes 75472, 44912, 955, 27856,
  2841 and 1740); not decoded.

## Vertex skin data (2026-11-16, read by `Geometry::skin`)

The last 8 bytes of a vertex are four weights (`u8`) and four bone indices (`u8`), in that order:
stride 36 bytes 28..32 and 32..36, stride 32 bytes 24..28 and 28..32. Checked on all vertices of
AirElemental and Anakonda:

- a slot has a weight exactly when its index is not `0xFF` (0 mismatches in 2799 vertices);
- the weights add up to 255 or 256 (2416 and 383 of 2799 vertices in AirElemental);
- the stored index is **three times the bone number** (values 0, 3, ... 84, so bones 0..28 in
  AirElemental, below the 46 of the skeleton; the shader's register layout), and it indexes the
  bones in the order of the skeleton (see "Bind pose" below for the check: the vertices of a bone
  sit around that bone in the skeleton order, not in the order of the 46 `u16` table).

Other bytes of a 36-byte vertex: 20..23 is a normal as three unsigned bytes (x / 255 x 2 - 1, length
1 in all 2000 vertices tested) followed by `ff`; bytes 24..28 of a 36-byte vertex hold something
else (tangent?), not decoded. Stride 32: bytes 20..23 are the normal as well.

## Bind pose (2026-11-16, `Skeleton::world_pose` / `local_pose`)

**The bone entries hold the inverse bind transform**: with `M` the stored 3x3 matrix (read as nine
numbers in file order, row by row) and `p` the stored position, the bone sits at `-(M * p)` in model space
and `M` is its rotation in model space. Evidence:

- Vertices whose first weight is 200 or more, grouped by bone: the centre of each group is **0.55 units**
  from the bone computed this way in AirElemental (25 bones with 15 or more vertices; the mean distance
  to a random bone is 2.65) and **0.21** in Anakonda (random 3.05). The other readings tried (position as
  is, `-p`, `-M^T * p`, positions composed down the parents) all fit as badly as chance.
- The position of a bone relative to its parent, `parentRotation^T * (position - parentPosition)`, equals
  the first three parameters of an animation channel **exactly**: `Spine1` `(0.42, 0.009, -0.09)`,
  `R_forearm` `(-0.328, 0.006, -0.945)`, `Head` `(0, -1.529, 0.596)` (see `skeletal-animation.md`).
- Binary glTF written from this (bones as nodes with these local transforms, inverse bind matrices
  `[M^T | p]`, `JOINTS_0` and `WEIGHTS_0`) loads in Three.js and the bind pose reproduces every vertex
  to 1e-5 (AirElemental 2799 vertices, Anakonda 1290). That check is circular for the matrices
  (they are built to cancel) and only proves the file is consistent; the two bullets above are the real evidence.

## Blob list (2026-11-17, verified on 295 sample files)

Every geometry, animation clip and map `terrainDump` file in our samples (295 files) is an exact list of blobs, with ids 0, 1, 2 first in geometry files (`adv blobs <file>` prints the list; `adv_core::blob`). The inflated file is a plain list of blobs: `u32 localID`, `u32 size`, `size` bytes. In the
database record of the model (`Geometry`) the vertex buffer, index buffer and skeleton are
declared as blobs with `localID` 0, 1 and 2, which is exactly what the "unknown" words above
are. Later IDs hold further blobs (KD-tree geometry, portal and area fragments; the "count 3,
sizes ..." tail seen in AirElemental). The vertex declaration (stride, component types and
offsets), the sub-meshes (model elements with vertex and index ranges per LOD) and the
material to texture binding are **not in the `.bin`**; they are in the database record. See
[../TOOLS-STUDY.md](../TOOLS-STUDY.md).

## Declared stride (2026-11-23)
`geometry::declared_stride(xdb)` reads the `<stride>` of the first `<vertexDeclarations>` block of a Geometry `.xdb` descriptor (accepted range 16 to 256 bytes). It is a helper with three tests; the CLI does not yet pass a descriptor, so `--stride` remains the way to set it. Status: draft; the object-image decode is NOT done.
