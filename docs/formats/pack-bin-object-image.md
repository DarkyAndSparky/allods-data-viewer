# pack.bin object image (7.0): reading the objects

Scope: how the resource objects inside `data/Bin/pack.bin` are laid out in client 7.0, enough to walk Geometry and Material records (roadmap M6). Reverse engineered from the 7.0 client data and checked against the matching xdb sources.

## The data block is a memory image

`pack.bin` is one zlib stream. Inflated it is a relocatable memory image, not a set of stored files. The block chain is Header, TxtFiles, Metadata, Data, Fixes, PakFileRefs, Packs; the objects live in the **Data** block and the **Fixes** block is the relocation table for the pointers inside it.

Each resource is a compiled C++ struct: little endian u32 words, floats, flags, embedded fixed size members, inline NUL terminated strings and pointers.

## Pointers are self relative

A pointer field holds a u32 offset measured from its own address:

```
target_address = address_of_the_pointer_field + stored_u32
```

To follow a pointer you add the stored value to the position of the field itself, not to the start of the block. A stored 0 is null.

Arrays are a pointer and a count side by side:

```
(rel_ptr, u32 count)
```

Resolve `rel_ptr` the same way to reach the first element; `count` elements follow contiguously.

A `std::vector` style member can instead appear as a triple of absolute addresses (begin, end, capacity) in the `0x01000000` to `0x38000000` band. Those are live addresses from the build machine and move whenever data moves, so read them as "an array of `(end - begin) / stride` elements" and do not trust the absolute values.

## The Fixes block lists every pointer

Near the end of the file the **Fixes** block is a flat table of 8 byte entries, one per pointer word in the Data block (database id, the pointer location, a type tag). It lets a reader find and relocate every pointer exactly, and lets a differ tell a pointer word from a data word without guessing.

## Objects are read at fixed offsets

Within an object the scalar fields sit at fixed byte offsets for that class. Worked example, the SkeletalAnimation record, checked across the whole 7.0 set against the xdb values:

| field | offset | type |
| --- | --- | --- |
| blendTime | +0x7C | f32 |
| endFrame | +0x80 | f32 (frame count) |
| fps | +0x84 | f32 |
| scriptID | +0x8C | i32 |

scriptID at +0x8C matched 49,970 of 49,970 records (a wrong record control sat near 2.6 percent). The point is the model: once the self relative pointers are resolved, each class is a plain struct read at fixed offsets.

## Applying this to Geometry and Material (M6)

Geometry and Material records have the same shape: a fixed size head read at fixed offsets, with self relative pointers to the sub arrays (vertex and index buffers, submesh tables, texture and material references). Walk them by resolving the pointers with the rule above and reading each array as `(rel_ptr, count)`. The remaining per class work is only which offset is which field for Geometry and Material, not how to navigate the image.

## Version note

This is the 7.0 layout. Newer clients (15.0 and up) replace the Header with a 12 byte version buffer, drop TxtFiles, add an id table and use 64 bit fields, so the offsets and the pointer width differ there.
