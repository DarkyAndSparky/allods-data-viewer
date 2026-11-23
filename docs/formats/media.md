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
- **Name table (verified 2026-11-11, two more banks):** it starts with one u32 offset per stream (relative to the start of the table), followed by the NUL-terminated names.
- **Decoding verified (2026-11-10):** `vgmstream-cli` (source of 2026-11-05, ISC licence, built on Linux with libvorbis) decodes our files.
- **Built-in decoding (2026-11-14):** see [vorbis-fsb.md](vorbis-fsb.md). `adv audio export` uses the built-in decoder by default and optionally can invoke `vgmstream-cli` for compatibility checks.
- **Why decoding is not trivial:** FMOD stores Vorbis without the Vorbis setup header; the CRC in chunk
  11 selects a setup (codebooks) from a table known to FMOD. The built-in decoder uses the setup table
  distributed with this project. The setup-table work is attributed in `NOTICE`.
- The batch kit (`BSBtoFSB`) only inflates and strips 4 bytes; decoding there is done by
  other external tools.

## Video

- `.ogv` files are Ogg containers stored uncompressed (zip method 0) in `Video.pak`.
- Verified on two files (the first ones checked): each has **one Theora stream and no audio stream**.

## Video header reader (2026-11-16, `adv_core::ogv`)

The window and `Preview::Video` show size, frame rate, frame count and length of an `.ogv`. Size and rate
come from the Theora identification header. The frame count comes from the position of the last page.
Status: header fields `verified` on the two real files checked earlier; the **length is not checked on a real
file yet** (synthetic tests only). No frame is decoded: Chromium-based WebView2 cannot play Theora, so the
window offers saving the file.

## In-window video playback (0.1.63, `adv_core::video`)

Status: `verified` on one real file (`Intro.ogv`, 1280 x 720, 4:2:0, 73 frames; every frame decoded, frame 60 checked by eye), `draft` for 4:2:2 / 4:4:4 and for pictures with a crop offset (no samples).

- Container: Ogg pages are split into packets by the lacing values; only the first stream whose first packet is `0x80 "theora"` is used; page checksums are not verified; packet size and count obey limits.
- Pictures: decoded by the `theora-rs` crate 0.1.0 (MPL-2.0, a Rust port of libtheora). A panic inside the decoder is turned into an error. Frames leave the core as planar YCbCr cropped to the picture dimensions.
- The decoder keeps its position, so playing forward costs one decode per frame; seeking back restarts from frame 0.
- Limits: picture side at most 8192 and area at most 33.5 million pixels.
- Not verified: playback inside the Windows window (WebView2), long videos, memory use on 4K files.

## Sound of `.ogv` (2026-11-22, `draft`)

The soundtrack is a standard Vorbis stream in its own logical Ogg stream (first packet `0x01 "vorbis"`). The
identification header gives channels (byte 11), rate (bytes 12..15) and the two block sizes (low and high
nibble of byte 28, as powers of two); the third packet is the setup header, later packets are audio. The
decoder of FSB Vorbis is reused with those block sizes. Checked on a synthetic file (440 Hz, mono, 16 kHz)
made with ffmpeg; **not yet checked on a real game video** (samples with sound were not available here).
