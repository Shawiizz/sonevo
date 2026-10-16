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
  newId,
  Note,
  Pattern,
  PPQ,
  PluginInfo,
  SampleInfo,
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
  /** The sounds of the song read so far, by name; null while being read or unreadable. */
  readonly samples = signal<Record<string, SampleInfo | null>>({});
  readonly error = signal<string | null>(null);

  readonly song = computed(() => this.view()?.song ?? null);
  readonly bar = computed(() => barLength(this.song()?.signature ?? { beats: 4, unit: 4 }));

  readonly patternId = signal<Id | null>(null);
  readonly channelId = signal<Id | null>(null);
  readonly deviceId = signal<Id | null>(null);
  readonly dock = signal<Dock>('rack');
  readonly metronome = signal(false);

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

  /** The device shown in the device panel: one chosen, or the channel's instrument. */
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
    // the keyboards plugged in play the channel chosen
    effect(() => void api.playWithKeyboards(this.channel()?.id ?? null).catch(() => undefined));
  }

  async start(): Promise<void> {
    const [view, devices, plugins] = await Promise.all([api.song(), api.devices(), api.plugins()]);
    this.devices.set(devices);
    this.plugins.set(plugins);
    this.show(view);
    await api.onStatus((status) => this.status.set(status));
  }

  /** The song as the engine's side sent it back. */
  show(view: View): void {
    this.view.set(view);
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
    if (device.kind.type !== 'builtin') return null;
    const name = device.kind.name;
    return this.devices().find((d) => d.name === name) ?? null;
  }

  /** A device's setting: as set, or its default. */
  value(device: Device, param: string): number {
    const set = device.params?.[param];
    if (set !== undefined) return set;
    return this.info(device)?.params.find((p) => p.id === param)?.default ?? 0;
  }

  /** The sound `name`, read once and kept; null until read. */
  sample(name: string): SampleInfo | null {
    const known = this.samples();
    if (!(name in known)) {
      this.samples.update((s) => ({ ...s, [name]: null }));
      api
        .sampleInfo(name)
        .then((info) => this.samples.update((s) => ({ ...s, [name]: info })))
        .catch(() => undefined);
    }
    return known[name] ?? null;
  }

  /** Ticks to `seconds` at the song's tempo. */
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

  /** A new channel playing `name`, a built-in instrument; selected once added. */
  async addChannel(name: string, label: string, sample?: string): Promise<void> {
    const instrument: Device = { id: newId(), kind: { type: 'builtin', name }, params: {}, sample };
    await this.addChannelOf(instrument, label);
  }

  /** A new channel playing `plugin`. */
  async addPlugin(plugin: PluginInfo): Promise<void> {
    const kind = { type: 'clap' as const, bundle: plugin.bundle, plugin: plugin.id };
    await this.addChannelOf({ id: newId(), kind, params: {} }, plugin.name);
  }

  /** The plugin a device plays, if it is one installed. */
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

  /**
   * A new track holding an automation of `target`, flat at `value` (0 to 1) over the song,
   * named `name`: its line then drawn in the playlist.
   */
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

  selectPattern(id: Id): void {
    this.patternId.set(id);
    // a pattern played alone follows the one chosen
    if (this.view()?.patternMode) void this.setMode(id);
  }

  async setMode(pattern: Id | null): Promise<void> {
    this.show(await api.setMode(pattern));
  }

  togglePlay(): void {
    if (this.status().playing) void api.pause();
    else void api.play();
  }
}
