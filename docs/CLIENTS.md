# Client versions studied

Collected from user-made file lists and sample packs (no game files are stored in this repository). Dates are the install dates from the launcher log where known.

| Client | Pak layout | Notes |
|---|---|---|
| Current client (builds 572..626 in the install log, 2026) | `data/Packs`, `.pak` with Mini/HiRes pairs (`World_*.Mini.pak`, `*.HiRes.pak`), `Texts.pak`, `BaseLoc*.pak` | `Profiles/game.version` about 38 KB (18.0 depot); newer `.bsb` voices are FSB5 or zlib wrapped; map chunks per `docs/formats/map-chunk.md` |
| 11.0.12 (Steam depot) | same family, `BaseLoceng_eu.pak`, `game.version` about 46 KB, 3 plain `.xdb` in `SampleLocalization.pak` | `.bsb` are zlib + FSB4 in older builds |
| Warp (separate game on the engine) | `Characters.Mini.pak`, `Creatures.Mini.pak`, `Video.pak`, `BaseLocWarprus*.pak`, `Profiles/Warp/game.version` | no `.xdb`, 113k `.bin` entries |
| Cloud Pirates 1.5 and 1.7 (game version 7.0) | `Solo_*.Mini.pak`, `BaseLoc<lang>.pak`, `Version.pak`, `LuaCompiled*.pak` | `game.version` about 6.5 KB (1.5) and 12.7 KB (1.7); six `.loc` languages; ships `.ttf` per language |
| Helper (dev kit with LocKit) | `TextRus.pak`, `Tests.pak` (110 MB), `Server/*.Server.pak` | `Tests.pak` holds plain XML `.xdb`, `.template`, `.cfg`, Lua and Python; the only place with the XML resource tree in a pak |
| Map editor package | `GameText.pak`, `GameTextV2.pak`, `GameTextV7.pak`, `Interface.Other.pak`, 144 `.pdb` | loose `data` tree (about 238k files) with `.xdb` only for `StaticObject`, `ZoneResource`, `ZoneLights`, `Sound2DTassel`, `Character`; `.bin` records without a visible database |

Open question for all clients: where the binary resource database lives (the file that holds `Geometry`, `Texture`, `Material` records). Candidates: `Version.pak`, `Client.Mini.pak`, or a `Datapackinfo.bin`-style index.
