# File formats

Notes in this folder describe formats in our own words, see
[CONTRIBUTING.md](../../CONTRIBUTING.md) for the rules and
[REFERENCES.md](../REFERENCES.md) for sources.

| Format | Status | Note |
|---|---|---|
| `.pak` archive | verified | [pak.md](pak.md) |
| `.loc` localization, layout 1 | verified | [loc.md](loc.md) |
| `.loc` localization, layout 2 (64 bit) | verified (no keys) | [loc.md](loc.md) |
| Textures `.bin` | draft (container verified) | [texture-bin.md](texture-bin.md) |
| Community texture list `textures_database.bin` (TDB2) | decoded, see texture-heuristics | [texture-list-tdb2.md](texture-list-tdb2.md) |
| Size and format guessing for textures without `.xdb` | verified on 438 textures | [texture-heuristics.md](texture-heuristics.md) |
| Game version file `Profiles/game.version` | verified on four real files (version and install manifest) | [game-version.md](game-version.md) |
| FMOD Vorbis streams in FSB5 (codec 15) | verified against vgmstream | [vorbis-fsb.md](vorbis-fsb.md) |
| Resource databases `.bin` | draft | [bin-db.md](bin-db.md) |
| Audio `.fsb` / `.bsb` (FSB5, Vorbis), video `.ogv` (Theora) | partly verified | [media.md](media.md) |
| Model geometry `.(Geometry).bin` | draft (vertices and triangles located) | [geometry-bin.md](geometry-bin.md) |
| Pack pointer fixes (7.0 Bits32 `Bin/pack.bin`, block 4) | verified layout; tags 0/1/6/7 not decoded | [pack-fixes.md](pack-fixes.md) |
| Collision mesh data (layout only) | draft (in-memory layout from debug symbols; file unknown) | [collision-mesh.md](collision-mesh.md) |
| Skeletal animation `.(SkeletalAnimation).bin` | draft (header read by the app; tracks decoded by the core with strict checks; pose semantics open) | [skeletal-animation.md](skeletal-animation.md) |
| Client map chunks (`terrainDump`, splat maps, lightmaps) | draft (files and container seen; contents unknown) | [map-chunk.md](map-chunk.md) |
| NDb record layouts from PDB symbols (1.x editor) | draft (field lists only; serialization unknown) | [ndb-pdb-layouts.md](ndb-pdb-layouts.md) |
| `Version/DataPacksInfo.bin` pak and file list | draft | [data-packs-info.md](data-packs-info.md) |
| Pictures (`.ico`, `.dds`, JPEG/GIF/BMP size) | verified on one icon and one DDS | [pictures.md](pictures.md) |
| Materials (model to texture binding) | unknown in `.bin`; lives in the database record (Geometry, ModelElement, Material); layouts known for 1.1.02.0 only | [../TOOLS-STUDY.md](../TOOLS-STUDY.md) |
