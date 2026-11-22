# Roadmap

Short table below. Full plan with scope, acceptance criteria and risks per milestone (Russian): [ROADMAP-DETAILED.md](ROADMAP-DETAILED.md).

Goal: one tool to browse, extract, convert and (into new archives) pack all data of the game client.

| # | Milestone | Result | Status (2026-11-19, 0.1.45) |
|---|---|---|---|
| M0 | Skeleton | Cargo workspace, `web/`, tests, CI, license, docs | done |
| M1 | Archives and localization | read `.pak` and both `.loc` layouts; CLI `list`, `extract`, `search` | done |
| M2 | Textures | `.bin` texture to DDS and PNG | done for DXT1/3/5 (size and format guessed, cycled with Ctrl+click, or taken from the resource database); raw 16/32-bit and L8 selectable; ICO, DDS, PNG/JPEG/GIF/BMP viewing; TGA decoder missing |
| M3 | Audio | open `.fsb` and `.bsb`, list streams, export WAV and MP3, batch export | partial: FSB5 Vorbis to WAV and in-window player work; MP3 export 320 kbps CBR (0.1.62, `rusty_mp3`); FSB4 IMA ADPCM decoded (0.1.61), other FSB4 codecs open |
| M4 | Video | extract `.ogv`, open in the system player | partial: header info and extraction; in-window playback done (0.1.63, `theora-rs`) |
| M5 | Desktop app | Tauri window, file tree, text and texture preview, audio player, About dialog | done; Windows build used by the maintainer since 0.1.3, visual checks of the newest UI pending |
| M6 | Resource databases | `.bin` databases to JSON/XML, version detection | partial: 64-bit V2 reader (`db`), 32-bit reader (`db32`: Cloud Pirates 1.5, client 10.0 `pack.bin` and `Maps_*.bin`), texture sizes feed the viewer; JSON/XML export and the object image (Geometry, Material) not decoded |
| M7 | Models | export geometry to glTF | done (OBJ and GLB); some models (`Ship4_Hull`, `ElfFemale`) do not read yet |
| M8 | Materials | find how a model refers to its textures, export glTF with textures | not started; record layouts are known from PDBs and the XML database, binding to real data is next |
| M9 | 3D viewer | Three.js preview of models **with textures applied** | partial: viewer works without textures |
| M10 | Pack | write a **new** `.pak` from a folder (zlib layer, CRC32), never touching originals | not started (no `adv pack` yet); detailed in [SPEC](SPEC-audit-and-modding.md) block C |
| M11 | Skeletons and animations | decode clips, export glTF animation, play in the viewer | partial: decoder and glTF export done; player open |
| M12 | Maps and locations viewer | open map tiles and locations, render chunks top-down, assemble a global map with zoom and pan | not started; `Maps_*.bin` turned out to be resource databases of the same family as `pack.bin` (names, texture sizes readable, terrain objects not) |
| M13 | Scene render export | export the current view (a map area or a model scene) as PNG, and as SVG where the scene can be vector (for the Atlas of Allods) | not started |

### Work from the audit and modding specification

[SPEC-audit-and-modding.md](SPEC-audit-and-modding.md) (Russian, 2026-11-19) adds three blocks. Its version numbers are shifted by one, because 0.1.45 carries the accumulated study results instead of A-08/A-01/A-03.

| Block | Content | Status |
|---|---|---|
| A | Reliability and security: pack cache locking, cancellation and skipped-file report, `save_as` through a Rust-side dialog, per-format limits, Windows name collisions, fuzz targets, doc refresh, CI pinning, splitting `app.ts` | not started (A-08 doc refresh next) |
| B | Facts from reference projects into `docs/formats` (geometry stride, materials, texture formats, animation records, XDB shape) | not started; partly covered by `ndb-pdb-layouts.md` |
| C | Mod pipeline: `extract-all` with manifest, `adv pack`, patch packs, `verify-roundtrip`, `.loc` writing and TSV/JSON exchange, user warnings | not started (this is M10 in detail) |

Planned order: 0.1.46 A-08, A-01, A-03; 0.1.47 C-01, A-05, streaming extract; 0.1.48 C-02, C-04, C-08; 0.1.49 A-02, rest of A-04; 0.1.50 C-05, C-07; 0.1.51 C-03, A-07; 0.1.52 B-block and C-06.

### Study backlog (client files from the maintainer, no code until a result)

- Resource database of the current 18.0 client: where texture type and flag words live, real `pack.bin` check of the 64-bit path (not verified yet).
- Texture type/flag words in the 32-bit layout (`kind`, `alpha`), terrain objects in `Maps_*.bin`, Geometry and Material records.
- Cloud Pirates `.bsb` (FSB4/FSB5) and `.ogv`; Theora decoder choice (licences).
- `.loc` utilities of 2009 and ModdingDocuments, client 2.0 problem files.
- Feedback backlog (maintainer notes: `Desktop.zip` list and build 0.1.41 GUI notes). Done: Ctrl+wheel zoom and ICO (0.1.42), text sniffing and `.bak` text (0.1.42), jpg/png/gif/bmp (0.1.42), game version status (0.1.26), `.loc` table and text search (0.1.38/39), image size cycling (0.1.44), size from DB and raw texture formats (0.1.45). Open, none lost:
  - [x] Collapsible bottom panel and compact tree (0.1.49).
  - [ ] Viewer UI: image reset view done (0.1.50); model auto fit done (0.1.60); video scales to the window (0.1.63).
  - [x] Keyboard: arrow keys with hierarchy in the tree (0.1.49).
  - [x] Tab completion with suggestions in the path field (0.1.50; desktop command not compiled locally).
  - [x] Search and filters pinned while the list scrolls (sticky header); clear selection via Esc (0.1.51).
  - [x] Local search inside the selected pak/folder (0.1.52; desktop commands not compiled locally).
  - [x] Junk filter for Mods/addons (toggle, 0.1.56).
  - [x] Explorer-like folder view: crumbs and up (0.1.57). Back/forward history (0.1.59).
  - [x] `.cur` cursors and TGA decoder (0.1.54, synthetic tests only).
  - [x] ZIP browsing (0.1.58; split sets not supported).
  - [ ] Formats: office/PDF/table viewing (npm packages, approval needed), Lua decompile (after Atlas), `types.xml` question.
  - [ ] Encoding: `game.version` is shown as garbage text (binary header, needs its own parser); `readme.info` and similar cp1251 text shows mojibake (detect encoding); `SampleLocalization.*` and `DataPacksInfo.bin` parsers exist but are not wired into the app (checked 2026-11-19 on client 2.0 samples; `168362.rev` is fine).
  - [x] Cloud Pirates version lookup (0.1.55: game.version is found when `data`, `Packs` or the client root is opened). Open: "game data and archive unknown" wording.
  - [x] Models `Ship4_Hull`, `ElfFemale`, `ElfFemale_CutScene`, `HadaganFemale` (0.1.47, stride guess; not yet rendered on Windows).
  - [x] Export of text-like files to txt/html/xml (0.1.53).
  - [x] Export: MP3 (0.1.62). [x] In-window video playback (0.1.63).
  - [ ] Content: match replicas with characters and quests; old-format `.loc` hierarchy for Cloud Pirates.
  - [x] About dialog: nothing to remove (maintainer confirmed 2026-11-20).
  - [ ] Study: client 18.0 `pack.bin` (open `pack.rus.bin`/`pack.eng_eu.bin`, ask for 17.0 `pack.bin`), splat map pixel format, Texture type/flag words, terrain in `Maps_*.bin`.

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
- Think later: video export as MKV (lossless remux of the Theora stream); no MP4, it would need a re-encode and lose quality (maintainer decision 2026-11-21)
- Plugin system for new formats
- Browser build of the core via WebAssembly

## Maintainer wishes (2026-11-17)

- **Map and location viewer (M12).** The original map editor draws only a short distance, so
  assembling a whole allod meant stitching many screenshots taken at crooked angles. The viewer
  should render chunks from above (orthographic top-down), cover the whole location, and let the
  user zoom and pan the assembled global map. Needs the map tile formats first (server tiles
  `Tests/Maps/*/*_server.bin` are only partly known, see formats; client map data is unknown).
- **Scene render export (M13).** Save the current scene as a picture; PNG first, and SVG where the
  content is vector-friendly (outlines, regions, markers, labels over a raster underlay). The
  result is meant to go straight into the Allods Atlas the maintainer runs.
- **Tool study done (2026-11-17):** see [TOOLS-STUDY.md](TOOLS-STUDY.md) (what each community tool can do, what we take, open questions). MinimapTexture is a possible first source for the map viewer.
- **Later:** study the Pirates (Storm of Heaven) client, a separate game on game version 7.0.

## Out of scope

- Changing game files in place (packing always writes new files)
- Redistributing game assets
- Anything that circumvents the game's protections

Note: replacing files inside a game client can break the game updater or its terms of service.
- [x] FSB5 CELT (mode 12, FMOD CELT 0.11): decoder for the `Voice_*.bsb` banks of 18.0 and `Ambience_*.fsb` (port of libcelt 0.11, `crates/core/src/celt`); stereo and mono, frame size picked from the packet count; compared with the C reference on a Voice bank (max difference 1 LSB), 477 streams of 6 banks decode without errors. Not covered: the post-filter (FMOD's build has it compiled out), frame sizes other than 512 (not seen yet), listening comparison with the game.
- [x] SVG viewer: sanitised in the core (allow-list, `crates/core/src/svg.rs`) before it is drawn; see docs/formats/svg.md. Not checked in a browser yet.
- [x] Archive viewer: `.tar`, `.tar.gz`, `.tgz`, `.gz` (`crates/core/src/archive.rs`, docs/formats/archives.md). Open: `.7z`, `.rar`, `.tar.bz2`, `.tar.xz` (new dependencies, owner decision); command line support.
- [x] Archives `.7z`, `.xz`, `.tar.xz` (0.1.73); `.tar.bz2` and `.rar` have no suitable crate.
- [x] `.bev` (FEV project) viewer: names, event paths, languages (`crates/core/src/bev.rs`); event data `LGCY` not decoded. FSB4 codecs beyond IMA/PCM wait for samples.
- [ ] Streams of some banks show 8000 Hz / 0:00 / 2 ch (for example `GibberlingMale_deathFatalityEngineer_01`): check the sample header of those streams.
