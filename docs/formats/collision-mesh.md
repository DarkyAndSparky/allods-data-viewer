# Collision mesh data (in-memory layout)

Status: **draft**. Field names and sizes come from the debug symbols of the game's collision module (game 1.1.02.0); no code was taken. Which file on disk stores this, and its byte order and compression, are **unknown** until a sample is matched against it.

`static_vector<T>` is `{u32 offset, u32 count}` with the offset relative to the vector's own address (same convention as the geometry tail and animation data).

`MeshData` (64 bytes):

| Offset | Field | Type |
| --- | --- | --- |
| 0 | `faces` | static_vector of `Face` |
| 8 | `gridX` | static_vector of static_vector of u16 |
| 24 | `gridY` | same |
| 40 | `gridZ` | same |
| 56 | `materialEntries` | static_vector of `MaterialEntry` |

`Face` (24 bytes): a plane (4 floats, at 0) and a bounds record (8 bytes, at 16, element type not yet known).

`MaterialEntry` (8 bytes): `i32 firstFace` at 0, `u16 flags` at 4, `u8 soundType` at 6.

The three grids look like per-axis cell lists holding face indices (a broad-phase lookup). This is a reading of the names, not verified.

The same PDB contains no server map tile structures; the `Tests/Maps/*/*_server.bin` tiles are still undocumented beyond their header (`ff ff ff fe`, version 1, big-endian u16 data).
