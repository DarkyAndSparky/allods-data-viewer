# Audio and video

**Status: audio container verified, codec details partly verified; video verified.**
Checked on 2026-11-10 with real `.bsb`, `.fsb` and `.ogv` files.

## Audio banks

- A `.bsb` is a **zlib stream**. After inflating, the first 4 bytes are a u32 with the
  length of what follows (verified: 0x640 = 1600 for a 1600 byte file); the rest is a
  normal `.fsb`. Verified on two files.
- The `.fsb` files are **FSB5** (magic `FSB5`, version field 1). Header (60 bytes):
  magic, version, number of samples, size of the sample header table, size of the name
  table, size of the data, codec mode, 8 zero bytes, 16 byte hash, 8 bytes. Observed
  **codec mode 15 = Vorbis** in every checked file (a 7.4 MB music bank and two sound
  banks); total size = 60 + headers + names + data matched the file size exactly.
- Each sample has a 64 bit header: bit 0 "extra chunks follow", bits 1-4 frequency index
  (index 9 seen = 48 000 Hz), bit 5 stereo, bits 6-33 data offset in units of 32 bytes,
  bits 34+ sample count. Extra chunks (u32: bit 0 more, bits 1-24 size, bits 25+ type)
  seen: type 1 (1 byte, channel/flag), type 8 (1 byte, unknown), type 11 (Vorbis data:
  a u32 CRC32 followed by seek data).
- **Name table (verified 2026-11-11, two more banks):** it starts with one u32 offset per stream (relative to the start of the table), followed by the NUL-terminated names. `Voice_Kvator_Phrases_3_rus.fsb` (plain `.fsb`, 11 streams, 4.7 MB) and `Voice_Kvator_Phrases_2_2_rus.bsb` (zlib, 10 streams, 567 KB; inflated size minus 4 equals the length prefix) both parse, and our reader gives the same names, rates, channel counts and durations as `vgmstream-cli`. Frequency index 8 = 44 100 Hz is confirmed by the music streams of the first bank (stereo, 44 100 Hz); the other indices follow the FMOD table and are unchecked.
- **Decoding verified (2026-11-10):** `vgmstream-cli` (source of 2026-11-05, ISC licence, built on Linux with libvorbis) decodes our files: `Music_Zone_Jigran.fsb` (2 streams, 4 channels, 48 kHz, 1:58.957, name `Jigran_Town_Adaptive`) and a bank cut from `Ambience_AD05.bsb` (2 streams `chainshake01/02`, stereo). Chunk type 1 is the channel count (4 in the music bank). Output WAVs had plausible non-silent samples; not listened to.
- **Built-in decoding (2026-11-14):** see [vorbis-fsb.md](vorbis-fsb.md). `adv audio export` no longer needs vgmstream; the paragraph below is kept as the history of how the format was understood.
- **Why decoding is not trivial:** FMOD stores Vorbis without the Vorbis setup header; the CRC in chunk
  11 selects a setup (codebooks) from a table known to FMOD. Decoding or converting needs
  that table. The table lives inside vgmstream; we do not copy it, we call vgmstream as an
  external program (see DECISIONS D-014).
- The batch kit (`BSBtoFSB`) only inflates and strips 4 bytes; decoding there is done by
  other external tools.

## Video

- `.ogv` files are Ogg containers stored uncompressed (zip method 0) in `Video.pak`.
- Verified on two files: each has **one Theora stream and no audio stream**. Header:
  Theora 3.2.1, frame 1280 x 720, 30000/1001 (29.97) frames per second.

## Video header reader (2026-11-16, `adv_core::ogv`)

The window and `Preview::Video` show size, frame rate, frame count and length of an `.ogv`. Size and rate
come from the Theora identification header (fields of the public Theora specification: picture width and
height at bytes 14 and 17 of the first packet, frame rate numerator and denominator at 22 and 26, the
key-frame shift in the five bits after the quality bits at byte 40). The frame count comes from the position
of the last page of the stream: `(position >> shift) + (position & mask)`, plus one for versions before
3.2.1. Status: header fields `verified` on the two real files before; the **length is not checked on a real
file yet** (synthetic tests only). No frame is decoded: Chromium-based WebView2 cannot play Theora, so the
window offers saving the file.
