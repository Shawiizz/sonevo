import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
} from '@angular/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { VersionInfo } from '../core/models';
import { Studio } from '../core/studio.service';
import { Files } from '../shell/files.service';
import { Icon } from '../shared/icon';

/**
 * The song's versions: one made each time it is saved, and those named. One can be listened
 * to in place of the song, to compare by ear, or the song brought back to it.
 */
@Component({
  selector: 'app-versions-panel',
  imports: [Icon],
  template: `
    @if (!studio.view()?.folder) {
      <div class="s-empty">
        <p>
          {{
            t(
              'Save the song to keep its versions: each save is one, and you can name the ones that matter.'
            )
          }}
        </p>
        <button class="s-btn s-btn--primary" (click)="files.saveAs()">{{ t('Save as…') }}</button>
      </div>
    } @else {
      <form class="make" (submit)="make($event)">
        <input
          class="s-inline"
          [placeholder]="t('Name this version…')"
          [value]="name()"
          (input)="name.set($any($event.target).value)"
        />
        <button class="s-btn" type="submit" [disabled]="!name().trim()">{{ t('Keep') }}</button>
      </form>
      @if (studio.view()?.comparing) {
        <div class="comparing">
          <s-icon name="loop" />
          <span>{{ t('You hear an older version.') }}</span>
          <button class="s-btn" (click)="compare(null)">{{ t('Back to now') }}</button>
        </div>
      }
      <div class="list">
        @for (v of versions(); track v.id) {
          <div
            class="version"
            [class.named]="!v.automatic"
            [class.on]="studio.view()?.comparing === v.id"
          >
            <div class="text">
              <span class="name">{{ v.automatic ? t(v.name) : v.name }}</span>
              <span class="when">{{ when(v.time) }}</span>
            </div>
            <button
              class="s-bar-btn"
              [class.on]="studio.view()?.comparing === v.id"
              (click)="compare(studio.view()?.comparing === v.id ? null : v.id)"
              [title]="t('Listen to it in place of the song')"
            >
              <s-icon name="play" [filled]="true" [size]="12" />
            </button>
            <button
              class="s-bar-btn"
              (click)="restore(v)"
              [title]="t('Bring the song back to this version')"
            >
              <s-icon name="undo" />
            </button>
          </div>
        } @empty {
          <p class="s-empty">{{ t('No version yet: save the song, or name one above.') }}</p>
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
    .s-empty .s-btn {
      margin-top: 8px;
    }
    .make {
      display: flex;
      gap: 6px;
      padding: 10px;
      border-bottom: 1px solid var(--s-line);
      input {
        flex: 1;
        min-width: 0;
        height: 32px;
        border-color: var(--s-line-strong);
        &:focus {
          border-color: var(--s-accent);
        }
      }
    }
    .comparing {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 10px;
      padding: 8px 10px;
      border-radius: var(--s-radius);
      background: color-mix(in srgb, var(--s-warn) 14%, var(--s-raised));
      color: var(--s-warn);
      font-size: 12px;
      span {
        flex: 1;
      }
      .s-btn {
        height: 26px;
        padding: 0 10px;
        color: var(--s-text);
      }
    }
    .list {
      flex: 1;
      min-height: 0;
      overflow-y: auto;
      padding: 6px;
    }
    .version {
      display: flex;
      align-items: center;
      gap: 2px;
      padding: 6px 4px 6px 10px;
      border-radius: var(--s-radius);
      &:hover {
        background: var(--s-hover);
      }
      &.on {
        background: color-mix(in srgb, var(--s-accent) 14%, transparent);
      }
      .text {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
      }
      .name {
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        color: var(--s-text-2);
        font-size: 12px;
      }
      &.named .name {
        color: var(--s-text);
        font-weight: 600;
      }
      .when {
        color: var(--s-text-3);
        font-size: 11px;
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class VersionsPanel {
  protected readonly studio = inject(Studio);
  protected readonly files = inject(Files);
  private readonly i18n = inject(I18n);
  protected readonly t = this.i18n.t;
  protected readonly versions = signal<VersionInfo[]>([]);
  protected readonly name = signal('');
  private readonly folder = computed(() => this.studio.view()?.folder ?? null);

  constructor() {
    // read again as the song is saved: its dirty mark goes
    effect(() => {
      this.folder();
      this.studio.view()?.dirty;
      void this.load();
    });
  }

  private async load(): Promise<void> {
    try {
      this.versions.set(await api.versions());
    } catch (e) {
      this.studio.fail(e);
    }
  }

  protected when(seconds: number): string {
    const date = new Date(seconds * 1000);
    const locale = this.i18n.language() === 'fr' ? 'fr-FR' : 'en-GB';
    return date.toLocaleString(locale, { dateStyle: 'medium', timeStyle: 'short' });
  }

  protected async make(event: Event): Promise<void> {
    event.preventDefault();
    const name = this.name().trim();
    if (!name) return;
    try {
      this.studio.show(await api.makeVersion(name));
      this.name.set('');
      await this.load();
    } catch (e) {
      this.studio.fail(e);
    }
  }

  protected async compare(id: string | null): Promise<void> {
    try {
      this.studio.show(await api.compareVersion(id));
    } catch (e) {
      this.studio.fail(e);
    }
  }

  protected async restore(version: VersionInfo): Promise<void> {
    const t = this.t;
    const sure = await ask(
      t('Bring the song back to this version? The song as it is now is kept as a version first.'),
      {
        title: 'Sonevo',
        kind: 'info',
        okLabel: t('Go back'),
        cancelLabel: t('Cancel'),
      },
    );
    if (!sure) return;
    try {
      this.studio.show(await api.restoreVersion(version.id));
      await this.load();
    } catch (e) {
      this.studio.fail(e);
    }
  }
}
