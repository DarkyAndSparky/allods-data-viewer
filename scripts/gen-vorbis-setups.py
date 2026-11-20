#!/usr/bin/env python3
"""Builds crates/core/data/vorbis_setups.bin.z from vgmstream's vorbis_codebooks_fsb.h.

Usage: gen-vorbis-setups.py <path to vorbis_codebooks_fsb.h> <output>
The header comes from https://github.com/vgmstream/vgmstream (ISC license, see NOTICE).
Record format (before zlib): u32 crc32, u32 length, setup packet bytes; little endian.
"""
import re
import struct
import sys
import zlib

text = open(sys.argv[1], encoding="utf-8").read()
sizes = {m.group(1): int(m.group(2), 16)
         for m in re.finditer(r"\{0x([0-9a-f]{8}),0x([0-9a-f]+),vcb_", text)}
blob = b""
for m in sorted(re.finditer(r"static const uint8_t vcb_([0-9a-f]{8})\[\] = \{(.*?)\};", text, re.S),
                key=lambda m: m.group(1)):
    data = bytes(int(x, 16) for x in re.findall(r"0x([0-9a-f]{2})", m.group(2)))
    assert len(data) == sizes[m.group(1)], m.group(1)
    blob += struct.pack("<II", int(m.group(1), 16), len(data)) + data
open(sys.argv[2], "wb").write(zlib.compress(blob, 9))
print(len(sizes), "setups,", len(blob), "bytes before zlib")
