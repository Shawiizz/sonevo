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
import { inScale, isBlack, keyName, NAMES, Note, PPQ, SCALES, STEP } from '../core/models';
import { Studio } from '../core/studio.service';
import { Icon } from '../shared/icon';
import { hex, Surface } from '../shared/surface';

const KEYS = 64;
/** The notes copied last. */
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
 * side. Click to draw a note, drag it to move it, its end to make it longer; right-click
 * deletes. The other channels' notes show faintly behind.
 */
@Component({
  selector: 'app-piano-roll',
  imports: [Icon],
  template: `
    <div class="s-panel-head">
      <span class="swatch" [style.background]="studio.channel()?.color"></span>
      <span>{{ studio.channel()?.name }}</span>
      <span class="in">{{ t('in') }} {{ studio.pattern()?.name }}</span>
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
    .in {
      color: var(--s-text-3);
      font-weight: 400;
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
  private readonly host = viewChild.required<ElementRef<HTMLElement>>('host');
  private surface: Surface | null = null;

  protected readonly names = NAMES;
  protected readonly scales = SCALES;
  /** The scale lit, by its index in SCALES (-1 for none), and its root. */
  protected readonly scale = signal(-1);
  protected readonly root = signal(9);

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
  /** The notes as they are being changed, before the change is sent. */
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
    const middle = keys.length ? Math.round((Math.min(...keys) + Math.max(...keys)) / 2) : 60;
    const rows = Math.floor(
      (this.host().nativeElement.clientHeight - RULER - VELOCITY) / this.rowHeight,
    );
    this.top = Math.min(127, middle + Math.floor(rows / 2));
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

    // rows: the black keys' darker; with a scale, its notes' lighter and the others' darker
    const scale = SCALES[this.scale()]?.steps;
    for (let key = this.top; key >= 0; key--) {
      const y = this.y(key);
      if (y > bottom) break;
      const shade = scale
        ? inScale(key, this.root(), scale)
          ? key % 12 === this.root()
            ? 0x24213f
            : 0x1b1e27
          : 0x0e1014
        : isBlack(key)
          ? 0x111318
          : 0x15181e;
      g.rect(KEYS, y, width - KEYS, this.rowHeight).fill(shade);
      if (key % 12 === 0) g.rect(KEYS, y + this.rowHeight - 1, width - KEYS, 1).fill(0x2c323d);
    }
    // past the pattern's end, dimmed
    const end = this.x(length);
    if (end < width)
      g.rect(end, RULER, width - end, bottom - RULER).fill({ color: 0x000000, alpha: 0.45 });

    // the grid
    const first = Math.max(0, Math.floor(this.scrollX / STEP) * STEP);
    for (let tick = first; this.x(tick) < width; tick += STEP) {
      const x = this.x(tick);
      if (x < KEYS) continue;
      const strong = tick % bar === 0 ? 0x3a414f : tick % beat === 0 ? 0x252a33 : 0x1a1e25;
      if (tick % beat !== 0 && this.perTick * STEP < 8) continue;
      g.rect(Math.round(x), RULER, 1, bottom - RULER).fill(strong);
    }

    // the other channels' notes, faintly
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

    // the notes
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
        g.roundRect(x, y + 1, w - 1, this.rowHeight - 2, 3).stroke({ width: 1.5, color: 0xffffff });
      if (w > 34 && this.rowHeight >= 12) s.text(keyName(n.key), x + 4, y + 1, 0x0b0c10, 10);
    });

    // the velocity lane
    g.rect(0, bottom, width, VELOCITY).fill(0x0c0e12);
    g.rect(0, bottom, width, 1).fill(0x252a33);
    s.text(this.t('Velocity'), 8, bottom + 6, 0x8d95a5, 10);
    notes.forEach((n, i) => {
      const x = this.x(n.start);
      if (x < KEYS || x > width) return;
      const h = n.velocity * (VELOCITY - 12);
      const picked = this.selected.has(i);
      g.rect(x, height - 4 - h, 2, h).fill(picked ? 0xffffff : color);
      g.circle(x + 1, height - 4 - h, 3).fill(picked ? 0xffffff : color);
    });

    // the keyboard: white keys under, the black ones over their left
    g.rect(0, RULER, KEYS - 1, bottom - RULER).fill(0xd7dbe3);
    for (let key = this.top; key >= 0; key--) {
      const y = this.y(key);
      if (y > bottom) break;
      const lit = this.sounding === key;
      if (isBlack(key)) {
        g.rect(0, y, KEYS * 0.6, this.rowHeight).fill(lit ? color : 0x0b0c10);
      } else {
        if (lit) g.rect(0, y, KEYS - 1, this.rowHeight).fill(color);
        // where two white keys meet without a black one between
        if (key % 12 === 0 || key % 12 === 5)
          g.rect(0, y + this.rowHeight - 1, KEYS - 1, 1).fill(0x8d95a5);
      }
      if (key % 12 === 0) s.text(keyName(key), KEYS - 5, y + 1, 0x4a5160, 10, 1);
    }

    // the ruler
    g.rect(0, 0, width, RULER).fill(0x0f1115);
    g.rect(0, RULER - 1, width, 1).fill(0x252a33);
    for (let tick = Math.floor(this.scrollX / bar) * bar; this.x(tick) < width; tick += bar) {
      const x = this.x(tick);
      if (x < KEYS - 1) continue;
      g.rect(x, 6, 1, RULER - 6).fill(0x3a414f);
      s.text(String(tick / bar + 1), x + 4, 5, 0xbcc2cf, 11);
    }
    if (end < width) g.rect(end - 1, 0, 2, bottom).fill({ color: 0x8f8bf7, alpha: 0.7 });
  }

  /** What is drawn over the notes: a selection being made, the playhead. */
  private paintOver(s: Surface): void {
    const length = this.studio.pattern()?.length ?? this.studio.bar();
    const bottom = s.height - VELOCITY;
    const pattern = this.studio.pattern();
    // a selection being drawn
    if (this.drag?.kind === 'select') {
      const d = this.drag;
      const [x0, x1] = [Math.min(d.x0, d.x1), Math.max(d.x0, d.x1)];
      const [y0, y1] = [Math.min(d.y0, d.y1), Math.max(d.y0, d.y1)];
      s.over
        .rect(x0, y0, x1 - x0, y1 - y0)
        .fill({ color: 0x8f8bf7, alpha: 0.12 })
        .stroke({ width: 1, color: 0x8f8bf7 });
    }

    // where playing is, the pattern going round
    const view = this.studio.view();
    if (view?.patternMode && view.patternMode === pattern?.id && this.studio.status().playing) {
      const at = this.studio.status().position % Math.max(1, length);
      const x = this.x(at);
      if (x >= KEYS) s.over.rect(x, 0, 1.5, bottom).fill(0x5fd3a8);
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
      const note: Note = {
        start: this.snapped(this.tickAt(x)),
        length: this.lastLength,
        key: this.keyAt(y),
        velocity: 0.8,
      };
      this.working.push(note);
      index = this.working.length - 1;
      this.selected = new Set([index]);
      this.drag = {
        kind: 'resize',
        from: note.start + note.length,
        original: [...this.working],
        picked: [index],
      };
      this.sound(note.key);
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
        const dk = this.keyAt(y) - drag.from.key;
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
      // the selection copied right after itself
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
