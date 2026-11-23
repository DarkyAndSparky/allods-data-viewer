# Pack pointer-fix block (7.0, Bits32 layout)

**Status: layout verified (2026-11-23) on the 7.0 game pack `Bin/pack.bin`, read by `crates/core/src/fixes.rs`. Tag meanings partly unknown; the object image that the fixes apply to is NOT decoded yet.**

## Container
- Inflated pack, little-endian 32-bit words. The fix block is chunk 4 (`u32 id = 4`), with the entry count in the second word: `u32 count`, then `count` entries of `u32 d`, `u32 v`. The count is the number of ENTRIES, not bytes. The 7.0 game pack has `count = 6 527 459`; chunk 5 starts exactly at `count * 8` bytes after the entries (checked).
- Entry word `d`: `offset = (d >> 3) * 4` (byte offset relative to the data block) and `tag = d & 7`.
- The data block starts at inflated offset 66 029 848 (the block-3 header), the base that the fixup keys match.

## Tags (7.0 pack, measured)
| tag | count (game pack) | meaning |
| --- | ---: | --- |
| 0 | 2 982 074 | not decoded |
| 1 | 1 962 584 | not decoded |
| 5 | 542 566 | string pointer: `v` is a blob offset to a NUL-terminated text (about 17 % of tag-5 targets are printable on the sample; the rest are not yet classified) |
| 6 | 423 250 | not decoded (equals the xdb entry count; coincidence not checked) |
| 7 | 616 985 | not decoded |

On `SkeletalAnimation` objects, tag-5 fixups sit at object `+0x88` and `+0x4C`; the stored field is 0 there (61 499 of 62 212 raw-zero).

## Not established
- The write side (emit order, encoding) is not decoded; the order is not field-address order globally.
- The object image the offsets apply to (Geometry, Material) is not decoded here; see `geometry-bin.md`.

## Tests
`crates/core/src/fixes.rs`: success (offset, tag and value decode), truncated entries, corrupted chunk id and an over-limit count.
