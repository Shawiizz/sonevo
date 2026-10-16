import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
} from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Arp, Channel, Device, ParamInfo, PPQ, PresetInfo, sampleLabel } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files } from '../shell/files.service';
import { Icon } from '../shared/icon';
import { EqCurve } from '../shared/eq-curve';
import { Knob, toNormal } from '../shared/knob';
import { MenuItem, Menus } from '../shared/menu';
import { Rename } from '../shared/rename';

const ARP_ORDERS: { id: Arp['order']; label: string }[] = [
  { id: 'up', label: 'Up' },
  { id: 'down', label: 'Down' },
  { id: 'upDown', label: 'Up and down' },
  { id: 'random', label: 'Random' },
];
const ARP_STEPS = [
  { ticks: PPQ, label: '1/4' },
  { ticks: PPQ / 2, label: '1/8' },
  { ticks: PPQ / 4, label: '1/16' },
  { ticks: PPQ / 8, label: '1/32' },
  { ticks: PPQ / 3, label: '1/8 T' },
  { ticks: PPQ / 6, label: '1/16 T' },
];

const ABOUT: Record<string, string> = {
  'sonevo.synth':
    'A synthesizer: two oscillators make the tone, a filter makes it darker or brighter, envelopes shape how each note starts and fades.',
  'sonevo.drums':
    'Drum sounds made from scratch: choose a sound, then tune it and make it shorter or longer.',
  'sonevo.sampler': 'Plays a sound file, higher or lower with the key pressed.',
  'sonevo.strings':
    'Plucked strings: a burst of noise rings round a loop as long as the note, as a string does. A guitar, a harp, a koto.',
  'sonevo.fm':
    'An FM synth: one wave bends another, for electric pianos, bells and metallic basses. Brightness is how much; it falls as the note goes on.',
  'sonevo.eq': 'An equalizer: turns the lows, the middle or the highs up or down.',
  'sonevo.compressor':
    'A compressor: brings the loud moments down so the sound is more even and punchy.',
  'sonevo.delay': 'A delay: echoes of the sound, in time with the song.',
  'sonevo.reverb': 'A reverb: the sound as if played in a room or a hall.',
  'sonevo.distortion': 'A distortion: drives the sound until it grits.',
  'sonevo.filter': 'A filter: lets only the lows (or the highs) through; turn the cutoff to sweep.',
  'sonevo.limiter': 'A limiter, for the master: nothing goes past the ceiling, so nothing clips.',
  'sonevo.chorus':
    'A chorus: copies of the sound a little late, so it sounds wider and thicker. Short and with feedback, a flanger.',
  'sonevo.phaser': 'A phaser: notches sweeping up and down the sound, for a swirling, moving tone.',
  'sonevo.crusher':
    'A lo-fi crusher: fewer bits and a lower sample rate, for the grit of an old sampler.',
  'sonevo.autopan':
    'An auto-pan: the sound moves from one side to the other, in time with the song.',
};

/**
 * The device panel: the settings of the instrument or effect chosen, as knobs; choices as
 * buttons; the sampler's sound.
 */
@Component({
  selector: 'app-device-panel',
  imports: [EqCurve, Icon, Knob],
  template: `
    @if (shown(); as shown) {
      <div class="s-panel-head">
        <s-icon [name]="info()?.instrument ? 'synth' : 'effect'" />
        <span>{{ title() }}</span>
        <span class="s-grow"></span>
        @if (info() && shown.device.kind.type === 'builtin') {
          <button
            class="s-bar-btn presets"
            (click)="presetMenu($event, shown.device)"
            [title]="t('Presets')"
          >
            {{ t('Presets') }} <s-icon name="down" [size]="12" />
          </button>
        }
        @if (effect()) {
          <button
            class="s-bar-btn"
            [class.on]="shown.device.enabled !== false"
            (click)="toggle(shown.device)"
            [title]="t('On / off')"
          >
            <s-icon name="power" />
          </button>
        }
      </div>
      <div class="body">
        <p class="owner">{{ shown.owner }}</p>
        @if (about(); as about) {
          <p class="about">{{ t(about) }}</p>
        }
        @if (studio.plugin(shown.device)) {
          <button class="s-btn window" (click)="openWindow(shown.device)">
            <s-icon name="maximize" [size]="12" /> {{ t('Open its window') }}
          </button>
        }
        @if (info()?.name === 'sonevo.eq') {
          <s-eq-curve class="curve" [values]="settings(shown.device)" />
        }
        @if (info()?.name === 'sonevo.compressor' || info()?.name === 'sonevo.limiter') {
          <div class="reduction" [title]="t('How far it brings the sound down now')">
            <span>{{ t('Brought down') }}</span>
            <div class="bar"><div [style.width.%]="(reduction() * 100) / 24"></div></div>
            <span class="s-mono">-{{ reduction().toFixed(1) }} dB</span>
          </div>
        }
        @if (info()?.name === 'sonevo.compressor') {
          <label class="s-field listens">
            <span>{{ t('Listens to') }}</span>
            <select (change)="listen(shown.device, $any($event.target).value)">
              <option value="" [selected]="!shown.device.key">{{ t('Its own sound') }}</option>
              @for (insert of keys(); track insert.id) {
                <option [value]="insert.id" [selected]="shown.device.key === insert.id">
                  {{ insert.name }}
                </option>
              }
            </select>
          </label>
          @if (shown.device.key) {
            <p class="about">
              {{
                t(
                  'When that track gets loud, this one is brought down: the pumping of a bass under the kick.'
                )
              }}
            </p>
          }
        }
        @if (info()?.name === 'sonevo.sampler' && waveform(shown.device); as wave) {
          <svg class="waveform" viewBox="0 0 100 40" preserveAspectRatio="none">
            @for (p of wave.peaks; track $index) {
              <rect
                [attr.x]="$index"
                [attr.y]="20 - p * 19"
                width="0.7"
                [attr.height]="p * 38 + 0.5"
              />
            }
            @for (cut of wave.cuts; track $index) {
              <line [attr.x1]="cut" [attr.x2]="cut" y1="0" y2="40" />
            }
          </svg>
          @if (wave.cuts.length) {
            <button class="s-btn slices" (click)="slicePattern(shown.device)">
              {{ t('Make a pattern of its slices') }}
            </button>
          }
        }
        @if (info()?.name === 'sonevo.sampler') {
          <div class="sample">
            <s-icon name="sample" />
            <span>{{ sampleName(shown.device) }}</span>
            <button class="s-btn" (click)="chooseSample(shown.device)">{{ t('Choose…') }}</button>
          </div>
        }
        @for (group of groups(); track group.name) {
          @if (group.name) {
            <h4>{{ t(group.name) }}</h4>
          }
          <div class="params">
            @for (p of group.params; track p.id) {
              @if (p.scale === 'choice') {
                <div class="choice">
                  <span class="label">{{ t(p.name) }}</span>
                  <div class="options">
                    @for (c of p.choices; track $index) {
                      <button
                        [class.on]="value(shown.device, p) === $index"
                        (click)="set(shown.device, p, $index)"
                      >
                        {{ t(c) }}
                      </button>
                    }
                  </div>
                </div>
              } @else {
                <s-knob
                  (contextmenu)="knobMenu($event, shown.device, p)"
                  [value]="value(shown.device, p)"
                  [min]="p.min"
                  [max]="p.max"
                  [default]="p.default"
                  [log]="p.scale === 'log'"
                  [unit]="p.unit"
                  [label]="t(p.name)"
                  [size]="38"
                  (live)="live(shown.device, p, $event)"
                  (changed)="set(shown.device, p, $event)"
                />
              }
            }
          </div>
        }
        @if (channel(); as channel) {
          <h4>{{ t('Arpeggiator') }}</h4>
          <p class="about">
            {{ t('The keys held played one after the other instead of together.') }}
          </p>
          <div class="choice">
            <div class="options">
              <button [class.on]="!channel.arp" (click)="setArp(channel, null)">
                {{ t('Off') }}
              </button>
              @for (o of arpOrders; track o.id) {
                <button
                  [class.on]="channel.arp?.order === o.id"
                  (click)="setArp(channel, { order: o.id })"
                >
                  {{ t(o.label) }}
                </button>
              }
            </div>
          </div>
          @if (channel.arp; as arp) {
            <div class="choice">
              <span class="label">{{ t('Step') }}</span>
              <div class="options">
                @for (s of arpSteps; track s.ticks) {
                  <button
                    [class.on]="arp.step === s.ticks"
                    (click)="setArp(channel, { step: s.ticks })"
                  >
                    {{ s.label }}
                  </button>
                }
              </div>
            </div>
            <div class="choice">
              <span class="label">{{ t('Octaves') }}</span>
              <div class="options">
                @for (o of [1, 2, 3, 4]; track o) {
                  <button [class.on]="arp.octaves === o" (click)="setArp(channel, { octaves: o })">
                    {{ o }}
                  </button>
                }
              </div>
            </div>
            <div class="params">
              <s-knob
                [value]="arp.gate"
                [min]="0.1"
                [max]="1"
                [default]="0.5"
                [label]="t('Gate')"
                [size]="38"
                (changed)="setArp(channel, { gate: $event })"
              />
            </div>
          }
        }
        @if (!info() && !studio.plugin(shown.device)) {
          <p class="s-empty">{{ t('This plugin is not available on this computer.') }}</p>
        }
      </div>
    } @else {
      <div class="s-panel-head">{{ t('Device') }}</div>
      <p class="s-empty">{{ t('Choose a channel or an effect to see its settings.') }}</p>
    }
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      min-height: 0;
      height: 100%;
    }
    .body {
      flex: 1;
      min-height: 0;
      overflow-y: auto;
      padding: 10px 12px 16px;
    }
    .owner {
      margin: 0 0 4px;
      color: var(--s-text-3);
      font-size: 12px;
    }
    .about {
      margin: 0 0 12px;
      color: var(--s-text-2);
      font-size: 12px;
      line-height: 1.5;
    }
    h4 {
      margin: 14px 0 8px;
      color: var(--s-text-3);
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.05em;
      text-transform: uppercase;
    }
    .params {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));
      gap: 12px 4px;
    }
    .choice {
      grid-column: 1 / -1;
      display: flex;
      flex-direction: column;
      gap: 6px;
      .label {
        color: var(--s-text-2);
        font-size: 11px;
      }
    }
    .options {
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
      button {
        height: 24px;
        padding: 0 8px;
        border: 0;
        border-radius: 4px;
        background: var(--s-hover);
        color: var(--s-text-2);
        font-size: 11px;
        cursor: pointer;
        &:hover {
          color: var(--s-text);
        }
        &.on {
          background: var(--s-accent);
          color: var(--s-on-accent);
        }
      }
    }
    .window {
      margin-bottom: 12px;
    }
    .reduction {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 12px;
      color: var(--s-text-2);
      font-size: 12px;
      .bar {
        flex: 1;
        height: 6px;
        overflow: hidden;
        border-radius: 3px;
        background: var(--s-void);
        div {
          height: 100%;
          max-width: 100%;
          background: var(--s-warn);
          transition: width 60ms linear;
        }
      }
      .s-mono {
        min-width: 64px;
        text-align: right;
      }
    }
    .curve {
      margin-bottom: 12px;
    }
    .waveform {
      display: block;
      width: 100%;
      height: 64px;
      margin-bottom: 8px;
      border-radius: var(--s-radius);
      background: var(--s-void);
      rect {
        fill: var(--s-accent);
        opacity: 0.8;
      }
      line {
        stroke: var(--s-text);
        stroke-width: 0.4;
        vector-effect: non-scaling-stroke;
      }
    }
    .slices {
      margin-bottom: 12px;
    }
    .listens {
      margin-bottom: 12px;
    }
    .sample {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 12px;
      padding: 8px;
      border-radius: var(--s-radius);
      background: var(--s-void);
      span {
        flex: 1;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        font-size: 12px;
      }
      .s-btn {
        height: 26px;
        padding: 0 10px;
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class DevicePanel {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly files = inject(Files);
  private readonly menus = inject(Menus);
  private readonly rename = inject(Rename);

  protected readonly shown = computed(() => this.studio.device());
  protected readonly info = computed(() => {
    const shown = this.shown();
    return shown ? this.studio.info(shown.device) : null;
  });
  protected readonly title = computed(() => {
    const info = this.info();
    const device = this.shown()?.device;
    if (info && device?.kind.type === 'clap') return info.label;
    if (info) return this.t(info.label);
    if (!device || device.kind.type !== 'clap') return '';
    return this.studio.plugin(device)?.name ?? device.kind.plugin;
  });
  protected readonly effect = computed(() => {
    const song = this.studio.song();
    const id = this.shown()?.device.id;
    return (
      !!song && [...song.inserts, song.master].some((i) => i.effects?.some((e) => e.id === id))
    );
  });
  protected readonly about = computed(() => {
    const device = this.shown()?.device;
    if (device?.kind.type === 'clap') {
      return this.studio.plugin(device)
        ? 'A plugin installed on this computer, played in a process of its own: should it crash, Sonevo plays on.'
        : null;
    }
    return ABOUT[this.info()?.name ?? ''] ?? null;
  });

  protected readonly groups = computed(() => {
    const params = this.info()?.params ?? [];
    if (this.info()?.name !== 'sonevo.synth') return [{ name: '', params }];
    const pick = (ids: string[]) => params.filter((p) => ids.includes(p.id));
    return [
      {
        name: 'Oscillators',
        params: pick(['wave1', 'wave2', 'semitones2', 'detune2', 'mix', 'unison', 'spread']),
      },
      {
        name: 'Filter',
        params: pick([
          'cutoff',
          'resonance',
          'envAmount',
          'filterAttack',
          'filterDecay',
          'filterSustain',
          'filterRelease',
        ]),
      },
      { name: 'Volume', params: pick(['attack', 'decay', 'sustain', 'release', 'volume']) },
      { name: 'LFO', params: pick(['lfoTarget', 'lfoSync', 'lfoRate', 'lfoAmount']) },
      { name: 'Voices', params: pick(['voices', 'glide']) },
    ];
  });

  protected value(device: Device, p: ParamInfo): number {
    return this.studio.value(device, p.id);
  }

  /** The settings of `device` as they are now, those being turned as far as they are. */
  protected settings(device: Device): Record<string, number> {
    const turning = this.turning();
    const params = this.info()?.params ?? [];
    return Object.fromEntries(params.map((p) => [p.id, turning[p.id] ?? this.value(device, p)]));
  }

  /** How far the compressor or limiter shown brings the sound down: the most of the last half
   * second, falling back slowly. */
  protected readonly reduction = signal(0);
  private reductions: number[] = [];
  private measured: string | undefined;

  /** The settings being turned, before they are let go of. */
  private readonly turning = signal<Record<string, number>>({});

  protected live(device: Device, p: ParamInfo, value: number): void {
    this.turning.update((t) => ({ ...t, [p.id]: value }));
    void api.liveParam(device.id, p.id, value);
    const normal = toNormal(value, p.min, p.max, p.scale === 'log');
    const name = `${this.shown()?.owner ?? ''} · ${this.t(p.name)}`;
    this.studio.moved({ type: 'param', device: device.id, param: p.id }, normal, name);
  }

  protected set(device: Device, p: ParamInfo, value: number): void {
    this.turning.update(({ [p.id]: _, ...rest }) => rest);
    void api.liveParam(device.id, p.id, value);
    void this.studio.edit({ type: 'setParam', device: device.id, param: p.id, value });
    void this.studio.letGo({ type: 'param', device: device.id, param: p.id });
  }

  protected knobMenu(event: MouseEvent, device: Device, p: ParamInfo): void {
    const t = this.t;
    this.menus.show(event, [
      {
        label: t('Automate this setting'),
        run: () => {
          const normal = toNormal(this.value(device, p), p.min, p.max, p.scale === 'log');
          const name = `${this.shown()?.owner ?? ''} · ${t(p.name)}`;
          void this.studio.automate(
            { type: 'param', device: device.id, param: p.id },
            normal,
            name,
          );
        },
      },
      { label: t('Back to its default'), run: () => this.set(device, p, p.default) },
    ]);
  }

  /** The sampler's sound as a hundred peaks, and where its slices are cut, along 100. */
  protected waveform(device: Device): { peaks: number[]; cuts: number[] } | null {
    const info = device.sample ? this.studio.sample(device.sample) : null;
    if (!info?.peaks.length) return null;
    const peaks = Array.from({ length: 100 }, (_, i) => {
      const from = Math.floor((i / 100) * info.peaks.length);
      const to = Math.max(from + 1, Math.floor(((i + 1) / 100) * info.peaks.length));
      return Math.max(...info.peaks.slice(from, to));
    });
    const count = this.slices(device);
    const cuts = Array.from({ length: Math.max(0, count - 1) }, (_, i) => ((i + 1) * 100) / count);
    return { peaks, cuts };
  }

  private slices(device: Device): number {
    const choice = Math.round(this.studio.value(device, 'slices'));
    return choice > 0 ? 2 << choice : 0;
  }

  /** A pattern playing the slices one after the other, as long as the sound is at this tempo. */
  protected slicePattern(device: Device): void {
    const channel = this.channel();
    const info = device.sample ? this.studio.sample(device.sample) : null;
    const count = this.slices(device);
    if (!channel || !info || !count) return;
    const root = Math.round(this.studio.value(device, 'root'));
    const each = this.studio.ticks(info.seconds) / count;
    const notes = Array.from({ length: count }, (_, i) => ({
      start: Math.round(i * each),
      length: Math.max(1, Math.round(each)),
      key: root + i,
      velocity: 0.8,
    })).filter((n) => n.key <= 127);
    void this.studio.addPatternOf(`${channel.name} ${this.t('slices')}`, channel.id, notes);
  }

  constructor() {
    effect(() => {
      const id = this.shown()?.device.id;
      if (id !== this.measured) this.reductions = [];
      this.measured = id;
      const now = id ? (this.studio.status().reductions[id] ?? 0) : 0;
      this.reductions = [...this.reductions, now].slice(-15);
      this.reduction.set(Math.max(...this.reductions));
    });
    // a song changed, or another device shown: what the knobs say is the song's again
    effect(() => {
      this.studio.view();
      this.shown();
      this.turning.set({});
    });
  }

  protected readonly arpOrders = ARP_ORDERS;
  protected readonly arpSteps = ARP_STEPS;

  /** The channel whose instrument is shown, if it is one. */
  protected readonly channel = computed(() => {
    const id = this.shown()?.device.id;
    return this.studio.song()?.channels.find((c) => c.instrument.id === id) ?? null;
  });

  protected setArp(channel: Channel, change: Partial<Arp> | null): void {
    const arp: Arp | undefined = change
      ? { order: 'up', step: PPQ / 4, octaves: 1, gate: 0.5, ...channel.arp, ...change }
      : undefined;
    void this.studio.putChannel({ ...channel, arp });
  }

  protected async presetMenu(event: MouseEvent, device: Device): Promise<void> {
    const info = this.info();
    if (!info || device.kind.type !== 'builtin') return;
    const kind = device.kind.name;
    const t = this.t;
    const button = event.currentTarget as Element;
    event.stopPropagation();
    let presets: PresetInfo[] = [];
    try {
      presets = await api.presets(kind);
    } catch (e) {
      this.studio.fail(e);
      return;
    }
    const apply = (p: PresetInfo) => () => {
      const edits = info.params.map((param) => ({
        type: 'setParam' as const,
        device: device.id,
        param: param.id,
        value: p.values[param.id] ?? param.default,
      }));
      for (const edit of edits) void api.liveParam(device.id, edit.param, edit.value);
      void this.studio.edit({ type: 'batch', edits });
    };
    const saved = presets.filter((p) => !p.factory);
    const items: MenuItem[] = presets
      .filter((p) => p.factory)
      .map((p) => ({ label: t(p.name), run: apply(p) }));
    if (saved.length) {
      items.push(
        { separator: true, label: '' },
        ...saved.map((p) => ({ label: p.name, run: apply(p) })),
      );
    }
    items.push(
      { separator: true, label: '' },
      {
        label: t('Save these settings as a preset…'),
        run: () => void this.savePreset(device, kind),
      },
    );
    if (saved.length) {
      items.push({
        label: t('Delete a saved preset…'),
        run: () =>
          this.menus.show(
            event,
            saved.map((p) => ({
              label: p.name,
              danger: true,
              run: () => void api.deletePreset(kind, p.name).catch((e) => this.studio.fail(e)),
            })),
          ),
      });
    }
    this.menus.under(button, items);
  }

  private async savePreset(device: Device, kind: string): Promise<void> {
    const info = this.info();
    const name = await this.rename.ask(this.t('Name of the preset'), '');
    if (!info || !name) return;
    const values = Object.fromEntries(info.params.map((p) => [p.id, this.value(device, p)]));
    try {
      await api.savePreset(kind, name, values);
    } catch (e) {
      this.studio.fail(e);
    }
  }

  protected toggle(device: Device): void {
    this.change(device, { enabled: device.enabled === false });
  }

  /** The mixer tracks an effect may listen to: any but its own and the master. */
  protected readonly keys = computed(() => {
    const song = this.studio.song();
    const device = this.shown()?.device;
    if (!song || !device) return [];
    return song.inserts.filter((i) => !i.effects?.some((e) => e.id === device.id));
  });

  protected listen(device: Device, key: string): void {
    this.change(device, { key: key || undefined });
  }

  private change(device: Device, change: Partial<Device>): void {
    const song = this.studio.song();
    if (!song) return;
    for (const insert of [...song.inserts, song.master]) {
      if (insert.effects?.some((e) => e.id === device.id)) {
        void this.studio.putInsert({
          ...insert,
          effects: insert.effects.map((e) => (e.id === device.id ? { ...e, ...change } : e)),
        });
      }
    }
  }

  protected async openWindow(device: Device): Promise<void> {
    if (!(await api.openPluginWindow(device.id)))
      this.studio.fail(this.t('This plugin has no window of its own.'));
  }

  protected sampleName(device: Device): string {
    return device.sample ? sampleLabel(device.sample) : this.t('No sound yet');
  }

  protected async chooseSample(device: Device): Promise<void> {
    const [sound] = await this.files.importSounds();
    const song = this.studio.song();
    const channel = song?.channels.find((c) => c.instrument.id === device.id);
    if (!sound || !channel) return;
    await this.studio.putChannel({
      ...channel,
      name: sound.label,
      instrument: { ...device, sample: sound.sample },
    });
  }
}
