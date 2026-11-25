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
| 1.1.00.41 | client 1.1, `Bin.pak` holds `Bin/pack.bin` | 32-bit, same container as 7.0; texture records and sound records read, model elements not |
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

## How to test a new client

1. Open the `Packs` folder of the client in the viewer and note the build (shown in the title).
2. For each feature above try one file and write the result into the table.
3. A failing feature gets a line in `docs/GAPS.md` with the file name and the error text.

Where the resource database lives differs between clients (`BaseLoc*.pak`, `Bin.pak`,
`System*.pak` or a loose `Bin/pack.bin`); the viewer looks in all of them.
