# Audio and video

**Status: draft.** Derived from a third party batch kit (only its scripts were read) and
from the file listing of a video archive.

## Audio banks

- Sounds are packed with FMOD in `.fsb` (FMOD sample bank) and `.bsb` files.
- A `.bsb` is a **zlib stream**. After inflating, the first 4 bytes are dropped and the
  rest is a normal `.fsb` file.
- The kit then decodes the streams of the `.fsb` (they end up as Ogg Vorbis files in its
  workflow), converts Ogg to WAV and WAV to MP3 with `lame`. The step that cuts streams
  out of the `.fsb` is done by another tool that the kit does not contain.

To do: identify the FSB version and codec from real samples, then write the reader.

## Video

- `.ogv` files (Ogg container) stored uncompressed in a video archive. Codecs are
  presumably Theora and Vorbis, to be confirmed with a sample.
