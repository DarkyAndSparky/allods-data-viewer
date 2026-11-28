# Model to texture binding (7.0 pack)

Status: `draft`. Checked on `Warlock` and `ArchdemonMage` (client 7.0, `Bin/pack.bin`, Bits32) and, for client 1.1.00.41, on the whole pack (see below).
The 18.0 (64-bit) layout is not read yet.

## Where the binding lives

The geometry `.bin` holds only vertices, indices and bones. The split into elements and the
textures are in the Geometry record of the resource database (`X.(Geometry).xdb`, an object of
`pack.bin`). Offsets below are relative to the Geometry record (`root`, its `db_id`) and
to the data block; layout is from the 7.0 editor structs (MIT, see `NOTICE`).

| Field | Offset | Note |
|---|---|---|
| `modelElements` array | `root + 228` (pointer), `root + 232` (byte size) | element size 120 bytes |
| element `lods` array | element `+ 20` (pointer), `+ 24` (byte size) | `Lod` is 20 bytes (earlier notes said 60: that is three lods) |
| `Lod` | `+4` index begin, `+8` index end, `+12` vertex begin, `+16` vertex end | in index/vertex units; LOD 0 is the first |
| element `material.diffuseTexture` | element `+ 44` | pointer, fix tag 0 |
| element `material.params` | element `+ 52` | pointer, fix tag 0, to a `CommonMaterialParams` record ([common-material.md](common-material.md)) |

## How the pointers are resolved

Array pointers and `diffuseTexture` are not stored in the image: they are listed in the fix table
(block 4, see [pack-fixes.md](pack-fixes.md)). For a field at offset `o` the table has an entry
with `offset == o`; tag 1 gives the data-block offset of the array, tag 0 gives the `db_id` of
the referenced object. The `db_id` is looked up in the file list of the database; the name is
the Texture resource (`…(Texture).xdb`), and the picture is the `.bin` of the same name.

## Verified

- `Warlock`: element 0 uses `Warlock.(Texture)`, element 1 `WarlockWeapon.(Texture)`.
- `ArchdemonMage`: nine elements, textures `ArchidemonMageWeapon`, `ArchdemonMageSkirt`,
  `ArchdemonMage`, `ArchdemonMageHelm`, `ArchdemonMageFlow`, `ArchdemonMageBand` (the folder
  contains exactly these files). LOD 0 vertex ranges end at the vertex count of the `.bin`
  (7942 and 7808).
- Over the whole 7.0 pack 22 178 Geometry records have elements, 20 706 of them with a texture.
- `params` of the elements decode with the same 84-byte record as the root material: on `Warlock` the effect mask is `WarlockFX.(Texture)` and `WarlockWeaponFX.(Texture)` (files that exist in the model folder), the reflection map `Creatures/ReflectMaps/Refmap15`, the contour texture `ContourTex05`, shininess 25.
- Over the whole 7.0 pack: 147 951 elements, 142 969 with a diffuse texture, 58 064 with a `params` record (the others have no pointer), of those effect mask 29 792, environment reflection 31 374, environment specular 29 663, bump 0.
- A rendering of both models shows the pictures on the right parts (hands, weapon, skirt).

## Not known

- Effect textures that are not in the model's folder (parts stay untextured).
- How the effect mask, reflection and contour textures are combined by the game shader (the glTF export uses only the diffuse texture; no standard glTF slot fits them). No element in the pack has a bump texture.
- LOD 1 and 2 ranges are read from the same structure but not used.
- Skinned (`skinIndex`, `virtualOffset`) element data.

## Client 1.1 (record layout `V1_1_02_0`)

The 1.1 records are smaller and the fields lie elsewhere. The reader picks the layout from the
signature of the database (`adv-core::version`, `ElementLayout::for_family`); an unknown signature
keeps the 7.0 layout, so nothing that worked before changes.

| Field | Offset | Note |
|---|---|---|
| `modelElements` array | `root + 180` (pointer), `root + 184` (byte size) | element size 112 bytes |
| element `lods` array | element `+ 96` (pointer), `+ 100` (byte size) | `Lod` is 20 bytes |
| `Lod` | `+4` vertex end, `+8` vertex begin, `+12` index end, `+16` index begin | the order differs from 7.0 |
| element `material.diffuseTexture` | element `+ 84` | pointer, fix tag 0 |
| element `material.params` | element `+ 80` | nullable pointer; **not read** |
| `Geometry.vb32` (32-bit indices) | `root + 60` | the byte at `root + 329` is not this flag |
| `Geometry` record size | 400 bytes | |

Sources: the 1.1.02.0 structs of AllodsOnlineEditorTools (MIT) and the MIT Python exporter agree on
all of these, except that the exporter tests the byte at +329 for `vb32`; on the 1.1.00.41 database
that byte is 0 in all 5 189 records while the byte at +60 is 1 in 32 records (30 of them have more
than 65 535 vertices), so +60 is the flag.

Verified (2026-11-26, client 1.1.00.41 `Bin/pack.bin`): 5 189 Geometry records, 5 186 with elements,
39 585 elements, 38 675 with a diffuse texture. The Rust reader and an independent Python reader
give the same totals, and for `ElfFemale` (54 elements) the same index and vertex ranges and
the same texture names (checked on elements 0, 19 and 23). Nine elements have a `lods` array whose
byte size is not a multiple of 20; the Python reader rejects them, the Rust reader reads the first
entry. Not checked: the look of the textured models, the material scroll and transparency fields.
