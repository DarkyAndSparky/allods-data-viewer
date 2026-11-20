# `Profiles/game.version`

Status: **verified** on four real files (18.0.00.05, 11.0.00.12.1, two Warp 11.0.00.37 files); the parser reads each to its last byte.

| Offset | Size | Meaning |
|---|---|---|
| 0 | 4 | number, unknown (about the size of the rest) |
| 4 | 4 | tag `ver5` |
| 8 | 4 | little-endian length `n` of the version text |
| 12 | `n` | UTF-8 version text, for example `18.0.00.05` |
| 12+n | 4 | number, unknown (close to the zlib stream size but not exact) |
| 16+n | rest | zlib stream (starts `78 DA`) with the install manifest |

Manifest (after inflating, all numbers little endian):

1. `u32` directory count.
2. For each directory: `u32` record size (unknown use), text (`u32` length + bytes) path such as `/bin/`, `u32` file count.
3. For each file: text name, `u32` size, 32 bytes (two 16 byte values, probably checksums), 1 byte flag.

The manifest lists the files a launcher installs (`/bin/`, `/Profiles/`, `/data/Mods/...`), not the paks. The older `ver3` layout from other tools is not handled.

Implementation: `adv_core::gamever`. The Cloud Pirates sample folders had no `game.version`.
