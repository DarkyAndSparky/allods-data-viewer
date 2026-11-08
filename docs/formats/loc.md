# `.loc` localization files

**Status: draft.** Offsets below are little endian and unverified.

A `.loc` file stores named text resources. Names are UTF-8, text bodies appear to be
UTF-16 (two bytes per character).

## Layout (as observed)

| Offset | Size | Meaning |
|---|---|---|
| 0 | 16 | header |
| 4 | 4 | `sizes_pos`: position of the size table, relative to the end of the header |
| 12 | 4 | `item_count`: number of items |

After the 16 byte header (all positions below are relative to its end):

1. **Item table**: `item_count` records of 12 bytes: `pos` (u32), `str_size` (u32),
   `id` (u32).
2. **Name**: for an item, `str_size - 1` bytes of UTF-8 at `pos + 12 * id`
   (`str_size` probably includes a terminating zero).
3. **Size table** at `sizes_pos`: 8 bytes per `id`: `file_size` (u32), `file_pos` (u32).
4. **Body**: starts at `8 + sizes_pos + file_pos + item_count * 8` and is
   `file_size * 2` bytes long.

## Open questions

- Are `file_size` and `file_pos` counted in characters or in bytes?
- What do the unused header bytes contain?
- Several `.loc` files with identical content exist in different archives, the
  reader should deduplicate them.
