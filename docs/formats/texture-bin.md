# Textures stored as `.bin`

**Status: draft, container verified.** The container was checked on 2026-11-10 against
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
- To get a `.dds`: write a DDS header (FourCC, sizes) and the levels largest first.

Other texture related files seen: `.tga` (uncompressed, standard header), `.cur` cursors.
