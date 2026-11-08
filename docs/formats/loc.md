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

**Status: draft.** Seen in `pack.rus.loc` and `pack.eng_eu.loc` (about 45 MB each when
inflated), the two files are parallel (same structure, Russian and English text).

The payload starts with 64 bit little endian words:

```
0x162E0F62, 566294, then pairs (n, offset): (9,16) (15,40) (6,72) (6,72) ...
```

The first word is a magic number and the second is probably the item count
(identical in both files). The pairs that follow look like (length, position)
descriptors and are not decoded yet. Next step: decode them against the file body.

## Open questions

- Layout 2: what the descriptors mean and where keys and texts are stored.
- Which client versions use which layout.
- The meaning of the duplicate key.
