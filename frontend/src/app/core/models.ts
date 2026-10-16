/** A song as the engine's side holds it: the fields left out when empty may be missing. */

export type Id = string;
export type Ticks = number;

/** Ticks to a quarter note. */
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
}

export interface Note {
  start: Ticks;
  length: Ticks;
  key: number;
  velocity: number;
}

export interface Pattern {
  id: Id;
  name: string;
  color: string;
  length: Ticks;
}

export interface Track {
  id: Id;
  name: string;
  mute?: boolean;
}

export type Target =
  | { type: 'channelVolume'; channel: Id }
  | { type: 'channelPan'; channel: Id }
  | { type: 'insertVolume'; insert: Id }
  | { type: 'insertPan'; insert: Id }
  | { type: 'param'; device: Id; param: string }
  | { type: 'tempo' };

export interface Point {
  at: Ticks;
  value: number;
  curve?: number;
}

export type ClipContent =
  | { type: 'pattern'; pattern: Id }
  | { type: 'audio'; sample: string; gain: number }
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
  clips: Clip[];
  inserts: Insert[];
  master: Insert;
  loopRegion?: Region;
}

export interface View {
  song: Song;
  /** By pattern, by channel. */
  notes: Record<Id, Record<Id, Note[]>>;
  folder: string | null;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  patternMode: Id | null;
  missing: string[];
  /** The version played in place of the song, to compare by ear. */
  comparing: string | null;
}

export interface VersionInfo {
  id: string;
  name: string;
  /** Made as the song was saved, not named. */
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
  language: string | null;
  recent: string[];
  folders: string[];
  pluginFolders: string[];
  welcomed: boolean;
}

/** A sound of the song by its own name: the file's name without its hash or extension. */
export function sampleLabel(name: string): string {
  return name.replace(/ ?[0-9a-f]{16}\.[^.]+$/, '') || name;
}

/** A sound of the song: how long it lasts, and its outline. */
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

/** A plugin installed on this computer. */
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
  | { type: 'batch'; edits: Edit[] };

/** The colours channels, patterns and mixer tracks take in turn. */
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

/** A bar's length in ticks. */
export function barLength(signature: Signature): Ticks {
  return (signature.beats * PPQ * 4) / Math.max(1, signature.unit);
}

/** A position as bars, beats and sixteenths, each from 1. */
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

/** The scales the piano roll lights, by the steps of their notes above their root. */
export const SCALES: { name: string; steps: number[] }[] = [
  { name: 'Major', steps: [0, 2, 4, 5, 7, 9, 11] },
  { name: 'Minor', steps: [0, 2, 3, 5, 7, 8, 10] },
  { name: 'Minor pentatonic', steps: [0, 3, 5, 7, 10] },
  { name: 'Major pentatonic', steps: [0, 2, 4, 7, 9] },
  { name: 'Blues', steps: [0, 3, 5, 6, 7, 10] },
  { name: 'Dorian', steps: [0, 2, 3, 5, 7, 9, 10] },
];

/** Whether `key` is a note of the scale `scale` from `root` (0 is C). */
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
