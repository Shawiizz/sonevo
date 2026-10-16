import {
  ChangeDetectionStrategy,
  Component,
  computed,
  HostListener,
  inject,
  signal,
} from '@angular/core';
import { api } from './core/api';
import { Kept } from './core/models';
import { I18n } from './core/i18n';
import { Dock, Studio } from './core/studio.service';
import { Browser } from './features/browser';
import { ChannelRack } from './features/channel-rack';
import { DevicePanel } from './features/device-panel';
import { Mixer } from './features/mixer';
import { PatternEditor } from './features/pattern-editor';
import { PianoRoll } from './features/piano-roll';
import { Playlist } from './features/playlist';
import { VersionsPanel } from './features/versions-panel';
import { Files, TEMPLATES } from './shell/files.service';
import { Command, Palette } from './shell/palette';
import { ExportDialog } from './shell/export-dialog';
import { Guide } from './shell/guide';
import { RecoveryDialog } from './shell/recovery-dialog';
import { SettingsDialog } from './shell/settings-dialog';
import { MixCheck } from './shell/mix-check';
import { Shortcuts } from './shell/shortcuts';
import { Titlebar } from './shell/titlebar';
import { Transport } from './shell/transport';
import { Icon } from './shared/icon';
import { Menus } from './shared/menu';
import { RenameField } from './shared/rename';

const GUIDE_SEEN = 'sonevo.guide.seen';

function seen(): boolean {
  try {
    return localStorage.getItem(GUIDE_SEEN) === '1';
  } catch {
    return false;
  }
}

/** The computer's keys as a piano, by where they are whatever the layout: two octaves. */
const PIANO: Record<string, number> = {
  KeyZ: 0,
  KeyS: 1,
  KeyX: 2,
  KeyD: 3,
  KeyC: 4,
  KeyV: 5,
  KeyG: 6,
  KeyB: 7,
  KeyH: 8,
  KeyN: 9,
  KeyJ: 10,
  KeyM: 11,
  Comma: 12,
  KeyQ: 12,
  Digit2: 13,
  KeyW: 14,
  Digit3: 15,
  KeyE: 16,
  KeyR: 17,
  Digit5: 18,
  KeyT: 19,
  Digit6: 20,
  KeyY: 21,
  Digit7: 22,
  KeyU: 23,
  KeyI: 24,
  Digit9: 25,
  KeyO: 26,
  Digit0: 27,
  KeyP: 28,
};

@Component({
  selector: 'app-root',
  imports: [
    Browser,
    ChannelRack,
    DevicePanel,
    ExportDialog,
    Guide,
    Icon,
    Mixer,
    PatternEditor,
    Palette,
    PianoRoll,
    Playlist,
    RecoveryDialog,
    RenameField,
    SettingsDialog,
    MixCheck,
    Shortcuts,
    Titlebar,
    Transport,
    VersionsPanel,
  ],
  template: `
    <app-titlebar
      (settings)="settingsOpen.set(true)"
      (palette)="paletteOpen.set(true)"
      (guide)="guideOpen.set(true)"
    />
    <app-transport />
    <div class="body">
      <aside class="side left"><app-browser /></aside>
      <main class="center">
        @if (studio.editing()) {
          <app-pattern-editor />
        } @else {
          <section class="playlist" [style.flex-basis.%]="100 - dockShare()">
            <app-playlist />
          </section>
          <div class="splitter" (pointerdown)="split($event)"></div>
          <section class="dock" [style.flex-basis.%]="dockShare()">
            <div class="tabs">
              @for (tab of tabs; track tab.id) {
                <button
                  class="tab"
                  [class.on]="studio.dock() === tab.id"
                  (click)="studio.dock.set(tab.id)"
                >
                  <s-icon [name]="tab.icon" />
                  {{ t(tab.label) }}
                  <span class="s-key">{{ tab.key }}</span>
                </button>
              }
              <span class="grow"></span>
              <span class="piano-hint">
                <s-icon name="keyboard" /> {{ t('Your keyboard plays the channel chosen') }} ·
                {{ t('octave') }} {{ octave() }}
              </span>
            </div>
            <div class="dock-body">
              @switch (studio.dock()) {
                @case ('rack') {
                  <app-channel-rack />
                }
                @case ('roll') {
                  <app-piano-roll />
                }
                @case ('mixer') {
                  <app-mixer />
                }
              }
            </div>
          </section>
        }
      </main>
      <aside class="side right">
        <div class="side-tabs">
          <div class="s-seg" role="radiogroup">
            <button
              role="radio"
              [attr.aria-checked]="side() === 'device'"
              (click)="side.set('device')"
            >
              {{ t('Device') }}
            </button>
            <button
              role="radio"
              [attr.aria-checked]="side() === 'versions'"
              (click)="side.set('versions')"
            >
              {{ t('Versions') }}
            </button>
          </div>
        </div>
        <div class="side-body">
          @if (side() === 'device') {
            <app-device-panel />
          } @else {
            <app-versions-panel />
          }
        </div>
      </aside>
    </div>

    @if (menus.open(); as menu) {
      <div
        class="menu-catch"
        (pointerdown)="menus.close()"
        (contextmenu)="$event.preventDefault(); menus.close()"
      ></div>
      <div
        class="s-popover"
        [style.left.px]="menuX(menu.x)"
        [style.top.px]="menuY(menu.y, menu.items.length)"
      >
        @for (item of menu.items; track $index) {
          @if (item.separator) {
            <div class="s-menu-sep"></div>
          } @else {
            <button
              class="s-menu-item"
              [class.danger]="item.danger"
              [disabled]="item.disabled"
              (click)="menus.close(); item.run?.()"
            >
              {{ item.label }}
              @if (item.key) {
                <span class="s-key">{{ item.key }}</span>
              }
            </button>
          }
        }
      </div>
    }
    @if (kept().length) {
      <app-recovery-dialog [kept]="kept()" (closed)="kept.set([])" (done)="forgot($event)" />
    }
    <s-rename />
    @if (guideOpen() && !kept().length) {
      <app-guide (closed)="closeGuide()" />
    }
    @if (files.exporting()) {
      <app-export-dialog (closed)="files.exporting.set(false)" />
    }
    @if (studio.checking()) {
      <app-mix-check (closed)="studio.checking.set(false)" />
    }
    @if (shortcutsOpen()) {
      <app-shortcuts (closed)="shortcutsOpen.set(false)" />
    }
    @if (settingsOpen()) {
      <app-settings-dialog (closed)="settingsOpen.set(false)" />
    }
    @if (paletteOpen()) {
      <app-palette [commands]="commands()" (closed)="paletteOpen.set(false)" />
    }
    @if (toast(); as toast) {
      <div class="toast" [class.bad]="!!studio.error()">{{ toast }}</div>
    }
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      height: 100vh;
    }
    .body {
      flex: 1;
      min-height: 0;
      display: flex;
    }
    .side {
      flex: none;
      min-height: 0;
      background: var(--s-base);
      &.left {
        width: 230px;
        border-right: 1px solid var(--s-line);
      }
      &.right {
        width: 290px;
        display: flex;
        flex-direction: column;
        border-left: 1px solid var(--s-line);
      }
    }
    .side-tabs {
      flex: none;
      display: flex;
      align-items: center;
      height: var(--s-panel-head);
      padding: 0 8px;
      border-bottom: 1px solid var(--s-line);
    }
    .side-body {
      flex: 1;
      min-height: 0;
    }
    .center {
      flex: 1;
      min-width: 0;
      display: flex;
      flex-direction: column;
    }
    .playlist,
    .dock {
      min-height: 0;
      flex-grow: 0;
      flex-shrink: 0;
      background: var(--s-base);
    }
    .playlist {
      flex-grow: 1;
    }
    .dock {
      display: flex;
      flex-direction: column;
    }
    .splitter {
      flex: none;
      height: 5px;
      background: var(--s-void);
      border-top: 1px solid var(--s-line);
      border-bottom: 1px solid var(--s-line);
      cursor: row-resize;
      &:hover {
        background: var(--s-line-strong);
      }
    }
    .tabs {
      flex: none;
      display: flex;
      align-items: center;
      gap: 2px;
      height: var(--s-panel-head);
      padding: 0 6px;
      border-bottom: 1px solid var(--s-line);
    }
    .tab {
      height: 30px;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 0 12px;
      border: 0;
      border-radius: var(--s-radius);
      background: transparent;
      color: var(--s-text-2);
      font-weight: 500;
      cursor: pointer;
      .s-key {
        color: var(--s-text-3);
        font-size: 10px;
      }
      &:hover {
        color: var(--s-text);
        background: var(--s-hover);
      }
      &.on {
        color: var(--s-text);
        background: var(--s-raised);
        box-shadow: inset 0 -2px 0 var(--s-accent);
      }
    }
    .grow {
      flex: 1;
    }
    .piano-hint {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding-right: 8px;
      color: var(--s-text-3);
      font-size: 11px;
    }
    .dock-body {
      flex: 1;
      min-height: 0;
    }
    .menu-catch {
      position: fixed;
      inset: 0;
      z-index: 19;
    }
    .s-menu-item.danger {
      color: var(--s-danger);
    }
    .toast {
      position: fixed;
      bottom: 18px;
      left: 50%;
      z-index: 50;
      max-width: 70vw;
      padding: 10px 16px;
      transform: translateX(-50%);
      border: 1px solid var(--s-line-strong);
      border-radius: var(--s-radius-panel);
      background: var(--s-raised);
      box-shadow: 0 12px 32px rgb(0 0 0 / 0.5);
      &.bad {
        border-color: var(--s-danger);
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class App {
  protected readonly studio = inject(Studio);
  protected readonly menus = inject(Menus);
  protected readonly t = inject(I18n).t;
  protected readonly files = inject(Files);

  protected readonly tabs: { id: Dock; label: string; icon: string; key: string }[] = [
    { id: 'rack', label: 'Channel rack', icon: 'rack', key: 'F6' },
    { id: 'roll', label: 'Piano roll', icon: 'piano', key: 'F7' },
    { id: 'mixer', label: 'Mixer', icon: 'mixer', key: 'F9' },
  ];
  protected readonly side = signal<'device' | 'versions'>('device');
  protected readonly settingsOpen = signal(false);
  protected readonly shortcutsOpen = signal(false);
  protected readonly guideOpen = signal(!seen());
  protected readonly kept = signal<Kept[]>([]);

  constructor() {
    void api.kept().then((kept) => this.kept.set(kept));
  }

  protected closeGuide(): void {
    this.guideOpen.set(false);
    try {
      localStorage.setItem(GUIDE_SEEN, '1');
    } catch {
      // without storage it shows again next time: no harm
    }
  }

  protected forgot(k: Kept): void {
    this.kept.update((all) => all.filter((x) => x.path !== k.path));
  }
  protected readonly paletteOpen = signal(false);
  protected readonly dockShare = signal(46);
  protected readonly octave = signal(4);
  private readonly held = new Map<string, { channel: string; key: number }>();

  protected readonly toast = computed(() => {
    const error = this.studio.error();
    return error ? this.t(error) : (this.files.busy() ?? this.files.notice());
  });

  protected readonly commands = computed<Command[]>(() => {
    const t = this.t;
    const list: Command[] = [
      { label: t('Play / pause'), key: 'Space', run: () => this.studio.togglePlay() },
      { label: t('Stop'), run: () => api.stop() },
      { label: t('New song'), key: 'Ctrl+N', run: () => this.files.newSong() },
      { label: t('Open the demo'), run: () => this.files.newSong('demo') },
      ...TEMPLATES.map((s) => ({
        label: t('New song from the template {name}', { name: t(s.label) }),
        run: () => this.files.newSong(s.id),
      })),
      { label: t('Open…'), key: 'Ctrl+O', run: () => this.files.open() },
      { label: t('Save'), key: 'Ctrl+S', run: () => this.files.save() },
      { label: t('Save as…'), key: 'Ctrl+Shift+S', run: () => this.files.saveAs() },
      { label: t('Export as WAV or FLAC…'), key: 'Ctrl+E', run: () => this.files.exportWav() },
      { label: t('Import a MIDI file…'), run: () => this.files.importMidi() },
      { label: t('Export the pattern as MIDI…'), run: () => this.files.exportMidi() },
      { label: t('Export the song as MIDI…'), run: () => this.files.exportMidi(true) },
      { label: t('Undo'), key: 'Ctrl+Z', run: () => this.studio.undo() },
      { label: t('Redo'), key: 'Ctrl+Y', run: () => this.studio.redo() },
      { label: t('Show the channel rack'), key: 'F6', run: () => this.studio.dock.set('rack') },
      { label: t('Show the piano roll'), key: 'F7', run: () => this.studio.dock.set('roll') },
      { label: t('Show the mixer'), key: 'F9', run: () => this.studio.dock.set('mixer') },
      { label: t('New pattern'), run: () => this.studio.addPattern() },
      { label: t('Add a mixer track'), run: () => this.studio.addInsert() },
      { label: t('Add a synth'), run: () => this.studio.addChannel('sonevo.synth', t('Synth')) },
      { label: t('Add drums'), run: () => this.studio.addChannel('sonevo.drums', t('Drums')) },
      {
        label: t('Play the pattern alone'),
        run: () => this.studio.setMode(this.studio.pattern()?.id ?? null),
      },
      { label: t('Play the whole song'), run: () => this.studio.setMode(null) },
      { label: t('Cut every sound'), run: () => api.panic() },
      { label: t('Settings'), run: () => this.settingsOpen.set(true) },
      { label: t('Shortcuts'), key: 'F1', run: () => this.shortcutsOpen.set(true) },
      { label: t('Check the mix'), run: () => this.studio.checking.set(true) },
    ];
    for (const p of this.studio.song()?.patterns ?? []) {
      list.push({
        label: `${t('Edit pattern')} ${p.name}`,
        run: () => this.studio.selectPattern(p.id),
      });
    }
    for (const c of this.studio.song()?.channels ?? []) {
      list.push({
        label: `${t('Piano roll of')} ${c.name}`,
        run: () => {
          this.studio.channelId.set(c.id);
          this.studio.dock.set('roll');
        },
      });
    }
    return list;
  });

  protected menuX(x: number): number {
    return Math.min(x, window.innerWidth - 240);
  }

  protected menuY(y: number, items: number): number {
    return Math.min(y, window.innerHeight - items * 31 - 16);
  }

  protected split(event: PointerEvent): void {
    const splitter = event.currentTarget as HTMLElement;
    splitter.setPointerCapture(event.pointerId);
    const center = splitter.parentElement!.getBoundingClientRect();
    const move = (e: PointerEvent) => {
      const share = ((center.bottom - e.clientY) / center.height) * 100;
      this.dockShare.set(Math.min(85, Math.max(18, share)));
    };
    const up = () => {
      splitter.removeEventListener('pointermove', move as EventListener);
      splitter.removeEventListener('pointerup', up);
    };
    splitter.addEventListener('pointermove', move as EventListener);
    splitter.addEventListener('pointerup', up);
  }

  private typing(event: KeyboardEvent): boolean {
    const target = event.target as HTMLElement;
    return target.closest('input, textarea, select, [contenteditable]') !== null;
  }

  @HostListener('window:keydown', ['$event'])
  protected keydown(event: KeyboardEvent): void {
    if (this.typing(event) || this.settingsOpen() || this.paletteOpen()) return;
    const ctrl = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();
    if (ctrl) {
      const run: Record<string, () => unknown> = {
        z: () => (event.shiftKey ? this.studio.redo() : this.studio.undo()),
        y: () => this.studio.redo(),
        s: () => (event.shiftKey ? this.files.saveAs() : this.files.save()),
        o: () => this.files.open(),
        n: () => this.files.newSong(),
        e: () => this.files.exportWav(),
        k: () => this.paletteOpen.set(true),
      };
      if (run[key]) {
        event.preventDefault();
        void run[key]();
      }
      return;
    }
    if (event.code === 'Space') {
      event.preventDefault();
      this.studio.togglePlay();
      return;
    }
    if (event.key === 'Escape' && this.studio.editing() && !this.menus.open()) {
      event.preventDefault();
      void this.studio.closePattern();
      return;
    }
    if (event.key === 'F1') {
      event.preventDefault();
      this.shortcutsOpen.set(true);
      return;
    }
    const docks: Record<string, Dock> = { F6: 'rack', F7: 'roll', F9: 'mixer' };
    if (docks[event.key]) {
      event.preventDefault();
      if (this.studio.editing()) void this.studio.closePattern();
      this.studio.dock.set(docks[event.key]);
      return;
    }
    if (event.code === 'NumpadAdd' || event.code === 'NumpadSubtract') {
      this.octave.update((o) =>
        Math.min(8, Math.max(0, o + (event.code === 'NumpadAdd' ? 1 : -1))),
      );
      return;
    }
    const step = PIANO[event.code];
    const channel = this.studio.channel();
    if (step === undefined || !channel || event.repeat || event.altKey) return;
    if (this.held.has(event.code)) return;
    const note = Math.min(127, (this.octave() - 1) * 12 + step);
    this.held.set(event.code, { channel: channel.id, key: note });
    void api.note(channel.id, note, 0.8, true);
    this.studio.played(note, 0.8, true);
  }

  @HostListener('window:keyup', ['$event'])
  protected keyup(event: KeyboardEvent): void {
    const held = this.held.get(event.code);
    if (!held) return;
    this.held.delete(event.code);
    void api.note(held.channel, held.key, 0, false);
    this.studio.played(held.key, 0.8, false);
  }

  @HostListener('window:blur')
  protected blur(): void {
    for (const held of this.held.values()) void api.note(held.channel, held.key, 0, false);
    this.held.clear();
  }
}
