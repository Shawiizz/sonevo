import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { builtinName, COLORS, Device, Insert, newId } from '../core/models';
import { Studio } from '../core/studio.service';
import { Icon } from '../shared/icon';
import { Knob } from '../shared/knob';
import { MenuItem, Menus } from '../shared/menu';
import { gainToDb, Meter } from '../shared/meter';
import { Spectrum } from '../shared/spectrum';
import { Rename } from '../shared/rename';

/** The fader's travel: from silence to +2 dB, unity at 80%. */
function faderToGain(position: number): number {
  return position <= 0 ? 0 : Math.pow(position / 0.8, 2.5) * 1;
}

function gainToFader(gain: number): number {
  return gain <= 0 ? 0 : 0.8 * Math.pow(gain, 1 / 2.5);
}

const MAX_FADER = gainToFader(1.25);

/**
 * The mixer: the master and every mixer track as a strip, its effects at the top, its
 * level, pan, mute and solo, where it goes. Channels are sent to a track from the rack.
 */
/** The effect copied last, to be pasted on any mixer track. */
let copied: Device | null = null;

@Component({
  selector: 'app-mixer',
  imports: [Icon, Knob, Meter, Spectrum],
  template: `
    <div class="strips">
      @for (insert of strips(); track insert.id) {
        <div
          class="strip"
          [class.master]="insert.id === master()?.id"
          [style.--color]="insert.color"
          (contextmenu)="stripMenu($event, insert)"
        >
          <div class="name" [title]="named(insert.name)">
            <span class="num s-mono">{{ number(insert) }}</span>
            <span class="label">{{ named(insert.name) }}</span>
          </div>
          <div class="effects">
            @for (effect of insert.effects ?? []; track effect.id) {
              <button
                class="effect"
                [class.off]="effect.enabled === false"
                [class.shown]="studio.device()?.device?.id === effect.id"
                (click)="show(effect)"
                (contextmenu)="effectMenu($event, insert, effect)"
              >
                {{ label(effect) }}
              </button>
            }
            <button class="effect add" (click)="addMenu($event, insert)">
              <s-icon name="plus" [size]="12" /> {{ t('Effect') }}
            </button>
          </div>
          @if (insert.sends?.length) {
            <div class="sends">
              @for (send of insert.sends; track send.to) {
                <div
                  class="send"
                  [title]="t('How much is sent to {name}', { name: nameOf(send.to) })"
                >
                  <span>→ {{ nameOf(send.to) }}</span>
                  <s-knob
                    [value]="send.amount"
                    [min]="0"
                    [max]="1"
                    [default]="0.5"
                    [size]="20"
                    [format]="percent"
                    (changed)="setSend(insert, send.to, $event)"
                  />
                </div>
              }
            </div>
          }
          <s-knob
            [value]="insert.pan"
            [min]="-1"
            [max]="1"
            [default]="0"
            [size]="28"
            [format]="panText"
            (live)="livePan(insert, $event)"
            (changed)="setPan(insert, $event)"
          />
          <div class="level">
            <div
              class="fader"
              (pointerdown)="dragFader($event, insert)"
              (dblclick)="setVolume(insert, 1)"
              [title]="t('Double-click: back to 0 dB')"
            >
              <div class="groove"></div>
              <div
                class="cap"
                [style.bottom.%]="(gainToFader(insert.volume) / maxFader) * 100"
              ></div>
            </div>
            <div class="meters">
              <s-meter [peak]="peak(insert)" />
              <s-meter [peak]="peak(insert)" />
            </div>
          </div>
          <span class="db s-mono">{{ dbText(insert.volume) }}</span>
          <div class="buttons">
            <button
              class="toggle"
              [class.on]="insert.mute"
              (click)="toggle(insert, 'mute')"
              [title]="t('Mute')"
            >
              M
            </button>
            @if (insert.id !== master()?.id) {
              <button
                class="toggle solo"
                [class.on]="insert.solo"
                (click)="toggle(insert, 'solo')"
                [title]="t('Solo')"
              >
                S
              </button>
            }
          </div>
          @if (insert.id !== master()?.id) {
            <button class="out" (click)="outputMenu($event, insert)" [title]="t('Where it goes')">
              → {{ outputName(insert) }}
            </button>
          } @else {
            <span class="out">{{ t('Speakers') }}</span>
          }
        </div>
      }
      <div
        class="spectrum"
        [title]="t('The master across the spectrum: the lows on the left, the highs on the right')"
      >
        <span class="title">{{ t('Spectrum') }}</span>
        <s-spectrum />
        <button class="s-btn check" (click)="studio.checking.set(true)">
          {{ t('Check the mix') }}
        </button>
      </div>
      <button class="new" (click)="studio.addInsert()" [title]="t('Add a mixer track')">
        <s-icon name="plus" [size]="18" />
      </button>
    </div>
  `,
  styles: `
    :host {
      display: block;
      height: 100%;
      overflow-x: auto;
      overflow-y: hidden;
    }
    .strips {
      display: flex;
      gap: 6px;
      height: 100%;
      padding: 8px;
    }
    .spectrum {
      flex: none;
      order: -1;
      width: 220px;
      display: flex;
      flex-direction: column;
      gap: 6px;
      padding: 8px;
      margin-right: 8px;
      border-radius: var(--s-radius-panel);
      background: var(--s-raised);
      .title {
        color: var(--s-text-2);
        font-size: 12px;
      }
      s-spectrum {
        flex: 1;
        min-height: 0;
      }
    }
    .strip {
      flex: none;
      width: 96px;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 6px;
      padding: 6px 6px 8px;
      border-radius: var(--s-radius-panel);
      border-top: 3px solid var(--color);
      background: var(--s-raised);
      min-height: 0;
      &.master {
        width: 110px;
        margin-right: 8px;
        background: color-mix(in srgb, var(--s-accent) 8%, var(--s-raised));
      }
    }
    .name {
      width: 100%;
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 12px;
      font-weight: 600;
      .num {
        color: var(--s-text-3);
        font-size: 10px;
      }
      .label {
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
      }
    }
    .effects {
      width: 100%;
      display: flex;
      flex-direction: column;
      gap: 3px;
      min-height: 0;
      max-height: 120px;
      overflow-y: auto;
    }
    .effect {
      height: 22px;
      padding: 0 6px;
      border: 0;
      border-radius: 4px;
      background: var(--s-hover);
      color: var(--s-text);
      font-size: 11px;
      text-align: left;
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
      cursor: pointer;
      &.off {
        color: var(--s-text-3);
        text-decoration: line-through;
      }
      &.shown {
        box-shadow: inset 2px 0 0 var(--s-accent);
      }
      &.add {
        display: flex;
        align-items: center;
        gap: 4px;
        background: transparent;
        color: var(--s-text-3);
        border: 1px dashed var(--s-line-strong);
        &:hover {
          color: var(--s-text);
        }
      }
    }
    s-knob ::ng-deep .value {
      font-size: 9px;
    }
    .sends {
      width: 100%;
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .send {
      display: flex;
      align-items: center;
      gap: 4px;
      span {
        flex: 1;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        color: var(--s-text-3);
        font-size: 11px;
      }
      s-knob {
        min-width: 22px;
      }
      s-knob ::ng-deep .value {
        display: none;
      }
    }
    .level {
      flex: 1;
      min-height: 80px;
      display: flex;
      gap: 6px;
    }
    .fader {
      position: relative;
      width: 26px;
      cursor: ns-resize;
      touch-action: none;
    }
    .groove {
      position: absolute;
      left: 11px;
      top: 6px;
      bottom: 6px;
      width: 4px;
      border-radius: 2px;
      background: var(--s-void);
    }
    .cap {
      position: absolute;
      left: 1px;
      width: 24px;
      height: 12px;
      margin-bottom: -6px;
      border-radius: 3px;
      background: var(--s-text-2);
      box-shadow:
        inset 0 -2px 0 rgb(0 0 0 / 0.3),
        0 0 0 1px rgb(0 0 0 / 0.4);
    }
    .meters {
      display: flex;
      gap: 2px;
      padding: 6px 0;
    }
    .db {
      color: var(--s-text-3);
      font-size: 10px;
    }
    .buttons {
      display: flex;
      gap: 4px;
    }
    .toggle {
      width: 26px;
      height: 22px;
      padding: 0;
      border: 0;
      border-radius: 4px;
      background: var(--s-hover);
      color: var(--s-text-2);
      font-size: 11px;
      font-weight: 700;
      cursor: pointer;
      &.on {
        background: var(--s-danger);
        color: #1a0605;
      }
      &.solo.on {
        background: var(--s-warn);
        color: #1a1203;
      }
    }
    .out {
      width: 100%;
      height: 22px;
      padding: 0 6px;
      border: 0;
      border-radius: 4px;
      background: var(--s-void);
      color: var(--s-text-3);
      font-size: 11px;
      line-height: 22px;
      text-align: center;
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
    }
    button.out {
      cursor: pointer;
      &:hover {
        color: var(--s-text);
      }
    }
    .new {
      flex: none;
      width: 44px;
      border: 1px dashed var(--s-line-strong);
      border-radius: var(--s-radius-panel);
      background: transparent;
      color: var(--s-text-3);
      cursor: pointer;
      &:hover {
        color: var(--s-text);
        background: var(--s-raised);
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Mixer {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  protected readonly named = inject(I18n).named;
  private readonly menus = inject(Menus);
  private readonly rename = inject(Rename);
  protected readonly gainToFader = gainToFader;
  protected readonly maxFader = MAX_FADER;

  protected readonly master = computed(() => this.studio.song()?.master ?? null);
  protected readonly strips = computed(() => {
    const song = this.studio.song();
    return song ? [song.master, ...song.inserts] : [];
  });

  protected number(insert: Insert): string {
    const song = this.studio.song();
    if (!song || insert.id === song.master.id) return 'M';
    return String(song.inserts.findIndex((i) => i.id === insert.id) + 1);
  }

  protected label(effect: Device): string {
    const info = this.studio.info(effect);
    return info ? this.t(info.label) : (this.studio.plugin(effect)?.name ?? '?');
  }

  protected peak(insert: Insert): number {
    const status = this.studio.status();
    return insert.id === this.master()?.id ? status.master : (status.inserts[insert.id] ?? 0);
  }

  protected readonly panText = (v: number) =>
    Math.abs(v) < 0.01 ? 'C' : `${Math.round(Math.abs(v) * 100)}${v < 0 ? 'L' : 'R'}`;

  protected dbText(gain: number): string {
    const db = gainToDb(gain);
    return Number.isFinite(db) ? `${db > 0 ? '+' : ''}${db.toFixed(1)} dB` : '−∞';
  }

  protected readonly percent = (v: number) => `${Math.round(v * 100)}%`;

  protected nameOf(id: string): string {
    return this.studio.song()?.inserts.find((i) => i.id === id)?.name ?? '';
  }

  protected setSend(insert: Insert, to: string, amount: number): void {
    const sends = (insert.sends ?? []).map((s) => (s.to === to ? { ...s, amount } : s));
    void this.studio.putInsert({ ...insert, sends });
  }

  protected outputName(insert: Insert): string {
    const song = this.studio.song();
    const to = song?.inserts.find((i) => i.id === insert.output);
    return to ? to.name : this.t('Master');
  }

  protected show(effect: Device): void {
    this.studio.deviceId.set(effect.id);
  }

  protected livePan(insert: Insert, pan: number): void {
    void api.liveInsert(insert.id, insert.volume, pan);
  }

  protected setPan(insert: Insert, pan: number): void {
    void this.studio.putInsert({ ...insert, pan });
  }

  protected setVolume(insert: Insert, volume: number): void {
    void api.liveInsert(insert.id, volume, insert.pan);
    void this.studio.putInsert({ ...insert, volume });
  }

  protected toggle(insert: Insert, what: 'mute' | 'solo'): void {
    void this.studio.putInsert({ ...insert, [what]: !insert[what] });
  }

  protected dragFader(event: PointerEvent, insert: Insert): void {
    if (event.button !== 0) return;
    const fader = event.currentTarget as HTMLElement;
    fader.setPointerCapture(event.pointerId);
    const box = fader.getBoundingClientRect();
    let volume = insert.volume;
    const cap = fader.querySelector('.cap') as HTMLElement;
    const at = (e: PointerEvent) => {
      const position = Math.min(1, Math.max(0, (box.bottom - 6 - e.clientY) / (box.height - 12)));
      volume = Math.min(1.25, faderToGain(position * MAX_FADER));
      cap.style.bottom = `${position * 100}%`;
      void api.liveInsert(insert.id, volume, insert.pan);
    };
    at(event);
    const up = () => {
      fader.removeEventListener('pointermove', at as EventListener);
      fader.removeEventListener('pointerup', up);
      void this.studio.putInsert({ ...insert, volume });
    };
    fader.addEventListener('pointermove', at as EventListener);
    fader.addEventListener('pointerup', up);
  }

  protected addMenu(event: MouseEvent, insert: Insert): void {
    const add = (kind: Device['kind']) => {
      const effect: Device = { id: newId(), kind, params: {} };
      void this.studio.putInsert({ ...insert, effects: [...(insert.effects ?? []), effect] });
      this.studio.deviceId.set(effect.id);
    };
    const builtins: MenuItem[] = this.studio
      .devices()
      .filter((d) => !d.instrument)
      .map((d) => ({ label: this.t(d.label), run: () => add({ type: 'builtin', name: d.name }) }));
    const plugins: MenuItem[] = this.studio
      .plugins()
      .filter((p) => !p.instrument)
      .map((p) => ({
        label: p.name,
        run: () => add({ type: 'clap', bundle: p.bundle, plugin: p.id }),
      }));
    const pasted: MenuItem[] = copied
      ? [
          {
            label: this.t('Paste the effect copied'),
            run: () => {
              const effect = { ...structuredClone(copied!), id: newId() };
              void this.studio.putInsert({
                ...insert,
                effects: [...(insert.effects ?? []), effect],
              });
              this.studio.deviceId.set(effect.id);
            },
          },
          { separator: true, label: '' },
        ]
      : [];
    this.menus.below(
      event,
      plugins.length
        ? [...pasted, ...builtins, { separator: true, label: '' }, ...plugins]
        : [...pasted, ...builtins],
    );
  }

  protected effectMenu(event: MouseEvent, insert: Insert, effect: Device): void {
    const t = this.t;
    const effects = insert.effects ?? [];
    const index = effects.findIndex((e) => e.id === effect.id);
    const moved = (to: number) => {
      const list = effects.filter((e) => e.id !== effect.id);
      list.splice(to, 0, effect);
      return list;
    };
    this.menus.show(event, [
      { label: t('Show its settings'), run: () => this.show(effect) },
      { label: t('Copy it, settings and all'), run: () => (copied = structuredClone(effect)) },
      {
        label: effect.enabled === false ? t('Turn on') : t('Turn off'),
        run: () =>
          this.studio.putInsert({
            ...insert,
            effects: effects.map((e) =>
              e.id === effect.id ? { ...e, enabled: e.enabled === false } : e,
            ),
          }),
      },
      {
        label: t('Move up'),
        disabled: index === 0,
        run: () => this.studio.putInsert({ ...insert, effects: moved(index - 1) }),
      },
      {
        label: t('Move down'),
        disabled: index === effects.length - 1,
        run: () => this.studio.putInsert({ ...insert, effects: moved(index + 1) }),
      },
      { separator: true, label: '' },
      {
        label: t('Remove'),
        danger: true,
        run: () =>
          this.studio.putInsert({ ...insert, effects: effects.filter((e) => e.id !== effect.id) }),
      },
    ]);
  }

  protected outputMenu(event: MouseEvent, insert: Insert): void {
    const song = this.studio.song();
    if (!song) return;
    const t = this.t;
    const others = song.inserts.filter((i) => i.id !== insert.id);
    const items: MenuItem[] = [
      { label: t('Master'), run: () => this.studio.putInsert({ ...insert, output: undefined }) },
      ...others.map((o) => ({
        label: o.name,
        run: () => this.studio.putInsert({ ...insert, output: o.id }),
      })),
    ];
    if (others.length) {
      items.push({ separator: true, label: '' });
      for (const o of others) {
        const sent = insert.sends?.find((s) => s.to === o.id);
        items.push({
          label: sent
            ? t('Stop sending to {name}', { name: o.name })
            : t('Also send to {name}', { name: o.name }),
          run: () =>
            this.studio.putInsert({
              ...insert,
              sends: sent
                ? (insert.sends ?? []).filter((s) => s.to !== o.id)
                : [...(insert.sends ?? []), { to: o.id, amount: 0.5 }],
            }),
        });
      }
    }
    this.menus.below(event, items);
  }

  protected stripMenu(event: MouseEvent, insert: Insert): void {
    const song = this.studio.song();
    if (!song || insert.id === song.master.id) return;
    const t = this.t;
    this.menus.show(event, [
      {
        label: t('Rename…'),
        run: () => {
          void this.rename
            .ask(t('Name of the mixer track'), this.named(insert.name))
            .then((name) => {
              if (name) void this.studio.putInsert({ ...insert, name });
            });
        },
      },
      {
        label: t('Change the colour'),
        run: () =>
          this.studio.putInsert({
            ...insert,
            color: COLORS[(COLORS.indexOf(insert.color) + 1) % COLORS.length],
          }),
      },
      {
        label: t('Automate its volume'),
        run: () =>
          this.studio.automate(
            { type: 'insertVolume', insert: insert.id },
            insert.volume / 1.25,
            `${this.named(insert.name)} · ${t('Volume')}`,
          ),
      },
      { separator: true, label: '' },
      {
        label: t('Delete the mixer track'),
        danger: true,
        run: () => this.studio.edit({ type: 'removeInsert', id: insert.id }),
      },
    ]);
  }

  protected isBuiltin = builtinName;
}
