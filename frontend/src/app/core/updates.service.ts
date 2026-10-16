import { Injectable, signal } from '@angular/core';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { Update, check } from '@tauri-apps/plugin-updater';
import { api } from './api';

/** How often a newer release is looked for, while Sonevo runs. */
const EVERY = 6 * 60 * 60 * 1000;

/**
 * The version running, and a newer release once one is published on GitHub: downloaded, its
 * signature checked, installed, and Sonevo started again in it.
 */
@Injectable({ providedIn: 'root' })
export class Updates {
  readonly version = signal('');
  readonly available = signal<Update | null>(null);
  /** While the update downloads: the share done, from 0 to 1. */
  readonly progress = signal<number | null>(null);

  constructor() {
    void getVersion().then((v) => this.version.set(v));
    void this.check();
    setInterval(() => void this.check(), EVERY);
  }

  /** Whether a newer release was found; offline, or with none published, none is. */
  async check(): Promise<boolean> {
    try {
      this.available.set(await check());
    } catch (e) {
      console.warn('no release information', e);
    }
    return this.available() !== null;
  }

  /** Installs the update, the song's changes kept first so that they are offered back. */
  async install(): Promise<void> {
    const update = this.available();
    if (!update || this.progress() !== null) return;
    let total = 0;
    let done = 0;
    this.progress.set(0);
    try {
      await api.keepNow();
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        else if (event.event === 'Progress') {
          done += event.data.chunkLength;
          if (total) this.progress.set(Math.min(1, done / total));
        }
      });
      await relaunch();
    } finally {
      this.progress.set(null);
    }
  }
}
