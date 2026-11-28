# Reference projects

Projects we studied. What we may take from each depends on its license.

| Project | License | Language | What it gives us | How we use it |
|---|---|---|---|---|
| AllodsOnlineEditorTools (Paulus) | MIT | C# | Documented `.bin` database format (V1, V2 x86/x64), ImHex patterns, per-version struct layouts, texture `.bin` to DDS/PNG, JSON/XML export | Format knowledge; code may be ported with attribution |
| AOParser (koronabora) | MIT | C++ | Parser for `.xdb` data into spreadsheets | Format knowledge; code may be ported with attribution |
| SarnautCore tools | Apache-2.0 | Rust | Content addressed asset store (BLAKE3), XDB to YAML extractors, ideas for a manifest | Code may be reused with attribution and a NOTICE entry |
| SarnautCore client | AGPL-3.0 | C# (Godot) | Converted-asset viewer, zone and lighting notes | **No code is copied.** Only facts about formats and behavior |
| AODataViewer (mrkriv) | none written; the author told the maintainer in person (2026-11): "do what you want, no claims" | C# | Idea of the viewer, the `.loc` reader behavior | Still no code is copied (no written license; MIT-compatible reuse needs one). Formats re-derived from samples. Ask the author to add a license file to make reuse clean |
| AOUIEditor (Ootsutsuki22) | MIT | C# | UI widget prototypes, `.xdb` widget data | Format knowledge; code may be ported with attribution |

Notes:

- Windows check of 0.1.10 (maintainer, 2026-11-11): fmt, clippy, 12+52 tests, web typecheck/test/build, `info`/`list`/`extract`/`loc dump`/`search`/`audio list`/`audio export` (11 WAV via vgmstream) all passed.

- AOTextureViewer (Yuri Grachev / Altair), closed `.exe`, no license text. The project owner has the author's permission to study it (the author abandoned the project); behaviour is documented in [formats/texture-heuristics.md](formats/texture-heuristics.md) and re-implemented, no code or data file is copied into the repository. Ask the author to confirm in writing or add a license file.
- SarnautCore client is AGPL-3.0 in its own README; the owner says the author allows study. Because MIT and AGPL do not mix, still no code is copied, only format facts.

- The two copies of AllodsOnlineEditorTools that were supplied are identical.
- None of these projects reads raw `.(Geometry).bin` or `.(SkeletalAnimation).bin`. The SarnautCore client
  only loads output of an external converter (`ao-godot-converter`, Apache-2.0, not supplied), so it gives
  no dequantization or channel-to-bone facts. Raw formats come from our own analysis, see [formats](formats/README.md).
- Facts taken from the SarnautCore client (concepts only): a model is a list of named surfaces (parts), each
  with its own texture and visibility flags; a skin is a list of slots, each slot = one bone + one inverse
  bind matrix; bones have names, parent indices and rest transforms; head, hair and armor parts are all
  present in one mesh and a class appearance picks surfaces by name. Expect the same in the raw data:
  per-part texture binding lives in the resource DB, not in the geometry file.
- When code is ported, add the original copyright notice to a `NOTICE` file in the same
  commit.

## Added 2026-11-17

| Project | License | How we may use it |
|---|---|---|
| paulus-allods/AllodsOnlineEditorTools (C#, databases per client version, textures, geometry) | MIT (Paulus) | May be ported with a NOTICE entry; its `doc/bin-format.md` and hexpat patterns are facts |
| ao-tools by gsomgsom, kosh, SLA (PHP: `.loc` extractor and compiler, `game.version` viewer and truncator) | none stated | Facts only (formats described in its `formats/*.txt`); no code copying |
| paulus-allods/blender-import (Blender importer of geometry) | none stated | Facts only |
| paulus-allods/AllodsOnlineDatabaseUnpacker ("abandoned unpacker") | none stated | Facts only |

## Added 2026-11-20 (links from the maintainer)

| Project | License | How we may use it |
|---|---|---|
| paulus-allods/AllodsOnlineEditorTools (checked again) | MIT | As above. Its README lists support for 1.1.02.0, 3.0.0.x, 4.0.02.4x, 7.0.00.7x and 14.0.01.71 and parsing only for Cloud Pirates 1.7.7 and 17.0.01.55 |
| paulus-allods/AllodsOnlineDatabaseUnpacker (archived 2023-11-21, C#, client 1.1.02.0) | none shown | Facts only. Its README says the inputs are `Bin/pack.bin`, `Bin/Maps_*.bin`, `System/index.bin` and `Types/*.xml` |
| DarkyAndSparky/ao-srv-1.1.02.0 (server build 1.1.02.0, release archives, `STRUCTURE.md`) | none shown | Reference for file layout only; nothing is copied |
| DarkyAndSparky/ao-clt-1.1.00.41 (client 1.1.00.41 English, `STRUCTURE.md`) | none shown | Same |
| DarkyAndSparky/ao-helper-rv168362 (tools and Loc kit for revision 168362) | none shown | Same |
| AllodsDevelopersCommunity/AllodsLegacy-server-data (XDB data of the 4.0.4.42 USGala server, 2018) | none shown | Facts only; useful to learn real `.xdb` resource names and folder layout, nothing is copied into this repository |

## Added 2026-11-26 (tools from the maintainer's audit)

| Project | License | How we may use it |
|---|---|---|
| enoly/allods-resource-exporter (Python, client 1.1.02.0; one commit of 2026-11-23) | MIT (`LICENSE`, "Allods Resource Exporter contributors") | Code may be ported with an entry in `NOTICE`; so far only facts (record offsets, animation atom layout, collision and particle layouts) are used, nothing is copied. Its own docs are `docs/*.md` in the repository |
| AllodsOnlineEditorTools v2 archive (identical to the checked version) | MIT | As above |
| AllodsOnlineDatabaseUnpacker dev / master archives (identical content) | none shown | Facts only; it loads the game's own DLLs, which we do not do |
| blender-import (Blender 2.90 add-on) | none shown | Facts only |
| AODataViewer (NeoAxis based viewer) | none stated; maintainer-confirmed permission recorded earlier | Facts and interface ideas only; its 109 binary DLLs are not ours |
| AOUIEditor | MIT | Facts about UI add-ons, may be ported with `NOTICE` |
| AllodsV4_Developer (injected DLL: time faking, file hooks, client code patch) | none shown | **Not used**: it patches the running client, which is out of scope (`docs/ROADMAP.md`) |
