import {
  ChangeDetectionStrategy,
  Component,
  effect,
  inject,
  signal,
  untracked,
} from '@angular/core';
import { Studio } from '../core/studio.service';

/** How far a bar falls each status, from 0 to 1: it rises at once, falls back slowly. */
const FALL = 0.025;
/** Where 100 Hz, 1 kHz and 10 kHz are along the bands, from 30 Hz to 16 kHz. */
const MARKS = [
  { label: '100', at: Math.log(100 / 30) / Math.log(16000 / 30) },
  { label: '1k', at: Math.log(1000 / 30) / Math.log(16000 / 30) },
  { label: '10k', at: Math.log(10000 / 30) / Math.log(16000 / 30) },
];

/** The master's spectrum: the lows on the left, the highs on the right. */
@Component({
  selector: 's-spectrum',
  template: `
    <svg viewBox="0 0 100 100" preserveAspectRatio="none">
      <defs>
        <linearGradient id="prism" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="100" y2="0">
          <stop offset="0" stop-color="#ff5442" />
          <stop offset="0.5" stop-color="#a6e25c" />
          <stop offset="1" stop-color="#8f7bff" />
        </linearGradient>
      </defs>
      @for (v of shown(); track $index) {
        <rect
          [attr.x]="($index * 100) / shown().length + 0.3"
          [attr.y]="100 - v * 100"
          [attr.width]="100 / shown().length - 0.6"
          [attr.height]="v * 100"
        />
      }
    </svg>
    @for (m of marks; track m.label) {
      <span class="mark s-mono" [style.left.%]="m.at * 100">{{ m.label }}</span>
    }
  `,
  styles: `
    :host {
      position: relative;
      display: block;
      padding-bottom: 14px;
    }
    svg {
      display: block;
      width: 100%;
      height: 100%;
      border-radius: 4px;
      background: var(--s-void);
    }
    rect {
      fill: url(#prism);
      opacity: 0.85;
    }
    .mark {
      position: absolute;
      bottom: 0;
      transform: translateX(-50%);
      color: var(--s-text-3);
      font-size: 9px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Spectrum {
  private readonly studio = inject(Studio);
  protected readonly marks = MARKS;
  protected readonly shown = signal<number[]>([]);

  constructor() {
    effect(() => {
      const heard = this.studio.status().spectrum;
      const before = untracked(this.shown);
      this.shown.set(heard.map((v, i) => Math.max(v, (before[i] ?? 0) - FALL)));
    });
  }
}
