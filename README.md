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

## Trying the engine

The engine's first spike plays a pattern into a small synth of its own, the synth in a
process of its own, and measures what it costs ([results](docs/spikes/engine.md)):

```
cargo build --release -p engine-spike -p sonevo-plugin-host -p sonevo-test-synth
target/release/engine-spike --seconds 10
target/release/engine-spike --kill-every 2      # the plugin's process killed every two seconds
target/release/engine-spike --asio <driver> --buffer 128
target/release/engine-spike --here              # the plugin in the engine's process, to compare
```

ASIO needs libclang to build: `pip install --target .tools/libclang libclang==18.1.1`.

## Licence

GPL-3.0-or-later.
