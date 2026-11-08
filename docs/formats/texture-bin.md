# Textures stored as `.bin`

**Status: draft.** Described by the MIT licensed reference project (see
[REFERENCES.md](../REFERENCES.md)); not yet checked by us against real files.

- The file is a **zlib stream**.
- Inflated, it is a sequence of chunks: `id` (s32), `size` (s32), then `size` bytes of
  pixel data. Reading stops after the chunk whose `id` is 0.
- The chunks are stored **smallest mip level first**; reverse them to get the largest first.
- The data are block compressed (DXT style). Width, height, mip count and the
  compression type are **not in the file**: they come from the texture's description in
  the resource database ([bin-db.md](bin-db.md)). The compression type becomes the
  FourCC of a DDS header.
- To get a `.dds`: write a DDS header with the FourCC and sizes, then the mips largest first.

Other texture related files seen in the databases: `.tga`, `.psd`.
