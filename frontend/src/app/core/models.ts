/** A song as the engine's side holds it: the fields left out when empty may be missing. */

export type Id = string;
export type Ticks = number;

export const PPQ = 960;
export const STEP = PPQ / 4;

export interface Signature {
  beats: number;
  unit: number;
}

export type DeviceKind =
  { type: 'builtin'; name: string } | { type: 'clap'; bundle: string; plugin: string };

export interface Device {
  id: Id;
  kind: DeviceKind;
  params?: Record<string, number>;
  /** Missing is on. */
  enabled?: boolean;
  sample?: string;
  /** For an effect: the mixer track it listens to instead of its own sound (a sidechain). */
  key?: Id;
}

export interface Channel {
  id: Id;
  name: string;
  color: string;
  instrument: Device;
  volume: number;
  pan: number;
  mute?: boolean;
  insert?: Id;
  /** Its notes played one after the other instead of together, when on. */
  arp?: Arp;
}

export interface Arp {
  order: 'up' | 'down' | 'upDown' | 'random';
  /** How far apart its notes are, in ticks. */
  step: Ticks;
  octaves: number;
  /** How much of its step each note lasts, from 0.1 to 1. */
  gate: number;
}

export interface Note {
  start: Ticks;
  length: Ticks;
  key: number;
  velocity: number;
}

/** Settings ready made, or saved, for a built-in device. */
export interface PresetInfo {
  name: string;
  factory: boolean;
  values: Record<string, number>;
}

/** What a MIDI file holds: its first tempo, and its tracks that play notes. */
export interface Midi {
  tempo: number | null;
  tracks: { name: string; notes: Note[] }[];
}

export interface Pattern {
  id: Id;
  name: string;
  color: string;
  length: Ticks;
  /** From 0 to 1: how late every second sixteenth plays, up to half a sixteenth. */
  swing?: number;
}

export interface Marker {
  id: Id;
  at: Ticks;
  name: string;
}

export interface Track {
  id: Id;
  name: string;
  mute?: boolean;
  /** The mixer track its sound clips play into; none is the master. */
  insert?: Id;
}

export type Target =
  | { type: 'channelVolume'; channel: Id }
  | { type: 'channelPan'; channel: Id }
  | { type: 'insertVolume'; insert: Id }
  | { type: 'insertPan'; insert: Id }
  | { type: 'param'; device: Id; param: string }
  | { type: 'tempo' };

/** A target as a key: the same whatever order its fields came in. */
export function targetKey(target: Target): string {
  switch (target.type) {
    case 'channelVolume':
    case 'channelPan':
      return `${target.type} ${target.channel}`;
    case 'insertVolume':
    case 'insertPan':
      return `${target.type} ${target.insert}`;
    case 'param':
      return `param ${target.device} ${target.param}`;
    case 'tempo':
      return 'tempo';
  }
}

export interface Point {
  at: Ticks;
  value: number;
  curve?: number;
}

export type ClipContent =
  | { type: 'pattern'; pattern: Id }
  | {
      type: 'audio';
      sample: string;
      gain: number;
      fadeIn?: Ticks;
      fadeOut?: Ticks;
      reverse?: boolean;
    }
  | { type: 'automation'; target: Target; points: Point[] };

export interface Clip {
  id: Id;
  track: Id;
  start: Ticks;
  length: Ticks;
  offset?: Ticks;
  content: ClipContent;
}

export interface Send {
  to: Id;
  amount: number;
}

export interface Insert {
  id: Id;
  name: string;
  color: string;
  volume: number;
  pan: number;
  mute?: boolean;
  solo?: boolean;
  effects?: Device[];
  sends?: Send[];
  output?: Id;
}

export interface Region {
  start: Ticks;
  end: Ticks;
}

export interface Song {
  name: string;
  tempo: number;
  signature: Signature;
  channels: Channel[];
  patterns: Pattern[];
  tracks: Track[];
  /** The clips of the arrangement played, named `arrangement`. */
  clips: Clip[];
  arrangement: string;
  /** The other arrangements, to be played in its place. */
  arrangements?: { name: string; clips: Clip[] }[];
  inserts: Insert[];
  master: Insert;
  loopRegion?: Region;
  /** Names along the playlist's ruler, in time order. */
  markers?: Marker[];
}

export interface View {
  song: Song;
  /** By pattern, by channel: from the engine, only the patterns whose notes changed. */
  notes: Record<Id, Record<Id, Note[]>>;
  folder: string | null;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  patternMode: Id | null;
  missing: string[];
  comparing: string | null;
  /** Counted up with each view: one come after a later one is out of date. */
  revision: number;
}

export interface VersionInfo {
  id: string;
  name: string;
  automatic: boolean;
  /** Seconds since 1970. */
  time: number;
}

export interface AudioStatus {
  device: string | null;
  sampleRate: number;
  latencyMs: number;
  failed: boolean;
  error: string | null;
}

export interface Status {
  position: number;
  playing: boolean;
  master: number;
  channels: Record<Id, number>;
  inserts: Record<Id, number>;
  audio: AudioStatus;
  /** While recording, the loudest the input was since the last look. */
  recording: number | null;
  /** The most of its time a block took to play: 1 is all of it. */
  load: number;
  /** How loud the master played across the spectrum, from the lows to the highs, 0 to 1. */
  spectrum: number[];
  /** How far each compressor and limiter brought the sound down, in dB. */
  reductions: Record<Id, number>;
}

export interface ParamInfo {
  id: string;
  name: string;
  min: number;
  max: number;
  default: number;
  unit: string;
  scale: 'linear' | 'log' | 'choice';
  choices: string[];
}

/** A plugin's setting as the plugin has it now, written its way ("-3.2 dB"). */
export interface PluginValue {
  id: string;
  value: number;
  text: string;
}

export interface DeviceInfo {
  name: string;
  label: string;
  instrument: boolean;
  params: ParamInfo[];
}

export interface OutputInfo {
  driver: 'wasapi' | 'asio';
  name: string;
  default: boolean;
}

export interface Settings {
  driver: string;
  device: string | null;
  buffer: number | null;
  input: string | null;
  language: string | null;
  theme: string | null;
  accent: string | null;
  recent: string[];
  folders: string[];
  pluginFolders: string[];
  welcomed: boolean;
  /** Whether recording starts a bar early, the bar left out of what is kept. */
  countIn: boolean;
}

/** A sound of the song by its own name: the file's name without its hash or extension. */
export function sampleLabel(name: string): string {
  return name.replace(/ ?[0-9a-f]{16}\.[^.]+$/, '') || name;
}

export interface SampleInfo {
  seconds: number;
  peaks: number[];
}

/** A song an earlier run kept and never saved. */
export interface Kept {
  path: string;
  name: string;
  /** The folder it is saved in, if it ever was. */
  origin: string | null;
  /** Seconds since 1970. */
  time: number;
}

export interface PluginInfo {
  bundle: string;
  id: string;
  name: string;
  vendor: string;
  instrument: boolean;
}

export interface Entry {
  name: string;
  path: string;
  folder: boolean;
}

export type Edit =
  | {
      type: 'setSong';
      name: string;
      tempo: number;
      signature: Signature;
      loopRegion: Region | null;
    }
  | { type: 'putChannel'; channel: Channel; index: number | null }
  | { type: 'removeChannel'; id: Id }
  | { type: 'putPattern'; pattern: Pattern; index: number | null }
  | { type: 'removePattern'; id: Id }
  | { type: 'setNotes'; pattern: Id; channel: Id; notes: Note[] }
  | { type: 'putTrack'; track: Track; index: number | null }
  | { type: 'removeTrack'; id: Id }
  | { type: 'putClip'; clip: Clip }
  | { type: 'removeClips'; ids: Id[] }
  | { type: 'putInsert'; insert: Insert; index: number | null }
  | { type: 'removeInsert'; id: Id }
  | { type: 'setMaster'; insert: Insert }
  | { type: 'setParam'; device: Id; param: string; value: number }
  | { type: 'keepArrangementAs'; name: string }
  | { type: 'switchArrangement'; index: number }
  | { type: 'renameArrangement'; name: string }
  | { type: 'removeArrangement'; index: number }
  | { type: 'putMarker'; marker: Marker }
  | { type: 'removeMarker'; id: Id }
  | { type: 'batch'; edits: Edit[] };

export const COLORS = [
  '#5ab0f0',
  '#f2b84b',
  '#8f8bf7',
  '#ef6f6c',
  '#5fd3a8',
  '#e88ad6',
  '#b4dc5a',
  '#4fd0d6',
];

/** Sixty-four random bits, as sixteen hexadecimal digits, as the song's ids are. */
export function newId(): Id {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

export function builtinName(device: Device): string | null {
  return device.kind.type === 'builtin' ? device.kind.name : null;
}

export function barLength(signature: Signature): Ticks {
  return (signature.beats * PPQ * 4) / Math.max(1, signature.unit);
}

export function formatPosition(ticks: Ticks, signature: Signature): string {
  const beat = (PPQ * 4) / Math.max(1, signature.unit);
  const bar = beat * signature.beats;
  const t = Math.max(0, ticks);
  const bars = Math.floor(t / bar) + 1;
  const beats = Math.floor((t % bar) / beat) + 1;
  const steps = Math.floor((t % beat) / STEP) + 1;
  return `${bars}.${beats}.${steps}`;
}

export const NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

export const SCALES: { name: string; steps: number[] }[] = [
  { name: 'Major', steps: [0, 2, 4, 5, 7, 9, 11] },
  { name: 'Minor', steps: [0, 2, 3, 5, 7, 8, 10] },
  { name: 'Minor pentatonic', steps: [0, 3, 5, 7, 10] },
  { name: 'Major pentatonic', steps: [0, 2, 4, 7, 9] },
  { name: 'Blues', steps: [0, 3, 5, 6, 7, 10] },
  { name: 'Dorian', steps: [0, 2, 3, 5, 7, 9, 10] },
];

/** Chord progressions known to work, as degrees of a major or a minor scale, a bar each. */
export const PROGRESSIONS: { name: string; scale: number; degrees: number[] }[] = [
  { name: 'Pop: I V vi IV', scale: 0, degrees: [0, 4, 5, 3] },
  { name: 'Sad: vi IV I V', scale: 0, degrees: [5, 3, 0, 4] },
  { name: 'Fifties: I vi IV V', scale: 0, degrees: [0, 5, 3, 4] },
  { name: 'Jazz: ii V I', scale: 0, degrees: [1, 4, 0, 0] },
  { name: 'Epic: i VI III VII', scale: 1, degrees: [0, 5, 2, 6] },
  { name: 'Dark: i iv VI v', scale: 1, degrees: [0, 3, 5, 4] },
  { name: 'Flamenco: i VII VI V', scale: 1, degrees: [0, 6, 5, 4] },
];

/** The keys of the chord on `degree` of the seven-note scale `steps` from `root`, around `base`. */
export function degreeChord(degree: number, steps: number[], root: number, base: number): number[] {
  return [0, 2, 4].map((third) => {
    const at = degree + third;
    return base + root + steps[at % steps.length] + 12 * Math.floor(at / steps.length);
  });
}

/**
 * The keys of a chord from `key`, of `notes` notes: a scale's own, a note in two of it apart
 * (its chords sound right in it, minor or major as it wants); without a scale, major.
 */
export function chordKeys(
  key: number,
  notes: number,
  root: number,
  scale: number[] | null,
): number[] {
  if (!scale) return [0, 4, 7, 10].slice(0, notes).map((step) => key + step);
  const steps: number[] = [];
  for (let k = key; steps.length < 7 && k < key + 24; k++) {
    if (inScale(k, root, scale)) steps.push(k);
  }
  if (!steps.length || steps[0] !== key) return [key];
  return [0, 2, 4, 6].slice(0, notes).map((i) => steps[i] ?? key);
}

export function inScale(key: number, root: number, scale: number[]): boolean {
  return scale.includes((((key - root) % 12) + 12) % 12);
}

/** A key's name, middle C (60) being C4. */
export function keyName(key: number): string {
  return `${NAMES[key % 12]}${Math.floor(key / 12) - 1}`;
}

export function isBlack(key: number): boolean {
  return [1, 3, 6, 8, 10].includes(key % 12);
}
