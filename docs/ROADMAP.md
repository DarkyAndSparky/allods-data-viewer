# Roadmap

Short table below. Full plan with scope, acceptance criteria and risks per milestone (Russian): [ROADMAP-DETAILED.md](ROADMAP-DETAILED.md).

Goal: one tool to browse, extract, convert and (into new archives) pack all data of the game client.

| # | Milestone | Result |
|---|---|---|
| M0 | Skeleton | Cargo workspace, `web/`, tests, CI, license, docs (done) |
| M1 | Archives and localization | read `.pak` and both `.loc` layouts; CLI `list`, `extract`, `search` |
| M2 | Textures | `.bin` texture to DDS and PNG |
| M3 | Audio | open `.fsb` and `.bsb`, list streams, export WAV and MP3, batch export |
| M4 | Video | extract `.ogv`, open in the system player |
| M5 | Desktop app | Tauri window, file tree, text and texture preview, audio player, About dialog |
| M6 | Resource databases | `.bin` databases to JSON/XML, version detection |
| M7 | Models | export geometry to glTF |
| M8 | Materials | find how a model refers to its textures, export glTF with textures |
| M9 | 3D viewer | Three.js preview of models **with textures applied** |
| M10 | Pack | write a **new** `.pak` from a folder (zlib layer, CRC32), never touching originals |
| M11 | Skeletons and animations | |

After M1 the tool is already useful without any GUI.

## Why this order

M1 to M4 need only formats we can verify from samples we already have
(see [formats](formats/README.md)). M6 reuses the work documented by the MIT licensed
reference project ([REFERENCES.md](REFERENCES.md)). Models and materials (M7, M8) need
sample files and a research step: how a model refers to its materials and textures is
still unknown.

## Backlog (ideas)

- Full text search across all files and across localization keys
- Hex view
- Compare two client versions
- Localization browser, side by side Russian and English
- In-app video playback (transcode or a decoder)
- Plugin system for new formats
- Browser build of the core via WebAssembly

## Out of scope

- Changing game files in place (packing always writes new files)
- Redistributing game assets
- Anything that circumvents the game's protections

Note: replacing files inside a game client can break the game updater or its terms of service.
