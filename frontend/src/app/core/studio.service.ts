import { computed, effect, Injectable, signal } from '@angular/core';
import { api } from './api';
import {
  barLength,
  Channel,
  COLORS,
  Device,
  DeviceInfo,
  Edit,
  Id,
  Insert,
  Midi,
  newId,
  Note,
  Pattern,
  ParamInfo,
  PPQ,
  PluginInfo,
  STEP,
  SampleInfo,
  Song,
  Status,
  Target,
  View,
} from './models';

export type Dock = 'rack' | 'roll' | 'mixer';

const SILENT: Status = {
  position: 0,
  playing: false,
  master: 0,
  channels: {},
  inserts: {},
  audio: { device: null, sampleRate: 0, latencyMs: 0, failed: false, error: null },
  recording: null,
  load: 0,
  spectrum: [],
};

/**
 * The song open, as the engine's side last sent it, and what the interface has chosen in
 * it: the pattern edited, the channel, the device shown.
 */
@Injectable({ providedIn: 'root' })
export class Studio {
  readonly view = signal<View | null>(null);
  readonly status = signal<Status>(SILENT);
  readonly devices = signal<DeviceInfo[]>([]);
  readonly plugins = signal<PluginInfo[]>([]);
  private readonly pluginParams = signal<Record<string, ParamInfo[]>>({});
  private readonly asked = new Set<string>();
  readonly samples = signal<Record<string, SampleInfo>>({});
  readonly error = signal<string | null>(null);

  readonly song = computed(() => this.view()?.song ?? null);
  readonly bar = computed(() => barLength(this.song()?.signature ?? { beats: 4, unit: 4 }));

  readonly patternId = signal<Id | null>(null);
  readonly channelId = signal<Id | null>(null);
  readonly deviceId = signal<Id | null>(null);
  readonly dock = signal<Dock>('rack');
  readonly metronome = signal(false);
  /** Keys played are written into the pattern as it goes round. */
  readonly writing = signal(false);
  private statusAt = 0;
  /** The keys down being written, and where in the pattern each went down. */
  private readonly down = new Map<number, { at: number; velocity: number }>();

  readonly pattern = computed<Pattern | null>(() => {
    const song = this.song();
    if (!song) return null;
    return song.patterns.find((p) => p.id === this.patternId()) ?? song.patterns[0] ?? null;
  });

  readonly channel = computed<Channel | null>(() => {
    const song = this.song();
    if (!song) return null;
    return song.channels.find((c) => c.id === this.channelId()) ?? song.channels[0] ?? null;
  });

  readonly device = computed<{ device: Device; owner: string } | null>(() => {
    const song = this.song();
    if (!song) return null;
    const id = this.deviceId();
    for (const c of song.channels) {
      if (c.instrument.id === id) return { device: c.instrument, owner: c.name };
    }
    for (const insert of [...song.inserts, song.master]) {
      const effect = insert.effects?.find((e) => e.id === id);
      if (effect) return { device: effect, owner: insert.name };
    }
    const channel = this.channel();
    return channel ? { device: channel.instrument, owner: channel.name } : null;
  });

  constructor() {
    effect(() => void api.playWithKeyboards(this.channel()?.id ?? null).catch(() => undefined));
  }

  async start(): Promise<void> {
    const [view, devices, plugins] = await Promise.all([api.song(), api.devices(), api.plugins()]);
    this.devices.set(devices);
    this.plugins.set(plugins);
    this.show(view);
    await api.onStatus((status) => {
      this.status.set(status);
      this.statusAt = performance.now();
    });
    await api.onKey((key, velocity, on) => this.played(key, velocity, on));
  }

  show(view: View): void {
    // the notes not sent are those already here
    const before = this.view()?.notes ?? {};
    const notes: View['notes'] = {};
    let lost = false;
    for (const p of view.song.patterns) {
      const known = view.notes[p.id] ?? before[p.id];
      if (known) notes[p.id] = known;
      else lost = true;
    }
    this.view.set({ ...view, notes });
    if (lost) void api.song().then((whole) => this.show(whole));
    const song = view.song;
    if (!song.patterns.some((p) => p.id === this.patternId())) {
      this.patternId.set(song.patterns[0]?.id ?? null);
    }
    if (!song.channels.some((c) => c.id === this.channelId())) {
      this.channelId.set(song.channels[0]?.id ?? null);
    }
  }

  async edit(edit: Edit): Promise<boolean> {
    try {
      this.show(await api.edit(edit));
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  fail(e: unknown): void {
    this.error.set(String(e));
    setTimeout(() => this.error.set(null), 5000);
  }

  async undo(): Promise<void> {
    this.show(await api.undo());
  }

  async redo(): Promise<void> {
    this.show(await api.redo());
  }

  info(device: Device): DeviceInfo | null {
    if (device.kind.type === 'clap') return this.pluginInfo(device);
    const name = device.kind.name;
    return this.devices().find((d) => d.name === name) ?? null;
  }

  value(device: Device, param: string): number {
    const set = device.params?.[param];
    if (set !== undefined) return set;
    return this.info(device)?.params.find((p) => p.id === param)?.default ?? 0;
  }

  sample(name: string): SampleInfo | null {
    if (!this.asked.has(name)) {
      this.asked.add(name);
      api
        .sampleInfo(name)
        .then((info) => this.samples.update((s) => ({ ...s, [name]: info })))
        .catch(() => undefined);
    }
    return this.samples()[name] ?? null;
  }

  ticks(seconds: number): number {
    return (seconds * (this.song()?.tempo ?? 120) * PPQ) / 60;
  }

  notes(pattern: Id, channel: Id): Note[] {
    return this.view()?.notes[pattern]?.[channel] ?? [];
  }

  setNotes(pattern: Id, channel: Id, notes: Note[]): Promise<boolean> {
    const sorted = [...notes].sort((a, b) => a.start - b.start || a.key - b.key);
    return this.edit({ type: 'setNotes', pattern, channel, notes: sorted });
  }

  putChannel(channel: Channel, index: number | null = null): Promise<boolean> {
    return this.edit({ type: 'putChannel', channel, index });
  }

  putInsert(insert: Insert): Promise<boolean> {
    const song = this.song();
    if (song && insert.id === song.master.id) return this.edit({ type: 'setMaster', insert });
    return this.edit({ type: 'putInsert', insert, index: null });
  }

  async addChannel(name: string, label: string, sample?: string): Promise<void> {
    const instrument: Device = { id: newId(), kind: { type: 'builtin', name }, params: {}, sample };
    await this.addChannelOf(instrument, label);
  }

  async addPlugin(plugin: PluginInfo): Promise<void> {
    const kind = { type: 'clap' as const, bundle: plugin.bundle, plugin: plugin.id };
    await this.addChannelOf({ id: newId(), kind, params: {} }, plugin.name);
  }

  private pluginInfo(device: Device): DeviceInfo | null {
    const kind = device.kind;
    if (kind.type !== 'clap') return null;
    const key = `${kind.bundle}\n${kind.plugin}`;
    // asked once, without a signal written: this is read from computed values
    if (!this.asked.has(key)) {
      this.asked.add(key);
      api
        .pluginParams(kind.bundle, kind.plugin)
        .then((params) => this.pluginParams.update((p) => ({ ...p, [key]: params })))
        .catch(() => undefined);
    }
    const params = this.pluginParams()[key];
    const plugin = this.plugin(device);
    if (!params || !plugin) return null;
    return { name: kind.plugin, label: plugin.name, instrument: plugin.instrument, params };
  }

  plugin(device: Device): PluginInfo | null {
    const kind = device.kind;
    if (kind.type !== 'clap') return null;
    return this.plugins().find((p) => p.id === kind.plugin) ?? null;
  }

  private async addChannelOf(instrument: Device, label: string): Promise<void> {
    const song = this.song();
    if (!song) return;
    const channel: Channel = {
      id: newId(),
      name: label,
      color: COLORS[song.channels.length % COLORS.length],
      instrument,
      volume: 0.8,
      pan: 0,
    };
    if (await this.putChannel(channel)) {
      this.channelId.set(channel.id);
      this.deviceId.set(null);
    }
  }

  async addPattern(): Promise<void> {
    const song = this.song();
    if (!song) return;
    const pattern: Pattern = {
      id: newId(),
      name: `Pattern ${song.patterns.length + 1}`,
      color: COLORS[song.patterns.length % COLORS.length],
      length: this.bar(),
    };
    if (await this.edit({ type: 'putPattern', pattern, index: null })) {
      this.selectPattern(pattern.id);
    }
  }

  /** The MIDI file at `path` made a new pattern, each of its tracks played by a synth of its own. */
  async importMidi(path: string): Promise<void> {
    const song = this.song();
    if (!song) return;
    let midi: Midi;
    try {
      midi = await api.midiFile(path);
    } catch (e) {
      this.fail(e);
      return;
    }
    const name = path
      .split(/[\\/]/)
      .pop()!
      .replace(/\.midi?$/i, '');
    const channels: Channel[] = midi.tracks.map((track, i) => ({
      id: newId(),
      name: track.name || (midi.tracks.length > 1 ? `${name} ${i + 1}` : name),
      color: COLORS[(song.channels.length + i) % COLORS.length],
      instrument: { id: newId(), kind: { type: 'builtin', name: 'sonevo.synth' }, params: {} },
      volume: 0.8,
      pan: 0,
    }));
    const bar = this.bar();
    let end = 0;
    for (const n of midi.tracks.flatMap((t) => t.notes)) end = Math.max(end, n.start + n.length);
    const pattern: Pattern = {
      id: newId(),
      name,
      color: COLORS[song.patterns.length % COLORS.length],
      length: Math.max(bar, Math.ceil(end / bar) * bar),
    };
    const done = await this.edit({
      type: 'batch',
      edits: [
        ...channels.map((channel) => ({ type: 'putChannel' as const, channel, index: null })),
        { type: 'putPattern', pattern, index: null },
        ...midi.tracks.map((track, i) => ({
          type: 'setNotes' as const,
          pattern: pattern.id,
          channel: channels[i].id,
          notes: track.notes,
        })),
      ],
    });
    if (done) {
      this.selectPattern(pattern.id);
      this.channelId.set(channels[0].id);
      this.dock.set('roll');
    }
  }

  /** A new pattern named `name`, `notes` for `channel` in it, as many bars long as they need. */
  async addPatternOf(name: string, channel: Id, notes: Note[]): Promise<void> {
    const song = this.song();
    if (!song) return;
    const bar = this.bar();
    const end = notes.reduce((last, n) => Math.max(last, n.start + n.length), 0);
    const pattern: Pattern = {
      id: newId(),
      name,
      color: COLORS[song.patterns.length % COLORS.length],
      length: Math.max(bar, Math.ceil(end / bar) * bar),
    };
    const done = await this.edit({
      type: 'batch',
      edits: [
        { type: 'putPattern', pattern, index: null },
        { type: 'setNotes', pattern: pattern.id, channel, notes },
      ],
    });
    if (done) {
      this.selectPattern(pattern.id);
      this.dock.set('roll');
    }
  }

  async addInsert(): Promise<Insert | null> {
    const song = this.song();
    if (!song) return null;
    const insert: Insert = {
      id: newId(),
      name: `Track ${song.inserts.length + 1}`,
      color: COLORS[song.inserts.length % COLORS.length],
      volume: 1,
      pan: 0,
    };
    return (await this.edit({ type: 'putInsert', insert, index: null })) ? insert : null;
  }

  /** A new track holding an automation of `target`, flat at `value` (0 to 1) over the song. */
  async automate(target: Target, value: number, name: string): Promise<void> {
    const song = this.song();
    if (!song) return;
    const bar = this.bar();
    const length = Math.max(
      bar * 4,
      Math.ceil(Math.max(0, ...song.clips.map((c) => c.start + c.length)) / bar) * bar,
    );
    const track = { id: newId(), name, mute: false };
    const clip = {
      id: newId(),
      track: track.id,
      start: 0,
      length,
      content: {
        type: 'automation' as const,
        target,
        points: [
          { at: 0, value },
          { at: length, value },
        ],
      },
    };
    await this.edit({
      type: 'batch',
      edits: [
        { type: 'putTrack', track, index: null },
        { type: 'putClip', clip },
      ],
    });
  }

  setSong(
    change: Partial<Pick<Song, 'name' | 'tempo' | 'signature' | 'loopRegion'>>,
  ): Promise<boolean> {
    const song = this.song();
    if (!song) return Promise.resolve(false);
    const next = { ...song, ...change };
    return this.edit({
      type: 'setSong',
      name: next.name,
      tempo: next.tempo,
      signature: next.signature,
      loopRegion: next.loopRegion ?? null,
    });
  }

  /** A copy of `pattern` and its notes, placed after it: the copy, once made. */
  async duplicatePattern(pattern: Pattern, extra: Edit[] = []): Promise<Pattern | null> {
    const copy: Pattern = { ...pattern, id: newId(), name: `${pattern.name} (2)` };
    const notes = this.view()?.notes[pattern.id] ?? {};
    const index = (this.song()?.patterns.findIndex((p) => p.id === pattern.id) ?? -1) + 1;
    const done = await this.edit({
      type: 'batch',
      edits: [
        { type: 'putPattern', pattern: copy, index },
        ...Object.entries(notes).map(([channel, list]) => ({
          type: 'setNotes' as const,
          pattern: copy.id,
          channel,
          notes: list,
        })),
        ...extra.map((edit) =>
          edit.type === 'putClip' && edit.clip.content.type === 'pattern'
            ? {
                ...edit,
                clip: { ...edit.clip, content: { type: 'pattern' as const, pattern: copy.id } },
              }
            : edit,
        ),
      ],
    });
    return done ? copy : null;
  }

  selectPattern(id: Id): void {
    this.patternId.set(id);
    if (this.view()?.patternMode) void this.setMode(id);
  }

  async setMode(pattern: Id | null): Promise<void> {
    this.show(await api.setMode(pattern));
  }

  /** Where playing is now, between two statuses, in the pattern going round. */
  private patternTick(): number | null {
    const view = this.view();
    const pattern = this.pattern();
    const status = this.status();
    if (!view?.patternMode || !pattern || !status.playing) return null;
    const since = (performance.now() - this.statusAt) / 60000;
    const tick = status.position + since * (this.song()?.tempo ?? 120) * PPQ;
    return tick % Math.max(1, pattern.length);
  }

  /** While writing, a key held goes into the pattern playing as a note, on the grid's steps. */
  played(key: number, velocity: number, on: boolean): void {
    if (!this.writing()) return;
    const at = this.patternTick();
    const pattern = this.pattern();
    const channel = this.channel();
    if (at === null || !pattern || !channel) return;
    if (on) {
      this.down.set(key, { at, velocity });
      return;
    }
    const pressed = this.down.get(key);
    this.down.delete(key);
    if (!pressed) return;
    const start = pressed.at;
    const step = STEP;
    const from = (Math.round(start / step) * step) % pattern.length;
    let length = at - start;
    if (length <= 0) length += pattern.length;
    const note: Note = {
      start: from,
      length: Math.max(step, Math.round(length / step) * step),
      key,
      velocity: pressed.velocity || 0.8,
    };
    void this.setNotes(pattern.id, channel.id, [...this.notes(pattern.id, channel.id), note]);
  }

  togglePlay(): void {
    if (this.status().recording !== null) void this.stopRecording();
    else if (this.status().playing) void api.pause();
    else void api.play();
  }

  async toggleRecord(): Promise<void> {
    if (this.status().recording !== null) {
      await this.stopRecording();
      return;
    }
    try {
      await api.record();
    } catch (e) {
      this.fail(e);
    }
  }

  async stopRecording(): Promise<void> {
    try {
      this.show(await api.stopRecording());
    } catch (e) {
      this.fail(e);
    }
  }
}
