# `.pak` archives

**Status: draft.** Observations that still have to be verified against real files.

## Container

- A `.pak` file is a standard ZIP archive.
- The client keeps archives in `data/Packs/` and, in some versions, `data_warp/Packs/`.
  Entries of both folders form one virtual file system. Paths inside use `/`.

## Entry data

- The data of an entry, after the ZIP layer is decoded, may be wrapped in one more
  layer: a **zlib** stream.
- Such an entry starts with the two bytes `0x78 0x01`, `0x78 0x5E`, `0x78 0x9C` or
  `0x78 0xDA`. The rest of the entry is a raw deflate stream.
- An entry that does not start with one of these pairs is stored as is.
- An entry that starts like zlib but fails to inflate should be treated as plain data.

## Open questions

- Are there encrypted or differently compressed entries?
- Do duplicate paths exist across archives, and which one wins?
