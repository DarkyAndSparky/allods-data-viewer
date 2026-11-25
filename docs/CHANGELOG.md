# Changelog

What changed between delivered builds. Facts only. One section per version, newest first.
Rule: every new build adds its section here before the archive is made (see [VERSIONING.md](VERSIONING.md)).

Entries for 0.1.14 to 0.1.44 were rebuilt from the delivery messages of the project chat (2026-11-19); where a message gave no detail the entry says so.

## 0.1.82

### Fixed
- Texture size guess (no database): a wide picture is preferred over a tall one with the same short side, and equal-size candidates are decoded and scored by the luminance step across 4x4 block edges. On 14 108 textures of client 1.1 the size is right in 98 % of cases (30 % before).
- Database pak choice: the pak is taken only when it holds `Bin/pack.bin` itself (a localisation pak with other files in `Bin/` was picked first and hid the database).
- Model textures: parts without a picture get the plain model colour instead of the glTF default white.
- Client 1.1 `Texture` record (width +52, height +56, mips +60, type +32) is read; the 7.0 layout is still tried first.

### Added
- "Texture" list next to "Apply textures": textures from the model folder and the sibling `Textures` folder (up to 300). A chosen texture covers every element; without a database entry the whole mesh gets it. Interface texts `texPick`, `texAuto` in Russian and English.
- Tests: 1.1 texture layout (and a rejected implausible mip count), orientation by content, flat data keeps the default, plain colour for untextured parts, database pak needs `Bin/pack.bin`.

### Not verified
- The texture list and the white-model fix in the real window on Windows; the cause of "texture not found" on the maintainer's 7.0 client (his exact error text is not known). Model elements of client 1.1 are not bound (record layout differs).

## 0.1.81

### Added
- Folder view has a "List / Thumbnails" switch (remembered between runs). In the thumbnail view every texture and model gets a small picture: textures from the decoded image, models as a front-view still drawn by one shared offscreen renderer. Pictures load when a tile scrolls into view, two at a time; leaving the folder drops the queue and frees the pictures. Models above 3 000 000 indices and models where WebGL is unavailable show "No preview". Folders and other files show a symbol.
- Interface texts `viewList`, `viewThumbs`, `thumbNone` in Russian and English. Web test for the job queue (limit and order, clearing).

### Not verified
- The look and speed of the tile view in the real window on Windows (large folders, models with many parts); the web build and tests pass, the desktop app was not compiled here.

## 0.1.80

### Added
- `adv db sound <pack.bin> [--filter]`: lists the `FMODProject` records (event file and banks) and the `CharacterSoundVariation` records (default project and FMOD event paths with the stored number) of a 32-bit database. The file may be zlib compressed or already inflated.
- Core module `soundbind` (`all_characters`, `all_projects`) with bounds-checked reads and 4 tests (success, missing fixes, truncated image and bad sizes, pointers outside the image).
- `docs/formats/sound-binding.md`: layouts of `CharacterSoundVariation` and `FMODProject`, the link from `VisCharacterTemplate`, and who points at sound projects (draft).

### Verified
- On the `pack.bin` of client 1.1.00.41: 11 character records and 14 projects are read; the events of `ElfMale` use the `Voice` project and their names occur in `SFX/Voice/Voice.bev`.

### Not verified
- Creature sounds (played by effect records), the meaning of the stored number and of the second array, 7.0 and 18.0 databases, display in the window.

## 0.1.79

### Fixed
- Kind lists (the filter row at the bottom of the side panel) showed only the first 1000 files with no way to see the rest. The list now holds up to 200 000 files and is shown in pages of 200 with Previous/Next buttons and a "from-to of total (page X of Y)" counter.
- The resource database is also looked for in `Bin*.pak` and `System*.pak` (not Mini/HiRes), not only in `BaseLoc*.pak`, so "Apply textures" works for clients that keep `Bin/pack.bin` there. Test `base_pak_also_finds_bin_pak_and_skips_packs_without_a_database`.

### Not verified
- The paging in the real window on Windows; loading a list of tens of thousands of files was not timed.
- That the maintainer's 7.0 client keeps its database in one of these paks.

## 0.1.78

### Fixed
- "Apply textures" failed for unpacked 7.0 data: the model had to sit inside a pak and the database in a `BaseLoc*.pak` under the opened folder. Now a model is matched to the database by the longest tail of its path, so it can be a loose file or in any pak, and a loose `Bin/pack.bin` below the opened folder is used when there is no `BaseLoc*.pak`.
- The error of the switch now names the cause: no database found (open the client folder), or the model is not in the database.

### Not verified
- On Windows in the real window. Checked on Linux with a folder of paks, a folder of loose files plus `Bin/pack.bin`, and a folder without the database (error text).

## 0.1.77

### Added
- `binding::ElementBinding::params`: per-element `CommonMaterialParams` (bump, effect mask, environment reflection, environment specular texture names, shininess, specular intensity) read through the element's `params` pointer; test `element_params_are_read_through_their_pointers`.
- Statistics of the 7.0 pack in `docs/formats/model-binding.md`.

### Not verified
- The new fields are not used by the viewer or the glTF export (no standard glTF slot for them).

## 0.1.76

### Fixed
- `adv db info` and the database reader open the current 18.0 `pack.bin` (build 1148): the file-name table is absent there and the struct names sit in directory slot 1. The old order is tried first; the new one is accepted only when every entry is a `struct NDb::` name.

### Added
- Test `build_1148_has_structs_in_table_one_and_no_names`.

### Not verified
- Names of resources of 18.0 are still unknown, so models of 18.0 get no textures (the root names are expected in `System/index.bin`, not seen yet).

## 0.1.75

### Added
- `adv-core::binding`: element ranges (LOD 0) and `diffuseTexture` of every Geometry record of a 7.0 `pack.bin`, read through the pointer-fix table.
- `Geometry::to_glb_textured`, `preview::mesh_glb_textured`: glTF split into one primitive per element with a PNG base colour texture per material; the BIN chunk is padded to 4 bytes.
- `Session::glb_textured` (textures are looked up next to the model, in the same archive), desktop command `get_textured_glb`.
- Model view: "Apply textures" switch, works together with the animation clips; wireframe also applies to textured models.
- `docs/formats/model-binding.md`.

### Changed
- Merged the maintainer's changes from GitHub `main` up to `ee395ee` (CELT bands, FSB4, texture reader, stale-answer guards in the web UI, cookie 0.18.3, url 2.5.9, README and format notes).

### Not verified
- `adv-desktop` (not compiled here), the switch in the real window on Windows.
- Models of client 18.0 (no binding yet), effect textures from other folders.

## 0.1.74

### Added
- Model view: a list of animation clips of the model (files `.(SkeletalAnimation).bin` in its folder and in an `Animations` folder next to it), a pause/play button and a "Loop" checkbox (on by default). The model is shown skinned and plays the chosen clip; "No animation" returns the static mesh. Texts in ru and en.
- `Session::clips_of`, `Session::glb_animated`, `adv_core::preview::mesh_glb_animated`; app commands `get_clips`, `get_animated_glb`.
- Tests: world pose of a scaled bone matrix, a clip that is not a clip, clips next to a model.

### Changed
- Merged GitHub `main` (pull requests 3, 4, 5, 6): `fixes.rs`, `material.rs`, `declared_stride`, formats notes `pack-bin-object-image.md`, `pack-fixes.md`, `common-material.md`, `INSTALL.md`; vite 8.3.8, zip 9.7.3.
- `crates/desktop/icons/icon.icns` recompressed without loss (2 070 377 to 1 946 628 bytes) to fit the repository size limit.
- Added `CONTRIBUTORS.md` and a Contributors section in both READMEs.
- `docs/formats/geometry-bin.md` regrouped: scaled bones next to the bind pose, the database record notes under one heading with links to the new format notes.

### Fixed
- Models of client 18.0 (bone matrices with a uniform scale 1.2): the skeleton pose is inverted with the scale and the `.glb` nodes carry the scale ratio, so clips of `Mirianna` are accepted. Models of 7.0 give the same pose (the numbers in the `.glb` differ in the seventh digit).

### Known
- Checked: `.glb` of ArchdemonMage (7.0) and Mirianna (18.0) with clips load and play in a headless Chromium (the mesh stays whole and the time runs); not checked in the app (`adv-desktop` is not built here) and no side-by-side look at the motion against the game.
- Bone `Twist_L` of Mirianna has a different scale (0.815) and its rest translation differs from the clip.
- Bones with a non-uniform scale are not handled.

## 0.1.73

### Added
- Archives `.7z`, `.xz`, `.tar.xz`, `.txz` open as folders (`adv_core::archive`: `Kind::SevenZ`, `TarXz`, `Xz`, `SevenZArchive`).
- Dependencies `lzma-rust2` 0.25.0 (2026-11-01, Apache-2.0) and `sevenz-rust2` 0.25.0 (2026-11-06, Apache-2.0), both pure Rust, `default-features = false`; dev features `encoder`/`compress` for the tests.
- Tests: names of the kinds, `.tar.xz` and `.xz`, every cut and a damaged byte of an xz stream, inflate cap, `.7z` listing and reading, unsafe path skipped, every cut and every damaged byte of a `.7z` (no panic).
- `docs/THIRD-PARTY.md`, `docs/DEPENDENCIES.md` and the list in the app got the two components.

### Known
- Not checked on real `.7z`/`.xz` files, only on synthetic ones; not checked in the app.
- `.7z` with AES, bzip2, ppmd, zstd or brotli is not read (the features are off). A solid `.7z` is decoded from the start of its block for every file.
- `.tar.bz2` and `.rar` are not supported: no suitable crate (bzip2 0.6.1 is from 2025; `unrar` wraps non-free UnRAR code).

## 0.1.72

### Added
- `.bev` files (FMOD Designer `FEV ` projects, zlib) are previewed: format version, project name, languages, chunk sizes, plain names and event/wave paths from the string table (`adv_core::bev`, preview kind `bev`). The labels come from the interface language files (ru and en). The event data (`LGCY`, `EPRP`, `OBCT`) is listed by size and not decoded.
- Tests: project, languages and strings (plain and zlib), data without a container, every cut of the file and of the zlib stream, a damaged string table (count too large, offset outside, huge count), a chunk past the end, random damage (2000 files), inflate cap; preview of a `.bev` container and of junk named `.bev`.
- `docs/formats/fsb4.md`: `.bev` layout.

### Known
- Checked on `VoiceDialogs03.bev` (916 strings) in the core; not checked in the app. Which event plays which sound bank is not decoded.
- FSB4 codecs other than IMA ADPCM and raw PCM are still not supported: the only FSB4 sample is the Cloud Pirates bank (IMA, all 119 streams decode), so another codec cannot be checked.

## 0.1.71

### Added
- Archives `.tar`, `.tar.gz`, `.tgz` and `.gz` open like folders in the viewer (`adv_core::archive::Archive`, no new dependency; `flate2` was already used). TAR: header checksum, octal and base-256 sizes, GNU long names, pax `path` and `size`, ustar `prefix`; directories, links and other types are skipped with a reason; unsafe paths (`..`, absolute), duplicates, over-limit entries and cut-off data are skipped with a reason; a damaged header after the first ends the listing and keeps earlier entries; a damaged first header is an error. GZ: inflated into memory with the output cap and the ratio guard; a tar inside is detected by the `ustar` signature, otherwise the file is one entry named without `.gz`.
- Tests: list and read, empty tar, `.tar.gz`/`.tgz`/`.gz`, single gz, long names, pax, prefix, unsafe/duplicate/link entries, size limit, base-256 sizes, truncated tar, corrupted headers, gzip bomb and damage, every cut and random damage without panic, kinds by file name; session: a folder with `.tar`, `.tgz` and `.gz` is browsed and read.
- `docs/formats/archives.md`.

### Changed
- The session opens any archive through `Archive` (ZIP/`.pak` behaviour is unchanged). Error text for a file that is not a folder or an archive lists the supported extensions.
- `flate2` added to the dev-dependencies of `adv-session` (workspace version, tests only).

### Known
- `adv` (command line) still opens only `.pak`/`.zip`; `.7z`, `.rar`, `.tar.bz2`, `.tar.xz` need new dependencies and an owner decision.
- Not checked in the app on Windows.

## 0.1.70

### Changed
- Replaces the page-side SVG detection of 0.1.69 (removed): the file is sanitised first, the page only draws the clean copy.

### Added
- `.svg` files are sanitised in the core before they are shown (`adv_core::svg::sanitize`, no new dependency): a small XML reader, then a new document is written from allow-lists of elements (shapes, text, gradients, patterns, clip/mask, markers, filters and their primitives, `use`, `image`), attributes and values. Removed: scripts, `foreignObject`, animation (`animate`, `set`), `style` elements, `a`, `iframe` and any other element not on the list (with its content); event handlers and other attributes not on the list; links other than `#fragment` (and inline `data:image/png|jpeg|gif|webp;base64` on `image`); `url(...)` that does not point at a `#fragment`, `javascript:`, `expression(`, `@import`, backslashes in values. Files with a DOCTYPE subset or entities (other than the five XML ones and numeric ones), mismatched tags, a root other than `svg`, nesting over 128, over 200000 elements or over 4 MiB are refused and shown as text. The namespace is rewritten on the root.
- Preview kind `svg` (clean copy, original text, number and names of removed items). The page draws the clean copy as an `<img>` (second layer: scripts and outside links do not run in an image either). Buttons Picture / Source; a line tells what was removed or that nothing had to be. Texts in ru and en.
- Tests (core): clean file, namespace, scripts and handlers, foreign content, animation and style elements, links, `url()` values, unknown and prefixed attributes, comments/PI/DOCTYPE, CDATA and escaping, entities and entity bombs, malformed files, limits, repeated names, UTF-16, every cut and random damage of a sample without panic, nothing active in the output; preview of a dirty SVG and of a broken SVG (shown as text).

### Known
- Not checked in a browser here (type check and tests only); `adv-desktop` mapping of the new preview kind is not compiled here.
- SVG export of scenes (M13) is separate.

## 0.1.69

### Added
- `.svg` files are drawn as pictures (an `<img>` from a blob, so scripts and outside links do not run); buttons Picture / Source switch to the text; an SVG the browser cannot draw falls back to the source with a note. Only whole files qualify (the first element is `<svg`, after prolog, comments and doctype); files cut at 256 KiB stay text. Texts in ru and en.
- Tests: SVG detection (plain, with prolog/comment/doctype/BOM, other extension, cut file, non-svg root).

### Known
- Not checked in a browser here (only type check and unit tests); SVG export of scenes (M13) is separate.

## 0.1.68

### Added
- About dialog: "Components and licenses" list (name, version, licence, purpose in the interface language, link). Data in `web/src/third-party.json`.
- `scripts/third-party.mjs generate|check`: builds the list from `cargo metadata` and `web/node_modules` (direct runtime dependencies, `@tauri-apps/api` as bundled), plus code ported into the repository (libcelt); writes `docs/THIRD-PARTY.md` with all 420 Rust packages of the build. `check` fails when the files are out of date, a direct dependency has no ru/en purpose in `scripts/third-party-purpose.json`, or a licence is not on the allowed list (an OR expression passes when one alternative is allowed). Added to `check-all.sh` and `check-all.ps1`.
- Tests: licence expressions (`scripts/third-party.test.mjs`); the dialog lists components in both languages, escapes names, links only `https`; every component has ru and en text.

## 0.1.67

### Added
- FSB5 mode 12 (CELT, FMOD CELT 0.11): decoder in `adv_core::celt` (port of libcelt 0.11, float build without the post-filter): range decoder, Laplace, PVQ/CWRS, pulse cache and band allocation, band shape decoding with stereo split and folding, anti-collapse, inverse MDCT, de-emphasis. Used by `Voice_*.bsb` of 18.0 clients and `Ambience_*.fsb`; works in the viewer, `adv audio export` (WAV and MP3).
- FMOD packet framing (`0xf30dc317`, size, packet) and frame size detection from the packet count.
- Tests: zero packet gives a full frame, mono frame, truncated packet, unsupported layouts, damaged packets of several modes do not panic; framed stream, header length wins over whole frames, frame size from packet count, cut-off or unframed stream is an error.
- NOTICE entry for the Xiph BSD code.

### Changed
- Error for FSB5 mode 12 no longer says "not supported"; other unsupported codecs keep the codec name in the error.

### Known
- Checked against the C reference on one Voice stream (max difference 1 LSB) and by decoding 477 streams of 6 Voice banks without errors; `Ambience_*.fsb` were not available for the check; not compared by ear with the game.
- Packets with the post-filter flag are rejected as corrupted (not seen in samples).

## 0.1.66

### Added
- Sound of `.ogv` videos: the Vorbis track of the Ogg file is decoded in the core (`adv_core::video::ogg_audio`, block sizes taken from the identification header, so any standard libvorbis stream works), sent to the page as WAV (`video_audio` call and command) and played next to the picture; while it plays it is the clock of the picture (frames follow `currentTime`), after its end the wall clock continues. A video without sound plays as before.
- Loop checkbox for the video player and for the bottom sound player.
- Tests: tiny synthetic video with a 440 Hz mono track (ffmpeg, kept as hex): rate, channels, length, frequency; no track in a silent video; truncated and corrupted files do not panic.

### Changed
- Video layout: the picture is scaled to the free height of the window (aspect kept), controls are under it, the video line (size, rate, length, frames) is under the controls.
- All file views: the file name is a smaller bold line, the save buttons are a bar under the data that stays at the bottom of the view.
- Starting a video pauses the bottom sound player and clears the old error message.

### Fixed
- Changing the language added another title, sound control and close button to the bottom player each time (it was filled again without being cleared).

### Known
- Not verified: sound sync on Windows (checked in Chromium without sound, with a stub for the app calls), `adv-desktop` compile.
- Video export to MKV, settings page, SVG, archive and office viewers, CELT are still open.

## 0.1.65

### Fixed
- FSB5 channel layout: bits 5..6 of the sample mode word are read as a field (0 = mono, 1 = stereo, 2 = 6 channels, 3 = 8 channels). Music banks of 18.0 clients store adaptive and some other tracks as 6-channel Vorbis; they were read as mono and decoded as noise (`Siege_warfare_adaptive`, `NihazAdaptive`, `JungleAdaptive`, `IllusionWorldAdaptive`, `Conquer_adaptive`, `ASS_Battle01`). Checked on the maintainer's samples: smooth output, no clipping.
- Video player: a negative frame index could be requested after Play (`invalid args 'start' ... integer '-1'`); the playback clock now never goes below its start.

### Changed
- WAV with more than two channels is written with the WAVE_FORMAT_EXTENSIBLE header and a speaker mask (6 channels: 5.1, 8 channels: 7.1).
- MP3 export of sounds with 3 to 16 channels mixes them down to stereo (5.1/7.1 by the usual layout without the low-frequency channel; other counts: even channels left, odd right); documented as a lossy step. A channel count from a chunk of type 1 (for example 10) is used as is.

### Known
- FSB5 mode 12 (CELT): `Ambience_TP.fsb`, `Ambience_ForestSpring.fsb`, `Voice_*.bsb` of 18.0 clients are still not decoded. A reference decoder of the old CELT bitstream (libcelt 0.11, from the Xiph history) decodes the voice streams, so a port is possible.
- The sound of `.ogv` videos is not played yet; no LOOP option; the video layout does not fit the window yet (reported by the maintainer, planned).
- `NoSOUND` streams are 0.06 s of silence at 8000 Hz (valid, not damaged).
- Not verified: `adv-desktop` compile; playback of 6-channel WAV in the Windows window.

## 0.1.64

### Fixed
- FSB5 data offset: bit 6 of the sample mode word no longer counts as part of the offset (offset is bits 7..33 in units of 32 bytes). Mono music banks with that bit set (`Music_ASS.fsb`, `Music_Astral_hangar.fsb`) read their first packet 16 bytes late and decoded to a few hundred samples while reporting success; they now decode to full length, as do all 90 streams of `Music_Zone.fsb`.
- A Vorbis stream that ends before 98% of its declared length is now an error instead of a short successful file.
- WAV and MP3 buttons of a sound-bank stream propose the stream name as the file name (was the bank name for every stream).

### Changed
- Sound banks: Play, Save WAV and Save MP3 have a fixed width (long labels are cut), long stream names are cut with the full name in a tooltip.
- One sound player is pinned to the bottom of the window, above the status bar (title of the playing stream, spinner while decoding, close button); playing continues while browsing other files.
- Error text for unsupported FSB5 codecs names the codec (for example "CELT (FSB5 mode 12)").

### Known
- FSB5 mode 12 (CELT), used by the `Voice_*.bsb` banks of 18.0 clients, is not decoded (documented in `docs/formats/vorbis-fsb.md`).
- Not verified: `adv-desktop` compile; the bottom player in the Windows window (checked in Chromium with a stub).

## 0.1.63

### Added
- In-window video playback for `.ogv` (Theora): canvas player with Play/Pause, a position slider and a time label; frames are decoded in the app and converted on the page. New core module `adv_core::video` (Ogg packet reader with limits, Theora decoding, planar YCbCr frames), session calls `video_open` and `video_frames`, commands of the same names in `adv-desktop`.
- Dependency `theora-rs` 0.1.0 (MPL-2.0, released 2026-10-08, no runtime dependencies), approved by the maintainer; NOTICE updated.
- Activity indicators: a bar at the top of the window appears when an app call stays pending for more than 0.3 s; a spinner shows while the player loads frames.
- Tests: tiny synthetic Theora video (made with ffmpeg from its test pattern, kept as hex in `video_fixture.rs`): geometry and frame count, rewind, truncated and corrupted files (no panics), oversize picture refused; web tests for the YCbCr to RGB conversion.
- Strings `videoPlay`, `videoPause`, `videoBuffering`, `videoPosition`, `videoFailed` (ru, en); `videoNote` now says playback is available in the app only.

### Not verified
- `adv-desktop` (`video_open`, `video_frames`) was not compiled here; playback inside the Windows window; behaviour on long or 4K videos. The player was checked in Chromium with synthetic frames; the decoder on the real `Intro.ogv` (73 frames, one frame viewed).

## 0.1.62

### Added
- MP3 export of sounds: `adv audio export --format mp3` (default `wav`), "Save MP3" button next to each stream in the app, `Session::audio_mp3`, `save_as` format `mp3`. Highest constant bitrate for the sample rate (320 kbps for 32/44.1/48 kHz, 160 kbps for MPEG-2 rates). Rates MP3 cannot carry and more than two channels are refused with a message. `--format mp3` is not available with `--vgmstream`.
- Dependency `rusty_mp3` 1.1.0 (Apache-2.0, released 2026-11-19, no runtime dependencies), approved by the maintainer.
- Tests: stereo tone round trip (length, rate, signal-to-noise), mono MPEG-2 rate, refused rate/channels/empty sound, 10-sample sound. Real streams: signal-to-noise 34.5 to 51.5 dB (see `docs/formats/mp3-export.md`).
- Strings `saveMp3`, `filterMp3` (ru, en).

### Not verified
- `adv-desktop` (`save_as` with `mp3`) was not compiled here; no listening test; Windows run.

## 0.1.61

### Added
- FSB4 sound banks (Cloud Pirates `.bsb`/`.fsb`): list, player and WAV export (`adv audio list/export`, the app). Codecs: IMA ADPCM (mono verified on a real bank, all 119 streams decode) and raw PCM 8/16-bit (draft). Other codecs fail with their mode value. Tests: synthetic success (step rule, length cut, stereo order, PCM), refused codec, truncated and corrupted banks, damaged step index.
- README.md and README.ru.md: the install and build guide (Windows and Linux) moved in from the maintainer's INSTALL.md; paths use `%USERPROFILE%\Downloads\BUILDS` and `<version>`; the developer section is now "Develop and test".
- `docs/formats/fsb4.md`, with a note on `.bev` (zlib, FMOD Designer `FEV FMT` project; not parsed yet).

### Changed
- `adv audio list` prints the bank kind (FSB4 or FSB5).
- Not verified: stereo block order, raw PCM, in-app playback of an FSB4 stream on Windows.

## 0.1.60

### Changed
- Model viewer: the start and "reset view" camera distance is computed from the bounding sphere and the field of view for both the height and the width of the window, so a model fits a narrow window too (before, a fixed multiple of the radius was used). The layout is measured before the first camera placement.
- Not verified: no WebGL check on a real model in this session.

## 0.1.59

### Added
- Folder view: Back and Forward buttons walk through the folders opened in this view (history of up to 100 entries; opening a new folder after going back drops the forward entries). Strings in Russian and English.

## 0.1.58

### Added
- Plain `.zip` files (mods, add-ons) are browsed like `.pak` archives: they open as folders in the tree and the folder view, take part in the name and text search, can be opened directly (Open file dialog accepts `.zip`) and complete in the path box. Test: a `.ZIP` with a nested folder (listing, reading, completion, opening directly, a text file is refused).

### Not verified
- Only simple, single-file ZIPs written by the test helper; split sets (`.z01`, `.z02`) and encrypted or ZIP64 archives were not tried. Windows and the Tauri window not checked.

## 0.1.57

### Added
- Explorer-like folder view: double-click a folder in the tree to list it in the main area, with clickable path crumbs (data root and every folder), an "Up one level" button, folders that open in place and files that open in the viewer. The list follows the type filter, sort order and the Mods/addons filter. A file view shows the crumbs of its folder above the title; clicking a crumb opens that folder. Strings in Russian and English.

### Not verified
- Checked in Chromium against a stubbed bridge (open folder, enter subfolder, open file, crumb, up); Windows and the Tauri window not checked. No back/forward history yet.

## 0.1.56

### Added
- Filter "Hide Mods and addons folders" (Russian and English, remembered): folders named `Mods` or `addons` (any case) disappear from the tree, and files under such folders from the name search, the text search and the type lists. A name like `Modsy` is not hidden. Off by default. Tests: `isJunkName`, `isJunkPath`.

### Not verified
- Checked in Chromium against a stubbed bridge (tree, search); Windows and the Tauri window not checked.
- Not done: Explorer-like folder navigation (address bar crumbs, back/up) from the same backlog item.

## 0.1.55

### Changed
- `game.version` is also looked up two folders above the opened folder (and in their `Profiles`), so opening `data/Packs` finds `client/Profiles/game.version`. Checked with a real Cloud Pirates 1.5 `game.version` (version `1.5.32`, head `ver5`) placed as `client/Profiles/game.version`, opened as `client`, `client/data`: the version is read; the `Packs` case is covered by a synthetic test.

### Not verified
- Desktop build (`adv-desktop`) and Windows; the status line text itself was not looked at in the window.

## 0.1.54

### Added
- TGA pictures (`.tga`) open as images: colour-mapped, true colour and gray, plain or run-length encoded, 8/15/16/24/32 bits, any origin (bottom or top, left or right). Alpha is used only when the descriptor declares attribute bits. Tests: bottom-left and top-left origin, 32-bit alpha rule, RLE, palette, gray, 16-bit, and damaged input (cut off, wrong type, zero and huge size, palette index out of range, cut-off RLE run).
- Windows cursors (`.cur`) open as images (same container as `.ico`; the hot spot is ignored).

### Not verified
- Both decoders were tested on synthetic files only, no real game files; Windows visual check not done.

## 0.1.53

### Added
- Export of text files and `.loc` files as `.txt`, `.html` or `.xml` (three buttons in the file view, Russian and English). Text is decoded first (UTF-8, UTF-16 with a mark, else Windows-1251). `.loc`: `.txt` has one `key<TAB>text` line per item, `.xml` has `<item id key>` elements. HTML and XML are escaped and characters XML cannot carry are dropped. Core: `Session::export_text` with a test (escaping, `.loc`, unknown format, missing file, damaged `.loc`); the `save_as` command accepts `txt`, `html` and `xml`.

### Not verified
- Desktop build (`adv-desktop`): the `save_as` change was edited without compiling the desktop crate; the save dialog was not run (buttons checked in Chromium against a stubbed bridge).

## 0.1.52

### Added
- Search can be limited to a folder: the folder last clicked in the tree becomes the scope, and the checkbox "Only in: <folder>" (Russian and English) limits both the name search and the text search to it. Core: `Session::search_in` and `Session::search_text_in` (a test covers a pak and a subfolder); the desktop commands `search` and `search_text` take an optional `scope`.

### Not verified
- Desktop build (`adv-desktop`): the changed `search` and `search_text` commands were edited without compiling the desktop crate. The UI was checked in Chromium against a stubbed bridge (the checkbox, the label, and `scope` reaching the command).

## 0.1.51

### Added
- Esc on a focused file row (tree or search results) clears the selection: highlight removed, preview closed, back to the hint.

### Not verified
- Desktop build (`adv-desktop`) and Windows; checked in Chromium against a stubbed bridge only.
- Already in place and now confirmed in Chromium: the search box and filters stay pinned at the top of the side panel while the list scrolls (sticky header).

## 0.1.50

### Added
- Tab completion in the path box: Tab fills the common start of the matching folders and `.pak` files and lists the candidates under the box (click picks one, Esc or Enter closes). Core: `adv_session::complete_path` (ASCII case-insensitive, at most 50 results) with a test; desktop command `complete_path`; web helper `commonPrefix` with a test.
- Image view: "Reset view" button (fit to the window, scrolled to the top left); the image is centered.

### Not verified
- Desktop build (`adv-desktop`): the new `complete_path` command was written without compiling the desktop crate. Windows paths with drive letters were tested in Chromium against a stubbed bridge only; the Rust function was tested with `/` paths on Linux.
- Still open: auto fit/center for model and video views.

## 0.1.49

### Added
- Keyboard navigation in the file tree: Up/Down move between visible rows, Home/End jump to the first/last row, Right opens a folder (or enters it when open), Left closes it (or goes to the parent folder). Folders carry `aria-expanded`.
- The bottom status bar can be folded to a thin strip and unfolded (button at its left, state remembered; strings in Russian and English).

### Changed
- The tree is more compact: indent per level 1.1 rem to 0.6 rem, rows without vertical padding.

### Not verified
- Desktop build (`adv-desktop`) and Windows. The tree and the status bar were checked in Chromium against a stubbed desktop bridge (keys, fold, screenshots), not inside the Tauri window.
- Not done from the same list: Tab completion in the path field (needs a new desktop command), "default view" button and auto fit for image/model/video.

## 0.1.48

### Changed
- `game.version` for the status line is now looked up in the opened folder, its `Profiles` folder, the parent folder and the parent's `Profiles` folder (names case-insensitive), so opening `data` or the client root both show the game version; before, only `Profiles/game.version` directly under the opened folder was read.

### Not verified
- Desktop build (`adv-desktop`) and Windows visual checks.

## 0.1.47

### Fixed
- Models `Ship4_Hull`, `ElfFemale`, `ElfFemale_CutScene` and `HadaganFemale` (client 2.0) now open: the vertex stride guess prefers the candidate (24, 28, 32 or 36) with finite positions; before, a smaller stride that also divided the block was taken and the file was rejected for non-finite positions. Regression test added.

### Docs
- `formats/geometry-bin.md`: the stride rule and the four verified models.

### Not verified
- Desktop build (`adv-desktop`), Windows visual checks; the models were checked as parsed data (finite positions, indices inside the vertex block), not rendered.

## 0.1.46

### Added
- `game.version` opens in the viewer as a listing: the version line `<version> (<tag>)`, then each directory with its files and sizes. A damaged file still opens as before.

### Changed
- Text without a byte order mark is decoded as UTF-8 when valid, else as Windows-1251 (old Russian client files such as `readme.info` no longer show as garbage).

### Docs
- `REFERENCES.md`: six repositories from the maintainer with license status; `formats/bin-db.md`: V2 table order cross-checked with EditorTools, notes on the 1.1 server `System/index.bin` (resource name index, ~194 000 paths); note that the client has no `System/index.bin`: it is not found in 1.1 (`index.srv`, 6 bytes) or 18.0.7 `System.Mini.pak`.

### Not verified
- Desktop build (`adv-desktop`) and Windows visual checks; `game.version` listing tested on synthetic data only.

## 0.1.45

### Added
- `adv-core::db32`: reader for the 32-bit resource database layout (Cloud Pirates 1.5 `Bin/pack.bin`, client 10.0 `Bin/pack.bin` and `Bin/Maps_*.bin`). Names with checksum verification, struct names, `Texture` records in two layouts (100 and 104 bytes), `texture_sizes()`.
- `adv-core::db::Database::texture_sizes()` for the 64-bit V2 layout.
- The session finds `BaseLoc*.pak` (Russian first, then English, HiRes and Mini skipped), reads `Bin/pack.bin` and `Bin/Maps_<map>.bin` once on the first texture preview and uses the texture size from it as the default; a missing or damaged database is ignored.
- Texture pixel formats RGB565, RGBA4444, BGRA8888 and L8 in the size list (each entry shows its format); `preview_sized` in `adv-core`.
- Splat maps (file name contains `SplatMap`) default to square RGB565.
- Documentation: `docs/SPEC-audit-and-modding.md`, roadmap status column and spec blocks in `ROADMAP.md`, section 13 in `ROADMAP-DETAILED.md`, notes on the Cloud Pirates and 10.0 databases in `formats/bin-db.md`, this file.

### Changed
- `formats/bin-db.md`: notes on client 17.0 `pack.bin` (4081 named resources, ids elsewhere).
- `formats/bin-db.md`: notes on client 18.0 `pack.rus.bin`/`pack.eng_eu.bin` (numeric ids, no names).
- `ROADMAP.md`: maintainer feedback backlog itemised (done/open).
- `Preview::Image` carries `labels` (pixel format per size candidate); the desktop DTO and the size selector show them.
- `.hi.bin` mip records (levels 1 and 0) documented as verified on 15 HiRes packs of client 18.0.

### Fixed
- Default size candidate no longer shifts when duplicate candidates are removed.
- `db::probe_layout`: the session classifies a resource database from its first 48 inflated bytes before inflating it; the client 18.0 `pack.bin` (720 MB inflated, layout not supported) is skipped instead of being inflated and failing.

### Not verified
- Windows build of `adv-desktop` (`commands.rs` changed without a local build) and the visual check of the new size list.
- 64-bit database path: the real 18.0 `pack.bin` does NOT parse (different 64-bit revision, see `formats/bin-db.md`); the session skips it (layout probe) and keeps guessing texture sizes. Probe checked on real files: 18.0 `Unsupported`, 10.0 and Cloud Pirates `Bits32`; the V2 branch (client 17.0) is covered by a synthetic test only.

## 0.1.44

### Added
- Texture size candidates: Ctrl+click on the size line cycles through them; an `Auto` button restores the guess.

## 0.1.43

### Added
- `adv-core::gamever`: parser of `game.version` (`ver5` manifest: directories, files, sizes, hash pairs).
- `adv-core::packsinfo`: parser of `Version/DataPacksInfo.bin` (packs and files with index, size, attribute, stamp). Neither is shown in the app or the CLI yet.
- Documentation: `formats/game-version.md`, `formats/data-packs-info.md`, `formats/ndb-pdb-layouts.md`, `docs/CLIENTS.md` (client matrix), Cloud Pirates master database section in `formats/bin-db.md`.

## 0.1.42

### Added
- `adv-core::picture`: ICO, DIB and DDS (DXT1/3/5, 24/32-bit) decoders; PNG, JPEG, GIF, BMP size reading.
- Picture preview for `.ico`, `.dds`, `.gif`, `.png`, `.jpg`, `.bmp`; text preview sniffing for unknown files and extra text extensions (`log md yml yaml toml bat ps1 sh xaml rev version ver`); backup suffixes (`.bak .old .orig .tmp`) are ignored when classifying.
- Zoom for pictures: Ctrl+wheel and a slider with a "fit" option.
- Scripts `scripts/check-all`, `run-app`, `samples-test`, `collect-client-info` (`.ps1` with UTF-8 BOM and CRLF, `.sh`); `docs/BUILD-WINDOWS.md` updated with a quick path.

### Fixed
- Four clippy 1.99 errors (`chunks_exact`) in `geometry.rs` and `preview.rs`.

## 0.1.41
- Tests on real packs (read from `ADV_SAMPLES`, skipped without it).
- Notes on community tools (`TOOLS-STUDY.md`) and a draft of the map chunk format (`formats/map-chunk.md`).
- Windows: the build compiled and the window started (maintainer's log); clippy 1.99 findings fixed in 0.1.42.

## 0.1.40
- Rebuild for the Windows run of the `.loc` table, text-search jump, status bar revision and language switch; Windows build and run instructions in `docs/BUILD-WINDOWS.md`.

## 0.1.39
- `.loc` files open as a table of entries (number, key, text) with a filter.
- A click on a text-search result opens the `.loc` at that entry.
- Session: cache of the parsed `.loc` and paged results.

## 0.1.38
- Search inside texts: option "search inside texts" in the window; works on UTF-16 text files (`TextRus.pak`, 82 062 files) and on `.loc` layouts 1 and 2.
- Web tests: 38.

## 0.1.37
- `adv geom glb --anim`: `.glb` export with animation.
- Fixed: the first 0.1.37 archive had `tracing-core` altered in `Cargo.lock` by a `sed` replacement; the archive was rebuilt. Version changes go through `scripts/version.mjs` since then.

## 0.1.36
- `adv_core::animation::decode`: positions, scales and rotation quaternions per bone and frame; all 32 clips of three samples decode.
- Build revision from `.rev` in the status bar.
- Notes on collision data.

## 0.1.35
- Documentation: animation channel rules (mask, position and scale pairs, angles) in `formats/skeletal-animation.md`; PDB type dumps.

## 0.1.34
- Documentation brought in line with the code (no code change).

## 0.1.33
- `.ogv` files show frame size, frame rate, frame count and length in the window.

## 0.1.32
- Documentation: animation structure (no code change).

## 0.1.31
- Bone rest pose decoded; glTF export contains the skeleton and vertex weights.

## 0.1.30
- Skeleton and vertex weights are read from the geometry file.

## 0.1.29
- Window fixes; per-item details not recorded.

## 0.1.28
- Three window audit items, including a large "Open folder" button in the empty state and labels for the filters.

## 0.1.27
- Documentation: parent index per bone found in the skeleton (no code change).

## 0.1.26
- Game data version from `Profiles/game.version` shown in the status bar ("Data: 17.0.4.2.78"); the program version is labelled separately.

## 0.1.25
- Status bar with counters and a filter by file type.

## 0.1.24
- The file list can be resized and collapsed.

## 0.1.23
- Documentation: facts from community converters added to `REFERENCES.md`.

## 0.1.22
- Animation clip header decoded in part (30 frames per second, frame count, bone count); the window shows frames, rate, duration and bones; filter "Animations".

## 0.1.21
- `adv geom glb`: binary glTF 2.0.

## 0.1.20
- File type taken from the name (`Name.(Geometry).bin`, `.(Texture).bin`, `.(SkeletalAnimation).bin`), so filters need no file reads.
- Geometry tail identified as a skeleton (draft in `formats/geometry-bin.md`).

## 0.1.18 to 0.1.19
- Filters by type and sorting; folder picker and "Save as..." through `tauri-plugin-dialog`.
- Strict CSP and window capabilities limited to open and save dialogs (0.1.15 to 0.1.19); virtual paths checked in `adv-session`.

## 0.1.17
- Search by file name across the data folder and inside all `.pak` files.

## 0.1.16
- 3D model viewer (solid and wireframe, Z-up toggle, view reset).
- Interface translation moved to `i18next` with Russian plural rules; Russian and English keys checked by a test.

## 0.1.15
- `adv-session` crate (one tree for folders and `.pak`, reading, preview, audio decoding).
- Tauri 2.12 window: file tree, text preview, texture preview with size choice, audio, Russian/English switch, folder drag and drop.
- `cargo deny` exception for `proc-macro-error` (comes from gtk-rs on Linux).

## 0.1.14
- `adv tree` and the `browse` module (directory tree from `.pak` files, file type detection).

## 0.1.13
- `adv audio export` with a built-in Vorbis decoder (FSB5).

## 0.1.12
- `adv db find`: find resource databases inside packs.

## 0.1.11
- `adv tex convert` guesses size and format without `.xdb` (`--index`, `--tdb`).
- `adv geom info` and `adv geom obj`.

## 0.1.10
- `adv tex convert --db <pack.bin> [--db-id]`: size and format from the database.

## 0.1.9
- `adv db textures`: type, size and mip count of `Texture` objects.

## 0.1.8
- `adv db list` shows the object type; the closed pipeline cannot panic.

## 0.1.7
- Resource database: `adv db info`, `list`, `packs`.

## 0.1.6
- Textures: size and format from the neighbouring `.xdb`; `--crop`.

## 0.1.5
- Audio: `adv audio list` and `adv audio export` (WAV).

## 0.1.4
- Fixes after the first Windows build: clippy 1.99 lint in `loc.rs`; `check-repo` works without `.git`.

## 0.1.3
- Textures: `adv tex info` and `adv tex convert` (DXT1/3/5 to PNG and DDS).

## 0.1.2
- `adv verify`; `--json` for `info` and `list`.

## 0.1.1
- Reading `.pak` and `.loc`; commands `info`, `list`, `extract`, `loc dump`, `search`; CodeQL, gitleaks, Actions v7.
