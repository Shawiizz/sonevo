# Sonevo

A free and open source music studio: patterns, a piano roll, a playlist and a mixer, with
the instruments and effects you already have.

Sonevo is at its very start. What it aims for:

- **Most of what a music studio does**: patterns and a step sequencer, a piano roll that
  knows scales and chords, a playlist to arrange them, a mixer, automation, export.
- **Your plugins**: the VST3 and CLAP instruments and effects installed on your computer,
  each run apart, so that one that crashes never takes the song with it.
- **An open project format**: a song is a folder of plain files, its samples beside them,
  readable and diffable.
- **Versions of a song**: its history kept, variants of a pattern or an arrangement to
  compare by ear, any earlier state brought back.
- **A real-time engine** in Rust: sample-accurate, no allocation on the audio thread, the
  same code for playing and for exporting.

Windows first (ASIO and WASAPI), macOS and Linux after.

## Licence

GPL-3.0-or-later.
