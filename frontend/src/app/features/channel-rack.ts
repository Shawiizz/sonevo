import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { builtinName, Channel, COLORS, Edit, Insert, newId, Note, STEP } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files } from '../shell/files.service';
import { Icon } from '../shared/icon';
import { Knob } from '../shared/knob';
import { MenuItem, Menus } from '../shared/menu';
import { Meter } from '../shared/meter';
import { Rename } from '../shared/rename';
import { SOUND_DRAG } from './playlist';

interface Row {
  channel: Channel;
  notes: Note[];
  /** Every note a step long on a step: shown as steps, else as a small roll. */
  steps: boolean[] | null;
  icon: string;
}

const STEP_KEY = 60;

/** Beats a style each, the steps of a bar by drum sound: 0 kick, 1 snare, 2 closed hat, 3 clap,
 * 4 open hat. */
const BEATS: { name: string; steps: Record<number, number[]> }[] = [
  {
    name: 'House',
    steps: { 0: [0, 4, 8, 12], 3: [4, 12], 2: [2, 6, 10, 14], 4: [14] },
  },
  {
    name: 'Hip-hop',
    steps: { 0: [0, 7, 10], 1: [4, 12], 3: [4, 12], 2: [0, 2, 4, 6, 8, 10, 12, 14] },
  },
  {
    name: 'Trap',
    steps: { 0: [0, 10], 3: [8], 1: [8], 2: [0, 2, 4, 6, 8, 10, 11, 12, 13, 14, 15] },
  },
  {
    name: 'Breakbeat',
    steps: { 0: [0, 10], 1: [4, 12], 3: [4, 12], 2: [0, 2, 4, 6, 8, 10, 12, 14], 4: [7] },
  },
];
const DRUM_SOUNDS = ['Kick', 'Snare', 'Closed hat', 'Clap', 'Open hat', 'Tom', 'Rim', 'Cowbell'];

/**
 * The channel rack: every instrument of the song, a row each, with its steps in the pattern
 * edited. A step clicked is a note there; a channel's name clicked shows its instrument.
 */
@Component({
  selector: 'app-channel-rack',
  imports: [Icon, Knob, Meter],
  template: `
    @if (head() && studio.pattern(); as pattern) {
      <div class="head">
        <span class="dot" [style.background]="pattern.color"></span>
        <span class="title">{{ pattern.name }}</span>
        <span class="what">{{ t('The pattern edited: its steps below.') }}</span>
        <span class="grow"></span>
        <button
          class="s-bar-btn"
          [class.on]="studio.view()?.patternMode"
          (click)="hearAlone()"
          [title]="t('Play the pattern chosen, over and over')"
        >
          {{ studio.view()?.patternMode ? t('Back to the whole song') : t('Hear it alone') }}
        </button>
      </div>
    }
    <div class="rows" (dragover)="dragOver($event)" (drop)="drop($event)">
      @for (row of rows(); track row.channel.id) {
        <div
          [attr.data-channel]="row.channel.id"
          class="row"
          [class.selected]="row.channel.id === studio.channel()?.id"
          (contextmenu)="rowMenu($event, row.channel)"
        >
          <button
            class="led"
            [class.on]="!row.channel.mute"
            (click)="toggleMute(row.channel)"
            [title]="t('Mute')"
          ></button>
          <s-knob
            [value]="row.channel.pan"
            [min]="-1"
            [max]="1"
            [default]="0"
            [size]="24"
            [format]="panText"
            (live)="livePan(row.channel, $event)"
            (changed)="setPan(row.channel, $event)"
          />
          <s-knob
            [value]="row.channel.volume"
            [min]="0"
            [max]="1"
            [default]="0.8"
            [size]="24"
            [format]="volumeText"
            (live)="liveVolume(row.channel, $event)"
            (changed)="setVolume(row.channel, $event)"
          />
          <button
            class="name"
            [style.--color]="row.channel.color"
            (click)="select(row.channel)"
            (dblclick)="openRoll(row.channel)"
            (pointerdown)="preview(row.channel, true)"
            (pointerup)="preview(row.channel, false)"
            (pointerleave)="preview(row.channel, false)"
          >
            <s-icon [name]="row.icon" [size]="13" />
            <span>{{ row.channel.name }}</span>
          </button>
          <s-meter class="meter" [peak]="studio.status().channels[row.channel.id] ?? 0" />
          <button
            class="route s-mono"
            (click)="routeMenu($event, row.channel)"
            [title]="t('Mixer track')"
          >
            {{ routeName(row.channel) }}
          </button>
          @if (row.steps; as steps) {
            <div class="steps">
              @for (on of steps; track $index) {
                <button
                  class="step"
                  [class.on]="on"
                  [class.alt]="isAltBeat($index)"
                  [style.--color]="row.channel.color"
                  (pointerdown)="toggleStep($event, row, $index)"
                ></button>
              }
            </div>
          } @else {
            <button
              class="roll"
              (click)="openRoll(row.channel)"
              [title]="t('Open in the piano roll')"
            >
              <svg [attr.viewBox]="'0 0 ' + length() + ' 24'" preserveAspectRatio="none">
                @for (n of row.notes; track $index) {
                  <rect
                    [attr.x]="n.start"
                    [attr.y]="rollY(row.notes, n.key)"
                    [attr.width]="n.length"
                    height="3"
                    [attr.fill]="row.channel.color"
                  />
                }
              </svg>
            </button>
          }
        </div>
      } @empty {
        <div class="s-empty">{{ t('No channel yet: add an instrument below.') }}</div>
      }
    </div>
    <div class="foot">
      <button class="s-bar-btn" (click)="addMenu($event)">
        <s-icon name="plus" /> {{ t('Add a channel') }}
      </button>
      <button
        class="s-bar-btn"
        (click)="beatMenu($event)"
        [title]="t('The drums play a beat of the style chosen')"
      >
        <s-icon name="dice" /> {{ t('Write a beat for me') }}
      </button>
      <span class="hint">{{
        head()
          ? t('Click a step to place a note; double-click a name for the piano roll.')
          : t('Click a step to place a note; click a name to see its notes below.')
      }}</span>
      @if (studio.pattern(); as pattern) {
        <div
          class="swing"
          [title]="t('Swing: every second sixteenth played a little late, for groove')"
        >
          <span>{{ t('Swing') }}</span>
          <s-knob
            [value]="pattern.swing ?? 0"
            [default]="0"
            [size]="22"
            (changed)="setSwing($event)"
          />
          <span class="s-mono">{{ ((pattern.swing ?? 0) * 100).toFixed(0) }}%</span>
        </div>
      }
    </div>
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      min-height: 0;
      height: 100%;
    }
    .head {
      flex: none;
      display: flex;
      align-items: center;
      gap: 8px;
      height: 36px;
      padding: 0 12px;
      border-bottom: 1px solid var(--s-line);
      .dot {
        width: 10px;
        height: 10px;
        border-radius: 3px;
      }
      .title {
        font-weight: 600;
      }
      .what {
        color: var(--s-text-3);
        font-size: 12px;
      }
      .grow {
        flex: 1;
      }
      .on {
        color: var(--s-accent);
      }
    }
    .rows {
      flex: 1;
      min-height: 0;
      overflow: auto;
      padding: 6px 0;
    }
    .row {
      display: flex;
      align-items: center;
      gap: 6px;
      height: 40px;
      padding: 0 10px;
      /* a long pattern's steps scroll the rack across, never squeeze the row */
      width: max-content;
      min-width: 100%;
      > * {
        flex: none;
      }
      &.selected {
        background: var(--s-raised);
      }
      s-knob ::ng-deep .value {
        display: none;
      }
      s-knob {
        min-width: 28px;
      }
    }
    .led {
      width: 10px;
      height: 10px;
      padding: 0;
      border: 0;
      border-radius: 50%;
      background: var(--s-line-strong);
      cursor: pointer;
      &.on {
        background: var(--s-play);
        box-shadow: 0 0 6px color-mix(in srgb, var(--s-play) 60%, transparent);
      }
    }
    .name {
      width: 150px;
      height: 28px;
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 0 10px;
      border: 0;
      border-left: 3px solid var(--color);
      border-radius: var(--s-radius);
      background: var(--s-hover);
      color: var(--s-text);
      text-align: left;
      cursor: pointer;
      span {
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
      }
      &:hover {
        background: color-mix(in srgb, var(--color) 18%, var(--s-hover));
      }
    }
    .meter {
      height: 26px;
      width: 4px;
    }
    .route {
      width: 34px;
      height: 24px;
      padding: 0;
      border: 1px solid var(--s-line);
      border-radius: var(--s-radius);
      background: var(--s-void);
      color: var(--s-text-2);
      font-size: 11px;
      cursor: pointer;
      &:hover {
        border-color: var(--s-line-strong);
        color: var(--s-text);
      }
    }
    .steps {
      display: flex;
      gap: 3px;
      padding-left: 6px;
    }
    .step {
      width: 22px;
      height: 28px;
      padding: 0;
      border: 0;
      border-radius: 4px;
      background: var(--s-step);
      cursor: pointer;
      &.alt {
        background: var(--s-step-alt);
      }
      &:nth-child(4n + 1) {
        margin-left: 4px;
      }
      &:hover {
        background: var(--s-step-hover);
      }
      &.on {
        background: var(--color);
        box-shadow: inset 0 -3px 0 rgb(0 0 0 / 0.25);
      }
    }
    .roll {
      flex: 1;
      max-width: 820px;
      height: 28px;
      margin-left: 10px;
      padding: 2px 4px;
      border: 1px solid var(--s-line);
      border-radius: var(--s-radius);
      background: var(--s-void);
      cursor: pointer;
      svg {
        width: 100%;
        height: 100%;
        display: block;
      }
      &:hover {
        border-color: var(--s-line-strong);
      }
    }
    .swing {
      display: flex;
      align-items: center;
      gap: 6px;
      margin-left: auto;
      color: var(--s-text-2);
      font-size: 12px;
      .s-mono {
        min-width: 32px;
        color: var(--s-text-3);
      }
      s-knob ::ng-deep .value {
        display: none;
      }
    }
    .foot {
      flex: none;
      display: flex;
      align-items: center;
      gap: 16px;
      padding: 6px 10px;
      border-top: 1px solid var(--s-line);
    }
    .hint {
      color: var(--s-text-3);
      font-size: 12px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class ChannelRack {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  /** Whether the pattern edited is named above the steps: not where it already is. */
  readonly head = input(true);

  protected hearAlone(): void {
    const view = this.studio.view();
    void this.studio.setMode(view?.patternMode ? null : (this.studio.pattern()?.id ?? null));
  }
  private readonly named = inject(I18n).named;
  private readonly menus = inject(Menus);
  private readonly files = inject(Files);
  private readonly rename = inject(Rename);

  protected readonly length = computed(() => this.studio.pattern()?.length ?? this.studio.bar());
  private readonly beat = computed(() => (960 * 4) / (this.studio.song()?.signature.unit ?? 4));

  protected readonly rows = computed<Row[]>(() => {
    const song = this.studio.song();
    const pattern = this.studio.pattern();
    if (!song || !pattern) return [];
    const count = Math.max(1, Math.round(pattern.length / STEP));
    return song.channels.map((channel) => {
      const notes = this.studio.notes(pattern.id, channel.id);
      const onSteps = notes.every((n) => n.start % STEP === 0 && n.length <= STEP);
      let steps: boolean[] | null = null;
      if (onSteps) {
        steps = new Array(count).fill(false);
        for (const n of notes) {
          const i = n.start / STEP;
          if (i < count) steps[i] = true;
        }
      }
      return { channel, notes, steps, icon: this.iconOf(channel) };
    });
  });

  private iconOf(channel: Channel): string {
    switch (builtinName(channel.instrument)) {
      case 'sonevo.drums':
        return 'drum';
      case 'sonevo.sampler':
        return 'sample';
      default:
        return 'synth';
    }
  }

  protected isAltBeat(step: number): boolean {
    const perBeat = this.beat() / STEP;
    return Math.floor(step / perBeat) % 2 === 1;
  }

  protected readonly panText = (v: number) =>
    Math.abs(v) < 0.01 ? 'C' : `${Math.round(Math.abs(v) * 100)}${v < 0 ? 'L' : 'R'}`;
  protected readonly volumeText = (v: number) => `${Math.round(v * 100)}%`;

  protected routeName(channel: Channel): string {
    const song = this.studio.song();
    if (!channel.insert || !song) return 'M';
    const index = song.inserts.findIndex((i) => i.id === channel.insert);
    return index < 0 ? 'M' : String(index + 1);
  }

  protected rollY(notes: Note[], key: number): number {
    const keys = notes.map((n) => n.key);
    const low = Math.min(...keys);
    const high = Math.max(...keys);
    const span = Math.max(1, high - low);
    return 20 - ((key - low) / span) * 18;
  }

  protected select(channel: Channel): void {
    this.studio.channelId.set(channel.id);
    this.studio.deviceId.set(null);
  }

  protected openRoll(channel: Channel): void {
    this.select(channel);
    this.studio.dock.set('roll');
  }

  private previewing: string | null = null;

  protected preview(channel: Channel, on: boolean): void {
    if (on) {
      this.previewing = channel.id;
      void api.note(channel.id, STEP_KEY, 0.8, true);
    } else if (this.previewing === channel.id) {
      this.previewing = null;
      void api.note(channel.id, STEP_KEY, 0, false);
    }
  }

  protected toggleStep(event: PointerEvent, row: Row, step: number): void {
    if (event.button !== 0 && event.button !== 2) return;
    const pattern = this.studio.pattern();
    if (!pattern) return;
    const start = step * STEP;
    const on = row.steps?.[step] ?? false;
    const notes = on
      ? row.notes.filter((n) => n.start !== start)
      : [...row.notes, { start, length: STEP, key: STEP_KEY, velocity: 0.8 }];
    if (!on)
      void api
        .note(row.channel.id, STEP_KEY, 0.8, true)
        .then(() => setTimeout(() => api.note(row.channel.id, STEP_KEY, 0, false), 120));
    void this.studio.setNotes(pattern.id, row.channel.id, notes);
  }

  /** A step-long note every `every` steps of the pattern, in place of its notes. */
  protected beatMenu(event: MouseEvent): void {
    this.menus.below(event, [
      ...BEATS.map((beat) => ({ label: this.t(beat.name), run: () => this.writeBeat(beat.steps) })),
      { separator: true, label: '' },
      { label: this.t('A little different'), run: () => this.vary() },
    ]);
  }

  /** The drums' steps in the pattern changed a little: a hit more or fewer, softer hats between. */
  private vary(): void {
    const song = this.studio.song();
    const pattern = this.studio.pattern();
    if (!song || !pattern) return;
    const steps = Math.floor(pattern.length / STEP);
    const edits: Edit[] = [];
    for (const channel of song.channels) {
      if (builtinName(channel.instrument) !== 'sonevo.drums') continue;
      const notes = [...this.studio.notes(pattern.id, channel.id)];
      if (!notes.length) continue;
      const taken = new Set(notes.map((n) => Math.round(n.start / STEP)));
      const free = Array.from({ length: steps }, (_, i) => i).filter((i) => !taken.has(i));
      const hat = Math.round(this.studio.value(channel.instrument, 'sound')) === 2;
      if (hat && free.length) {
        // ghost notes, soft, a few of the steps free
        for (const step of free.filter(() => Math.random() < 0.25)) {
          notes.push({ start: step * STEP, length: STEP, key: STEP_KEY, velocity: 0.35 });
        }
      } else if (Math.random() < 0.5 && free.length) {
        const step = free[Math.floor(Math.random() * free.length)];
        notes.push({ start: step * STEP, length: STEP, key: STEP_KEY, velocity: 0.7 });
      } else if (notes.length > 1) {
        // one off the beat let go of, those on it kept
        const off = notes.filter((n) => Math.round(n.start / STEP) % 4 !== 0);
        const gone = off[Math.floor(Math.random() * off.length)];
        if (gone) notes.splice(notes.indexOf(gone), 1);
      }
      notes.sort((a, b) => a.start - b.start || a.key - b.key);
      edits.push({ type: 'setNotes', pattern: pattern.id, channel: channel.id, notes });
    }
    if (edits.length) void this.studio.edit({ type: 'batch', edits });
  }

  /** Each drum channel's notes in the pattern made those of its sound in `steps`, every bar. */
  private writeBeat(steps: Record<number, number[]>): void {
    const song = this.studio.song();
    const pattern = this.studio.pattern();
    if (!song || !pattern) return;
    const bar = this.studio.bar();
    const edits: Edit[] = [];
    for (const channel of song.channels) {
      if (builtinName(channel.instrument) !== 'sonevo.drums') continue;
      const sound = Math.round(this.studio.value(channel.instrument, 'sound'));
      const hits = steps[sound];
      if (!hits) continue;
      const notes: Note[] = [];
      for (let at = 0; at < pattern.length; at += bar) {
        for (const step of hits) {
          const start = at + step * STEP;
          if (start < pattern.length) {
            notes.push({
              start,
              length: STEP,
              key: STEP_KEY,
              velocity: step % 4 === 0 ? 0.9 : 0.7,
            });
          }
        }
      }
      edits.push({ type: 'setNotes', pattern: pattern.id, channel: channel.id, notes });
    }
    if (!edits.length) {
      this.studio.fail(this.t('Add drums first: a kick, a snare, a clap or hats.'));
      return;
    }
    void this.studio.edit({ type: 'batch', edits });
  }

  private fill(channel: Channel, every: number): void {
    const pattern = this.studio.pattern();
    if (!pattern) return;
    const key = this.studio.notes(pattern.id, channel.id)[0]?.key ?? STEP_KEY;
    const notes: Note[] = [];
    for (let start = 0; start < pattern.length; start += every * STEP) {
      notes.push({ start, length: STEP, key, velocity: 0.8 });
    }
    void this.studio.setNotes(pattern.id, channel.id, notes);
  }

  /** Every note `by` steps later, those past the end round to the start. */
  private shift(channel: Channel, by: number): void {
    const pattern = this.studio.pattern();
    if (!pattern) return;
    const length = pattern.length;
    const notes = this.studio.notes(pattern.id, channel.id).map((n) => ({
      ...n,
      start: (((n.start + by * STEP) % length) + length) % length,
    }));
    void this.studio.setNotes(pattern.id, channel.id, notes);
  }

  protected dragOver(event: DragEvent): void {
    if (event.dataTransfer?.types.includes(SOUND_DRAG)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'copy';
    }
  }

  /** A sound dropped: on a sampler, its new sound; anywhere else, a sampler of its own. */
  protected async drop(event: DragEvent): Promise<void> {
    const path = event.dataTransfer?.getData(SOUND_DRAG);
    if (!path) return;
    event.preventDefault();
    const id = (event.target as Element).closest('[data-channel]')?.getAttribute('data-channel');
    const [sound] = await this.files.import([path]);
    if (!sound) return;
    const channel = this.studio.song()?.channels.find((c) => c.id === id);
    if (channel && builtinName(channel.instrument) === 'sonevo.sampler') {
      await this.studio.putChannel({
        ...channel,
        instrument: { ...channel.instrument, sample: sound.sample },
      });
    } else {
      await this.studio.addChannel('sonevo.sampler', sound.label, sound.sample);
    }
  }

  protected setSwing(swing: number): void {
    const pattern = this.studio.pattern();
    if (pattern)
      void this.studio.edit({ type: 'putPattern', pattern: { ...pattern, swing }, index: null });
  }

  protected toggleMute(channel: Channel): void {
    void this.studio.putChannel({ ...channel, mute: !channel.mute });
  }

  protected livePan(channel: Channel, pan: number): void {
    void api.liveChannel(channel.id, channel.volume, pan);
  }

  protected setPan(channel: Channel, pan: number): void {
    void this.studio.putChannel({ ...channel, pan });
  }

  protected liveVolume(channel: Channel, volume: number): void {
    void api.liveChannel(channel.id, volume, channel.pan);
  }

  protected setVolume(channel: Channel, volume: number): void {
    void this.studio.putChannel({ ...channel, volume });
  }

  protected routeMenu(event: MouseEvent, channel: Channel): void {
    const song = this.studio.song();
    if (!song) return;
    const t = this.t;
    const items: MenuItem[] = [
      { label: t('Master'), run: () => this.studio.putChannel({ ...channel, insert: undefined }) },
      ...song.inserts.map((insert, i) => ({
        label: `${i + 1} · ${this.named(insert.name)}`,
        run: () => this.studio.putChannel({ ...channel, insert: insert.id }),
      })),
      { separator: true, label: '' },
      { label: t('A new mixer track'), run: () => this.routeToNew(channel) },
    ];
    this.menus.below(event, items);
  }

  /** A mixer track of the channel's own, named and coloured as it is: undone in one. */
  private async routeToNew(channel: Channel): Promise<void> {
    const insert: Insert = {
      id: newId(),
      name: channel.name,
      color: channel.color,
      volume: 1,
      pan: 0,
    };
    await this.studio.edit({
      type: 'batch',
      edits: [
        { type: 'putInsert', insert, index: null },
        { type: 'putChannel', channel: { ...channel, insert: insert.id }, index: null },
      ],
    });
  }

  protected rowMenu(event: MouseEvent, channel: Channel): void {
    const song = this.studio.song();
    if (!song) return;
    const t = this.t;
    const index = song.channels.findIndex((c) => c.id === channel.id);
    this.menus.show(event, [
      { label: t('Open in the piano roll'), run: () => this.openRoll(channel) },
      {
        label: t('Rename…'),
        run: () => {
          void this.rename.ask(t('Name of the channel'), channel.name).then((name) => {
            if (name) void this.studio.putChannel({ ...channel, name });
          });
        },
      },
      {
        label: t('Change the colour'),
        run: () => {
          const next = COLORS[(COLORS.indexOf(channel.color) + 1) % COLORS.length];
          void this.studio.putChannel({ ...channel, color: next });
        },
      },
      { label: t('Duplicate'), run: () => this.duplicate(channel, index) },
      {
        label: t('Automate its volume'),
        run: () =>
          this.studio.automate(
            { type: 'channelVolume', channel: channel.id },
            channel.volume,
            `${channel.name} · ${t('Volume')}`,
          ),
      },
      {
        label: t('Automate its pan'),
        run: () =>
          this.studio.automate(
            { type: 'channelPan', channel: channel.id },
            (channel.pan + 1) / 2,
            `${channel.name} · ${t('Pan')}`,
          ),
      },
      { separator: true, label: '' },
      {
        label: t('Move up'),
        disabled: index === 0,
        run: () => this.studio.putChannel(channel, index - 1),
      },
      {
        label: t('Move down'),
        disabled: index === song.channels.length - 1,
        run: () => this.studio.putChannel(channel, index + 1),
      },
      { separator: true, label: '' },
      ...[2, 4, 8].map((every) => ({
        label: t('Fill every {n} steps', { n: every }),
        run: () => this.fill(channel, every),
      })),
      { label: t('Move its notes a step later'), run: () => this.shift(channel, 1) },
      { label: t('Move its notes a step earlier'), run: () => this.shift(channel, -1) },
      {
        label: t('Clear its notes in this pattern'),
        run: () => {
          const pattern = this.studio.pattern();
          if (pattern) void this.studio.setNotes(pattern.id, channel.id, []);
        },
      },
      {
        label: t('Delete the channel'),
        danger: true,
        run: () => this.studio.edit({ type: 'removeChannel', id: channel.id }),
      },
    ]);
  }

  private duplicate(channel: Channel, index: number): void {
    const copy: Channel = {
      ...channel,
      id: newId(),
      name: `${channel.name} 2`,
      instrument: { ...channel.instrument, id: newId(), params: { ...channel.instrument.params } },
    };
    void this.studio.putChannel(copy, index + 1);
  }

  protected addMenu(event: MouseEvent): void {
    const t = this.t;
    const drums = DRUM_SOUNDS.map((sound, i) => ({
      label: `${t('Drums')} · ${t(sound)}`,
      run: () => this.addDrum(t(sound), i),
    }));
    this.menus.below(event, [
      { label: t('Synth'), run: () => this.studio.addChannel('sonevo.synth', t('Synth')) },
      { label: t('FM synth'), run: () => this.studio.addChannel('sonevo.fm', t('FM synth')) },
      { label: t('Strings'), run: () => this.studio.addChannel('sonevo.strings', t('Strings')) },
      { label: t('Sampler, with a sound…'), run: () => this.addSampler() },
      { separator: true, label: '' },
      ...drums,
    ]);
  }

  private async addDrum(name: string, sound: number): Promise<void> {
    await this.studio.addChannel('sonevo.drums', name);
    const channel = this.studio.channel();
    if (channel && sound > 0) {
      await this.studio.putChannel({
        ...channel,
        instrument: { ...channel.instrument, params: { sound } },
      });
    }
  }

  private async addSampler(): Promise<void> {
    const [sound] = await this.files.importSounds();
    if (sound) await this.studio.addChannel('sonevo.sampler', sound.label, sound.sample);
  }
}
