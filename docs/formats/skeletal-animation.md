# Skeletal animation `X.<Clip>.(SkeletalAnimation).bin`

**Status: draft (2026-11-15). The header and the layout of the channel records are understood; the
meaning of the channel parameters and the link between channels and bones are not.**
Samples: 14 clips of `AirElemental` (the owner's `Desktop.zip`, 2026-11-10): Attack, Death, Hit,
HitCritical, Idle, Idle01, KnockdownEnd/Loop/Start, Ready, ReadySpell, Run, SpellCastDirected, SpellCastOmni.
The reader `adv_core::animation` uses only the first part (header); the app shows it.

## Container (verified on 14 clips)

- A zlib stream (`78 9c`). All values little endian.
- Inflated layout: `u32 0`, `u32 payload` (equals inflated length minus 8 in all 14 clips), then the header below.

## Header (verified on 14 clips)

| Offset | Value | Meaning |
| --- | --- | --- |
| 8 | u16 `30` | frames per second (30 in all clips) |
| 10 | u16 frames | frame count: 25 (Hit), 31 (Ready and others), 46, 51, 61, 91 (Idle01), 121 (Death). Both end frames are counted: `(frames - 1) / 30` gives 0.8 s, 1 s, 2 s, 3 s, 4 s for the round ones |
| 12 | u32 `408` | size of the descriptor area (the same in every clip) |
| 16 | u32 `46` | bone count (the geometry file of AirElemental has the same number) |

The size of the clip data grows with `frames` by 6 bytes per frame and the vectors of every channel
grow by `frames * components`, which is how the frame count was confirmed.

## Offsets and vectors (verified on `Hit` and `Death`)

Offsets in this file are **relative to the address of the field itself**: a vector is a pair
`(u32 offset, u32 count)` at address `p` and its data lie at `p + offset`. Following the pointers
of the first channel record of `Hit` leads to a name, four floats and 75 `i16` values, in that order.

## Channel records (draft, checked on `Hit`: 21 records, each followed by a bone name)

A record is `u32 tag, vector data, vector params` (the fields `tag`, then `offset, count` twice,
at 4-byte aligned addresses from byte 408 on). A search for the pattern "`tag`, `data.count == components * frames`,
`params.count == components + 1`" finds 21 records in `Hit`. The bone name is the zero-terminated string just
before the params of the record (`Spine1`, `L_arm`, `R_arm`, `R_forearm`, `Hips`, `column_4`, `dust`,
`R_finger_5/6/7`, `L_finger_4/5/6/7`, `Head`, `R_braslet`, `L_braslet`, `L_sholder`, `R_sholder`, `R_hand`,
`L_hand`). The other bones of the 46 have no record in this clip (they are not animated in it).

| Tag | Components | Records in Hit | Params (first values) |
| --- | --- | --- | --- |
| `0x0003000f` | 3 | 13 (Spine1, L_arm, R_arm, R_forearm, Head, braslets, sholders, hands) | `(x, y, z, 1.0)`, e.g. `L_arm (1.014, 0.507, 0.176)`, `R_arm (-1.014, -0.507, -0.176)`, `L_sholder (0.393, -0.599, 0.367)`, `R_sholder (-0.393, -0.599, 0.367)` |
| `0x00060008` | 6 | 3 (Hips, column_4, dust) | six floats, mostly near zero apart from two values |
| `0x00040007` | 4 | 7 (finger bones) | `(a, b, c, -0.0, ...)`, mirrored in sign between the L and R finger of the same number |

- `data` holds `frames * components` values of 16 bits, **interleaved by frame** and **signed**.
- The params of the 3-component records look like a base vector of the bone (a position): left and right
  bones are mirrored in x, as a symmetrical model would be. The fourth float is 1.0 (a scale?). The `i16`
  data are probably offsets from that base, but the factor that converts them (for example `1 / 32768`)
  is **not known**. With `i16 / 32768` the first frames of `L_arm` give offsets of about 0.1 to 0.16, which
  is plausible for an arm of length 1; this is a guess, not a result.
- Hypothesis for the 4- and 6-component records (rotations, or rotation plus something) is not tested.
- A direct comparison of these base vectors with the 13-value bone entries in the tail of the geometry file did
  not give matches (no bone of the geometry has the position `(1.014, 0.507, 0.176)`), so the geometry
  table is stored in another space or has another element layout.

## Findings of 2026-11-15 (second pass, still draft)

- **The params of a 3-component channel are a local rest position of a bone, with the sign flipped.**
  Take the 52-byte bone entries from the geometry tail (`geometry-bin.md`), treat the matrix and position as
  world values and the last `u32` as the parent index, and compute the local position
  `parentRotation^T * (position - parentPosition)`. Two channels of `Hit` then match exactly: the channel
  `(-0.328, 0.006, -0.945)` has the same length (1.000) as bone 4's local position `(0.333, -0.005, 0.943)`,
  and `(0, -1.529, 0.596)` is bone 26's `(0, 1.529, -0.596)` negated. The other 3-component channels
  (`(1.014, 0.507, 0.176)` and so on) match no local position, so not every `0x3000f` channel is a
  translation, or its frame is different. Only two matches: treat as a lead, not a result.
- **Clips loop.** In `Hit` the first and last frame are equal in the 3- and 6-component channels, which
  fits an idle or hit clip that returns to its start.
- **6-component channels hold two vectors per frame.** The first three values run through the whole range of
  `i16` and wrap around (`-30032`, then `28840`, `22320`, ...), the way an angle does when 65536 stands for a
  full turn; the last three change slowly and look like a position. This is a hypothesis; the params of such a
  channel are seven floats, the last one `1.0`.
- **4-component finger channels** start with `(0, 0, 0, 0)`, and after that three of the four values stay
  constant for many frames while one grows smoothly; a frame with `32767` follows the first one. A
  quaternion in this form is not confirmed.

**Update 2026-11-16:** the geometry file does carry the bone names and parents (`geometry-bin.md`,
"Skeleton in the tail block"), so a channel can be bound to a bone by its name. With that,
the channel `(-0.328, 0.006, -0.945)` belongs to `R_forearm` (bone 4, parent `R_arm`),
`(0, -1.529, 0.596)` to `Head` (bone 26, parent `Spine1`) and `(0.42, 0.009, -0.09)` to `Spine1`.
**With the bind pose read correctly (the file stores the inverse bind transform, see `geometry-bin.md`,
"Bind pose") these are exactly the bones' local rest positions, no sign flip.** So the first
three parameters of a 3-component channel are the rest translation of the bone; the data is
probably an offset from it. The scale of the data is still unknown.

## Records are in bone order (2026-11-16, checked on `Hit` of AirElemental)

The channel records are **consecutive, 20 bytes each, one per bone, in the bone order of the geometry
file**: 46 records starting at byte 420, and the name written near each record's params is the name of
bone `k` in the geometry skeleton (`Spine1`, `L_arm`, `L_forearm`, ..., `L_hand_con`; all 46 agree). So a
clip binds to a model by bone index, and the names only confirm it. The earlier idea of finding records by
the shape `params.count == components + 1` missed those with other tags.

Record: `u32 tag`, `u32 data_offset`, `u32 data_count`, `u32 params_offset`, `u32 params_count`, offsets relative to
the address of their own field (`data` at `record + 4 + data_offset`, `params` at `record + 12 + params_offset`).
Tags seen in `Hit` and what they carry (components = `tag >> 16`; the low 16 bits differ and are not understood):

| Tag | Components | Params | Bones |
| --- | --- | --- | --- |
| `0x7f` | 0 (no data) | 7 | bones that do not move: `column`, `Slot_*`, `Global`, `VisualSceneNode`, `R_hand_con`, ... |
| `0x3000f` | 3 | 4 | arms, shoulders, hands, `Head`, `Spine1`, bracelets |
| `0x60008` | 6 | 7 | `Hips`, `column_4`, `dust` |
| `0x30078` | 3 | 10 | `column_1..3`, `*_braslet_con` |
| `0x40007` | 4 | 5 | fingers 5, 7, 6 of each hand |
| `0x20037` | 2 | 7 | fingers 4 and 3 (right), 2 and 3 (left) |
| `0x30027`, `0x30017` | 3 | 6 | fingers 2 and 1 |
| `0x1006f` | 1 | 6 | `L_forearm` |

For a `0x3000f` record the first three params are the bone's rest translation (verified for three bones),
the fourth is `1.0`. Its data is 25 x 3 `i16`, small in magnitude for most bones, and the vector length
(x / 32767) stays below 1 for most; two channels reach 1.0 and two 0.997, so the data is not simply a
quaternion's x, y, z in every case. Some finger channels look like curves that pass through the full
`i16` range and wrap (an angle with 65536 as a full turn); the first values of those channels are zero or
the extreme value, which suggests the first frames hold something other than samples. Not decoded.

## Tail block (verified: identical in all 14 clips)

The last 120 bytes of every clip are the same: 28 bytes (floats `1.00085, -0.006056, 0, 1.0` and
integers 491, 65105, 65295) and then 46 `u16` values, **the same permutation of 0..45 that is stored in
the tail of `AirElemental.(Geometry).bin`** (see `geometry-bin.md`). That ties the clips to this model's
bone order. What the permutation maps (geometry order to skeleton order, or the opposite) is not known.

## Not known

- The factor that turns the `i16` values into numbers, and whether the 3-component channels are positions
  (offsets from the params) or something else; what the 4- and 6-component channels hold (rotations?).
- Which model bone index a named channel belongs to: the permutation in the tail gives an order, but the
  geometry table does not reproduce the names' positions (see above).
- The unit of time between frames beyond `fps` (it looks uniform).

## Next steps

1. Dequantise one channel under several hypotheses and check with the bind pose of the bones from
   the geometry tail (`geometry-bin.md`): a correct rule gives small angles near the bind pose.
2. Compare the rest pose (clip `Ready`, frame 0) with the bind matrices.
3. With the resource database from the owner, look for the skeleton type that holds the bone names.

## Layout confirmed by the game's debug symbols (game 1.1.02.0)

Status: **verified** (field names and sizes read from the vendor's own PDB; format facts only, no code taken).

- `SkeletalAnim` (44 bytes): u16 fps, u16 clipLength, then five `{u32 offset, u32 count}` vectors (self-relative offsets): animationAtoms, animationNames, nameOrder (u16), materialAtoms, materialNames.
- `AnimAtom` (20 bytes): u16 type, u16 length, trackData (u16 vector), floatData (f32 vector). This is our 20-byte bone record.
- Decoded form `UncompressedAnimAtom` (44 bytes): position vec3, scale f32, euler vec3, rotation quat, so the 7 float params most likely describe position, scale and euler tracks.
- `MaterialAtom` (20 bytes): u16 type, u16 length, trackData (u8 vector), floatData; material params are transparency, translateU, translateV.
- Skeleton (32 bytes): nodeInfos, nodeNames, nameOrder, transforms (local bind, 3x4 floats, 48 bytes each). NodeInfo (52 bytes): inverse bind 3x4 + u16 parent at +48.
- Open: the track quantisation (how trackData maps to values using floatData).

### Findings of 2026-11-16 (parameters of bone records)

Checked on AirElemental (46 bones), status **verified** for the first point, **draft** for the rest.

- For bones without data (tag `0x7f`) the seven floats are `position xyz`, `scale`, then three zeros. Position equals the local rest translation from the skeleton `transforms` table exactly; scale equals the length of the rest matrix columns (for example 2.2 or 0.9 on helper bones).
- For tag `0x3000f` the four floats are `position xyz` and `scale` (1.0); the three data channels per frame are then most likely the rotation (euler angles, a full turn as 65536). The position is again the rest translation.
- Tag low bits look like a mask of channels stored as constants (`0x7f` all seven constants; `0x0f` position and scale; `0x08` scale only, with six animated channels). This is a reading of the pattern, not verified.
- Not found: the euler axis order and signs. A search over all 48 axis orders and sign choices, comparing the decoded rotation of any frame with the skeleton rest rotation, gave no fit (best mean max-error 0.29 over 11 bones), so either the encoding of the angles differs or the clips are not close to the rest pose. Do not implement a decoder until this is settled.

### Findings of 2026-11-16 (sampling rules read from the game's own code)

Source: reading the sampling routine of the game's animation module (game 1.1.02.0) for format facts only; no code copied. Status: **verified** for the track layout and position/scale decoding (plausible on AirElemental: decoded positions stay within about 0.5 units of the rest translation and frame 0 equals the last frame in looping clips), **verified** for the angle unit and quaternion (see below).

The low 16 bits of an atom tag (`type`) are a mask over seven channels, in this order: position x, y, z, scale, then yaw, pitch, roll. A set bit means the channel is a constant, an unset bit means it is animated per frame. The high 16 bits (`length`) are the count of animated channels.

- Constant float channels (position, scale) take one `f32` from `floatData`.
- Animated float channels take a pair `(min, step)` from `floatData` and one **unsigned** `u16` per frame from `trackData`: `value = min + u16 * step`. Step values are tiny (about 6e-6), so they print as 0 when rounded.
- Float channels are read first (position x, y, z, scale), the angles after them. Per frame, the `u16` values are consumed in channel order, only for animated channels.
- Angles are three `u16` words (yaw, pitch, roll). A constant angle occupies a 4-byte slot of `floatData` whose low 16 bits are the value. The game builds sine and cosine tables of `2π · k / 65536` (a high-byte and a low-byte table combined with the angle-sum formula) and uses the result as a **half angle**: the quaternion `[x, y, z, w]` is the Hamilton product `Qz(yaw) · Qy(pitch) · Qx(roll)` where `Qa(u)` is the rotation quaternion with half angle `2π · u / 65536`. So a word of 32768 is one full turn of the rotation and 65536 is two turns (the sign of q is irrelevant). Verified by running the game's own conversion routine on random inputs and matching the formula exactly.
- Plausibility against the skeleton: for 11 animated bones of AirElemental the decoded rotation comes within 0.0 to 0.12 (max matrix element difference) of the skeleton's rest rotation at some frame, against 0.29 for every earlier convention. Not proof, but all 32 clips of three samples decode with every consistency rule below satisfied.
- Consistency rules (all hold on every sample clip): animated count (high 16 bits) equals 7 minus the number of set mask bits; track length equals animated count times frames; float count equals the constants plus two per animated float channel; tracks hold one word per animated channel per frame, frame by frame.
- Open: whether a clip's pose replaces or adds to the skeleton's local rest pose (static bones with all-zero angles sit under skeleton rest rotations that are not identity); the meaning of mask bits above 0x7f.
- Example: tag `0x60008` = scale constant, six animated channels (three positions with `(min, step)` pairs, three angles): `floatData` has 3 pairs + 1 scale = 7 floats.
- Earlier note that data are `i16` is superseded: they are `u16`.

### Pose semantics (draft, 2026-11-16)

A glTF export of AirElemental with decoded clips (`adv geom glb --anim`) was posed in Three.js and compared with the bind pose (mean vertex displacement, model size about 6 units). Reading a clip's pose as the **absolute local transform that replaces the skeleton's local rest pose** gave the smallest displacement in every clip tried (Idle 1.16, Hit 0.85, Ready 1.21, Run 1.27, against 1.7 to 2.6 for the inverted rotation and 1.9 to 2.0 for rest × clip or clip × rest). Using only the clip's rotations or only its positions fits worse. The residual of about 1 unit is expected, since a clip is not the bind pose; this is not proof. Scale is not exported to glTF yet.

## Header words are a blob header (2026-11-17, claimed)

The first two words `u32 0; u32 payload` are a blob header (`localID` 0, `size`), the same
container as in geometry files. The database record `SkeletalAnimation` carries `fps`,
`startFrame`, `endFrame`, `looped`, `speed`, `blendTime` and events; see
[../TOOLS-STUDY.md](../TOOLS-STUDY.md).

## Client 1.1.00.41: clips, bone names, material tracks (checked 2026-11-26)

Status: **verified** for reading; the link between track order and the skeleton of the model is **not verified** on this client.

- The format of the section above holds for client 1.1.00.41. All 5 776 clips of `Characters.pak` (2 576) and `Creatures.pak` (3 200) pass `animation::decode` with all its checks: 336 449 frames, 453 266 bone tracks.
- Poses match the sampling code of the MIT Python exporter (written for 1.1.02.0): 289 clips (every 20th), frames first, middle and last of every bone, 68 424 poses. Largest difference: position 2.3e-6, scale 5.3e-7, quaternion `|dot|` at least 0.9999992 (about 0.0016 degrees). So the meaning of the channels is confirmed: positions and scale are `min + step * code`, the three angles are `word * 2π / 65536` and combine as the product of rotations about z, y and x.
- Bone names: the name table at inflated offset 20 (`{u32 offset from the entry, u32 byte length}` per entry, text padded with zeros) has one name per bone track in all 5 776 clips. `animation::bone_names` reads it. The `nameOrder` array (offset 28) is a permutation used for lookups by name; it is not read.
- Material tracks: the material records at offset 36 and their names at offset 44 (`MaterialAtom`, 20 bytes: `u16` channel flags, `u16` sample size, byte vector of samples, an empty float vector). Flags: 1 transparency (1 byte per frame), 2 U shift (`u16`), 4 V shift (`u16`); the sample size is the sum of the flagged channels and the vector holds exactly one sample per frame. Transparency is the amount removed (opacity `1 - value`); the shifts are fractions of a turn that wrap around. Channels that are not flagged are 0, not taken from the float list. `animation::material_tracks` reads them; `adv anim` prints them.
- On the 5 776 clips 32 have material tracks (111 tracks: transparency only 102, V shift only 6, U shift only 2, transparency and V shift 1). Example: `ArchdemonMage.Death` (181 frames, 94 bones) fades 8 materials. 57 samples of 6 clips agree with the exporter (largest difference 3.7e-7).
- Not verified: that track `i` belongs to bone `i` of the model's skeleton on 1.1 (the names allow the check against the bone names of the geometry); that the pictures move correctly in the window; the interpolation between frames.

### Binding of tracks to the skeleton on client 1.1.00.41 (checked 2026-11-26)

**Bind by name, not by number.** The older sections say that a clip binds to the model by bone index and that the names only confirm it; that holds for most clips but not for all on this client. All 5 776 clips of `Characters.pak` and `Creatures.pak` were compared with the skeleton names of the geometry in their model folder (`.../<Model>/Animations/X.<Clip>.(SkeletalAnimation).bin` and `.../<Model>/*.(Geometry).bin`):

| Relation of the clip's bone names to the skeleton | Clips |
|---|---|
| identical names, same order | 4 233 |
| same set of names, other order | 36 |
| fewer bones than the skeleton, names in the skeleton's order (for example 110 of 116 on `ElfFemale`) | 1 491 |
| a name that is not in the skeleton (`KaniaFemale.SpecialAttakUnarmed`) | 1 |

So 1 527 of 5 776 clips (26 %) cannot be bound by number: 1 491 have another track count and 36 would be bound to the wrong bones. `Clip::names` carries the bone name of every track; `Geometry::to_glb_animated` binds by name: bones without a track keep their bind pose, tracks without a bone are dropped, a repeated name animates the bone once, and a clip with no name table falls back to the by-number rule (one track per bone).

End to end (every clip with the geometry of its folder): 5 716 of 5 761 clips give an animated GLB; 444 294 of 452 531 skeleton bones are animated in those files. The 45 that fail (`Creatures/EvilEyeBoss` 9 clips, `Creatures/GolemJunes` 36) fail because each of the two models has 2 bones whose local rotation is a reflection (determinant not 1); the GLB writer accepts only proper rotations (existing rule of `rig_json`), this is not new. Not checked: that the animated models look right in a viewer.
