# CommonMaterialParams in the 7.0 pack (rootMaterial)

Status: the rootMaterial path is pinned on texture fields at scale; scalar and flag fields are decoded at
their reference offsets but are constant in the sampled set and are not an independent check.

## Layout (record of 84 bytes, data-block relative)

Field offsets follow the EditorTools `CommonMaterialParams` layout (`StructSize 84`, `V7_0_00_76`), from
Allods Online Editor Tools by Paulus (MIT). Cited only; no code copied.

| Offset | Field | Kind |
|---|---|---|
| +24 | bumpTexture | pointer (texture) |
| +32 | color | int |
| +36 | contourColorModifier | int |
| +40 | effectMaskTexture | pointer (texture) |
| +48 | envReflectionTexture | pointer (texture) |
| +56 | envSpecularTexture | pointer (texture) |
| +64 | shininess | f32 |
| +68 | specularColorModifier | int |
| +72 | specularIntensity | f32 |
| +76..+81 | bumpDisabled, selfillum, useEnvContourColorFactor, useEnvSpecularColorFactor, useMaskColor, vertexBakedLight | bool |

## How the record is reached

A Geometry object (pack entry type `Geometry`) carries a DbIdRef pointer-fix (tag 0) at data-block offset
`+0x128` (`rootMaterial`). Its value is the data-block offset of the 84-byte record. The record has its own
type fix (tag 7) at `+0`, and the texture pointers are fixes at `+40`, `+48` and `+56`.

## Evidence (base 856, 2026-11-23)

- Oracle: Geometry `.xdb` files with `<rootMaterial type="CommonMaterialParams">` (5,498 blocks).
- Texture check: the DbIdRef target's pack entry name equals the xdb href. Control: the href of a different
  oracle whose href differs.

| Field | True match | Control |
|---|---|---|
| effectMaskTexture +40 | 1,595 / 1,595 | 0 / 1,595 |
| envReflectionTexture +48 | 2,181 / 2,181 | 0 / 2,181 |
| envSpecularTexture +56 | 1,496 / 1,496 | 0 / 1,496 |
| bumpTexture +24 | no oracle has a bumpTexture href | n/a |

Scripts: `aotools/aopack/pack/rootmat_effectmask_rootblock_856.py`; output
`aotools/out/fixes/rootmat_effectmask_rootblock_856_2026-11-23.txt`.

Earlier `<params>`-only scans dropped the rootMaterial blocks and are superseded; the
`<rootMaterial type=...>` spelling is the one the Geometry xdbs use.

## Not yet decoded

- Scalar and flag fields: read at the offsets above; constant across the oracle set.
- Per-element materials (`modelElements[].material`, a 40-byte wrapper with `diffuseTexture` at +8 and `params` at +16): the `params` pointer leads to the same 84-byte record and decodes with `parse_common_material` (checked on `Warlock`, see [model-binding.md](model-binding.md)); the scalar fields are still not an independent check.
