# `.loc` localization files

A `.loc` file stores named UTF-16 texts. In every sample the file on disk is a
**zlib stream** (see [pak.md](pak.md)); everything below describes the inflated payload.
Integers are little endian.

Two layouts exist. They are told apart by the first four bytes of the payload:
`00 00 00 00` is layout 1, `62 0F 2E 16` is layout 2.

## Layout 1 (32 bit)

**Status: verified** on three files (21 693, 150 587 and 301 590 items; every item
decoded, the last text ends exactly at the end of the file).

Header, 16 bytes:

| Offset | Type | Meaning |
|---|---|---|
| 0 | u32 | always 0 |
| 4 | u32 | `sizes_pos`, position of the size table |
| 8 | u32 | always 8 |
| 12 | u32 | `count`, number of items |

All positions below are measured from the end of the header (add 16 to get the file offset).

1. **Item table** right after the header: `count` records of 12 bytes,
   `pos` (u32), `str_size` (u32), `id` (u32). `id` equals the record index.
2. **Key** (the name): `str_size` bytes of UTF-8 at the address of the record itself
   plus `pos` (the offset is relative to the start of the 12 byte record).
   The last byte is a terminating zero, so the key is `str_size - 1` bytes.
3. **Size table** at `sizes_pos`: `count` records of 8 bytes, `length` (u32) and `pos` (u32).
4. **Text** of item `id`: `length` UTF-16 code units (so `2 * length` bytes) at
   `end_of_size_table + 8 + pos`, followed by a two byte zero terminator. Texts are
   padded to a multiple of 4 bytes, and `pos` of the next text accounts for the padding.

Text may contain markup such as `<span><r name="name"/> ...</span>` and `\r\n`
line breaks. Keys look like file paths, for example
`Characters/.../Something.(MobWorld).Name.txt`.

Observations: keys are unique except for one pair in the largest sample; the
three files hold different client versions and different content.

## Layout 2 (64 bit)

**Status: verified (structure), keys unknown.** Checked on 2026-11-10 against four real
files from two client versions: `pack.rus.loc` and `pack.eng_eu.loc` from a current
`Texts_x64.pak` and the same two names from an older "new format" pack. For every item of
all four files the text ends with a UTF-16 NUL exactly where the descriptor says, and the
last text ends exactly at the end of the file.

The payload (after the zlib layer) is a memory image. All numbers are 64 bit little endian.

| Where | Meaning |
|---|---|
| 0 | `magic`: `0x2689119F` (current client) or `0x162E0F62` (older client). Differs per client version; layout is the same in both. |
| 8 | `words`: number of 64 bit words in the descriptor table. `items = words / 2` (555 862 words, 277 431 items now; 566 294 and 283 147 in the older version). |
| 16 | `items` descriptors of 16 bytes: `length` (u64, UTF-16 units without the terminator) and `offset` (u64). |
| 16 + 16*items | 12 bytes of unknown purpose (u32 = 1, then a u64 that looks like a pointer into the file). |
| 16 + 16*items + 12 = `base` | Start of the string area. |

Text of item `i`: `length` UTF-16LE units at file position `base + offset`, followed by
a 2 byte NUL. Strings are 8 byte aligned and **deduplicated**: equal texts share one
offset (194 816 distinct texts for 277 431 items in the current Russian file).

**There are no keys in this layout.** Items are addressed by index; the key (resource
name) must come from the resource database (`pack.bin`), which we do not have yet.
Layout 1 stores keys next to the texts; layout 2 does not.

Reading (checked arithmetic everywhere): require `words` even, `16 + 8*words + 12 <= len`,
and for each item `base + offset + 2*length + 2 <= len`.

## Open questions

- Layout 2: the 12 bytes after the descriptor table; how item indexes map to resource names (needs `pack.bin`).
- Which client versions use which layout.
- The meaning of the duplicate key.
