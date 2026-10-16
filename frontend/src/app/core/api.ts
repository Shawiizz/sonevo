import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  AudioStatus,
  DeviceInfo,
  Edit,
  Entry,
  Id,
  OutputInfo,
  Settings,
  VersionInfo,
  Status,
  Ticks,
  View,
} from './models';

/** The commands of the engine's side, typed. */
export const api = {
  song: () => invoke<View>('song'),
  edit: (edit: Edit) => invoke<View>('edit', { edit }),
  undo: () => invoke<View>('undo'),
  redo: () => invoke<View>('redo'),
  newSong: (demo: boolean) => invoke<View>('new_song', { demo }),
  openSong: (folder: string) => invoke<{ view: View; mended: string[] }>('open_song', { folder }),
  saveSong: () => invoke<boolean>('save_song'),
  saveSongAs: (folder: string) => invoke<View>('save_song_as', { folder }),
  importSample: (path: string) => invoke<string>('import_sample', { path }),
  play: () => invoke<void>('transport', { action: { type: 'play' } }),
  pause: () => invoke<void>('transport', { action: { type: 'pause' } }),
  stop: () => invoke<void>('transport', { action: { type: 'stop' } }),
  seek: (tick: Ticks) =>
    invoke<void>('transport', { action: { type: 'seek', tick: Math.round(tick) } }),
  metronome: (on: boolean) => invoke<void>('transport', { action: { type: 'metronome', on } }),
  panic: () => invoke<void>('transport', { action: { type: 'panic' } }),
  setMode: (pattern: Id | null) => invoke<View>('set_mode', { pattern }),
  note: (channel: Id, key: number, velocity: number, on: boolean) =>
    invoke<void>('note', { channel, key, velocity, on }),
  liveParam: (device: Id, param: string, value: number) =>
    invoke<void>('live', { change: { type: 'param', device, param, value } }),
  liveChannel: (id: Id, volume: number, pan: number) =>
    invoke<void>('live', { change: { type: 'channel', id, volume, pan } }),
  liveInsert: (id: Id, volume: number, pan: number) =>
    invoke<void>('live', { change: { type: 'insert', id, volume, pan } }),
  devices: () => invoke<DeviceInfo[]>('devices'),
  outputs: () => invoke<OutputInfo[]>('outputs'),
  settings: () => invoke<Settings>('settings'),
  setSettings: (settings: Settings) => invoke<AudioStatus>('set_settings', { settings }),
  exportWav: (path: string, depth: string, sampleRate: number) =>
    invoke<string[]>('export_wav', { path, depth, sampleRate }),
  browse: (folder: string) => invoke<Entry[]>('browse', { folder }),
  places: () => invoke<Entry[]>('places'),
  versions: () => invoke<VersionInfo[]>('versions'),
  makeVersion: (name: string) => invoke<View>('make_version', { name }),
  restoreVersion: (id: string) => invoke<View>('restore_version', { id }),
  compareVersion: (id: string | null) => invoke<View>('compare_version', { id }),
  onStatus: (handler: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>('status', (e) => handler(e.payload)),
};
