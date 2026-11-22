# FSB4 sound banks

Status: `verified` for the container and for mono IMA ADPCM (one real bank, 119 streams, all decode);
`draft` for stereo order and raw PCM (no sample seen); other codecs `unknown`.

Found in Cloud Pirates 1.5: `Voice_KaniaMale_Animation.bsb`. The `.bsb` wrapper is the same as for FSB5
(zlib, then a 4 byte length prefix); inside, the data starts with `FSB4` instead of `FSB5`.

## Layout (little endian)

Header, 48 bytes: `"FSB4"`, stream count, size of all sample headers, size of all stream data, version
(`0x00040000`), bank mode (`0x20` seen), 8 zero bytes, 16 byte hash (not checked).

Sample header (80 bytes seen; the first `u16` is its own size, the reader accepts 64 or more):

| Offset | Field |
|---|---|
| 0 | `u16` header size |
| 2 | name, 30 bytes, zero padded |
| 32 | `u32` length in samples (frames) |
| 36 | `u32` data length in bytes |
| 40, 44 | loop start, loop end |
| 48 | `u32` mode flags |
| 52 | `i32` frequency in Hz |
| 56..62 | default volume, pan, priority |
| 62 | `u16` channel count |

Stream data follows the sample headers, streams back to back in header order.

## Codec

Mode flag `0x00400000` = IMA ADPCM (seen: mode `0x00500020`, mono, 44 100 Hz). One block per channel is 36 bytes:
`i16` predictor, `u8` step index, one reserved byte, 32 bytes of nibbles (low nibble first), giving 64 samples.
The header predictor and index are the state before the first nibble; the header itself is not a sample.
Check: after 64 nibbles the state equals the next block's header (exact for the first blocks of the sample bank).
Stereo: blocks of the channels are assumed to alternate per 64 frames (not verified).

Raw PCM (draft): mode bit `0x10` = 16-bit, `0x08` = 8-bit (`0x80` unsigned) when no codec bit is set.
Any other codec bit (for example MPEG `0x00200000`) is reported as unsupported with the mode value.

## `.bev`

Status: `verified` for the container, project name, languages and the string table on two real files (`VoiceDialogs03.bev`, 28 874 bytes, and `Voice.bev` of Cloud Pirates); `unknown` for the event data.

A `.bev` is a zlib stream (FMOD Designer event project, `FEV `). The inflated data starts with a block that is not understood (128 bytes in `VoiceDialogs03.bev`, 80 in `Voice.bev`; the reader searches for the RIFF header in the first 1 KiB), then a RIFF `FEV ` container (little endian chunks, `tag`, `u32 size`, padded to even):

| Chunk | Content |
|---|---|
| `FMT ` | `u32` format version (`0x00450000` seen) |
| `LIST` `PROJ` | the project, with the chunks below |
| `OBCT` | table of `u32` numbers (counts of object kinds; meaning not known) |
| `PROP` | `u32` length, project name with a NUL (`VoiceDialogs03`) |
| `LGCY` | the event data (322 763 bytes in the sample); not decoded |
| `EPRP` | 184 bytes of `u32`/`f32` values; not decoded |
| `STRR` | `u32` count, `count` `u32` offsets into the text that follows, NUL-terminated strings |
| `LANG` | `u32` count, then `u32` length and a NUL-terminated name for each (`english`, `french`, `german`, `russian`, `turkish`), a trailing `u32` |

The string table of `VoiceDialogs03.bev` has 916 strings: language suffixes (`_rus`, `_eng`, ...), names of events and parameters (`Keeper1`, `Morok01`, `(distance)`, `occlusion`) and full paths of events, for example `/zones/Kvator/Phrases/Kvator_Wedding_Rada_01`. The viewer lists these and the chunk sizes; the link between an event and the sound banks (`LGCY`) is not decoded.
