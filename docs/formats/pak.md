# `.pak` archives

**Status: verified** for the container (WinRAR and `unzip` open every sample).

## Container

- A `.pak` file is a standard ZIP archive.
- The client keeps archives in `data/Packs/` (and in some versions `data_warp/Packs/`).
  Entries of all of them form one virtual file system. Paths inside use `/`.
- Entries can be **stored** (method 0) or **deflated** (method 8). Both occur: the
  video archive stores `.ogv` files uncompressed, some text archives are deflated.
  Every entry carries a CRC32.

## Entry data

- After the ZIP layer is decoded, the data may be wrapped in one more layer: a
  **zlib** stream. The localization archives are an example: the single entry of a
  text pak (`Bin/pack.loc`) is a zlib stream, even when the ZIP layer is deflated too
  (so it is compressed twice).
- Such an entry starts with `0x78` followed by `0x01`, `0x5E`, `0x9C` or `0xDA`.
- An entry that does not start with one of these pairs is plain data.
- An entry that starts like zlib but fails to inflate should be treated as plain data.

## Samples seen

| Archive | Entries |
|---|---|
| text pak, old client | `Bin/pack.loc` |
| text pak, new client | `Bin/pack.rus.loc`, `Bin/pack.eng_eu.loc` |
| video pak | `.ogv` files in folders per event |

## Open questions

- Encrypted or otherwise compressed entries.
- Duplicate paths across archives and which one wins.
