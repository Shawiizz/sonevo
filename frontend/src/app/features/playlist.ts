import {
  afterNextRender,
  ChangeDetectionStrategy,
  Component,
  effect,
  ElementRef,
  inject,
  OnDestroy,
  signal,
  viewChild,
} from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Clip, Id, newId, Point, PPQ, sampleLabel, STEP, Track } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files } from '../shell/files.service';
import { Icon } from '../shared/icon';
import { Menus } from '../shared/menu';
import { hex, Surface } from '../shared/surface';

const HEADERS = 160;
const RULER = 26;
const EDGE = 7;

type Drag =
  | { kind: 'move'; x: number; y: number; original: Clip[]; moved: boolean }
  | { kind: 'resize'; x: number; original: Clip }
  | { kind: 'select'; x0: number; y0: number; x1: number; y1: number }
  | { kind: 'ruler'; tick: number; moved: boolean; end: number }
  | { kind: 'erase'; erased: Set<Id> }
  | { kind: 'scroll'; x: number; y: number; scrollX: number; scrollY: number }
  | { kind: 'point'; clip: Id; index: number };

/** Where an automation clip's line is drawn, under its name. */
interface Body {
  clip: Clip;
  points: Point[];
  x: number;
  top: number;
  height: number;
}

/** The mime type a sound of the browser is dragged with. */
export const SOUND_DRAG = 'application/x-sonevo-sound';

/**
 * The playlist: the song's tracks, and on them the patterns, sounds and automation placed
 * in time. Click an empty spot to place the pattern edited; drag a clip to move it, its end
 * to make it longer (a pattern repeats); right-click deletes. The ruler moves where playing
 * starts, and dragged along it, sets the loop.
 */
@Component({
  selector: 'app-playlist',
  imports: [Icon],
  template: `
    <div class="s-panel-head">
      <span>{{ t('Playlist') }}</span>
      <span class="hint">{{ t('Click to place') }} « {{ studio.pattern()?.name }} »</span>
      <span class="s-grow"></span>
      <div class="s-seg" role="radiogroup">
        <button
          role="radio"
          [attr.aria-checked]="tool() === 'draw'"
          (click)="tool.set('draw')"
          [title]="t('Place, move and stretch clips')"
        >
          <s-icon name="pencil" [size]="12" />
        </button>
        <button
          role="radio"
          [attr.aria-checked]="tool() === 'cut'"
          (click)="tool.set('cut')"
          [title]="t('Cut a clip in two where clicked')"
        >
          <s-icon name="cut" [size]="12" />
        </button>
      </div>
      <div class="s-seg" role="radiogroup" [title]="t('Clips snap to')">
        @for (s of snaps; track s.label) {
          <button role="radio" [attr.aria-checked]="snap() === s.ticks" (click)="snap.set(s.ticks)">
            {{ t(s.label) }}
          </button>
        }
      </div>
      <button class="s-bar-btn" (click)="zoom(1.25)" [title]="t('Zoom in')">+</button>
      <button class="s-bar-btn" (click)="zoom(0.8)" [title]="t('Zoom out')">−</button>
      <button class="s-bar-btn" (click)="addTrack()" [title]="t('Add a track')">
        <s-icon name="plus" /> {{ t('Track') }}
      </button>
    </div>
    <div
      class="canvas"
      #host
      tabindex="0"
      (pointerdown)="down($event)"
      (pointermove)="move($event)"
      (pointerup)="up($event)"
      (dblclick)="double($event)"
      (wheel)="wheel($event)"
      (contextmenu)="$event.preventDefault()"
      (keydown)="key($event)"
      (dragover)="dragOver($event)"
      (drop)="drop($event)"
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
    .hint {
      color: var(--s-text-3);
      font-weight: 400;
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
export class Playlist implements OnDestroy {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly menus = inject(Menus);
  private readonly files = inject(Files);
  private readonly host = viewChild.required<ElementRef<HTMLElement>>('host');
  private surface: Surface | null = null;

  protected readonly snaps = [
    { label: 'Bar', ticks: 0 },
    { label: 'Beat', ticks: PPQ },
    { label: 'Step', ticks: STEP },
  ];
  /** 0 snaps to bars, whatever the signature. */
  protected readonly snap = signal(0);
  protected readonly tool = signal<'draw' | 'cut'>('draw');
  protected readonly cursor = signal('default');

  private scrollX = 0;
  private scrollY = 0;
  private perTick = 28 / PPQ;
  private laneHeight = 54;
  private selected = new Set<Id>();
  private working: Clip[] | null = null;
  private drag: Drag | null = null;
  /** The loop as it is being dragged along the ruler. */
  private loopPreview: { start: number; end: number } | null = null;

  constructor() {
    afterNextRender(() => {
      this.surface = new Surface(this.host().nativeElement, (s) => this.paint(s));
      void this.surface.start();
    });
    effect(() => {
      this.studio.view();
      this.studio.pattern();
      this.working = null;
      this.surface?.redraw();
    });
    effect(() => {
      this.studio.status();
      if (!this.studio.view()?.patternMode) this.surface?.redraw();
    });
  }

  ngOnDestroy(): void {
    this.surface?.destroy();
  }

  private get clips(): Clip[] {
    return this.working ?? this.studio.song()?.clips ?? [];
  }

  private get tracks(): Track[] {
    return this.studio.song()?.tracks ?? [];
  }

  private step(): number {
    return this.snap() || this.studio.bar();
  }

  private x(tick: number): number {
    return HEADERS + (tick - this.scrollX) * this.perTick;
  }

  private tickAt(x: number): number {
    return this.scrollX + (x - HEADERS) / this.perTick;
  }

  private laneY(index: number): number {
    return RULER + index * this.laneHeight - this.scrollY;
  }

  private laneAt(y: number): number {
    return Math.floor((y - RULER + this.scrollY) / this.laneHeight);
  }

  private snapped(tick: number): number {
    const step = this.step();
    return Math.max(0, Math.floor(tick / step) * step);
  }

  private clipColor(clip: Clip): number {
    const song = this.studio.song();
    switch (clip.content.type) {
      case 'pattern': {
        const id = clip.content.pattern;
        return hex(song?.patterns.find((p) => p.id === id)?.color ?? '#8f8bf7');
      }
      case 'audio':
        return 0x4fd0d6;
      default:
        return 0xe88ad6;
    }
  }

  private clipName(clip: Clip): string {
    const song = this.studio.song();
    switch (clip.content.type) {
      case 'pattern': {
        const id = clip.content.pattern;
        return song?.patterns.find((p) => p.id === id)?.name ?? '';
      }
      case 'audio':
        return sampleLabel(clip.content.sample);
      default: {
        const target = clip.content.target;
        if (target.type === 'tempo') return this.t('Tempo');
        if (target.type === 'param') return target.param;
        return this.t('Automation');
      }
    }
  }

  private paint(s: Surface): void {
    const g = s.shapes;
    const { width, height } = s;
    const bar = this.studio.bar();
    const song = this.studio.song();
    if (!song) return;
    const tracks = this.tracks;

    // the lanes
    tracks.forEach((track, i) => {
      const y = this.laneY(i);
      if (y + this.laneHeight < RULER || y > height) return;
      g.rect(HEADERS, y, width - HEADERS, this.laneHeight).fill(i % 2 ? 0x101217 : 0x12151a);
      g.rect(HEADERS, y + this.laneHeight - 1, width - HEADERS, 1).fill(0x1c2028);
      if (track.mute)
        g.rect(HEADERS, y, width - HEADERS, this.laneHeight).fill({ color: 0x000000, alpha: 0.35 });
    });

    // the loop, then the grid
    const loop = this.loopPreview ?? song.loopRegion;
    if (loop) {
      const a = Math.max(HEADERS, this.x(loop.start));
      const b = this.x(loop.end);
      if (b > a) g.rect(a, RULER, b - a, height - RULER).fill({ color: 0x8f8bf7, alpha: 0.05 });
    }
    const beat = PPQ;
    const fine = this.perTick * beat >= 14;
    for (let tick = Math.floor(this.scrollX / beat) * beat; this.x(tick) < width; tick += beat) {
      const x = this.x(tick);
      if (x < HEADERS) continue;
      const isBar = tick % bar === 0;
      if (!isBar && !fine) continue;
      g.rect(Math.round(x), RULER, 1, height - RULER).fill(isBar ? 0x262b35 : 0x181b22);
    }

    // the clips
    for (const clip of this.clips) {
      const lane = tracks.findIndex((t) => t.id === clip.track);
      if (lane < 0) continue;
      const x = this.x(clip.start);
      const w = clip.length * this.perTick;
      const y = this.laneY(lane);
      if (x > width || x + w < HEADERS || y + this.laneHeight < RULER || y > height) continue;
      const color = this.clipColor(clip);
      const picked = this.selected.has(clip.id);
      const top = y + 2;
      const h = this.laneHeight - 4;
      g.roundRect(x + 1, top, Math.max(4, w - 2), h, 4).fill({ color, alpha: 0.22 });
      g.roundRect(x + 1, top, Math.max(4, w - 2), 15, 4).fill({ color, alpha: 0.85 });
      if (picked)
        g.roundRect(x + 1, top, Math.max(4, w - 2), h, 4).stroke({ width: 1.5, color: 0xffffff });
      if (w > 30) s.text(this.clipName(clip), Math.max(x, HEADERS) + 6, top + 1, 0x0b0c10, 11);
      this.paintContent(s, clip, x, top + 17, w, h - 19, color);
    }

    // the headers
    g.rect(0, RULER, HEADERS, height - RULER).fill(0x0f1115);
    tracks.forEach((track, i) => {
      const y = this.laneY(i);
      if (y + this.laneHeight < RULER || y > height) return;
      g.rect(0, y + this.laneHeight - 1, HEADERS, 1).fill(0x1c2028);
      g.circle(16, y + this.laneHeight / 2, 4.5).fill(track.mute ? 0x363c48 : 0x5fd3a8);
      s.text(track.name, 30, y + this.laneHeight / 2 - 8, track.mute ? 0x8d95a5 : 0xf0f2f6, 12);
    });
    g.rect(HEADERS - 1, RULER, 1, height - RULER).fill(0x252a33);

    // the ruler
    g.rect(0, 0, width, RULER).fill(0x0f1115);
    g.rect(0, RULER - 1, width, 1).fill(0x252a33);
    if (loop) {
      const a = Math.max(HEADERS, this.x(loop.start));
      const b = this.x(loop.end);
      if (b > a) g.rect(a, 2, b - a, RULER - 6).fill({ color: 0x8f8bf7, alpha: 0.45 });
    }
    const every = Math.max(1, Math.ceil(36 / (bar * this.perTick)));
    for (let tick = Math.floor(this.scrollX / bar) * bar; this.x(tick) < width; tick += bar) {
      const x = this.x(tick);
      const n = tick / bar;
      if (x < HEADERS - 1 || n % every !== 0) continue;
      g.rect(x, 8, 1, RULER - 8).fill(0x3a414f);
      s.text(String(n + 1), x + 4, 6, 0xbcc2cf, 11);
    }
    s.text(this.t('Tracks'), 12, 6, 0x8d95a5, 11);

    // a selection being drawn
    if (this.drag?.kind === 'select') {
      const d = this.drag;
      s.over
        .rect(
          Math.min(d.x0, d.x1),
          Math.min(d.y0, d.y1),
          Math.abs(d.x1 - d.x0),
          Math.abs(d.y1 - d.y0),
        )
        .fill({ color: 0x8f8bf7, alpha: 0.12 })
        .stroke({ width: 1, color: 0x8f8bf7 });
    }

    // where playing is
    if (!this.studio.view()?.patternMode) {
      const x = this.x(this.studio.status().position);
      if (x >= HEADERS && x <= width) {
        s.over.rect(x, 0, 1.5, height).fill(0x5fd3a8);
        s.over.poly([x - 5, 0, x + 6.5, 0, x + 0.75, 7]).fill(0x5fd3a8);
      }
    }
  }

  /** What a clip holds, drawn small inside it: notes, a line of automation, a sound. */
  private paintContent(
    s: Surface,
    clip: Clip,
    x: number,
    y: number,
    w: number,
    h: number,
    color: number,
  ): void {
    const g = s.shapes;
    const content = clip.content;
    if (h < 6) return;
    if (content.type === 'pattern') {
      const pattern = this.studio.song()?.patterns.find((p) => p.id === content.pattern);
      const byChannel = this.studio.view()?.notes[content.pattern] ?? {};
      if (!pattern) return;
      // a row for each key of each channel, the channels in the rack's order: a beat's
      // drums each on a line, a melody as it goes up and down
      const rows = new Map<string, number>();
      const notes: { row: string; start: number; length: number }[] = [];
      for (const channel of this.studio.song()?.channels ?? []) {
        const keys = [...new Set((byChannel[channel.id] ?? []).map((n) => n.key))].sort(
          (a, b) => b - a,
        );
        for (const key of keys) rows.set(`${channel.id} ${key}`, rows.size);
        for (const n of byChannel[channel.id] ?? [])
          notes.push({ row: `${channel.id} ${n.key}`, ...n });
      }
      if (!notes.length) return;
      const span = Math.max(6, rows.size);
      const rowHeight = h / span;
      const length = Math.max(1, pattern.length);
      const offset = clip.offset ?? 0;
      for (let from = -offset; from < clip.length; from += length) {
        for (const n of notes) {
          const at = from + n.start;
          if (at < 0 || at >= clip.length) continue;
          const nx = x + at * this.perTick;
          const nw = Math.max(1.5, Math.min(n.length, clip.length - at) * this.perTick - 1);
          const ny = y + (rows.get(n.row) ?? 0) * rowHeight + (span - rows.size) * rowHeight * 0.5;
          g.rect(nx, ny, nw, Math.max(1.5, rowHeight - 1)).fill({ color, alpha: 0.9 });
        }
      }
    } else if (content.type === 'automation') {
      const points = content.points;
      if (!points.length) return;
      const coords: number[] = [];
      const offset = clip.offset ?? 0;
      coords.push(x, y + h - points[0].value * h);
      for (const p of points) {
        const px = x + (p.at - offset) * this.perTick;
        coords.push(Math.max(x, Math.min(x + w, px)), y + h - p.value * h);
      }
      coords.push(x + w, y + h - points[points.length - 1].value * h);
      g.poly(coords, false).stroke({ width: 1.5, color });
      for (const p of points) {
        const px = x + (p.at - offset) * this.perTick;
        if (px >= x - 1 && px <= x + w + 1) g.circle(px, y + h - p.value * h, 3.5).fill(color);
      }
    } else {
      const info = this.studio.sample(content.sample);
      if (!info || !info.peaks.length) return;
      // the sound's outline, as long as it lasts at the song's tempo
      const length = this.studio.ticks(info.seconds);
      const offset = clip.offset ?? 0;
      const visible = Math.min(clip.length, length - offset);
      for (let i = 0; i < visible * this.perTick; i += 2) {
        const at = offset + i / this.perTick;
        const peak =
          info.peaks[
            Math.min(info.peaks.length - 1, Math.floor((at / length) * info.peaks.length))
          ];
        const v = Math.max(0.5, peak * h * 0.48);
        g.rect(x + i, y + h / 2 - v, 1.5, v * 2).fill({ color, alpha: 0.75 });
      }
    }
  }

  private point(event: PointerEvent | WheelEvent | DragEvent | MouseEvent): {
    x: number;
    y: number;
  } {
    const box = this.host().nativeElement.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }

  private hit(x: number, y: number): { clip: Clip; edge: boolean } | null {
    const lane = this.laneAt(y);
    const track = this.tracks[lane];
    if (!track) return null;
    const clips = this.clips;
    for (let i = clips.length - 1; i >= 0; i--) {
      const c = clips[i];
      if (c.track !== track.id) continue;
      const left = this.x(c.start);
      const right = this.x(c.start + c.length);
      if (x >= left && x <= right)
        return { clip: c, edge: right - x <= EDGE && right - left > EDGE * 2 };
    }
    return null;
  }

  protected down(event: PointerEvent): void {
    const { x, y } = this.point(event);
    const host = this.host().nativeElement;
    host.focus({ preventScroll: true });
    if (event.button === 1) {
      host.setPointerCapture(event.pointerId);
      this.drag = { kind: 'scroll', x, y, scrollX: this.scrollX, scrollY: this.scrollY };
      return;
    }
    if (y < RULER) {
      if (x < HEADERS) return;
      if (event.button === 2) {
        this.setLoop(null);
        return;
      }
      host.setPointerCapture(event.pointerId);
      const tick = this.snapped(this.tickAt(x));
      this.drag = { kind: 'ruler', tick, moved: false, end: tick };
      return;
    }
    if (x < HEADERS) {
      this.header(event, x, y);
      return;
    }
    host.setPointerCapture(event.pointerId);
    const body = this.automationAt(x, y);
    if (body) {
      this.editPoint(event, body, x, y);
      return;
    }
    if (event.button === 2) {
      this.drag = { kind: 'erase', erased: new Set() };
      this.working = [...this.clips];
      this.eraseAt(x, y);
      return;
    }
    const hit = this.hit(x, y);
    if (hit && this.tool() === 'cut') {
      this.cut(hit.clip, x);
      return;
    }
    if (!hit) {
      if (event.ctrlKey) {
        this.drag = { kind: 'select', x0: x, y0: y, x1: x, y1: y };
        if (!event.shiftKey) this.selected.clear();
        return;
      }
      this.selected.clear();
      this.place(x, y);
      return;
    }
    const clip = hit.clip;
    if (event.shiftKey) {
      if (this.selected.has(clip.id)) this.selected.delete(clip.id);
      else this.selected.add(clip.id);
    } else if (!this.selected.has(clip.id)) {
      this.selected = new Set([clip.id]);
    }
    if (clip.content.type === 'pattern') this.studio.patternId.set(clip.content.pattern);
    this.working = this.clips.map((c) => ({ ...c }));
    if (event.altKey) {
      // copies dragged away, the clips left in place
      const copies = this.working
        .filter((c) => this.selected.has(c.id))
        .map((c) => ({ ...c, id: newId() }));
      this.working.push(...copies);
      this.selected = new Set(copies.map((c) => c.id));
    }
    if (hit.edge && !event.altKey) {
      this.drag = { kind: 'resize', x, original: { ...clip } };
    } else {
      const original = this.working.filter((c) => this.selected.has(c.id)).map((c) => ({ ...c }));
      this.drag = { kind: 'move', x, y, original, moved: event.altKey };
    }
    this.surface?.redraw();
  }

  /** `clip` cut in two at the step nearest `x`: the second part goes on from where the first ends. */
  private cut(clip: Clip, x: number): void {
    const at = Math.round(this.tickAt(x) / this.step()) * this.step();
    if (at <= clip.start || at >= clip.start + clip.length) return;
    const first: Clip = { ...clip, length: at - clip.start };
    const second: Clip = {
      ...clip,
      id: newId(),
      start: at,
      length: clip.start + clip.length - at,
      offset: (clip.offset ?? 0) + (at - clip.start),
    };
    this.selected = new Set([second.id]);
    void this.studio.edit({
      type: 'batch',
      edits: [
        { type: 'putClip', clip: first },
        { type: 'putClip', clip: second },
      ],
    });
  }

  /** The automation clip whose line is under (x, y), if any. */
  private automationAt(x: number, y: number): Body | null {
    const hit = this.hit(x, y);
    if (!hit || hit.clip.content.type !== 'automation') return null;
    const lane = this.tracks.findIndex((t) => t.id === hit.clip.track);
    const top = this.laneY(lane) + 2 + 17;
    const height = this.laneHeight - 4 - 19;
    if (y < top || height < 6) return null;
    return {
      clip: hit.clip,
      points: hit.clip.content.points,
      x: this.x(hit.clip.start),
      top,
      height,
    };
  }

  private pointX(body: Body, point: Point): number {
    return body.x + (point.at - (body.clip.offset ?? 0)) * this.perTick;
  }

  private pointY(body: Body, point: Point): number {
    return body.top + body.height - point.value * body.height;
  }

  /** A point of an automation clip taken, added or (right-click) taken out. */
  private editPoint(event: PointerEvent, body: Body, x: number, y: number): void {
    const near = body.points.findIndex(
      (p) => Math.abs(this.pointX(body, p) - x) < 7 && Math.abs(this.pointY(body, p) - y) < 9,
    );
    const points = body.points.map((p) => ({ ...p }));
    if (event.button === 2) {
      if (near >= 0 && points.length > 1) {
        points.splice(near, 1);
        this.working = this.clips.map((c) =>
          c.id === body.clip.id ? this.withPoints(c, points) : c,
        );
        this.commit();
      }
      return;
    }
    let index = near;
    if (index < 0) {
      const at = Math.round(this.tickAt(x) - body.clip.start + (body.clip.offset ?? 0));
      const value = Math.min(1, Math.max(0, (body.top + body.height - y) / body.height));
      index = points.findIndex((p) => p.at > at);
      if (index < 0) index = points.length;
      points.splice(index, 0, { at, value });
    }
    this.selected = new Set([body.clip.id]);
    this.working = this.clips.map((c) => (c.id === body.clip.id ? this.withPoints(c, points) : c));
    this.drag = { kind: 'point', clip: body.clip.id, index };
    this.surface?.redraw();
  }

  private withPoints(clip: Clip, points: Point[]): Clip {
    if (clip.content.type !== 'automation') return clip;
    return { ...clip, content: { ...clip.content, points } };
  }

  /** The pattern edited placed where clicked, as long as it is. */
  private place(x: number, y: number): void {
    const track = this.tracks[this.laneAt(y)];
    const pattern = this.studio.pattern();
    if (!track || !pattern) return;
    const clip: Clip = {
      id: newId(),
      track: track.id,
      start: this.snapped(this.tickAt(x)),
      length: pattern.length,
      content: { type: 'pattern', pattern: pattern.id },
    };
    this.selected = new Set([clip.id]);
    this.working = [...this.clips, clip];
    this.drag = { kind: 'resize', x, original: clip };
    this.surface?.redraw();
  }

  protected move(event: PointerEvent): void {
    const { x, y } = this.point(event);
    const drag = this.drag;
    if (!drag) {
      const hit = y > RULER && x > HEADERS ? this.hit(x, y) : null;
      this.cursor.set(
        hit && this.tool() === 'cut'
          ? 'col-resize'
          : hit
            ? hit.edge
              ? 'ew-resize'
              : 'grab'
            : x > HEADERS && y > RULER
              ? 'copy'
              : 'default',
      );
      return;
    }
    switch (drag.kind) {
      case 'scroll':
        this.scrollX = Math.max(0, drag.scrollX - (x - drag.x) / this.perTick);
        this.scrollY = this.clampScroll(drag.scrollY - (y - drag.y));
        break;
      case 'move': {
        const step = this.step();
        const dt = Math.round((x - drag.x) / this.perTick / step) * step;
        const dl =
          Math.floor((y - RULER + this.scrollY) / this.laneHeight) -
          Math.floor((drag.y - RULER + this.scrollY) / this.laneHeight);
        if (dt !== 0 || dl !== 0) drag.moved = true;
        const tracks = this.tracks;
        const earliest = Math.min(...drag.original.map((c) => c.start));
        const shift = Math.max(-earliest, dt);
        for (const o of drag.original) {
          const lane = tracks.findIndex((t) => t.id === o.track);
          const to = tracks[Math.max(0, Math.min(tracks.length - 1, lane + dl))];
          const i = this.working!.findIndex((c) => c.id === o.id);
          if (i >= 0) this.working![i] = { ...o, start: o.start + shift, track: to.id };
        }
        break;
      }
      case 'resize': {
        const step = this.step();
        const end = Math.max(drag.original.start + step, Math.round(this.tickAt(x) / step) * step);
        const i = this.working!.findIndex((c) => c.id === drag.original.id);
        if (i >= 0) this.working![i] = { ...drag.original, length: end - drag.original.start };
        break;
      }
      case 'select': {
        drag.x1 = x;
        drag.y1 = y;
        const [t0, t1] = [this.tickAt(Math.min(drag.x0, x)), this.tickAt(Math.max(drag.x0, x))];
        const [l0, l1] = [this.laneAt(Math.min(drag.y0, y)), this.laneAt(Math.max(drag.y0, y))];
        const tracks = this.tracks;
        for (const c of this.clips) {
          const lane = tracks.findIndex((t) => t.id === c.track);
          if (lane >= l0 && lane <= l1 && c.start < t1 && c.start + c.length > t0)
            this.selected.add(c.id);
        }
        break;
      }
      case 'ruler': {
        const tick = this.snapped(this.tickAt(x));
        if (tick !== drag.tick) drag.moved = true;
        drag.end = tick;
        if (drag.moved) {
          const [a, b] = [Math.min(drag.tick, tick), Math.max(drag.tick, tick)];
          this.loopPreview = b > a ? { start: a, end: b } : null;
        }
        break;
      }
      case 'erase':
        this.eraseAt(x, y);
        break;
      case 'point': {
        const i = this.working!.findIndex((c) => c.id === drag.clip);
        const clip = this.working![i];
        if (!clip || clip.content.type !== 'automation') break;
        const lane = this.tracks.findIndex((t) => t.id === clip.track);
        const top = this.laneY(lane) + 2 + 17;
        const height = this.laneHeight - 4 - 19;
        const points = clip.content.points.map((p) => ({ ...p }));
        const before = points[drag.index - 1]?.at ?? 0;
        const after = points[drag.index + 1]?.at ?? clip.length + (clip.offset ?? 0);
        const at = Math.round(this.tickAt(x) - clip.start + (clip.offset ?? 0));
        points[drag.index] = {
          ...points[drag.index],
          at: Math.min(after, Math.max(before, at)),
          value: Math.min(1, Math.max(0, (top + height - y) / height)),
        };
        this.working![i] = this.withPoints(clip, points);
        break;
      }
    }
    this.surface?.redraw();
  }

  protected up(event: PointerEvent): void {
    const drag = this.drag;
    this.drag = null;
    const host = this.host().nativeElement;
    if (host.hasPointerCapture(event.pointerId)) host.releasePointerCapture(event.pointerId);
    if (!drag) return;
    if (drag.kind === 'ruler') {
      this.loopPreview = null;
      if (drag.moved) {
        const [a, b] = [Math.min(drag.tick, drag.end), Math.max(drag.tick, drag.end)];
        if (b > a) this.setLoop({ start: a, end: b });
      } else {
        void api.seek(drag.tick);
      }
      return;
    }
    if (drag.kind === 'move' && !drag.moved) {
      this.working = null;
      this.surface?.redraw();
      return;
    }
    if (
      drag.kind === 'move' ||
      drag.kind === 'resize' ||
      drag.kind === 'erase' ||
      drag.kind === 'point'
    ) {
      this.commit();
    }
    this.surface?.redraw();
  }

  private eraseAt(x: number, y: number): void {
    const hit = this.hit(x, y);
    if (!hit || !this.working || this.drag?.kind !== 'erase') return;
    this.drag.erased.add(hit.clip.id);
    this.working = this.working.filter((c) => c.id !== hit.clip.id);
  }

  /** The clips as changed, sent as one edit. */
  private commit(): void {
    const working = this.working;
    const song = this.studio.song();
    if (!working || !song) return;
    const before = new Map(song.clips.map((c) => [c.id, c]));
    const kept = new Set(working.map((c) => c.id));
    const removed = song.clips.filter((c) => !kept.has(c.id)).map((c) => c.id);
    const edits = working
      .filter((c) => JSON.stringify(c) !== JSON.stringify(before.get(c.id)))
      .map((clip) => ({ type: 'putClip' as const, clip }));
    if (removed.length) edits.push({ type: 'removeClips', ids: removed } as never);
    if (!edits.length) {
      this.working = null;
      return;
    }
    void this.studio.edit({ type: 'batch', edits });
  }

  private setLoop(region: { start: number; end: number } | null): void {
    const song = this.studio.song();
    if (!song) return;
    void this.studio.edit({
      type: 'setSong',
      name: song.name,
      tempo: song.tempo,
      signature: song.signature,
      loopRegion: region,
    });
  }

  private header(event: PointerEvent, x: number, y: number): void {
    const track = this.tracks[this.laneAt(y)];
    if (!track) return;
    if (event.button === 0 && x < 28) {
      void this.studio.edit({
        type: 'putTrack',
        track: { ...track, mute: !track.mute },
        index: null,
      });
      return;
    }
    const t = this.t;
    const index = this.tracks.indexOf(track);
    this.menus.show(event, [
      {
        label: t('Rename…'),
        run: () => {
          const name = prompt(t('Name of the track'), track.name);
          if (name)
            void this.studio.edit({ type: 'putTrack', track: { ...track, name }, index: null });
        },
      },
      {
        label: track.mute ? t('Unmute') : t('Mute'),
        run: () =>
          this.studio.edit({
            type: 'putTrack',
            track: { ...track, mute: !track.mute },
            index: null,
          }),
      },
      { label: t('Add a track below'), run: () => this.addTrack(index + 1) },
      { separator: true, label: '' },
      {
        label: t('Delete the track and its clips'),
        danger: true,
        disabled: this.tracks.length < 2,
        run: () => this.studio.edit({ type: 'removeTrack', id: track.id }),
      },
    ]);
  }

  protected double(event: MouseEvent): void {
    const { x, y } = this.point(event);
    const hit = y > RULER && x > HEADERS ? this.hit(x, y) : null;
    if (hit?.clip.content.type === 'pattern') {
      this.studio.patternId.set(hit.clip.content.pattern);
      this.studio.dock.set('rack');
    }
  }

  private clampScroll(y: number): number {
    const content = this.tracks.length * this.laneHeight;
    const view = this.host().nativeElement.clientHeight - RULER;
    return Math.max(0, Math.min(Math.max(0, content - view + 40), y));
  }

  protected wheel(event: WheelEvent): void {
    event.preventDefault();
    if (event.ctrlKey) {
      const { x } = this.point(event);
      const at = this.tickAt(x);
      this.perTick = Math.min(
        0.4,
        Math.max(1 / PPQ, this.perTick * (event.deltaY < 0 ? 1.15 : 0.87)),
      );
      this.scrollX = Math.max(0, at - (x - HEADERS) / this.perTick);
    } else if (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
      const delta = event.shiftKey ? event.deltaY : event.deltaX;
      this.scrollX = Math.max(0, this.scrollX + delta / this.perTick);
    } else if (event.altKey) {
      this.laneHeight = Math.max(30, Math.min(120, this.laneHeight + (event.deltaY < 0 ? 6 : -6)));
    } else {
      this.scrollY = this.clampScroll(this.scrollY + event.deltaY);
    }
    this.surface?.redraw();
  }

  protected zoom(by: number): void {
    this.perTick = Math.min(0.4, Math.max(1 / PPQ, this.perTick * by));
    this.surface?.redraw();
  }

  protected addTrack(index: number | null = null): void {
    const track: Track = { id: newId(), name: `Track ${this.tracks.length + 1}`, mute: false };
    void this.studio.edit({ type: 'putTrack', track, index });
  }

  protected key(event: KeyboardEvent): void {
    if ((event.key === 'Delete' || event.key === 'Backspace') && this.selected.size) {
      void this.studio.edit({ type: 'removeClips', ids: [...this.selected] });
      this.selected.clear();
    } else if (event.ctrlKey && event.key.toLowerCase() === 'a') {
      this.selected = new Set(this.clips.map((c) => c.id));
      this.surface?.redraw();
    } else if (event.ctrlKey && event.key.toLowerCase() === 'd' && this.selected.size) {
      const picked = this.clips.filter((c) => this.selected.has(c.id));
      const start = Math.min(...picked.map((c) => c.start));
      const end = Math.max(...picked.map((c) => c.start + c.length));
      const copies = picked.map((c) => ({ ...c, id: newId(), start: c.start + (end - start) }));
      this.selected = new Set(copies.map((c) => c.id));
      void this.studio.edit({
        type: 'batch',
        edits: copies.map((clip) => ({ type: 'putClip', clip })),
      });
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  }

  protected dragOver(event: DragEvent): void {
    if (event.dataTransfer?.types.includes(SOUND_DRAG)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'copy';
    }
  }

  /** A sound of the browser dropped on a track: an audio clip there. */
  protected async drop(event: DragEvent): Promise<void> {
    const path = event.dataTransfer?.getData(SOUND_DRAG);
    if (!path) return;
    event.preventDefault();
    const { x, y } = this.point(event);
    const track = this.tracks[this.laneAt(y)];
    if (!track || x < HEADERS) return;
    const [sound] = await this.files.import([path]);
    if (!sound) return;
    // as long as the sound, to the beat after its end
    const info = await api.sampleInfo(sound.sample).catch(() => null);
    const length = info
      ? Math.ceil(this.studio.ticks(info.seconds) / PPQ) * PPQ
      : this.studio.bar() * 2;
    const clip: Clip = {
      id: newId(),
      track: track.id,
      start: this.snapped(this.tickAt(x)),
      length,
      content: { type: 'audio', sample: sound.sample, gain: 1 },
    };
    void this.studio.edit({ type: 'putClip', clip });
  }
}
