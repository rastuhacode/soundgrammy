# Native audio fixtures

These are original, synthetic one-second signals generated for SoundGrammy's
native decoder tests. Left: 440 Hz at amplitude 5000/32768. Right: 660 Hz at
4000/32768. Stereo, 44,100 Hz, signed 16-bit PCM. No recordings or third-party
music are included. These fixtures may be redistributed under the repository's
GPL-3.0-only license.

`tone.wav` is generated with Python's `wave`, `math`, and `struct` modules.
The compressed variants were generated using FFmpeg 7.1 distributed with
imageio-ffmpeg 0.6.0, using these output arguments:

| File | FFmpeg output arguments |
| --- | --- |
| tone.mp3 | `-c:a libmp3lame -q:a 4 -id3v2_version 3` |
| tone.m4a | `-c:a aac -b:a 96k` |
| tone-alac.m4a | `-c:a alac` |
| tone.ogg | `-c:a libvorbis -q:a 3` |
| tone.flac | `-c:a flac` |
| tone.opus | `-c:a libopus` |
| tone.webm | `-c:a libopus` |

Input for each command: `-i tone.wav`. No encoder is required to run the tests.
The Opus files deliberately test `unsupported-format`; the separate ruopus
artwork analyzer is not used to claim native playback support.

`long-tone.mp3` repeats `tone.wav` 120 times with `-stream_loop 119`,
encoded with `-c:a libmp3lame -b:a 128k -id3v2_version 3`. Its two-minute
length tests a 75% seek across multiple missing 128 KiB download chunks.
