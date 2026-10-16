import {
  afterNextRender,
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  ElementRef,
  inject,
  OnDestroy,
  signal,
  viewChild,
} from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import {
  builtinName,
  chordKeys,
  degreeChord,
  inScale,
  isBlack,
  keyName,
  NAMES,
  Note,
  PPQ,
  PROGRESSIONS,
  SCALES,
  STEP,
} from '../core/models';
import { Studio } from '../core/studio.service';
import { Themes } from '../core/theme';
import { Icon } from '../shared/icon';
import { MenuItem, Menus } from '../shared/menu';
import { hex, palette, Surface } from '../shared/surface';

const KEYS = 64;
let copied: Note[] = [];
const RULER = 24;
const VELOCITY = 64;
const EDGE = 6;

type Drag =
  | { kind: 'move'; from: { tick: number; key: number }; original: Note[]; picked: number[] }
  | { kind: 'resize'; from: number; original: Note[]; picked: number[] }
  | { kind: 'select'; x0: number; y0: number; x1: number; y1: number; keep: Set<number> }
  | { kind: 'erase' }
  | { kind: 'velocity' }
  | { kind: 'scroll'; x: number; y: number; scrollX: number; top: number };

/**
 * The piano roll: the chosen channel's notes in the pattern edited, time across, keys up the
 * side; the other channels' notes faintly behind.
 */
@Component({
  selector: 'app-piano-roll',
  imports: [Icon],
  template: `
    <div class="s-panel-head">
      <span class="swatch" [style.background]="studio.channel()?.color"></span>
      <span>{{ studio.channel()?.name }}</span>
      <span class="in">{{ t('in') }} {{ studio.pattern()?.name }}</span>
      <button
        class="s-bar-btn write"
        [class.on]="studio.writing()"
        (click)="write()"
        [title]="t('Write the keys played into the pattern, as it goes round')"
      >
        <s-icon name="record" [filled]="true" [size]="10" /> {{ t('Write') }}
      </button>
      <button
        class="s-bar-btn"
        (click)="toolsMenu($event)"
        [title]="t('Change the notes chosen, or all of them')"
      >
        {{ t('Tools') }} <s-icon name="down" [size]="12" />
      </button>
      <span class="s-grow"></span>
      <select
        class="scale"
        [value]="root()"
        (change)="root.set(+$any($event.target).value)"
        [title]="t('The scale lit: its notes stand out')"
      >
        @for (n of names; track $index) {
          <option [value]="$index" [selected]="$index === root()">{{ n }}</option>
        }
      </select>
      <select class="scale" [value]="scale()" (change)="scale.set(+$any($event.target).value)">
        <option [value]="-1" [selected]="scale() < 0">{{ t('No scale') }}</option>
        @for (s of scales; track s.name) {
          <option [value]="$index" [selected]="$index === scale()">{{ t(s.name) }}</option>
        }
      </select>
      @if (scale() >= 0) {
        <button
          class="s-bar-btn"
          [class.on]="keepToScale()"
          (click)="keepToScale.set(!keepToScale())"
          [title]="t('Notes drawn or moved stay in the scale chosen')"
        >
          {{ t('In the scale only') }}
        </button>
      }
      <div class="s-seg" role="radiogroup" [title]="t('What a click draws')">
        @for (c of chords; track c.notes) {
          <button
            role="radio"
            [attr.aria-checked]="chord() === c.notes"
            (click)="chord.set(c.notes)"
          >
            {{ t(c.label) }}
          </button>
        }
      </div>
      <div class="s-seg" role="radiogroup" [title]="t('Notes snap to')">
        @for (s of snaps; track s.ticks) {
          <button role="radio" [attr.aria-checked]="snap() === s.ticks" (click)="snap.set(s.ticks)">
            {{ s.label }}
          </button>
        }
      </div>
      <button class="s-bar-btn" (click)="zoom(1.25)" [title]="t('Zoom in')">+</button>
      <button class="s-bar-btn" (click)="zoom(0.8)" [title]="t('Zoom out')">−</button>
      <button class="s-bar-btn" (click)="lengthen()" [title]="t('Make the pattern a bar longer')">
        <s-icon name="plus" /> {{ t('Bar') }}
      </button>
    </div>
    <div
      class="canvas"
      #host
      tabindex="0"
      (pointerdown)="down($event)"
      (pointermove)="move($event)"
      (pointerup)="up($event)"
      (wheel)="wheel($event)"
      (contextmenu)="$event.preventDefault()"
      (keydown)="key($event)"
      [style.cursor]="cursor()"
    ></div>
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      height: 100%;
      min-height: 0;
    }
    .swatch {
      width: 10px;
      height: 10px;
      border-radius: 3px;
    }
    .s-panel-head {
      overflow-x: auto;
      scrollbar-width: none;
      > *,
      .s-seg button {
        white-space: nowrap;
      }
      > :not(.s-grow) {
        flex: none;
      }
    }
    .in {
      color: var(--s-text-3);
      font-weight: 400;
    }
    .write.on {
      color: var(--s-record);
      background: color-mix(in srgb, var(--s-record) 14%, transparent);
    }
    .scale {
      height: 26px;
      padding: 0 6px;
      border: 1px solid var(--s-line);
      border-radius: var(--s-radius);
      background: var(--s-void);
      color: var(--s-text-2);
      font: inherit;
      font-size: 12px;
    }
    .canvas {
      flex: 1;
      min-height: 0;
      position: relative;
      overflow: hidden;
      outline: none;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PianoRoll implements OnDestroy {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly menus = inject(Menus);
  private readonly themes = inject(Themes);
  private readonly host = viewChild.required<ElementRef<HTMLElement>>('host');
  private surface: Surface | null = null;

  protected readonly names = NAMES;
  protected readonly scales = SCALES;
  /** The scale lit, by its index in SCALES (-1 for none), and its root. */
  protected readonly scale = signal(-1);
  protected readonly root = signal(9);
  protected readonly keepToScale = signal(false);

  protected readonly chords = [
    { label: 'Note', notes: 1 },
    { label: 'Chord', notes: 3 },
    { label: 'Chord 7', notes: 4 },
  ];
  /** How many notes a click draws: one, or a chord. */
  protected readonly chord = signal(1);

  protected readonly snaps = [
    { label: '1/32', ticks: STEP / 2 },
    { label: '1/16', ticks: STEP },
    { label: '1/8', ticks: STEP * 2 },
    { label: '1/4', ticks: PPQ },
    { label: '—', ticks: 1 },
  ];
  protected readonly snap = signal(STEP);
  protected readonly cursor = signal('default');

  private scrollX = 0;
  private perTick = 64 / PPQ;
  private top = 84;
  private rowHeight = 14;
  private selected = new Set<number>();
  private working: Note[] | null = null;
  private drag: Drag | null = null;
  private lastLength = PPQ / 2;
  private sounding: number | null = null;

  private readonly notes = computed(() => {
    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    return pattern && channel ? this.studio.notes(pattern.id, channel.id) : [];
  });

  constructor() {
    afterNextRender(() => {
      this.surface = new Surface(
        this.host().nativeElement,
        (s) => this.paint(s),
        (s) => this.paintOver(s),
      );
      void this.surface.start();
      this.centre();
    });
    effect(() => {
      this.notes();
      this.studio.pattern();
      this.themes.shown();
      this.snap();
      this.scale();
      this.root();
      this.working = null;
      this.selected = new Set([...this.selected].filter((i) => i < this.notes().length));
      this.surface?.redraw();
    });
    effect(() => {
      this.studio.status();
      if (this.studio.view()?.patternMode) this.surface?.redrawOver();
    });
  }

  ngOnDestroy(): void {
    this.surface?.destroy();
  }

  /** The notes in view, or middle C. */
  private centre(): void {
    const notes = this.notes();
    const keys = notes.map((n) => n.key);
    const [low, high] = keys.reduce(([a, b], k) => [Math.min(a, k), Math.max(b, k)], [127, 0]);
    const middle = keys.length ? Math.round((low + high) / 2) : 60;
    const rows = Math.floor(
      (this.host().nativeElement.clientHeight - RULER - VELOCITY) / this.rowHeight,
    );
    this.top = Math.min(127, middle + Math.floor(rows / 2));
  }

  /** `key`, or the nearest of the scale's when notes keep to it: the way `towards` goes first. */
  private inKey(key: number, towards = -1): number {
    const scale = SCALES[this.scale()]?.steps;
    if (!this.keepToScale() || !scale || inScale(key, this.root(), scale)) return key;
    for (let off = 1; off < 12; off++) {
      for (const k of towards > 0 ? [key + off, key - off] : [key - off, key + off]) {
        if (k >= 0 && k <= 127 && inScale(k, this.root(), scale)) return k;
      }
    }
    return key;
  }

  private get current(): Note[] {
    return this.working ?? this.notes();
  }

  private x(tick: number): number {
    return KEYS + (tick - this.scrollX) * this.perTick;
  }

  private tickAt(x: number): number {
    return this.scrollX + (x - KEYS) / this.perTick;
  }

  private y(key: number): number {
    return RULER + (this.top - key) * this.rowHeight;
  }

  private keyAt(y: number): number {
    return Math.max(0, Math.min(127, this.top - Math.floor((y - RULER) / this.rowHeight)));
  }

  private snapped(tick: number): number {
    const snap = this.snap();
    return Math.max(0, Math.floor(tick / snap) * snap);
  }

  private paint(s: Surface): void {
    const g = s.shapes;
    const { width, height } = s;
    const length = this.studio.pattern()?.length ?? this.studio.bar();
    const bottom = height - VELOCITY;
    const color = hex(this.studio.channel()?.color ?? '#8f8bf7');
    const bar = this.studio.bar();
    const beat = PPQ;

    const scale = SCALES[this.scale()]?.steps;
    for (let key = this.top; key >= 0; key--) {
      const y = this.y(key);
      if (y > bottom) break;
      const shade = scale
        ? inScale(key, this.root(), scale)
          ? key % 12 === this.root()
            ? palette().scaleRoot
            : palette().scaleIn
          : palette().scaleOut
        : isBlack(key)
          ? palette().rowBlack
          : palette().rowWhite;
      g.rect(KEYS, y, width - KEYS, this.rowHeight).fill(shade);
      if (key % 12 === 0)
        g.rect(KEYS, y + this.rowHeight - 1, width - KEYS, 1).fill(palette().gridOctave);
    }
    const end = this.x(length);
    if (end < width)
      g.rect(end, RULER, width - end, bottom - RULER).fill({ color: palette().shade, alpha: 0.45 });

    const first = Math.max(0, Math.floor(this.scrollX / STEP) * STEP);
    for (let tick = first; this.x(tick) < width; tick += STEP) {
      const x = this.x(tick);
      if (x < KEYS) continue;
      const strong =
        tick % bar === 0
          ? palette().gridBar
          : tick % beat === 0
            ? palette().gridBeat
            : palette().gridStep;
      if (tick % beat !== 0 && this.perTick * STEP < 8) continue;
      g.rect(Math.round(x), RULER, 1, bottom - RULER).fill(strong);
    }

    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    const song = this.studio.song();
    if (pattern && song) {
      for (const other of song.channels) {
        if (other.id === channel?.id) continue;
        for (const n of this.studio.notes(pattern.id, other.id)) {
          const x = this.x(n.start);
          if (x > width || this.x(n.start + n.length) < KEYS) continue;
          const y = this.y(n.key);
          if (y < RULER - this.rowHeight || y > bottom) continue;
          g.rect(x, y + 1, n.length * this.perTick, this.rowHeight - 2).fill({
            color: hex(other.color),
            alpha: 0.16,
          });
        }
      }
    }

    const notes = this.current;
    notes.forEach((n, i) => {
      const x = this.x(n.start);
      const w = Math.max(3, n.length * this.perTick);
      if (x > width || x + w < KEYS) return;
      const y = this.y(n.key);
      if (y < RULER - this.rowHeight || y > bottom) return;
      const picked = this.selected.has(i);
      g.roundRect(x, y + 1, w - 1, this.rowHeight - 2, 3).fill({
        color,
        alpha: 0.35 + n.velocity * 0.65,
      });
      if (picked)
        g.roundRect(x, y + 1, w - 1, this.rowHeight - 2, 3).stroke({
          width: 1.5,
          color: palette().picked,
        });
      if (w > 34 && this.rowHeight >= 12)
        s.text(keyName(n.key), x + 4, y + 1, palette().onColor, 10);
    });

    g.rect(0, bottom, width, VELOCITY).fill(palette().void);
    g.rect(0, bottom, width, 1).fill(palette().gridBeat);
    s.text(this.t('Velocity'), 8, bottom + 6, palette().text3, 10);
    notes.forEach((n, i) => {
      const x = this.x(n.start);
      if (x < KEYS || x > width) return;
      const h = n.velocity * (VELOCITY - 12);
      const picked = this.selected.has(i);
      g.rect(x, height - 4 - h, 2, h).fill(picked ? palette().picked : color);
      g.circle(x + 1, height - 4 - h, 3).fill(picked ? palette().picked : color);
    });

    g.rect(0, RULER, KEYS - 1, bottom - RULER).fill(palette().keyWhite);
    for (let key = this.top; key >= 0; key--) {
      const y = this.y(key);
      if (y > bottom) break;
      const lit = this.sounding === key;
      if (isBlack(key)) {
        g.rect(0, y, KEYS * 0.6, this.rowHeight).fill(lit ? color : palette().onColor);
      } else {
        if (lit) g.rect(0, y, KEYS - 1, this.rowHeight).fill(color);
        // where two white keys meet without a black one between
        if (key % 12 === 0 || key % 12 === 5)
          g.rect(0, y + this.rowHeight - 1, KEYS - 1, 1).fill(palette().text3);
      }
      if (key % 12 === 0) s.text(keyName(key), KEYS - 5, y + 1, palette().keyLabel, 10, 1);
    }

    g.rect(0, 0, width, RULER).fill(palette().base);
    g.rect(0, RULER - 1, width, 1).fill(palette().gridBeat);
    for (let tick = Math.floor(this.scrollX / bar) * bar; this.x(tick) < width; tick += bar) {
      const x = this.x(tick);
      if (x < KEYS - 1) continue;
      g.rect(x, 6, 1, RULER - 6).fill(palette().gridBar);
      s.text(String(tick / bar + 1), x + 4, 5, palette().text2, 11);
    }
    if (end < width) g.rect(end - 1, 0, 2, bottom).fill({ color: palette().accent, alpha: 0.7 });
  }

  private paintOver(s: Surface): void {
    const length = this.studio.pattern()?.length ?? this.studio.bar();
    const bottom = s.height - VELOCITY;
    const pattern = this.studio.pattern();
    if (this.drag?.kind === 'select') {
      const d = this.drag;
      const [x0, x1] = [Math.min(d.x0, d.x1), Math.max(d.x0, d.x1)];
      const [y0, y1] = [Math.min(d.y0, d.y1), Math.max(d.y0, d.y1)];
      s.over
        .rect(x0, y0, x1 - x0, y1 - y0)
        .fill({ color: palette().accent, alpha: 0.12 })
        .stroke({ width: 1, color: palette().accent });
    }

    const view = this.studio.view();
    if (view?.patternMode && view.patternMode === pattern?.id && this.studio.status().playing) {
      const at = this.studio.status().position % Math.max(1, length);
      const x = this.x(at);
      if (x >= KEYS) s.over.rect(x, 0, 1.5, bottom).fill(palette().play);
    }
  }

  /** The note under (x, y), the last drawn first; and whether it is its end. */
  private hit(x: number, y: number): { index: number; edge: boolean } | null {
    const notes = this.current;
    const key = this.keyAt(y);
    for (let i = notes.length - 1; i >= 0; i--) {
      const n = notes[i];
      if (n.key !== key) continue;
      const left = this.x(n.start);
      const right = this.x(n.start + n.length);
      if (x >= left && x <= right)
        return { index: i, edge: right - x <= EDGE && right - left > EDGE * 2 };
    }
    return null;
  }

  private point(event: PointerEvent | WheelEvent): { x: number; y: number } {
    const box = this.host().nativeElement.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }

  private bottom(): number {
    return this.host().nativeElement.clientHeight - VELOCITY;
  }

  protected down(event: PointerEvent): void {
    const { x, y } = this.point(event);
    const host = this.host().nativeElement;
    host.focus({ preventScroll: true });
    host.setPointerCapture(event.pointerId);
    if (event.button === 1) {
      this.drag = { kind: 'scroll', x, y, scrollX: this.scrollX, top: this.top };
      return;
    }
    if (y < RULER) return;
    if (y > this.bottom()) {
      if (event.button === 0) {
        this.drag = { kind: 'velocity' };
        this.working = [...this.current];
        this.velocityAt(x, y);
      }
      return;
    }
    if (x < KEYS) {
      this.sound(this.keyAt(y));
      return;
    }
    if (event.button === 2) {
      this.drag = { kind: 'erase' };
      this.working = [...this.current];
      this.eraseAt(x, y);
      return;
    }
    const hit = this.hit(x, y);
    if (event.ctrlKey && !hit) {
      const keep = event.shiftKey ? new Set(this.selected) : new Set<number>();
      this.drag = { kind: 'select', x0: x, y0: y, x1: x, y1: y, keep };
      return;
    }
    this.working = [...this.current];
    let index: number;
    if (hit) {
      index = hit.index;
      if (event.shiftKey) {
        if (this.selected.has(index)) this.selected.delete(index);
        else this.selected.add(index);
      } else if (!this.selected.has(index)) {
        this.selected = new Set([index]);
      }
      if (event.altKey) {
        // a copy dragged away, the notes left in place
        const copies = [...this.selected].map((i) => ({ ...this.working![i] }));
        const from = this.working.length;
        this.working.push(...copies);
        this.selected = new Set(copies.map((_, i) => from + i));
        index = from + [...this.selected].length - 1;
      }
    } else {
      const start = this.snapped(this.tickAt(x));
      const scale = SCALES[this.scale()]?.steps ?? null;
      const keys = chordKeys(this.inKey(this.keyAt(y)), this.chord(), this.root(), scale).filter(
        (k) => k <= 127,
      );
      const from = this.working.length;
      for (const key of keys)
        this.working.push({ start, length: this.lastLength, key, velocity: 0.8 });
      const picked = keys.map((_, i) => from + i);
      this.selected = new Set(picked);
      this.drag = {
        kind: 'resize',
        from: start + this.lastLength,
        original: [...this.working],
        picked,
      };
      this.sound(keys[0]);
      this.surface?.redraw();
      return;
    }
    const picked = [...this.selected];
    const original = this.working.map((n) => ({ ...n }));
    if (hit.edge && !event.altKey) {
      const n = this.working[index];
      this.drag = { kind: 'resize', from: n.start + n.length, original, picked };
    } else {
      this.drag = {
        kind: 'move',
        from: { tick: this.tickAt(x), key: this.keyAt(y) },
        original,
        picked,
      };
      this.sound(this.working[index].key);
    }
    this.surface?.redraw();
  }

  protected move(event: PointerEvent): void {
    const { x, y } = this.point(event);
    const drag = this.drag;
    if (!drag) {
      const hit = y > RULER && y < this.bottom() && x > KEYS ? this.hit(x, y) : null;
      this.cursor.set(
        hit
          ? hit.edge
            ? 'ew-resize'
            : 'grab'
          : x > KEYS && y > RULER && y < this.bottom()
            ? 'crosshair'
            : 'default',
      );
      return;
    }
    switch (drag.kind) {
      case 'scroll': {
        this.scrollX = Math.max(0, drag.scrollX - (x - drag.x) / this.perTick);
        this.top = Math.max(
          10,
          Math.min(127, drag.top + Math.round((y - drag.y) / this.rowHeight)),
        );
        break;
      }
      case 'move': {
        const snap = this.snap();
        const dt = Math.round((this.tickAt(x) - drag.from.tick) / snap) * snap;
        const anchor = drag.original[drag.picked[drag.picked.length - 1]];
        let dk = this.keyAt(y) - drag.from.key;
        // kept to the scale, the notes move together by what puts the last one in it
        if (anchor) dk = this.inKey(anchor.key + dk, Math.sign(dk)) - anchor.key;
        const lowest = Math.min(...drag.picked.map((i) => drag.original[i].start));
        const shift = Math.max(-lowest, dt);
        for (const i of drag.picked) {
          const o = drag.original[i];
          this.working![i] = {
            ...o,
            start: o.start + shift,
            key: Math.max(0, Math.min(127, o.key + dk)),
          };
        }
        const lead = this.working![drag.picked[drag.picked.length - 1]];
        if (lead && lead.key !== this.sounding) this.sound(lead.key);
        break;
      }
      case 'resize': {
        const snap = this.snap();
        const end = Math.round(this.tickAt(x) / snap) * snap;
        const delta = end - drag.from;
        for (const i of drag.picked) {
          const o = drag.original[i];
          this.working![i] = { ...o, length: Math.max(snap === 1 ? 10 : snap, o.length + delta) };
        }
        break;
      }
      case 'select': {
        drag.x1 = x;
        drag.y1 = y;
        const [t0, t1] = [this.tickAt(Math.min(drag.x0, x)), this.tickAt(Math.max(drag.x0, x))];
        const [k0, k1] = [this.keyAt(Math.max(drag.y0, y)), this.keyAt(Math.min(drag.y0, y))];
        this.selected = new Set(drag.keep);
        this.current.forEach((n, i) => {
          if (n.key >= k0 && n.key <= k1 && n.start < t1 && n.start + n.length > t0)
            this.selected.add(i);
        });
        break;
      }
      case 'erase':
        this.eraseAt(x, y);
        break;
      case 'velocity':
        this.velocityAt(x, y);
        break;
    }
    this.surface?.redraw();
  }

  protected up(event: PointerEvent): void {
    const drag = this.drag;
    this.drag = null;
    this.silence();
    this.host().nativeElement.releasePointerCapture(event.pointerId);
    if (drag?.kind === 'resize' && this.working) {
      const n = this.working[drag.picked[0]];
      if (n) this.lastLength = n.length;
    }
    if (drag && drag.kind !== 'select' && drag.kind !== 'scroll') this.commit();
    this.surface?.redraw();
  }

  private eraseAt(x: number, y: number): void {
    const hit = this.hit(x, y);
    if (!hit || !this.working) return;
    this.working.splice(hit.index, 1);
    this.selected.clear();
  }

  private velocityAt(x: number, y: number): void {
    if (!this.working) return;
    const height = this.host().nativeElement.clientHeight;
    const velocity = Math.max(0.05, Math.min(1, (height - 4 - y) / (VELOCITY - 12)));
    const targets = this.selected.size
      ? [...this.selected]
      : this.working
          .map((_, i) => i)
          .filter((i) => Math.abs(this.x(this.working![i].start) - x) < 5);
    for (const i of targets) this.working[i] = { ...this.working[i], velocity };
  }

  protected toolsMenu(event: MouseEvent): void {
    const t = this.t;
    const tools: [string, (notes: Note[]) => Note[]][] = [
      ['Humanize: a little off the grid and in strength', humanize],
      ['Legato: each note held to the next', (n) => legato(n, this.studio.pattern()?.length ?? 0)],
      ['Strum the chords', strum],
      ['Play them backwards', backwards],
      ['Twice as long', (n) => stretch(n, 2)],
      ['Half as long', (n) => stretch(n, 0.5)],
    ];
    const items: MenuItem[] = tools.map(([label, tool]) => ({
      label: t(label),
      run: () => this.change(tool),
    }));
    items.push(
      { separator: true, label: '' },
      {
        label: t('Write a chord progression…'),
        run: () =>
          this.menus.show(
            event,
            PROGRESSIONS.map((p) => ({ label: t(p.name), run: () => this.progression(p) })),
          ),
      },
      { label: t('Write a melody in the scale'), run: () => this.melody() },
      { label: t('Write a bass under the other channels'), run: () => this.bassLine() },
    );
    this.menus.below(event, items);
  }

  /** The pattern's notes for this channel made a progression's chords, a bar each, in its scale. */
  private progression(p: (typeof PROGRESSIONS)[number]): void {
    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    if (!pattern || !channel) return;
    this.scale.set(p.scale);
    const steps = SCALES[p.scale].steps;
    const bar = this.studio.bar();
    // as long as the progression at least
    const bars = Math.max(p.degrees.length, Math.round(pattern.length / bar));
    const notes: Note[] = [];
    for (let b = 0; b < bars; b++) {
      const degree = p.degrees[b % p.degrees.length];
      for (const key of degreeChord(degree, steps, this.root(), 48)) {
        notes.push({ start: b * bar, length: bar, key, velocity: 0.7 });
      }
    }
    void this.studio.edit({
      type: 'batch',
      edits: [
        {
          type: 'putPattern',
          pattern: { ...pattern, length: Math.max(pattern.length, bars * bar) },
          index: null,
        },
        { type: 'setNotes', pattern: pattern.id, channel: channel.id, notes },
      ],
    });
  }

  /** A bass for this channel: on each beat, the root of what the pattern's other channels play
   * there, in the octave of a bass. */
  private bassLine(): void {
    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    const song = this.studio.song();
    if (!pattern || !channel || !song) return;
    const others = song.channels
      .filter((c) => c.id !== channel.id && builtinName(c.instrument) !== 'sonevo.drums')
      .flatMap((c) => this.studio.notes(pattern.id, c.id));
    if (!others.length) {
      this.studio.fail(this.t('Write chords on another channel of the pattern first.'));
      return;
    }
    const notes: Note[] = [];
    let last: number | null = null;
    for (let at = 0; at < pattern.length; at += PPQ) {
      const sounding = others
        .filter((n) => n.start <= at && at < n.start + n.length)
        .map((n) => n.key);
      const root: number | null = sounding.length ? chordRoot(sounding) : last;
      if (root === null) continue;
      last = root;
      notes.push({
        start: at,
        length: Math.round(PPQ * 0.9),
        key: 36 + (root % 12),
        velocity: 0.85,
      });
    }
    void this.studio.setNotes(pattern.id, channel.id, notes);
  }

  /** A melody of the scale chosen, or the major one: a walk up and down it, on eighths and quarters. */
  private melody(): void {
    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    if (!pattern || !channel) return;
    const steps = SCALES[this.scale()]?.steps ?? SCALES[0].steps;
    const notes: Note[] = [];
    let degree = steps.length;
    for (let at = 0; at < pattern.length;) {
      const long = Math.random() < 0.35;
      const length = (long ? PPQ : PPQ / 2) * (Math.random() < 0.15 ? 2 : 1);
      // now and then a rest; the walk keeps to two octaves
      if (Math.random() > 0.15) {
        degree = Math.max(
          0,
          Math.min(2 * steps.length, degree + Math.round((Math.random() - 0.5) * 4)),
        );
        const key =
          60 +
          this.root() +
          steps[degree % steps.length] +
          12 * Math.floor(degree / steps.length) -
          12;
        notes.push({
          start: at,
          length: Math.min(length, pattern.length - at),
          key,
          velocity: 0.6 + Math.random() * 0.3,
        });
      }
      at += length;
    }
    void this.studio.setNotes(pattern.id, channel.id, notes);
  }

  /** The notes chosen, or every one when none is, changed by `tool`. */
  private change(tool: (notes: Note[]) => Note[]): void {
    const notes = this.current;
    const all = this.selected.size === 0;
    const picked = notes.filter((_, i) => all || this.selected.has(i));
    if (!picked.length) return;
    const changed = tool(picked);
    const kept = notes.filter((_, i) => !(all || this.selected.has(i)));
    this.working = [...kept, ...changed];
    this.selected = all ? new Set() : new Set(changed.map((_, i) => kept.length + i));
    this.commit();
  }

  private commit(): void {
    const pattern = this.studio.pattern();
    const channel = this.studio.channel();
    const working = this.working;
    if (!pattern || !channel || !working) return;
    const before = this.notes();
    const same =
      working.length === before.length &&
      working.every((n, i) => {
        const b = before[i];
        return (
          b &&
          n.start === b.start &&
          n.length === b.length &&
          n.key === b.key &&
          n.velocity === b.velocity
        );
      });
    if (same) {
      this.working = null;
      return;
    }
    // the selection follows the notes once sorted
    const picked = [...this.selected].map((i) => working[i]);
    const sorted = [...working].sort((a, b) => a.start - b.start || a.key - b.key);
    this.selected = new Set(picked.map((n) => sorted.indexOf(n)).filter((i) => i >= 0));
    void this.studio.setNotes(pattern.id, channel.id, sorted);
  }

  private sound(key: number): void {
    const channel = this.studio.channel();
    if (!channel) return;
    this.silence();
    this.sounding = key;
    void api.note(channel.id, key, 0.8, true);
  }

  private silence(): void {
    const channel = this.studio.channel();
    if (this.sounding !== null && channel) void api.note(channel.id, this.sounding, 0, false);
    this.sounding = null;
  }

  protected wheel(event: WheelEvent): void {
    event.preventDefault();
    if (event.ctrlKey) {
      const { x } = this.point(event);
      const at = this.tickAt(x);
      this.perTick = Math.min(
        1,
        Math.max(4 / PPQ, this.perTick * (event.deltaY < 0 ? 1.15 : 0.87)),
      );
      this.scrollX = Math.max(0, at - (x - KEYS) / this.perTick);
    } else if (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
      const delta = event.shiftKey ? event.deltaY : event.deltaX;
      this.scrollX = Math.max(0, this.scrollX + delta / this.perTick);
    } else {
      this.top = Math.max(10, Math.min(127, this.top - Math.sign(event.deltaY) * 3));
    }
    this.surface?.redraw();
  }

  protected zoom(by: number): void {
    this.perTick = Math.min(1, Math.max(4 / PPQ, this.perTick * by));
    this.surface?.redraw();
  }

  protected write(): void {
    const on = !this.studio.writing();
    this.studio.writing.set(on);
    const pattern = this.studio.pattern();
    if (on && pattern) {
      void this.studio.setMode(pattern.id).then(() => {
        if (!this.studio.status().playing) this.studio.togglePlay();
      });
    }
  }

  protected lengthen(): void {
    const pattern = this.studio.pattern();
    if (!pattern) return;
    void this.studio.edit({
      type: 'putPattern',
      pattern: { ...pattern, length: pattern.length + this.studio.bar() },
      index: null,
    });
  }

  protected key(event: KeyboardEvent): void {
    const notes = this.current;
    if ((event.key === 'Delete' || event.key === 'Backspace') && this.selected.size) {
      this.working = notes.filter((_, i) => !this.selected.has(i));
      this.selected.clear();
      this.commit();
    } else if (event.ctrlKey && event.key.toLowerCase() === 'a') {
      this.selected = new Set(notes.map((_, i) => i));
      this.surface?.redraw();
    } else if (event.ctrlKey && event.key.toLowerCase() === 'd' && this.selected.size) {
      const picked = [...this.selected].map((i) => notes[i]);
      const start = Math.min(...picked.map((n) => n.start));
      const end = Math.max(...picked.map((n) => n.start + n.length));
      const span =
        Math.ceil((end - start) / this.studio.bar()) * this.studio.bar() || this.studio.bar();
      const copies = picked.map((n) => ({ ...n, start: n.start + span }));
      this.working = [...notes, ...copies];
      this.selected = new Set(copies.map((_, i) => notes.length + i));
      this.commit();
    } else if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && this.selected.size) {
      const by = (event.key === 'ArrowUp' ? 1 : -1) * (event.shiftKey ? 12 : 1);
      this.working = notes.map((n, i) =>
        this.selected.has(i) ? { ...n, key: Math.max(0, Math.min(127, n.key + by)) } : n,
      );
      this.commit();
    } else if (
      event.ctrlKey &&
      (event.key.toLowerCase() === 'c' || event.key.toLowerCase() === 'x')
    ) {
      // copied as they are placed, to be pasted in any pattern, for any channel
      copied = [...this.selected].map((i) => ({ ...notes[i] }));
      if (event.key.toLowerCase() === 'x') {
        this.working = notes.filter((_, i) => !this.selected.has(i));
        this.selected.clear();
        this.commit();
      }
    } else if (event.ctrlKey && event.key.toLowerCase() === 'v' && copied.length) {
      // pasted where they were, chosen, to be dragged where wanted
      this.working = [...notes, ...copied.map((n) => ({ ...n }))];
      this.selected = new Set(copied.map((_, i) => notes.length + i));
      this.commit();
    } else if (event.key.toLowerCase() === 'q' && !event.ctrlKey) {
      const snap = this.snap();
      const all = this.selected.size === 0;
      this.working = notes.map((n, i) =>
        all || this.selected.has(i) ? { ...n, start: Math.round(n.start / snap) * snap } : n,
      );
      this.commit();
    } else if (event.key === 'Escape') {
      this.selected.clear();
      this.surface?.redraw();
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  }
}

/** Each note up to a sixty-fourth early or late, and a little softer or harder. */
function humanize(notes: Note[]): Note[] {
  const off = () => Math.random() * 2 - 1;
  return notes.map((n) => ({
    ...n,
    start: Math.max(0, Math.round(n.start + off() * (PPQ / 16))),
    velocity: Math.min(1, Math.max(0.05, n.velocity + off() * 0.1)),
  }));
}

/** Each note held until the next one starts, the last to the end of the pattern. */
function legato(notes: Note[], end: number): Note[] {
  const starts = [...new Set(notes.map((n) => n.start))].sort((a, b) => a - b);
  return notes.map((n) => {
    const next = starts.find((s) => s > n.start) ?? Math.max(end, n.start + n.length);
    return { ...n, length: Math.max(1, next - n.start) };
  });
}

/** The notes of each chord a little after one another, the lowest first, as a hand strums. */
function strum(notes: Note[]): Note[] {
  const apart = PPQ / 24;
  return notes.map((n) => {
    const below = notes.filter((o) => o.start === n.start && o.key < n.key).length;
    return { ...n, start: n.start + below * apart, length: Math.max(1, n.length - below * apart) };
  });
}

/** The notes in the opposite order in time, over the time they took. */
function backwards(notes: Note[]): Note[] {
  const from = notes.reduce((a, n) => Math.min(a, n.start), Infinity);
  const to = notes.reduce((a, n) => Math.max(a, n.start + n.length), 0);
  return notes.map((n) => ({ ...n, start: from + to - (n.start + n.length) }));
}

/** The notes played `by` times slower, from where the first starts. */
function stretch(notes: Note[], by: number): Note[] {
  const from = notes.reduce((a, n) => Math.min(a, n.start), Infinity);
  return notes.map((n) => ({
    ...n,
    start: Math.round(from + (n.start - from) * by),
    length: Math.max(1, Math.round(n.length * by)),
  }));
}

/** The root of the chord `keys` make: the one whose fifth and third are in it, else the lowest. */
function chordRoot(keys: number[]): number {
  const classes = new Set(keys.map((k) => k % 12));
  const root = [...classes].find(
    (r) => classes.has((r + 7) % 12) && (classes.has((r + 3) % 12) || classes.has((r + 4) % 12)),
  );
  return root ?? Math.min(...keys);
}
