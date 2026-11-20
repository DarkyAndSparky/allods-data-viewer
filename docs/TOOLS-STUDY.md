# Study of community tools (notes)

Written 2026-11-17 after reading four tool sets the maintainer supplied. Only **facts** are
recorded here, in our own words. License status of each tool is in
[REFERENCES.md](REFERENCES.md): EditorTools is MIT (code may be ported with a NOTICE entry),
the other three state no license (facts only, no code copying).

Status labels: **confirmed** = two independent sources or our own samples agree,
**claimed** = one source, not yet checked on our data, **open** = unknown.

## 1. AllodsOnlineEditorTools (Paulus, MIT, C#, 2026)

What it is: a CLI (`pack unpack/ls/info`, `texture bin export`, `utils compress/decompress`,
`info versions`) plus a library `ClientResources` that reads the game resource databases
(`pack.bin`, `Maps_*.bin`) of many client versions and writes `.xdb` (XML) or JSON.

What it knows that we did not:

1. **Struct layouts per client version** (V1_1_02_0, V3, V4, V7_0_00_76, V14_0_01_71, 32 bit).
   Each class has a size, field offsets and XML names. The 1.1.02.0 `Geometry` layout is
   **confirmed** by the older DatabaseUnpacker (same offsets, independent author).
   Client 17.x (64 bit, our current one) has **no layouts** there ("parse only").
2. **Geometry record** (what the `.xdb` of a model holds): vertex declarations (stride plus
   component type/offset for position, normal, colour, two UV sets, weights, indices), three
   buffers as blobs `(localID, size)` (vertex, index, skeleton), `modelElements` (name,
   material name, vertex declaration id, vertex buffer offset, LOD list with vertex and index
   ranges), materials with the diffuse texture reference, joints, scene nodes, parts, AABBs,
   KD-tree fragments (geometry, portal, area), occluders, LOD distances, flags (`vb32`,
   `useColors`, `lodModel`, `castShadows`, ...).
3. **Vertex element types** (enum, starts at 1): FLOAT1..4, SHORT2, SHORT4, COLOR4 (4 x u8,
   normalised), UBYTE4, USHORT2, USHORT4, HALF4, UNUSED. Skin: weights = COLOR4,
   indices = UBYTE4 (**confirmed** with our stride 32 reading: normal at 20, weights at 24,
   bone indices at 28).
4. **Texture type enum** (index = value in the database): DXT1, DXT3, DXT5, RGBA, L8, D24, D16,
   C565, C4444, RG16, R32F, RGBA16. Names for the "other formats" in
   [texture-bin.md](formats/texture-bin.md) (e.g. 2 097 152 bytes could be 1024x512 RGBA).
5. **Texture `.bin` container** exactly as we found it: zlib, chunks `(id, size, bytes)`,
   stops after id 0, smallest level first. **Confirmed.**
6. **Map structures** (V14, claimed): `MapResource` (name, description, internal name, map
   template, load distance, foliage distance, minimap light, `textureItem`),
   `MapRegion` (placed static objects with position, pitch/roll/yaw, scale ratio, template
   pointer; areas; layers; `binaryFile` and `binaryFileDown` for upper and lower level),
   `TerrainPackInfo` (compressed terrain file name, terra-gen up/down, splat maps, layer
   diffuse textures, lightmaps up/down, extra occlusion), `MinimapTexture` (a ready minimap
   picture with size, type, mips), `FoliageGeometry`, `ZoneResource`, `ScriptZone`.
7. **Database format document and ImHex patterns** (V1, V2 x86, V2 x64). We already read
   V2 x64; the document is a good cross-check for our `bin-db.md`.
8. **Engineering practice** worth copying in spirit: strict CLI parsing, per-version struct
   cache tests, XDB and JSON round-trip tests, a struct layout validation test, CI file.

What it cannot do: no 17.x layouts, DDS import is hidden/unfinished, no model or map viewer.

## 2. AllodsOnlineDatabaseUnpacker (no license, C#, 2020, client 1.1.02.0 only)

Loads the game's **own native DLLs** (`Main`, `libdb`) through P/Invoke and reads objects
from the running game memory. Needs the "Helper archive" and server type files. We do not
want this approach (runs foreign native code, Windows 32 bit only, one version).

Useful facts, all **confirmed** against EditorTools for 1.1.02.0: field offsets of
`Geometry` (size 400), `SkeletalAnimation`, `Collision`, `Blob` (`localID` at +8, `size`
at +4 of the object), enums.

`SkeletalAnimation` record: `fps` (float), `startFrame`, `endFrame`, `looped`, `speed`,
`blendTime`, `animation` blob, AABBs, `animationEvents` (name, id, time). So the
**clip file header `u32 0; u32 payload` is the blob header** (localID 0, size).

## 3. blender-import (Paulus, no license stated, Python)

Imports a model from `X.(Geometry).bin` plus `X.(Geometry).xdb` (the xdb is what the database
record looks like when exported). Facts:

- The inflated `.bin` is a list of blobs: `u32 localID, u32 size, bytes`. ID 0 = vertex
  buffer, 1 = index buffer, 2 = skeleton, further IDs = other blobs (KD-tree, portal,
  area fragments). **This explains the numbers 0/1/2 we saw and called "unknown u32"** in
  [geometry-bin.md](formats/geometry-bin.md). **Confirmed** on our samples by the layout we
  already read.
- Sub-meshes come from `modelElements` in the xdb: each has a vertex buffer offset and LODs
  with `[vertexBegin, vertexEnd)` and `[indexBegin, indexEnd)`, plus a material name that
  leads to the diffuse texture. That is the missing "model to texture" binding: it lives in
  the database, not in the `.bin`.
- Without the xdb the importer cannot work, same as we.

## 4. ao-tools (gsomgsom, kosh, SLA; no license; PHP; 2009-era client 1.0.03)

- `loc.extractor` / `loc.compiler`: the `.loc` of that client is our **layout 1**: a packed set of
  UTF-16 text files whose names are the keys. Our description in [loc.md](formats/loc.md) matches
  (record of 12 bytes, relative name offset, UTF-16 length, data after a 24 byte block).
  Extra facts: block 2 holds the total payload size and four zero words; the compiler
  pads each text with 1..4 zero bytes (always adds, even when aligned) and sorts names.
  Use only if we ever write `.loc` (translation mods).
- `game.version` of that era (called "ver3"): `u32 content size, u32 crc32, u32 dir count + 1,
  "ver3", u32 length, version string`, then directories with files (name, size, crc32,
  flag) and a trailer. Our [game-version.md](formats/game-version.md) describes the newer header
  (tag at offset 4). Both exist; the viewer must accept both. **Claimed**, untested on real
  files. `game.ver.truncator` cuts the file list, `game.ver.info` shows it.

## 5. What to take into the project (proposal, order is the maintainer's call)

| # | Item | Gain | Cost / risk |
|---|---|---|---|
| 1 | Describe the blob list of geometry `.bin` (IDs 0, 1, 2, ...) and fix the docs | removes an "unknown" | small |
| 2 | Read the animation header as a blob header; add IDs to docs | removes an "unknown" | small |
| 3 | Texture type enum; write correct DDS for RGBA, L8, C565, C4444, RG16, R32F, RGBA16 | all texture kinds | small, needs real samples |
| 4 | Accept the old `game.version` ("ver3") too | old clients | small, no real file seen |
| 5 | Port the 1.1.02.0 struct layouts (Geometry, ModelElement, Material, Texture) into core, MIT port with NOTICE | models with texture binding for the 1.1.02.0 client | medium; only that client |
| 6 | Derive x64 layouts for 17.x ourselves from our own databases and compare with V14 | models with textures for the current client | needs a database that contains Geometry records |
| 7 | Map viewer: start from `MinimapTexture` (ready picture), then terrain and objects | M12 | needs map databases and terrain formats |
| 8 | Struct layout validation tests and round trips like EditorTools | fewer regressions | small |

## 6. Open questions and what is needed

- Where are the Geometry and Material records of the **current** client? The master `pack.bin`
  has only a few texture names, and the listing of `data\Mods` and `data\Cache` we received
  has no database. Needed: a listing of `data\Bin` and `data\Packs` (names and sizes), and
  one or two `Maps_*.bin` files if they are not too big.
- The 1.1.02.0 client (revision 168362) is the one EditorTools describes best; do we have
  its `pack.bin` and `Bin.pak`? If yes, items 5 and 7 can be verified on real data.
- Pirates (client 7.0): `V7_0_00_76` layouts exist; wait for the maintainer's files.
- Unknown vertex bytes 24..28 for stride 36/32: should be explained by the vertex declaration
  (weights, indices or a second UV set); check once a declaration is read.
