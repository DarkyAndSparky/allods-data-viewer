# Resource databases (`.bin`)

**Status: draft; outer layout verified on two real databases (2026-11-11), memory image not decoded.**
Summary of the public notes of the MIT licensed reference project (see
[REFERENCES.md](../REFERENCES.md)), checked against our own samples below.

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

## Verified on real samples (2026-11-11)

Samples from the owner's client (current x64 client): `pack.rus.bin` (295 197 bytes,
2 077 114 inflated) and `Maps_ZoneContested4.bin` (1 023 511 bytes, 7 553 466 inflated).

- Both are one zlib stream. The inflated data starts with a **12 byte version buffer**
  (three u32: `0x592E02CC`, `0x473`, `0x2689119F`), identical in both files.
- **The third u32 `0x2689119F` equals the magic of the current `.loc` layout 2 files**
  (`Texts_x64.pak`). The older layout 2 magic `0x162E0F62` should therefore belong to an
  older database version. This ties a text file to its database (NOT verified for the older one).
- After the buffer come chunks: `u32 id`, `u64 size`, body. Chunk ids seen: **2, 3, 4**.
  Sizes in `pack.rus.bin`: 1 660 176 / 325 744 / 4 340; in the maps file: 1 701 816 /
  4 015 392 / 113 251. Chunks 2..4 end exactly where the next chunk header would start; the
  rest of the inflated data (86 806 bytes in `pack.rus.bin`, 1 722 959 in the maps file)
  is not chunk framed in this scheme and is **not understood** (it starts with small
  integers that look like offsets or pointers).
- Chunk 2 holds, among other things, UTF-8 strings: the resource file names (the maps file:
  hundreds of `Maps/ZoneContested4/040_040/0_4_MapRegion.xdb` and `..._TerrainPackInfo.xdb`)
  and struct type names such as `struct NDb::Texture`, `struct NDb::Geometry`,
  `struct NDb::SkeletalAnimation`, `struct NDb::UITexture`, `struct NDb::FMODProject`,
  `struct NDb::VisualMob`. `pack.rus.bin` names `Client/LocalizedResources.(LocalizedResources).xdb`.
  So `.xdb` names are the logical resources; the `.bin` files in the paks are their
  exported payloads.
- `types.xml` (owner supplied) defines the NDb structs. For `Texture` it lists the fields
  `mipSW`, `mipsNumber`, `generateMipChain`, `type` (the texture format,
  `TextureElementType`), `width`, `height`, `realWidth`, `realHeight`,
  `disableLODControl`. **So width, height and format of textures are in the database**;
  decoding them needs the memory image layout of `NDb::Texture`, which is not done.

## Other files of the same batch

- `VoiceDialogs03.bev` (28 874 bytes, zlib, 350 412 inflated): starts with `ec3448c1`,
  `0x558C4`, `0x1D`; byte entropy about 3.4 bits per byte, so structured, not encrypted.
  Format unknown.
- `Voice_AC5_rus.bsb`: an ordinary bank (one stereo 48 kHz stream `intro_ac5`, 116.95 s).

## The master `pack.bin` (2026-11-11)

Delivered by the owner as a split ZIP (`pack.zip` + `pack.z01..z03`, 71 007 136 bytes
deflated). Rebuilt by concatenating the parts (the first part starts with a 4 byte
`PK\x07\x08` spanning marker) and inflating the single member; its CRC32 `65159CAF`
matched the archive listing. The member `pack.bin` is 75 402 723 bytes and is itself a zlib
stream that inflates to **703 896 034 bytes**.

- Same 12 byte version buffer as the other two databases (`0x592E02CC`, `0x473`,
  `0x2689119F`). Chunks: id 2 = 28 071 792 bytes, id 3 = 551 261 168 bytes, id 4 =
  7 669 604 bytes, then 38 124 720 bytes that do not follow the chunk framing used before
  (a chunk with id 0 would fit by the framing, so the framing is probably right and id 0
  is a real chunk; not verified further).
- It holds game data tables, not the files of the models we have: 2017 UTF-8 resource names
  such as `Maps/Domination02/MapResource.xdb` and `Mechanics/Astral/Sectors/...xdb`, a
  large run of NUL-terminated paths of sounds (`SFX/Creatures/AstralDemonMage.bsb`), and
  only five `(Texture).xdb` names. There are **no** `...(Texture).bin`, `...(Geometry).bin`
  names and no `AstralDemon` texture paths. So the records of the textures and geometries we
  have are in other databases (the client has one `.bin` per map and probably per
  model group); their names are still to be found.

## Тип корневого объекта (проверено, 0.1.8)

Запись таблицы правок (чанк 4) с адресом, равным dbId объекта, видом 0 (`data & 3`) и установленным битом 2 («external») несёт тип объекта: её второе слово есть индекс в списке структур (чанк 2). Проверено на pack.rus.bin (LocalizedResources), Maps_ZoneContested4.bin (MapRegion, MapResource, TerrainPackInfo) и главной pack.bin (MediaSystemRoot, GameMechanics, Texture и др.). Записи вида 3 хранят смещения внутри образа данных, не индексы структур.

## Объект Texture в образе данных (гипотеза, 0.1.8+)

Объект `Texture` занимает 160 байт. Проверено на пяти текстурах `World/Generic/Water/Textures/*` главной pack.bin (dbId 364999776 + k·160). Слова u32 со смещения 120, по два на 8 байт:

| смещение | слово | что в образцах |
|---|---|---|
| +120 | A | 512, 512, 128, 512, 512; совпадает с E |
| +124 | B | всегда −1 (похоже на `mipSW`, «по умолчанию») |
| +128 | C | 4, 8, 6, 8, 8 (похоже на `mipsNumber`) |
| +132 | D | всегда 1 (похоже на `generateMipChain`) |
| +136 | E | ширина (512, 512, 128, 512, 512) |
| +140 | F | высота (32, 512, 128, 512, 512) |
| +144 | G | тип: 2 (DXT5) у Fresnel, Ring, Wave, WaveDiffuse; 0 (DXT1) у Noise; значения enum `TextureElementType`: DXT1=0, DXT3=1, DXT5=2, RGBA=3, L8=4, D24=5, D16=6 |
| +148 | H | совпадает с F |
| +152 | байты | флаги (`disableLODControl`, `alphaTex` и др.) |

Не проверено: какое из пар (A, E) и (F, H) есть `width`/`realWidth`, `height`/`realHeight`: во всех образцах они равны. Нужны текстуры с размерами не степени двойки (UI-атласы, `realWidth` ≠ `width`). Порядок полей в памяти не совпадает с `ChunkID` из types.xml (там ещё `alphaTex` и `binaryFileSize`).

Дополнение: в `pack.rus.bin` 1122 объекта `Texture` (типы всех объектов видны по записям-меткам, см. выше). Во всех A = E и F = H; B = −1 всегда; C от 6 до 9 (на 1–2 меньше log2 размера); D принимает значения 1, 2, 3, 5 (не булево, возможно `mipSW`); G = 0 или 2 (DXT1 или DXT5). Размеры не степени двойки в этой базе не встречаются, поэтому пары width/realWidth остаются неразличимыми.

## Cloud Pirates 1.5: the full master database (2026-11-18)

`BaseLoceng.pak` of Cloud Pirates 1.5 (game version 7.0) holds `Bin/pack.bin`: 11 129 506 bytes, one zlib stream, 65 608 296 bytes inflated. It is the full resource database (thousands of `Texture`, `Geometry`, `UITexture`, `GeometryPreset` and `VesselItem` names). The layout is the older 32-bit one: chunks are `u32 id`, `u32 size`, body (not the 12 byte version buffer of the x64 layout).

Chunks seen in order (id: size): 0: 8 (version hash), 1: 1 915 896 (text references), 2: 6 130 500 (metadata), 3: 45 670 336 (data image), 4: 1 476 687 (fixes). About 10.4 MB follow the fixes chunk (pak references and pak names, framing not yet checked).

Status: chunk framing **verified** on this one file; metadata tables, fixes and the image are **not decoded**. The record layouts needed are in `ndb-pdb-layouts.md` (in-memory offsets of an early 1.x build; the 7.0 build may differ, so each layout must be checked against real records). Where the current client keeps its master database is still open (`BaseLocall.pak` of 18.0 has only `Bin/Maps_*.bin`).

### Cloud Pirates 1.5: metadata names and the 100-byte Texture record (2026-11-18)

Metadata (chunk 2) name tables decode to 42 827 resource names; every name block's adler32 matches. `db_id` of a resource is the byte offset of its root object inside the data chunk (chunk 3 body). Status: **verified** on this one file for names and ids.

`Texture` object in this build is 100 bytes (not the 160 bytes of the x64 layout). Offsets from the object start, little-endian `u32`: +60 width, +68 mips number, +72 type (`TextureElementType`: 0 DXT1, 1 DXT3, 2 DXT5, 3 RGBA, 4 L8), +76 realWidth, +80 realHeight, +84 alpha flag (0 or 2), +88 height. Over 3 900 textures: type 2 = 3759, 1 = 142, 3 = 23, 4 = 1. Status: **verified** for width/height/mips/type against 61 textures whose `.bin` record sizes were checked; **draft** for the meaning of +84, which was 0 for every `.bin` whose records are 8 bytes per 4x4 block and 2 for every one with 16 bytes per block (the type word is 2 in both cases, so +84 decides DXT1 vs DXT5 here).

Texture `.bin` and `.hi.bin` after zlib are lists of `u32 mip, u32 size, data` records that consume the stream exactly. A `.bin` holds the small levels (mip 2 and up; e.g. 6 records for a 512x512 texture), the sibling `.hi.bin` holds mip 1 then mip 0. Status: **verified** on 61 `.bin` files (Cloud Pirates) and on all `.hi.bin` files of 15 HiRes paks (client 18.0): every one has records (1, 0) and ends exactly at the end of the stream.

Reader: `adv-core::db32` (0.1.45; used by the app for texture sizes, not yet by the CLI). In the 32-bit metadata chunk the directory pairs `(rel, count)` start at body offset 0 (no version buffer): pair 0 is the name table (65 521 buckets of `(rel, count)`, records of 12 bytes `rel, size, db_id`), pair 1 the 742 struct names (12-byte records `rel, size, unused`). Status: **verified** on one real file (42 827 names, 742 structs, all checksums).

### Client 10.0 `pack.bin` (2026-11-19)

The same 32-bit layout (chunks 0 hash, 1 text refs, 2 metadata, 3 data image, 4 fixes) is used by a client 10.0 `pack.bin` (zlib; 75.6 MB deflated, 355.8 MB inflated): 411 035 resources, 1 369 struct names, all name checksums valid. Status: **verified** for names and ids on one file.

The `Texture` object there is 104 bytes: one extra word before the width, so width at +64, `0xFFFF_FFFF` at +68, mips at +72, type at +76, real width/height at +80/+84, flag at +88, height at +92. `adv-core::db32` tells the two layouts apart by the position of that `0xFFFF_FFFF` word. Status: **draft** for the type and flag values in this build (widths, heights and mips are consistent: `Sword_1H_B_02CommonSteelFX` is 128x64 with 5 mips); the mapping of type 4/5/6/7/8 words seen in some textures is **unknown**.

### Map databases `Bin/Maps_<name>.bin` (client 10.0, 2026-11-19)

`BaseLoceng_eu.pak/Bin/` holds 270 files `Maps_*.bin` (about 0.02 to 2.5 MB each deflated) and no `pack.bin`. Each file is zlib inside the zip entry and is zlib again inside; the inflated payload is the same 32-bit chunk database as `pack.bin` (checked on `Maps_2ndCircle.bin`: 5 875 resources, 32 structs, names like `Maps/2ndCircle/040_040/0_6_SplatMap_0.(Texture).xdb`, `.../layers.(TextureAtlas).xdb`, `.../1_7_TerrainPackInfo.xdb`). Status: **verified** for names and ids on that one file; the map objects themselves (terrain, regions) are **unknown**.

Splat map `Texture` objects there read as 256x256, 1 mip, type word 1, flag word 7. The matching `.bin` is one record of 131 072 bytes, i.e. 2 bytes per pixel, and its words are mostly pure RGB565 primaries (`0xF800`, `0x07E0`, `0x001F`) and zero. Status: **draft** (the pixel format is inferred from size and values; the meaning of the type/flag words is unknown). The viewer therefore picks square RGB565 by default for files whose name contains `SplatMap`.

### Where the databases live and how the app uses them (2026-11-19)

Observed on the user's clients: `BaseLocrus.pak` (Russian 10.0 client), `BaseLocWarprus.pak` (Warp), `BaseLocall.pak` (Russian 18.0), `BaseLocall_x64.pak` (Russian 17.0), `BaseLoceng_eu.pak` (English 11.0); Cloud Pirates has one `BaseLoc<lang>.pak` per language (eng, fra, ger, pl, rus, tr). Each holds `Bin/Maps_*.bin`; `Bin/pack.bin` is present in some (Cloud Pirates, 10.0, Warp: 16.0 MB deflated) and not in the 18.0 pak seen so far. Status: **verified** for these names.

The session looks for `BaseLoc*.pak` (not HiRes/Mini) up to three folders below the opened folder, prefers `rus`, then `eng`, then the first by name, and reads texture sizes from `Bin/pack.bin`, then from `Bin/Maps_<map>.bin` for resources under `Maps/<map>/`. The database is read once on the first texture preview; only the small texture records are kept. A missing or damaged database is ignored and the old size guess is used. Verified end to end on a synthetic folder built from a real `Maps_2ndCircle.bin`: `2_6_SplatMap_1` opens as 256x256 RGB565.

Correction (2026-11-19): the client 18.0 `BaseLocall.pak` does contain `Bin/pack.bin` (78 MB), `Bin/pack.eng_eu.bin`, `Bin/pack.rus.bin` (about 290 KB each, purpose not examined) and the `Maps_*.bin` files; an earlier note saying it had only maps came from an incomplete sample. The session reads `pack.bin` with the 32-bit reader first and falls back to the 64-bit V2 reader (`db.rs`) for current clients. The 64-bit path is tested on synthetic data only; no real 18.0 `pack.bin` has been checked yet. Status: **not verified** on a real 18.0 file.

### Client 18.0 `pack.bin` (2026-11-19)

`BaseLocall.pak/Bin/pack.bin`: 72.9 MB deflated, 78.06 MB zlib stream, 723 MB inflated. It uses the 64-bit layout (12-byte version buffer, then chunk 2 with an `i64` size; metadata body 23 981 472 bytes). Directory pairs `(rel, count)` start at body offset 24: pair 0 `(65 521 buckets)`, pair 1 the 1 506 struct names (16-byte records, names such as `struct NDb::DefaultFactoryResource`), pairs 2 and 3 again 65 521 buckets, pair 4 empty. The current `db.rs` reader (written for the 17.0 layout, files in pair 1 and structs in pair 2) fails on this file with "struct name runs past end". The records of pair 0 are 16 bytes `(rel, size = 8, id)` pointing at 8-byte blocks without a name; the targets of pairs 2 and 3 lie beyond the metadata chunk. Where the resource names moved is **unknown**. Status: **unknown**; the 18.0 layout is a different revision of the 64-bit format and needs its own study.

Further observations (draft): pair 0 records are `(rel, 8, i64)` with ascending `i64` values (1 264, 64 342 056, 134 953 432, ...) pointing at 8-byte blocks, not names; pairs 2 and 3 hold 16-byte entries of two offsets that fall inside the data image. No resource names were found in the metadata chunk, so the names of this revision may live elsewhere. Consequence for the app: `adv-core::db::probe_layout` looks at the first 48 inflated bytes (V2 requires a small struct-name pair in slot 2) and the session skips such files without inflating them; texture sizes are guessed as before.

### Client 18.0 `pack.rus.bin` / `pack.eng_eu.bin` (draft, checked 2026-11-19)

Same 64-bit family as the 18.0 `pack.bin` (zlib stream, magic word `0xC98CCA86`, directory of five `(rel, count)` pairs from offset 24), 2 MB inflated each. Findings:
- Pair 1 is the struct-name table (16-byte records `(rel, length)`, 16 names in the Russian pack, 17 in the English one; names look like `struct NDb::Texture`). The big `pack.bin` has 1,506 of them.
- Pair 0 holds 65,521 buckets of 8 bytes `(rel, count)` and 16-byte records `(rel, 8, data offset)`; the 8 bytes at `rel` are a key `(u32 1, u32 serial id)`. The serial ids are consecutive, they are not name hashes and no file names are stored. The 1,362 resources here include localized `Texture` records (for example 128x128, 6 mips, type 2).
- Pairs 2 and 3 are two 65,521-bucket hash indexes with pairs of 64-bit values (id to offset and back, not verified in detail).
- Conclusion: in this revision resources are addressed by numeric id; the link between a pak file name (`X.(Texture).bin`) and the id is NOT found in these packs or in `pack.bin`. Texture sizes for 18.0 therefore stay unavailable; the viewer keeps guessing. Open: where the name-to-id link lives (possibly in the pak file entries or `.xdb` data not shipped in `Bin`).

### Client 17.0 `BaseLocall_x64.pak/Bin/pack.bin` (draft, checked 2026-11-19)

75.8 MB zlib stream (71.4 MB deflated), 64-bit layout, version word 1139 (18.0 uses 1147). Directory pairs from body offset 24: pair 0 `(65 521 buckets)`, pair 1 **4 081 named resources**, pair 2 1 498 struct names, pairs 3 and 4 `(65 521 buckets)`. Only the header and the name tables were read (the first 300 MB of the inflated stream); the data chunks were not parsed.
- Pair 0 keys are `(u32 1, u32 id, 0)`; ids are serial and a key lives in bucket `id mod 65 521`. As in 18.0, no names are stored in pair 0.
- Pair 1 is a table of 16-byte entries followed by a name block with paths such as `Maps/Domination02/MapResource.xdb` and `World/Generic/Water/Textures/WaterWave.(Texture).xdb`. Most of the ~520 names inspected are `MapResource`; a few are textures. So 17.0 names only a small set of resources; the rest, textures included, are known by numeric id only.
- Consequence: the old `db.rs` layout (files in pair 1, structs in pair 2) matches 17.0 in the numbering of pairs; whether its record decoding is right is still **not verified** (no real parse yet). Texture sizes by file name are possible only for the few named resources; 18.0 has no name table at all.
- Open: how a pak entry name such as `X.(Texture).bin` maps to an id; unknown for both 17.0 and 18.0.

### Table order, cross-check with EditorTools (2026-11-20)

The MIT project AllodsOnlineEditorTools (`doc/bin-format.md`) describes the 64-bit V2 directory as: 0 ObjId to DbId (65 521 buckets, 9-byte keys `(1, objId, pad)`), 1 DbId to file name (4 081 buckets; each block is delimiter 1, adler32 of the name, the name, a pad byte; bucket = adler32 mod count), 2 struct names, 3 ResId to DbId, 4 DbId to ResId. This agrees with what was found in the 17.0 `pack.bin` above (pair 1 with 4 081 entries and file names, pair 2 with 1 498 struct names). Root objects have names but no objId; every other object has an objId and no name. In the 18.0 `pack.bin` the file-name table is absent (pair 1 is the struct table), so the order shifted by one: that is a revision change (bytes 4..8 of the version buffer: 0x0472 then 0x0473 between two 64-bit builds, per that document), and root names must come from somewhere else; the Unpacker README lists `System/index.bin` as an input and is the next lead. Status: **draft**.

Client 1.1 (screenshot from the maintainer, 2026-11-20): `Packs/System.pak` (1 KB) holds a single file `System/index.srv` of 6 bytes, not an `index.bin`; the 18.0.7 `System.Mini.pak` holds only `System/Corks/DBNMesh.bin` and `System/Corks/Texture.bin`. A 6-byte file cannot carry a name table, so `System/index.bin` from the Unpacker README is **not** the source of 18.0 root names; where they live stays **unknown**.

### Server `System/index.bin` of the 1.1 server (draft, 2026-11-20)

Found in the server tree (`Allods_Server/server/game/data/System`, 21.2 MB; next to `index.srv` 7.7 MB, `DefaultFactory*.xdb`, `Statistics.xdb`); the client 1.1 `System.pak` only has a 6-byte `index.srv`. The server file is a serialized index, not a 32/64-bit resource database: it starts with `04 08 04 00 00 00 01 e7 ...` and then repeats records made of a length-prefixed class name (`gameMechanics.constructor.schemes.item.ItemResource`, `...LootTableResource`, `...VisualItem`, `...BuffResource`, `...QuestResource`, 101 distinct class names in this file) and a length-prefixed resource path ending in `.xdb`. About 194 000 paths were found by scanning, of them 24 155 `(Texture)`, 8 164 `(SkeletalAnimation)`, 6 305 `(VisObjectTemplate)`, 5 832 `(Geometry)`. Record framing and what the numbers around each name mean are **not** decoded. Use: a dictionary of real resource names for client 1.1; it does not help to name 17.0/18.0 resources. Status: **draft**.
