# Changelog

What changed between delivered builds. Facts only. One section per version, newest first.
Rule: every new build adds its section here before the archive is made (see [VERSIONING.md](VERSIONING.md)).

Entries for 0.1.14 to 0.1.44 were rebuilt from the delivery messages of the project chat (2026-11-19); where a message gave no detail the entry says so.

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
