# File formats

This directory is the source of truth for format descriptions in this project. If a
status or layout conflicts with an older planning or status document, prefer the
notes here. See [CONTRIBUTING.md](../../CONTRIBUTING.md) for the rules and
[REFERENCES.md](../REFERENCES.md) for sources.

| Format | Status | Note |
|---|---|---|
| `.pak` archive | verified | [pak.md](pak.md) |
| `.loc` localization, layout 1 | verified | [loc.md](loc.md) |
| `.loc` localization, layout 2 (64 bit) | verified (structure and text offsets; item keys require the resource database) | [loc.md](loc.md) |
| Textures `.bin` | draft (container verified) | [texture-bin.md](texture-bin.md) |
| Community texture list `textures_database.bin` (TDB2) | decoded, see texture-heuristics | [texture-list-tdb2.md](texture-list-tdb2.md) |
| Size and format guessing for textures without `.xdb` | verified on 438 textures | [texture-heuristics.md](texture-heuristics.md) |
| Game version file `Profiles/game.version` | verified on four real files (version and install manifest) | [game-version.md](game-version.md) |
| FMOD Vorbis streams in FSB5 (codec 15) | verified against vgmstream | [vorbis-fsb.md](vorbis-fsb.md) |
| FSB4 sound banks and `.bev` projects | partly verified | [fsb4.md](fsb4.md) |
| Audio `.fsb` / `.bsb` (FSB5, Vorbis), video `.ogv` (Theora) | partly verified | [media.md](media.md) |
| MP3 export | verified in core on real and synthetic streams | [mp3-export.md](mp3-export.md) |
| ZIP and common compressed archives | partly verified | [archives.md](archives.md) |
| Resource databases `.bin` | draft; selected layouts verified | [bin-db.md](bin-db.md) |
| Resource database objects in memory | draft | [pack-bin-object-image.md](pack-bin-object-image.md) |
| Model geometry `.(Geometry).bin` | draft (vertices and triangles located) | [geometry-bin.md](geometry-bin.md) |
| Common materials and texture bindings | draft | [common-material.md](common-material.md) |
| Pack pointer fixes (7.0 Bits32 `Bin/pack.bin`, block 4) | verified layout; tags 0/1/6/7 not decoded | [pack-fixes.md](pack-fixes.md) |
| Collision mesh data (layout only) | draft (in-memory layout from debug symbols; file unknown) | [collision-mesh.md](collision-mesh.md) |
| Skeletal animation `.(SkeletalAnimation).bin` | draft (header read by the app; tracks decoded by the core with strict checks; pose semantics open) | [skeletal-animation.md](skeletal-animation.md) |
| Client map chunks (`terrainDump`, splat maps, lightmaps) | draft (files and container seen; contents unknown) | [map-chunk.md](map-chunk.md) |
| NDb record layouts from PDB symbols (1.x editor) | draft (field lists only; serialization unknown) | [ndb-pdb-layouts.md](ndb-pdb-layouts.md) |
| `Version/DataPacksInfo.bin` pak and file list | draft | [data-packs-info.md](data-packs-info.md) |
| Pictures (`.ico`, `.dds`, JPEG/GIF/BMP size) | verified on one icon and one DDS | [pictures.md](pictures.md) |
| SVG sanitization | verified in core; browser rendering separate | [svg.md](svg.md) |
| Materials (model to texture binding) | draft for 7.0 (element ranges and `diffuseTexture` verified on two models); 18.0 not read | [model-binding.md](model-binding.md) |
| Sound to object binding | draft: record types found in 7.0 and 18.0, layouts unknown | [sound-binding.md](sound-binding.md) |
