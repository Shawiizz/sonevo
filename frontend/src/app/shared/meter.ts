import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
} from '@angular/core';
import { Studio } from '../core/studio.service';

const FLOOR = -60;
const TOP = 6;

export function gainToDb(gain: number): number {
  return gain > 0 ? 20 * Math.log10(gain) : -Infinity;
}

/**
 * A level meter: the loudest sample read, falling back slowly; red past 0 dB.
 */
@Component({
  selector: 's-meter',
  template: `<div class="bar" [style.height.%]="height()" [class.over]="over()"></div>`,
  styles: `
    :host {
      position: relative;
      display: block;
      width: 6px;
      border-radius: 2px;
      background: var(--s-void);
      overflow: hidden;
    }
    .bar {
      position: absolute;
      bottom: 0;
      left: 0;
      right: 0;
      background: linear-gradient(to top, var(--s-play) 0%, var(--s-play) 70%, var(--s-warn) 90%);
      background-size: 100% var(--h, 100%);
      transition: height 60ms linear;
    }
    .bar.over {
      background: var(--s-danger);
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Meter {
  readonly peak = input(0);
  private readonly shown = signal(0);
  protected readonly height = computed(() => {
    const db = gainToDb(this.shown());
    return Math.max(0, Math.min(100, ((db - FLOOR) / (TOP - FLOOR)) * 100));
  });
  protected readonly over = computed(() => this.shown() > 1);

  private readonly studio = inject(Studio);

  constructor() {
    effect(() => {
      // each status read, even with the same peak, lets the bar fall a little
      this.studio.status();
      const peak = this.peak();
      this.shown.update((was) => Math.max(peak, was * 0.82));
    });
  }
}
