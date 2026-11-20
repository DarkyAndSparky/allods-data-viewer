# `Version/DataPacksInfo.bin`

Status: **draft** (read to the end on Cloud Pirates 1.5 and 18.0 files; column meanings not confirmed).

Stored in `Version.pak` (stored raw in Cloud Pirates, zlib in 18.0: 3.7 MB packed, 35 MB text). Plain text, tab separated, one block per pak:

- header line: pak file name, file count, pak size in bytes (the size equals the real pak file size);
- one line per file: path inside the pak, index, size, attribute (32, 128 or 0), last column (one value for almost every file, a second for a minority);
- an empty line ends the block.

`GameCenterInfo.bin` in the same pak is 32 `#` characters, CRLF and a short number (for example `13.2000028`, `789`); meaning unknown.

Implementation: `adv_core::packsinfo`.
