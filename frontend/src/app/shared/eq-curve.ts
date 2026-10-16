import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';

type Shape = 'peak' | 'lowShelf' | 'highShelf';

/** A biquad's coefficients as the engine makes them, the first output's one divided out. */
export function biquad(
  shape: Shape,
  frequency: number,
  gain: number,
  q: number,
  rate: number,
): number[] {
  const a = 10 ** (gain / 40);
  const w = (2 * Math.PI * Math.min(Math.max(frequency, 10), rate * 0.49)) / rate;
  const [sin, cos] = [Math.sin(w), Math.cos(w)];
  const alpha = sin / (2 * Math.max(q, 0.05));
  const s = 2 * Math.sqrt(a) * alpha;
  const c =
    shape === 'peak'
      ? [1 + alpha * a, -2 * cos, 1 - alpha * a, 1 + alpha / a, -2 * cos, 1 - alpha / a]
      : shape === 'lowShelf'
        ? [
            a * (a + 1 - (a - 1) * cos + s),
            2 * a * (a - 1 - (a + 1) * cos),
            a * (a + 1 - (a - 1) * cos - s),
            a + 1 + (a - 1) * cos + s,
            -2 * (a - 1 + (a + 1) * cos),
            a + 1 + (a - 1) * cos - s,
          ]
        : [
            a * (a + 1 + (a - 1) * cos + s),
            -2 * a * (a - 1 + (a + 1) * cos),
            a * (a + 1 + (a - 1) * cos - s),
            a + 1 - (a - 1) * cos + s,
            2 * (a - 1 - (a + 1) * cos),
            a + 1 - (a - 1) * cos - s,
          ];
  return [c[0] / c[3], c[1] / c[3], c[2] / c[3], c[4] / c[3], c[5] / c[3]];
}

/** How much the biquad `c` lifts or cuts `frequency`, in dB. */
export function response(c: number[], frequency: number, rate: number): number {
  const w = (2 * Math.PI * frequency) / rate;
  const power = (x0: number, x1: number, x2: number) =>
    (x0 + x1 * Math.cos(w) + x2 * Math.cos(2 * w)) ** 2 +
    (x1 * Math.sin(w) + x2 * Math.sin(2 * w)) ** 2;
  return 10 * Math.log10(power(c[0], c[1], c[2]) / power(1, c[3], c[4]));
}

const RATE = 48000;
const [LOW, HIGH, RANGE] = [20, 20000, 18];

/** The three bands of the EQ together, as a curve from 20 Hz to 20 kHz, 18 dB up and down. */
@Component({
  selector: 's-eq-curve',
  template: `
    <svg viewBox="0 0 200 60" preserveAspectRatio="none">
      @for (f of grid; track f) {
        <line class="grid" [attr.x1]="f" [attr.x2]="f" y1="0" y2="60" />
      }
      <line class="zero" x1="0" x2="200" y1="30" y2="30" />
      <path [attr.d]="path()" />
    </svg>
  `,
  styles: `
    :host {
      display: block;
    }
    svg {
      display: block;
      width: 100%;
      height: 72px;
      border-radius: var(--s-radius);
      background: var(--s-void);
    }
    path {
      fill: none;
      stroke: var(--s-accent);
      stroke-width: 1.5;
      vector-effect: non-scaling-stroke;
    }
    .zero,
    .grid {
      stroke: var(--s-line);
      stroke-width: 1;
      vector-effect: non-scaling-stroke;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class EqCurve {
  /** The EQ's settings by name. */
  readonly values = input.required<Record<string, number>>();
  /** Where 100 Hz, 1 kHz and 10 kHz are along it. */
  protected readonly grid = [100, 1000, 10000].map(
    (f) => (Math.log(f / LOW) / Math.log(HIGH / LOW)) * 200,
  );
  protected readonly path = computed(() => {
    const v = this.values();
    const bands = [
      biquad('lowShelf', v['lowFrequency'], v['lowGain'], 0.707, RATE),
      biquad('peak', v['midFrequency'], v['midGain'], v['midQ'], RATE),
      biquad('highShelf', v['highFrequency'], v['highGain'], 0.707, RATE),
    ];
    const points: string[] = [];
    for (let i = 0; i <= 100; i++) {
      const f = LOW * (HIGH / LOW) ** (i / 100);
      const db = bands.reduce((sum, c) => sum + response(c, f, RATE), 0);
      const y = 30 - (Math.max(-RANGE, Math.min(RANGE, db)) / RANGE) * 29;
      points.push(`${i === 0 ? 'M' : 'L'}${(i * 2).toFixed(1)},${y.toFixed(2)}`);
    }
    return points.join(' ');
  });
}
