# NDb record layouts from debug symbols (1.x editor client)

Status: **draft** (field names, types and in-memory offsets are read from the PDB debug symbols of an early 1.x editor build; how the records are *serialized* in a resource database file is a separate question and is not decided here).

Source: the PDB files that ship with the map editor package (types `NDb::*` are present in Scene3D, SceneExport, HiLevelExport, GameData and others). Sizes are 32-bit in-memory sizes. Every record starts with a 20 byte base (vtable, name and bookkeeping) and a checksum field.

## What the layouts tell us (facts)

- `Geometry` (400 bytes): vertex declarations, vertex buffer, index buffer, skeleton, scene nodes, joints, `modelElements`, `parts`, LOD distances, KD-tree fragments (geometry, area, portal), occluders, flares, bounding boxes, `binaryFile` (path of the `.bin`), `version`, `vb32` (32-bit indices flag), `skeletalAnimation`.
- Blob reference `BLOb` (12 bytes): size and `localID`. This matches the blob list inside geometry and animation `.bin` files (ids 0 vertex, 1 index, 2 skeleton): the database says which blob is which.
- `VertexDeclaration`: stride plus seven components (position, normal, color, weights, indices, texcoord0, texcoord1), each with an element type and a byte offset.
- Vertex element types: FLOAT1..FLOAT4 = 1..4, SHORT2 = 5, SHORT4 = 6, COLOR4 = 7, UBYTE4 = 8, USHORT2 = 9, USHORT4 = 10, HALF4 = 11, UNUSED = 12.
- `ModelElement`: vertex declaration id, vertex buffer offset, skin index, name, material name, a material instance and `lods` (list of fragments).
- `GeometryFragment` (one LOD of one element): vertex range begin/end and index range begin/end.
- `MaterialInstance`: visible, transparency and diffuse texture references, scroll speeds, blend effect (ADD, ALPHA, ALPHA_ADD, COLOR, COLOR_ADD), parameter block.
- `ModelPart`: start and end element, name, area flag, bounding box. Sort modes: OFFSETS, FRONT_BACK, BACK_FRONT. Orientation modes: COMMON, WORLD_Z, Z_AXIS, BILLBOARD.
- `Texture` (92 bytes): wrap, width, height, real width and height, mip count, element type (same enum as in `texture.rs`), generate-mip-chain flag, source and binary file paths, alpha flag.
- `SkeletalAnimation` (160 bytes): start and end frame, speed, fps, looped, blend time, script name and id, animation events (name, id, time), animation blob, bounding boxes, binary file and version.
- `MapRegion` (184 bytes): zone lights, tiles, music and ambience grids, layers, objects, areas, and two binary files (`binaryFile`, `binaryFileDown`). `MinimapTexture` is 96 bytes.

## Use

These facts let us decode geometry once the record values are available: stride and component offsets replace our stride guessing; element LOD ranges give sub-meshes; the material instance binds textures. What is still missing is the *file* that stores those records for current clients (see `docs/CLIENTS.md`).
