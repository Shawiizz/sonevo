import { Injectable, signal } from '@angular/core';

export type Theme = 'dark' | 'light';

/** The theme shown: the one chosen, or the system's, followed as it changes. */
@Injectable({ providedIn: 'root' })
export class Themes {
  readonly shown = signal<Theme>('dark');
  private chosen: string | null = null;
  private readonly system = window.matchMedia('(prefers-color-scheme: light)');

  constructor() {
    this.system.addEventListener('change', () => this.apply());
  }

  set(theme: string | null): void {
    this.chosen = theme;
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
    this.shown.set(theme);
  }
}
