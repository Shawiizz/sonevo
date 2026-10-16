import { Injectable, signal } from '@angular/core';
import { setAccent } from '../shared/surface';

export type Theme = 'dark' | 'light';

/** The accents offered, each as it shows on the dark theme and on the light one. */
export const ACCENTS: Record<string, [string, string]> = {
  light: ['#f3f1eb', '#111316'],
  violet: ['#8f8bf7', '#6d5ce8'],
  blue: ['#5aa9ff', '#1f6fd6'],
  green: ['#4fd6a0', '#0f8a5f'],
  orange: ['#ffad5a', '#c76a12'],
  pink: ['#ff7ab8', '#c83a7f'],
};

/** The theme shown: the one chosen, or the system's, followed as it changes. */
@Injectable({ providedIn: 'root' })
export class Themes {
  /** Told again whenever the accent changes too, for the canvases to be drawn again. */
  readonly shown = signal<Theme>('dark', { equal: () => false });
  private chosen: string | null = null;
  private accent: string | null = null;
  private readonly system = window.matchMedia('(prefers-color-scheme: light)');

  constructor() {
    this.system.addEventListener('change', () => this.apply());
  }

  set(theme: string | null): void {
    this.chosen = theme;
    this.apply();
  }

  setAccent(accent: string | null): void {
    this.accent = accent && ACCENTS[accent] ? accent : null;
    const colours = this.accent ? ACCENTS[this.accent] : null;
    setAccent(
      colours ? parseInt(colours[0].slice(1), 16) : null,
      colours ? parseInt(colours[1].slice(1), 16) : null,
    );
    this.apply();
  }

  private apply(): void {
    const theme: Theme =
      this.chosen === 'light' || this.chosen === 'dark'
        ? this.chosen
        : this.system.matches
          ? 'light'
          : 'dark';
    document.documentElement.dataset['theme'] = theme;
    const style = document.documentElement.style;
    const colours = this.accent ? ACCENTS[this.accent] : null;
    if (colours) style.setProperty('--s-accent', colours[theme === 'light' ? 1 : 0]);
    else style.removeProperty('--s-accent');
    this.shown.set(theme);
  }
}
