# Model to texture binding (7.0 pack)

Status: `draft`. Checked on `Warlock` and `ArchdemonMage` (client 7.0, `Bin/pack.bin`, Bits32).
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
