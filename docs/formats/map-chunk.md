# Client map chunks (`Maps/<map>/<block>/<x>_<y>_*.bin`)

Status: **draft**. Observed on one client map pack (`Winter_Atoll_000_000_512_512.Client.pak`),
described in our own words. Nothing here is verified against the game.

## Files in a client map pack

Path: `Maps/<map>/<block>/<x>_<y>_<kind>` where `<block>` looks like `010_010` and `x`, `y` are
small integers (0..6 and 0..5 in the sample, so a block is a grid of chunks).

| Kind | Count in sample | Notes |
|---|---|---|
| `terrainDump.bin` | 30 | terrain of one chunk, about 23 KB; see below |
| `terrainDumpOcc.bin` | 30 | 80 bytes, occlusion data (floats), tiny |
| `SplatMap_N.(Texture).bin` | 19, 9, 4 for N = 0, 1, 2 | texture container (see [texture-bin.md](texture-bin.md)), 131 080 bytes for level 0; the layer blend weights |
| `lightmap.bin`, `lightmapDown.bin` | 41 each | 10 968 bytes, plus `.hi.bin` of 163 856 bytes; upper and lower level lighting |
| `lightvrt.bin` | 19 | 3 524 bytes, vertex lighting |

The database side (`MapRegion`, `TerrainPackInfo`, `MinimapTexture`) is described in
[../TOOLS-STUDY.md](../TOOLS-STUDY.md) and gives the file names, layers and splat maps.

## `terrainDump.bin` (what is seen)

- After the pak layer the file is a **blob** (checked by `adv blobs`): `u32 localID` (0), `u32 size`, then `size` bytes
  (23 580 of 23 588 in the sample). Same container as geometry files.
- The payload starts with a table of `u32` pairs that look like `(offset, count)` with
  self-relative offsets, the same kind as the skeleton tables in geometry files:
  `(0x5b60, 2)`, `(0x30, 17)`, `(0xc4, 4)`, `(0x5b50, 2)`, `(0x5bb8, 1)`, `(0x5bf4, 0)`,
  `(0x5bec, 0)`. Not confirmed.
- From payload offset `0xd4` there are 32-byte records of eight floats, for example
  `(240, 208, 75.18, 16, 16, 22.49, 0, 0)` and `(240, 240, 80.49, 16, 16, 16.98, 0, 0)`. They
  look like a cell origin (x, y), a height, a 16 by 16 size and a height range. A guess only.
- Floats such as 240, 208, 256 and 512 appear elsewhere, so coordinates are in world units
  inside a 512 by 512 chunk.
- Small tables of 4-byte entries (`01 01 00 00`, `2d 4c ff 00`, ...) sit at `0x38`; meaning unknown.

## Height-like data (second pass, still a guess)

Between payload offsets `0x478` and about `0x3a00` there are many runs of 9 to 105 consecutive
floats with values near 58 to 91 (flat stretches at 80.5 and 81.3). They look like terrain
heights, and each run ends with zeros. The first float of a run is often a small outlier
(1.2, 1.4). Tests for a row length from 2 to 39 found no clear two-dimensional grid, so these
are probably not plain row-major height grids; the order may follow cells or a tree. Runs of
exactly 9 floats near `0x2578` could be 9-vertex edges. Nothing is confirmed.

## What is unknown

The height grid itself, the meaning of each table, how layers map to splat maps, and how the
`lightmap` and `lightvrt` data are laid out. To draw a top-down map we need at least the
height grid (or use the minimap texture from the database, which needs a database we do not
have yet).

## Next steps

1. Find the height data: try a 17 by 17 or 33 by 33 grid of `u16` or `f32` in the payload.
2. Compare two chunks of the same map and several maps to see which tables change.
3. Read `MapRegion` and `TerrainPackInfo` when a database with them is available.
