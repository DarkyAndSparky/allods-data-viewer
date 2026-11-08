# Resource databases (`.bin`)

**Status: draft.** Summary of the public notes of the MIT licensed reference project
(see [REFERENCES.md](../REFERENCES.md)). We have not yet checked it against our own samples.

A database such as `pack.bin` is a **zlib stream**. The inflated payload is a
sequence of chunks (`id`, `size`, body) that holds a **raw memory image** of the game's
C++ objects plus a table that relocates the pointers:

| Chunk | Meaning |
|---|---|
| Header | 8 byte version hash that identifies the set of struct definitions |
| TxtFiles | ids of localized text references (older clients) |
| Metadata | bucketed tables: id to file name, struct names, resource ids |
| Data | the memory image |
| Fixes | pointer relocations (database id, direct pointer, type tag) |
| PakFileRefs | optional, offsets of pak references |
| Packs | optional, names of referenced `.pak` files (UTF-16) |

Newer clients (15.0 and later) changed the layout: a bare 12 byte version buffer instead
of a header chunk, no TxtFiles chunk, an extra id table, and 64 bit fields.

The memory image can only be decoded with the exact struct definitions of the client
version, so a reader needs per-version struct tables. That is a large reverse
engineering result and the main reason to reuse the reference project's work (MIT).

Resource file types the databases refer to: `.bin` `.tga` `.bev` `.bsb` `.fsb` `.psd`
`.lua` `.cur` `.ttf` `.hlsl` `.eh` `.ej` `.ogv`.
