# Engine spike: a plugin in a process of its own

What `spikes/engine` set out to learn: can a plugin be played in its own process, block by
block, without the cost being heard; and does the engine play on when that process dies?

The setup: a four-bar pattern at 120 BPM, cut into notes on their frame, played into
`plugins/test-synth` (a CLAP instrument), out through WASAPI. The plugin's process shares a
file mapping with the engine (the block's notes in, its sound out) and two events, one each
way; the engine waits for a block at most 60 % of the block's own length, and plays silence
when it is late.

## Measured (2026-10-16, Windows 11, release build)

| Plugin played | Blocks | Round trip, median | 99th percentile | Longest | Late blocks |
|---|---|---|---|---|---|
| In the engine's process | 480 frames (10 ms) | 20 µs | 40 to 70 µs | 97 µs | 0 |
| In a process of its own | 480 frames (10 ms) | 35 to 70 µs | 125 to 210 µs | 331 µs | 0 |

A process of its own costs about 30 µs a block at the median, under 200 µs at worst: 2 %
of a 10 ms block, 7 % of a 128-frame ASIO block (2.7 ms).

With the plugin's process killed every two seconds, the engine played on: each kill cost
two or three blocks of silence (20 to 30 ms) while the process started again, and nothing
else. Notes held at the time of the crash were lost until their next start.

WASAPI's shared mode put 16 to 32 ms between the callback and the speakers. ASIO opened
(Voicemeeter's drivers), but no low-latency interface was plugged in to measure: the UMC's
driver is installed, its interface was not there.

## What follows

- Out of process is the default: its cost is small next to what it saves.
- The engine should remember the notes being held and send them again to a process started
  again, so that a crash costs only the blocks of silence.
- Low latency is to be measured on the UMC interface through ASIO (128 frames and less),
  and WASAPI's exclusive or low-latency shared mode looked at for those without one.
- A block late now waits up to its budget in the audio callback; the engine's own work must
  fit in what is left, which a graph of many plugins in parallel will need measured again.
