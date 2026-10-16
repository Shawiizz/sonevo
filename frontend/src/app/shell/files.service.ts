import { inject, Injectable, signal } from '@angular/core';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Studio } from '../core/studio.service';

/** The songs to start from, a style each: the demo is house. */
export const TEMPLATES = [
  { id: 'demo', label: 'House' },
  { id: 'trap', label: 'Trap' },
  { id: 'lofi', label: 'Lo-fi' },
  { id: 'techno', label: 'Techno' },
];

/** New, open, save, export: what the File menu and its shortcuts do. */
@Injectable({ providedIn: 'root' })
export class Files {
  private readonly studio = inject(Studio);
  private readonly i18n = inject(I18n);
  readonly busy = signal<string | null>(null);
  readonly notice = signal<string | null>(null);

  private say(text: string, seconds = 4): void {
    this.notice.set(text);
    setTimeout(() => this.notice.set(null), seconds * 1000);
  }

  /** Whether the song changed since saved may be left: asked if it did. */
  async leave(): Promise<boolean> {
    if (!this.studio.view()?.dirty) return true;
    const t = this.i18n.t;
    const left = await ask(t('The song has changes not saved. Leave them?'), {
      title: 'Sonevo',
      kind: 'warning',
      okLabel: t('Leave them'),
      cancelLabel: t('Cancel'),
    });
    if (left) await api.leave();
    return left;
  }

  /** A new song: empty, or from the template named (`demo` is the demo's). */
  async newSong(template: string | null = null): Promise<void> {
    if (!(await this.leave())) return;
    this.studio.show(await api.newSong(template));
  }

  async open(folder?: string): Promise<void> {
    if (!(await this.leave())) return;
    const chosen =
      folder ??
      ((await open({ directory: true, title: this.i18n.t('Open a song (a .sonevo folder)') })) as
        string | null);
    if (!chosen) return;
    try {
      const opened = await api.openSong(chosen);
      this.studio.show(opened.view);
      if (opened.mended.length) this.say(opened.mended.join(' · '));
    } catch (e) {
      this.studio.fail(e);
    }
  }

  async save(): Promise<void> {
    try {
      if (await api.saveSong()) {
        this.studio.show(await api.song());
        this.say(this.i18n.t('Saved'));
      } else {
        await this.saveAs();
      }
    } catch (e) {
      this.studio.fail(e);
    }
  }

  async saveAs(): Promise<void> {
    const name = this.studio.song()?.name ?? 'Song';
    const chosen = await save({
      title: this.i18n.t('Save the song as'),
      defaultPath: `${name}.sonevo`,
    });
    if (!chosen) return;
    try {
      this.studio.show(await api.saveSongAs(chosen));
      this.say(this.i18n.t('Saved'));
    } catch (e) {
      this.studio.fail(e);
    }
  }

  readonly exporting = signal(false);

  async importMidi(): Promise<void> {
    const chosen = (await open({
      title: this.i18n.t('Import a MIDI file'),
      filters: [{ name: 'MIDI', extensions: ['mid', 'midi'] }],
    })) as string | null;
    if (chosen) await this.studio.importMidi(chosen);
  }

  /** The pattern chosen, or the whole song when `song`, as a MIDI file. */
  async exportMidi(song = false): Promise<void> {
    const pattern = this.studio.pattern();
    const name = song ? this.studio.song()?.name : pattern?.name;
    if (!name) return;
    const chosen = await save({
      title: this.i18n.t(song ? 'Export the song as MIDI' : 'Export the pattern as MIDI'),
      defaultPath: `${name}.mid`,
      filters: [{ name: 'MIDI', extensions: ['mid'] }],
    });
    if (!chosen) return;
    try {
      await api.exportMidi(song ? null : (pattern?.id ?? null), chosen);
      this.say(this.i18n.t('Exported'));
    } catch (e) {
      this.studio.fail(e);
    }
  }

  exportWav(): void {
    this.exporting.set(true);
  }

  async export(
    path: string,
    part: 'song' | 'loop' | 'stems',
    format: 'wav' | 'flac',
    depth: string,
    rate: number,
  ): Promise<void> {
    this.busy.set(this.i18n.t('Exporting…'));
    try {
      const done = await api.exportAudio(path, part, format, depth, rate);
      const t = this.i18n.t;
      if (done.missing.length) this.say(done.missing.join(' · '));
      else if (done.loudness !== null && done.peak !== null)
        this.say(
          t(
            'Exported: {lufs} LUFS, peaking at {peak} dB. Streaming services play songs at about -14 LUFS.',
            {
              lufs: done.loudness.toFixed(1),
              peak: done.peak.toFixed(1),
            },
          ),
          9,
        );
      else this.say(t('Exported'));
    } catch (e) {
      this.studio.fail(e);
    } finally {
      this.busy.set(null);
    }
  }

  /** Sounds chosen on the disk, copied into the song: the names it knows them by, and theirs. */
  async importSounds(): Promise<{ sample: string; label: string }[]> {
    const chosen = await open({
      multiple: true,
      title: this.i18n.t('Choose sounds'),
      filters: [{ name: 'Audio', extensions: ['wav', 'flac', 'mp3', 'ogg', 'aif', 'aiff'] }],
    });
    const paths = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
    return this.import(paths);
  }

  async import(paths: string[]): Promise<{ sample: string; label: string }[]> {
    const imported = [];
    for (const path of paths) {
      try {
        const sample = await api.importSample(path);
        imported.push({ sample, label: soundName(path) });
      } catch (e) {
        this.studio.fail(e);
      }
    }
    return imported;
  }
}

export function soundName(path: string): string {
  const file = path.split(/[\\/]/).pop() ?? path;
  return file.replace(/\.[^.]+$/, '');
}
