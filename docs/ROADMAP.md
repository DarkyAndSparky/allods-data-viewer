# Roadmap

| # | Milestone | Result |
|---|---|---|
| M0 | Skeleton | Cargo workspace, `web/`, tests, CI, license, docs |
| M1 | Archives and localization | read `.pak` and `.loc`; CLI commands `list`, `extract`, `search` |
| M2 | Textures | decode to PNG |
| M3 | Models | export geometry to glTF, opens in Blender |
| M4 | Materials | find out how a model refers to its textures, bind them, export glTF with textures |
| M5 | Desktop app | Tauri window, file tree, text and texture preview |
| M6 | 3D viewer | Three.js preview of models **with their textures applied** |
| M7 | Skeletons and animations | |

After M3 the project is already useful without any GUI.

## Textured models (M4 and M6)

The previous viewer showed only bare geometry. Showing a model with its textures
needs one thing we do not know yet: **how a model refers to its materials and
textures** (a material file, a name convention, or data inside the model).
M4 is a research milestone for this and needs real sample files: a few models
together with the files they use. The result is described in `docs/formats/`
before any code is written. M6 then only has to feed the bound textures to Three.js.

## Backlog (ideas)

- Full text search across all files
- Hex view
- Compare two client versions
- Batch export
- Localization browser and export
- Plugin system for new formats
- Browser build of the core via WebAssembly

## Out of scope

- Modifying or repacking game files
- Redistributing game assets
