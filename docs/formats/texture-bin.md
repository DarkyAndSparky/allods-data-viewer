# Textures stored as `.bin`

**Status: draft, container verified; DXT5 decoding checked visually on one real texture (0.1.3).** The container was checked on 2026-11-10 against
about 80 real files (`Texture`, `UITexture`, `.hi.bin`); pixel data was decoded for one
file. Width, height and pixel format are still not in the file.

- The file is a **zlib stream**.
- Inflated, it is a sequence of chunks: `id` (s32), `size` (s32), then `size` bytes. Reading
  stops at the end of the stream. **`id` is the mip level index** (0 = largest). Chunks are
  stored **smallest level first**, so the last chunk is the largest level.
- In every checked file the sum of `8 + size` equals the inflated length exactly.
- A texture is split in two files: `Name.(Texture).bin` holds the small levels (for
  example ids 6..2 with sizes 16, 64, 256, 1024, 4096) and `Name.(Texture).hi.bin` holds the
  largest levels (ids 1 and 0, or only 0). Each level is 4 times the previous one.
- Level sizes are consistent with block compression (4x4 blocks; 8 bytes per block for
  DXT1, 16 for DXT3/DXT5). Example: 131 072 bytes for level 0 of `I1_Stone_02`
  decodes to a clean 512 x 512 picture as **DXT1**; the same bytes read as DXT5 512 x 256
  give a doubled picture, so the guess is wrong. Size alone is ambiguous (one size fits
  DXT1 and DXT5 with different dimensions), so the real dimensions and format must come
  from the resource database ([bin-db.md](bin-db.md)) or be given by the user.
- Some chunks are not block compressed or have other sizes (e.g. 2 097 152 and 262 144
  bytes as a single level 0); the format of those is unknown.
- The resource kind is part of the **file name**: `X.(Texture).bin`, `X.(UITexture).bin`,
  `X.(Geometry).bin`, `X.(SkeletalAnimation).bin`, `X.(CollisionMesh).bin`,
  `X.(AttributeAnimation).bin`. Reading the name is therefore enough to choose a decoder.
- `adv tex convert` (0.1.3) takes `--size` and `--dxt`; the level size must match exactly or it refuses. Checked on `Creatures/Placeholders/AstralDemon.(Texture).bin`: 4096 bytes at mip 2 gives a coherent picture only as 256x256 DXT5 (the DXT1 reading gives garbage), so format guesses are easy to tell apart by eye.
- To get a `.dds`: write a DDS header (FourCC, sizes) and the levels largest first.

Other texture related files seen: `.tga` (standard Truevision header, decoded since 0.1.54: types 1/2/3 and RLE 9/10/11, 8 to 32 bits; **draft**, synthetic tests only), `.cur` cursors (same container as `.ico` with type word 2, decoded since 0.1.54; **draft**).

## Description files `.xdb` (verified 2026-11-11)

Next to a texture `.bin` the client (in `data/Mods`, and in addon packs) keeps an `.xdb`
with the same name: plain UTF-8 XML. For `X.(UITexture).bin` the file `X.(UITexture).xdb` holds
`<type>` (`DXT1` or `DXT5` in 253 files seen, 158 DXT5 and 95 DXT1), `<width>`, `<height>`
(size of mip 0 in the file), `<realWidth>`, `<realHeight>` (the used picture inside the
power-of-two texture, anchored at the top left), `<mipsNumber>`, `<binaryFileSize>` and
`<binaryFile href="...bin">`. Other `.xdb` kinds point at it (`UISingleTexture` ->
`UITexture`, `WidgetLayerTiledTexture` with a nine-slice `Layout`).

- Checked: `MainFrame.(UITexture).bin` (17 641 bytes, one level of 262 144 bytes) with its
  `.xdb` (DXT5, 512x512, real 386x325) gives a clean picture that fills exactly 386x325 pixels.
- 437 of 438 UI textures from five addon packs convert without errors using only the `.xdb`.
  The failing one, `Empty.(UITexture).bin`, has a hand-made `.xdb` that contradicts itself
  (width 32, realWidth 64, and the level is 64x64 DXT1); the converter correctly refuses it.
- The same fields exist in the game database as `NDb::Texture` (see `bin-db.md`); inside the
  paks the `.xdb` files themselves are absent.
