import { ChangeDetectionStrategy, Component, computed, input, output, signal } from '@angular/core';

export function toNormal(value: number, min: number, max: number, log: boolean): number {
  const v = Math.min(max, Math.max(min, value));
  const p = log && min > 0 ? Math.log(v / min) / Math.log(max / min) : (v - min) / (max - min);
  return Number.isFinite(p) ? p : 0;
}

export function fromNormal(position: number, min: number, max: number, log: boolean): number {
  const p = Math.min(1, Math.max(0, position));
  return log && min > 0 ? min * Math.pow(max / min, p) : min + p * (max - min);
}

export function formatValue(value: number, unit: string): string {
  if (unit === 's') {
    return value < 1 ? `${Math.round(value * 1000)} ms` : `${value.toFixed(2)} s`;
  }
  if (unit === 'Hz') {
    return value >= 1000
      ? `${(value / 1000).toFixed(value >= 10000 ? 1 : 2)} kHz`
      : `${value < 10 ? value.toFixed(2) : Math.round(value)} Hz`;
  }
  const abs = Math.abs(value);
  const figures = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  const shown = value.toFixed(figures);
  return unit ? `${shown} ${unit}` : shown;
}

/**
 * A knob: dragged up and down, finer with Shift; the wheel steps it; a double click brings it
 * back to its default. `live` as it turns, `changed` when it is let go of.
 */
@Component({
  selector: 's-knob',
  template: `
    <svg
      [attr.width]="size()"
      [attr.height]="size()"
      viewBox="0 0 40 40"
      (pointerdown)="down($event)"
      (dblclick)="reset()"
      (wheel)="wheel($event)"
    >
      <path class="track" [attr.d]="arc(0, 1)" />
      <path class="fill" [attr.d]="fillArc()" [style.stroke]="color()" />
      <line class="needle" x1="20" y1="20" [attr.x2]="needle().x" [attr.y2]="needle().y" />
    </svg>
    @if (label()) {
      <span class="label">{{ label() }}</span>
    }
    <span class="value s-mono">{{ text() }}</span>
  `,
  styles: `
    :host {
      display: inline-flex;
      flex-direction: column;
      align-items: center;
      gap: 2px;
      min-width: 44px;
      cursor: ns-resize;
      touch-action: none;
    }
    svg {
      display: block;
    }
    .track,
    .fill {
      fill: none;
      stroke-width: 3.5;
      stroke-linecap: round;
    }
    .track {
      stroke: var(--s-line-strong);
    }
    .needle {
      stroke: var(--s-text);
      stroke-width: 2;
      stroke-linecap: round;
    }
    .label {
      max-width: 72px;
      overflow: hidden;
      color: var(--s-text-2);
      font-size: 11px;
      white-space: nowrap;
      text-overflow: ellipsis;
    }
    .value {
      color: var(--s-text-3);
      font-size: 10px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Knob {
  readonly value = input.required<number>();
  readonly min = input(0);
  readonly max = input(1);
  readonly default = input(0);
  readonly log = input(false);
  /** Steps of one, for a choice among a few. */
  readonly whole = input(false);
  readonly unit = input('');
  readonly label = input('');
  readonly size = input(36);
  readonly color = input('var(--s-accent)');
  readonly format = input<((value: number) => string) | null>(null);

  readonly live = output<number>();
  readonly changed = output<number>();

  private readonly dragging = signal<number | null>(null);
  protected readonly shown = computed(() => this.dragging() ?? this.value());
  protected readonly position = computed(() =>
    toNormal(this.shown(), this.min(), this.max(), this.log()),
  );
  protected readonly text = computed(() => {
    const format = this.format();
    return format ? format(this.shown()) : formatValue(this.shown(), this.unit());
  });

  private readonly origin = computed(() =>
    this.min() < 0 && this.max() > 0 && !this.log()
      ? toNormal(0, this.min(), this.max(), false)
      : 0,
  );

  protected readonly fillArc = computed(() => {
    const [a, b] = [this.origin(), this.position()].sort((x, y) => x - y);
    return this.arc(a, b);
  });

  protected readonly needle = computed(() => {
    const angle = this.angle(this.position());
    return { x: 20 + Math.cos(angle) * 11, y: 20 + Math.sin(angle) * 11 };
  });

  private angle(p: number): number {
    return ((135 + p * 270) * Math.PI) / 180;
  }

  protected arc(from: number, to: number): string {
    const r = 16;
    const a = this.angle(from);
    const b = this.angle(Math.max(to, from + 0.0001));
    const large = (to - from) * 270 > 180 ? 1 : 0;
    return `M ${20 + Math.cos(a) * r} ${20 + Math.sin(a) * r} A ${r} ${r} 0 ${large} 1 ${20 + Math.cos(b) * r} ${20 + Math.sin(b) * r}`;
  }

  private settle(value: number): number {
    const v = Math.min(this.max(), Math.max(this.min(), value));
    return this.whole() ? Math.round(v) : v;
  }

  protected down(event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    const target = event.currentTarget as Element;
    target.setPointerCapture(event.pointerId);
    let position = this.position();
    let last = event.clientY;
    const move = (e: PointerEvent) => {
      const pixels = e.shiftKey ? 800 : 200;
      position = Math.min(1, Math.max(0, position + (last - e.clientY) / pixels));
      last = e.clientY;
      const value = this.settle(fromNormal(position, this.min(), this.max(), this.log()));
      if (value !== this.dragging()) {
        this.dragging.set(value);
        this.live.emit(value);
      }
    };
    const up = () => {
      target.removeEventListener('pointermove', move as EventListener);
      target.removeEventListener('pointerup', up);
      const value = this.dragging();
      this.dragging.set(null);
      if (value !== null && value !== this.value()) this.changed.emit(value);
    };
    target.addEventListener('pointermove', move as EventListener);
    target.addEventListener('pointerup', up);
  }

  protected reset(): void {
    this.live.emit(this.default());
    this.changed.emit(this.default());
  }

  protected wheel(event: WheelEvent): void {
    event.preventDefault();
    const step = this.whole()
      ? 1 / Math.max(1, this.max() - this.min())
      : event.shiftKey
        ? 0.005
        : 0.02;
    const position = this.position() + (event.deltaY < 0 ? step : -step);
    const value = this.settle(fromNormal(position, this.min(), this.max(), this.log()));
    this.live.emit(value);
    this.changed.emit(value);
  }
}
