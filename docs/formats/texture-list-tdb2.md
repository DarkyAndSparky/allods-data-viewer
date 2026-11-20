# Community texture list `textures_database.bin` (TDB2)

**Status: structure and meaning decoded (2026-11-11), see [texture-heuristics.md](texture-heuristics.md).** The 7 bytes are `u16 index`, `u8 format`, `u16 padW`, `u16 padH`. Source: the file that
ships next to a third-party viewer for Allods textures (version 1.6.1, 2023-12-02). We did not
read or decompile that program: its license is unknown (see `docs/REFERENCES.md`), so only the
data file was analysed, on 2026-11-10.

## Container

- The file is a zlib stream (21 311 bytes, 170 173 inflated).
- Inflated: `"TDB2"`, `count` (u32 LE; 2038), then `count` records back to back. The records
  end exactly at the end of the data.
- Record: `len` (u8), `len` bytes of UTF-8 path (starts with `/`, for example
  `/Interface/Common/Buttons/Standard/StandardNormal.(UITexture).bin`), then **7 bytes**.
- Paths cover 2 037 textures (`Interface` 1892, `Creatures` 88, `Characters` 37, `Ships` 10,
  `Items` 7, `Maps` 3, `Spells` 1) plus some folder-like entries; both `.bin` and `.hi.bin` occur.

## The 7 bytes (unknown)

Layout as seen: `b0 00 b2 b3 b4 b5 b6`.

- `b1` is always 0 in the samples counted. `b0` takes values 0..6 (mostly 3, 4, 5).
  `b2` takes values 0..3 (mostly 1 and 3). The most common tails `b3..b6` are all zero; others
  look like two small u16 values (for example `77 00 05 00`, `09 00 09 00`, `08 00 08 00`).
- The only pair we could check against real data: `Maps/7_2_Raid/050_050/0_0_SplatMap_0.(Texture).bin`
  has `03 00 01 00 00 00 00` and a single chunk (id 0) of 131 072 bytes (512x512 DXT1 or
  512x256 DXT5). This does not decide the meaning.
- Hypothesis (NOT VERIFIED): `b2` is a format code and `b0` a size class. To test it we need
  textures whose paths are in this list together with a picture of the right size.

## Why it matters

The list suggests that a community tool needed hand-collected size/format data too, which
agrees with our finding that `.bin` files do not store them. The game itself must have the data
in its resource database (`pack.bin` or `.xdb`); that is still the proper source (G-19).
