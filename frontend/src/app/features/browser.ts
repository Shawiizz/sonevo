import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Entry, sampleLabel } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files, soundName } from '../shell/files.service';
import { Icon } from '../shared/icon';
import { SOUND_DRAG } from './playlist';

/**
 * The browser: Sonevo's instruments, to add to the rack; and the sounds on the disk, to be
 * dragged into the playlist or played by a sampler.
 */
@Component({
  selector: 'app-browser',
  imports: [Icon],
  template: `
    <div class="s-panel-head">
      <div class="s-seg" role="radiogroup">
        <button
          role="radio"
          [attr.aria-checked]="tab() === 'instruments'"
          (click)="tab.set('instruments')"
        >
          {{ t('Instruments') }}
        </button>
        <button role="radio" [attr.aria-checked]="tab() === 'sounds'" (click)="openSounds()">
          {{ t('Sounds') }}
        </button>
      </div>
    </div>
    @if (tab() === 'instruments') {
      <div class="list">
        <h4>{{ t('Instruments') }}</h4>
        @for (d of instruments(); track d.name) {
          <button
            class="item"
            (click)="add(d.name, d.label)"
            [title]="t('Add to the channel rack')"
          >
            <s-icon
              [name]="
                d.name === 'sonevo.drums'
                  ? 'drum'
                  : d.name === 'sonevo.sampler'
                    ? 'sample'
                    : 'synth'
              "
            />
            <span>{{ t(d.label) }}</span>
            <s-icon class="plus" name="plus" [size]="12" />
          </button>
        }
        <h4>{{ t('Effects') }}</h4>
        <p class="hint">{{ t('Add effects from a mixer track, with its « + Effect ».') }}</p>
        @for (d of effects(); track d.name) {
          <div class="item static">
            <s-icon name="effect" />
            <span>{{ t(d.label) }}</span>
          </div>
        }
        <h4>{{ t('Plugins') }}</h4>
        @for (p of studio.plugins(); track p.bundle + p.id) {
          @if (p.instrument) {
            <button
              class="item"
              (click)="studio.addPlugin(p)"
              [title]="p.vendor + ' · ' + t('Add to the channel rack')"
            >
              <s-icon name="synth" />
              <span>{{ p.name }}</span>
              <s-icon class="plus" name="plus" [size]="12" />
            </button>
          } @else {
            <div class="item static" [title]="p.vendor">
              <s-icon name="effect" />
              <span>{{ p.name }}</span>
            </div>
          }
        } @empty {
          <p class="hint">{{ t('No CLAP plugin found on this computer yet.') }}</p>
        }
      </div>
    } @else {
      <div class="path">
        @if (trail().length > 1) {
          <button class="s-bar-btn" (click)="up()" [title]="t('Back')">
            <s-icon name="chevron" [size]="12" style="transform: rotate(180deg)" />
          </button>
        }
        <span class="where">{{ here()?.name ?? t('Places') }}</span>
        <button class="s-bar-btn" (click)="addFolder()" [title]="t('Add a folder')">
          <s-icon name="plus" />
        </button>
      </div>
      <div class="list">
        @for (e of entries(); track e.path) {
          <div class="row">
            <button
              class="item"
              [class.listening]="listening() === e.path"
              [draggable]="!e.folder && !isMidi(e)"
              (dragstart)="dragStart($event, e)"
              (click)="pick(e)"
              [title]="
                e.folder
                  ? e.path
                  : isMidi(e)
                    ? t('Click to make a pattern of it')
                    : t('Click to listen; drag into the playlist or the rack')
              "
            >
              <s-icon
                [name]="
                  e.folder
                    ? 'folder'
                    : isMidi(e)
                      ? 'piano'
                      : listening() === e.path
                        ? 'play'
                        : 'sound'
                "
                [filled]="listening() === e.path"
              />
              <span>{{ e.folder ? e.name : label(e) }}</span>
            </button>
            @if (!e.folder && !isMidi(e)) {
              <button
                class="s-bar-btn add"
                (click)="sampler(e)"
                [title]="t('Make a sampler of it')"
              >
                <s-icon name="plus" [size]="12" />
              </button>
            }
          </div>
        } @empty {
          <p class="s-empty">{{ t('No sound here.') }}</p>
        }
      </div>
    }
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      min-height: 0;
      height: 100%;
    }
    .list {
      flex: 1;
      min-height: 0;
      overflow-y: auto;
      padding: 4px 6px 12px;
    }
    h4 {
      margin: 12px 6px 6px;
      color: var(--s-text-3);
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.05em;
      text-transform: uppercase;
    }
    .hint {
      margin: 0 6px 6px;
      color: var(--s-text-3);
      font-size: 12px;
      line-height: 1.5;
    }
    .item {
      width: 100%;
      height: 30px;
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 0 8px;
      border: 0;
      border-radius: var(--s-radius);
      background: transparent;
      color: var(--s-text);
      text-align: left;
      cursor: pointer;
      span {
        flex: 1;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
      }
      s-icon {
        color: var(--s-text-3);
      }
      .plus {
        opacity: 0;
      }
      &:hover:not(.static) {
        background: var(--s-hover);
        .plus {
          opacity: 1;
        }
      }
      &.static {
        cursor: default;
        color: var(--s-text-2);
      }
    }
    .row {
      display: flex;
      align-items: center;
      .item {
        flex: 1;
        min-width: 0;
      }
      .add {
        opacity: 0;
        height: 26px;
        min-width: 26px;
      }
      &:hover .add {
        opacity: 1;
      }
    }
    .item.listening {
      color: var(--s-accent);
      s-icon {
        color: var(--s-accent);
      }
    }
    .path {
      flex: none;
      display: flex;
      align-items: center;
      gap: 4px;
      height: 36px;
      padding: 0 6px;
      border-bottom: 1px solid var(--s-line);
      .where {
        flex: 1;
        padding-left: 4px;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        color: var(--s-text-2);
        font-size: 12px;
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Browser {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly files = inject(Files);

  protected readonly tab = signal<'instruments' | 'sounds'>('instruments');
  protected readonly instruments = computed(() =>
    this.studio.devices().filter((d) => d.instrument),
  );
  protected readonly effects = computed(() => this.studio.devices().filter((d) => !d.instrument));
  protected readonly entries = signal<Entry[]>([]);
  /** The folders gone into, from a place; none at the places themselves. */
  protected readonly trail = signal<Entry[]>([]);
  protected readonly here = computed(() => this.trail().at(-1) ?? null);
  protected readonly listening = signal<string | null>(null);

  protected add(name: string, label: string): void {
    void this.studio.addChannel(name, this.t(label));
  }

  protected async openSounds(): Promise<void> {
    this.tab.set('sounds');
    if (!this.trail().length) await this.showPlaces();
  }

  private async showPlaces(): Promise<void> {
    this.trail.set([]);
    this.entries.set(await api.places());
  }

  protected isMidi(e: Entry): boolean {
    return /\.midi?$/i.test(e.name);
  }

  protected label(e: Entry): string {
    return sampleLabel(e.name);
  }

  protected async pick(e: Entry): Promise<void> {
    if (e.folder) {
      try {
        const entries = await api.browse(e.path);
        this.trail.update((t) => [...t, e]);
        this.entries.set(entries);
      } catch (error) {
        this.studio.fail(error);
      }
      return;
    }
    if (this.isMidi(e)) {
      await this.studio.importMidi(e.path);
      return;
    }
    const again = this.listening() === e.path;
    this.listening.set(again ? null : e.path);
    try {
      await api.listen(again ? null : e.path);
    } catch (error) {
      this.listening.set(null);
      this.studio.fail(error);
    }
  }

  protected async sampler(e: Entry): Promise<void> {
    const [sound] = await this.files.import([e.path]);
    if (sound) await this.studio.addChannel('sonevo.sampler', soundName(e.path), sound.sample);
  }

  protected async up(): Promise<void> {
    const trail = this.trail().slice(0, -1);
    const back = trail.at(-1);
    if (!back) {
      await this.showPlaces();
      return;
    }
    this.entries.set(await api.browse(back.path));
    this.trail.set(trail);
  }

  protected async addFolder(): Promise<void> {
    const folder = (await open({ directory: true, title: this.t('A folder of sounds') })) as
      string | null;
    if (!folder) return;
    const settings = await api.settings();
    if (!settings.folders.includes(folder)) {
      await api.setSettings({ ...settings, folders: [...settings.folders, folder] });
    }
    await this.showPlaces();
  }

  protected dragStart(event: DragEvent, e: Entry): void {
    event.dataTransfer?.setData(SOUND_DRAG, e.path);
    event.dataTransfer?.setData('text/plain', e.name);
  }
}
