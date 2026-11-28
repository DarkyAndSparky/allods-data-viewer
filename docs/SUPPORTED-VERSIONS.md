# Supported client versions

The viewer is built for every client generation (old and new). This table says, per generation
and feature, what has really been tried. It is updated when a build is tested.

Status words: `verified` (checked on real data of that client), `draft` (implemented, checked on
little data), `partial`, `unknown` (not tried), `no` (cannot be done yet, reason in the note).
Nothing here is guessed: an empty or `unknown` cell means nobody has tested it.

## Generations seen so far

| Generation | Where it comes from | Pack database layout |
|---|---|---|
| 7.0 | Cloud Pirates 1.5 / 1.7, `Bin/pack.bin` | 32-bit (`Bits32`), file names present |
| 1.1.00.41 | client 1.1, `Bin.pak` holds `Bin/pack.bin` and 78 `Bin/Maps_*.bin` | 32-bit, same container as 7.0; signature `2419FD3C1B6C738A`; texture records, sound records and model elements (texture per element) read |
| 1.1.02.0 | older clients (the Python exporter targets it) | not read by this project yet |
| 11.0.12 | Steam depot, `BaseLoceng_eu.pak` | unknown |
| 15.0 | x64 database | 64-bit (`V2`), see `docs/formats/bin-db.md` |
| 18.0 (builds 1139, 1147, 1148) | current client | 64-bit (`V2`); from 1147 the struct table moved to slot 1 and there is no name table |
| Warp | separate game on the engine | no `.xdb`, not tested |

## Features by generation

| Feature | 7.0 | 11.0.12 | 15.0 | 18.0 | Note |
|---|---|---|---|---|---|
| Open `.pak`, `.zip`, tar, 7z, xz | verified | unknown | unknown | verified | the container is the same, only 7.0 and 18.0 data were opened |
| Texture decode (DXT1/3/5, RGBA, ...) | verified | unknown | unknown | verified | `hi` siblings supported |
| Geometry `.bin` to glTF / OBJ | verified | unknown | unknown | verified | 18.0 uses 40-byte bones and uniform bone scale |
| Skeleton and animation clips | verified | unknown | unknown | verified | player in the model view; non-uniform bone scale not handled |
| Resource database read (`pack.bin`) | verified | unknown | partial | partial | 18.0: framing and struct names only, no resource names |
| Model to texture binding | draft | unknown | unknown | no | 7.0 from the Fixes table; 18.0 has no id to name table in `pack.bin` |
| Material textures (mask, reflection, contour) | draft | unknown | unknown | no | decoded for 7.0, not shown in the viewer |
| Sound `.bsb` (FSB4) / `.bev` | unknown | unknown | unknown | verified | some FSB4 codecs need samples |
| Texts `.loc` / `Texts.pak` | unknown | unknown | unknown | verified | |
| Map chunks | unknown | unknown | unknown | verified | see `docs/formats/map-chunk.md` |
| Video | unknown | unknown | unknown | verified | Windows check pending |

## Client 1.1.00.41 (checked 2026-11-26 on the client release `ao-clt-1.1.00.41`)

| Feature | Status | Note |
|---|---|---|
| Version signature | verified | `2419FD3C1B6C738A` in chunk 0 of all 79 databases of `Bin.pak`; `Profiles/game.version` says `1.1.00.41`. Not in the EditorTools table, which lists `5847DB469364493C` for 1.1.02.0 |
| Textured models (checkbox in the viewer, `glb_textured`) | verified on the exported file, not on screen | since 0.1.87 textures are found in all archives, any letter case: 4 875 of 5 032 models have at least one texture, 154 none, 3 not in the database. 0.1.83 to 0.1.86 left every 1.1 model white (textures were searched only in the model's own archive) |
| Model elements and texture per element | verified (record reading) | 5 189 Geometry records, 5 186 with elements, 39 585 elements, 38 675 with a diffuse texture; an independent Python reader (the MIT exporter, only its signature check relaxed) gives the same counts and the same ranges and textures for `ElfFemale`. Not checked: that the pictures look right in the window |
| Geometry `.bin` of 301 resources (characters, creatures, Kania) | verified (reading) | read by the exporter code with all its bounds checks passing; not rendered |
| `Texture` records | verified | 1 834 of 2 280 `.bin` textures match their record (mip count, level sizes); the other 446 have no record in `pack.bin` (their records are probably in `Maps_*.bin`, not read) |
| Skeletal animation clips | verified (reading and binding) | all 5 776 clips of `Characters.pak` and `Creatures.pak` decode; poses agree with the Python exporter on 68 424 samples; bone names and material tracks (transparency, texture shift) read. Tracks are bound to bones by name (26 % of the clips cannot be bound by number); 5 716 of 5 761 clips give an animated GLB. Not checked: the look in a viewer, playback of material tracks |
| 32-bit index buffers | partial | 32 of 5 189 Geometry records have the flag at +60 set (30 of them have more than 65 535 vertices). Whether our geometry reader handles them is not checked |
| `material.params` of elements | no | the record layout of 1.1 was not checked, the field is left empty |

Client 1.1.02.0 (signature `5847DB469364493C`) is recognised by signature only; no file of it was read here.

## How to test a new client

1. Open the `Packs` folder of the client in the viewer and note the build (shown in the title).
2. For each feature above try one file and write the result into the table.
3. A failing feature gets a line in `docs/GAPS.md` with the file name and the error text.

Where the resource database lives differs between clients (`BaseLoc*.pak`, `Bin.pak`,
`System*.pak` or a loose `Bin/pack.bin`); the viewer looks in all of them.
