# MP3 export

Status: `verified` on synthetic tones and on real streams (see below); not an unpacking format, only an output.

Sounds decode to 16-bit PCM first (FSB5 Vorbis, FSB4 IMA ADPCM); WAV is the lossless output, MP3 is lossy.
The encoder is the `rusty_mp3` crate 1.1.0 (pure Rust, Apache-2.0): constant bitrate, requested 320 kbps and snapped to
the highest legal value of the MPEG version.

| Sample rate | MPEG version | Bitrate used |
|---|---|---|
| 32000, 44100, 48000 Hz | MPEG-1 | 320 kbps |
| 16000, 22050, 24000 Hz | MPEG-2 | 160 kbps (format limit) |
| 8000, 11025, 12000 Hz | MPEG-2.5 | 160 kbps or less (format limit) |

Other sample rates (for example 4000 Hz) and more than two channels are refused with a message that points to WAV.

Quality (waveform signal-to-noise ratio of the decoded MP3 against the source PCM, measured 2026-11-21 with the crate's
own decoder): 18 voice streams of `Casual_rus.bsb` (FSB5): mean 36.2 dB, worst 34.5 dB; 30 streams of
`Voice_KaniaMale_Animation.bsb` (FSB4): mean 51.5 dB, worst 46.7 dB. The crate's own tests report PEAQ within about 0.1 ODG
of LAME from 160 kbps; the encoder is young and slightly behind LAME at lower bitrates. Listening tests were not done.
Files carry a LAME-style Info tag with encoder delay and padding; the crate's decoder keeps the delay (output is
a few frames longer), tag-aware players trim it.
