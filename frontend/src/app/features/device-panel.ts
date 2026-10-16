import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Device, ParamInfo, sampleLabel } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files } from '../shell/files.service';
import { Icon } from '../shared/icon';
import { Knob, toNormal } from '../shared/knob';
import { Menus } from '../shared/menu';

const ABOUT: Record<string, string> = {
  'sonevo.synth':
    'A synthesizer: two oscillators make the tone, a filter makes it darker or brighter, envelopes shape how each note starts and fades.',
  'sonevo.drums':
    'Drum sounds made from scratch: choose a sound, then tune it and make it shorter or longer.',
  'sonevo.sampler': 'Plays a sound file, higher or lower with the key pressed.',
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
};

/**
 * The device panel: the settings of the instrument or effect chosen, as knobs; choices as
 * buttons; the sampler's sound.
 */
@Component({
  selector: 'app-device-panel',
  imports: [Icon, Knob],
  template: `
    @if (shown(); as shown) {
      <div class="s-panel-head">
        <s-icon [name]="info()?.instrument ? 'synth' : 'effect'" />
        <span>{{ title() }}</span>
        <span class="s-grow"></span>
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
      { name: 'LFO', params: pick(['lfoTarget', 'lfoRate', 'lfoAmount']) },
    ];
  });

  protected value(device: Device, p: ParamInfo): number {
    return this.studio.value(device, p.id);
  }

  protected live(device: Device, p: ParamInfo, value: number): void {
    void api.liveParam(device.id, p.id, value);
  }

  protected set(device: Device, p: ParamInfo, value: number): void {
    void api.liveParam(device.id, p.id, value);
    void this.studio.edit({ type: 'setParam', device: device.id, param: p.id, value });
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
