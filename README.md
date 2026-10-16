# Sonevo

A free and open source music studio: patterns, a step sequencer, a piano roll, a playlist
and a mixer, Sonevo's own instruments and effects, and the plugins already on your computer.

![Sonevo](docs/sonevo.png)

## What it does

- **Start from something**: a demo, templates in a style (trap, lo-fi, techno) and your own,
  and some fifty presets, an 808 that glides to a big hall.
- **Make a beat in seconds**: the channel rack's steps, a click each, filled every 2, 4 or 8
  steps; swing per pattern; drums synthesized from scratch, a subtractive synth (one voice
  and glide, an LFO in time), an FM synth, plucked strings, a sampler that slices its sound
  a key each; your computer keyboard as a piano.
- **Write melodies**: a piano roll that lights the scale you choose and draws its chords in a
  click, writes chord progressions known to work and melodies from the scale, humanizes,
  strums and makes legato; velocity, copy and paste, quantize, and the other channels' notes
  faintly behind. An arpeggiator on any channel, for the notes written and those played.
- **Arrange**: patterns, sounds and automation placed, stretched and cut in the playlist;
  markers along the ruler; a loop; sounds faded, reversed, stretched to their clip or
  transposed, at their own pitch or length; knobs turned as the song plays recorded as
  automation.
- **Mix**: mixer tracks with effects (EQ and its curve, compressor that can listen to another
  track, delay, reverb, distortion, filter, limiter, chorus, phaser, lo-fi, auto-pan), sends,
  mute and solo; the master's spectrum live, and a mix check that says in plain words what is
  amiss.
- **Your plugins**: the CLAP instruments and effects installed, each played in a process of
  its own: one that crashes is started again, its notes held pressed again, and the song
  plays on. Their settings as knobs, automated; their own windows; their state saved with the
  song.
- **Record**: the microphone recorded into the song as it plays, into the mixer track chosen;
  MIDI keyboards play the channel chosen, and write into the pattern as it goes round.
- **An open format**: a song is a folder of plain files, a pattern's notes one a line,
  its sounds beside them named by what is in them; a change shows as a small diff. MIDI
  files in and out.
- **Versions of a song**: every save kept, the ones that matter named, any of them listened
  to in place of the song, or brought back.
- **Nothing lost**: the song kept as it changes, offered back after a crash.
- **Export**: the song, its loop, or each mixer track alone, to WAV or FLAC, with its
  loudness in LUFS.

Every device explains itself in a sentence, and a guide walks through a first song: Sonevo
is meant for those who have never made music too. English and French, dark and light, five
accents.

Windows first (WASAPI and ASIO), macOS and Linux after.

## Building

You need Rust (the version in `rust-toolchain.toml`), Node 24 and pnpm.

```
cd frontend && pnpm install && cd ..
cargo build -p sonevo-plugin-host
cd apps/sonevo-desktop && ../../frontend/node_modules/.bin/tauri dev
```

ASIO needs libclang to build: `pip install --target .tools/libclang libclang==18.1.1`.

The installer: `tauri build --config tauri.release.conf.json` from `apps/sonevo-desktop`.
It signs the update packages, so it needs `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Releases are made by pushing a tag equal to the version
(`0.2.0`): the workflow drafts the release, and publishing it offers the update to installed copies.

## The parts

| Folder | What it is |
| --- | --- |
| `crates/sonevo-project` | A song: its model, its edits and their undo, the folder it is saved as |
| `crates/sonevo-dsp` | Sonevo's own instruments and effects |
| `crates/sonevo-engine` | The real-time engine: a song compiled into a graph the audio thread swaps in, played live or to a file, never allocating on the audio thread |
| `crates/sonevo-audio` | The sound card, WASAPI or ASIO |
| `crates/sonevo-plugins` | The CLAP plugins found, and each played in a process of its own |
| `plugins/test-synth`, `plugins/test-effect` | A CLAP instrument and a CLAP effect of Sonevo's own, to try the hosting with |
| `crates/sonevo-clap`, `crates/sonevo-plugin-ipc`, `apps/plugin-host` | CLAP hosting, and the link between the engine and a plugin's process |
| `crates/sonevo-versions` | A song's versions, in a git repository of its own written by gitoxide |
| `apps/sonevo-desktop` | The application (Tauri v2) |
| `frontend` | Its interface (Angular; the piano roll and the playlist drawn on 2D canvases) |

The road ahead is in [docs/ROADMAP.md](docs/ROADMAP.md); the engine's first measures in
[docs/spikes/engine.md](docs/spikes/engine.md).

## Licence

GPL-3.0-or-later.
