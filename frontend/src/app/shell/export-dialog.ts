import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  output,
  signal,
} from '@angular/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Studio } from '../core/studio.service';
import { Files } from './files.service';

type Part = 'song' | 'loop' | 'stems';

/** Exporting: what is played to files, in what quality. */
@Component({
  selector: 'app-export-dialog',
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog">
      <h2>{{ t('Export') }}</h2>
      <section>
        <h3>{{ t('What') }}</h3>
        <div class="choices">
          <button [class.on]="part() === 'song'" (click)="part.set('song')">
            <strong>{{ t('The whole song') }}</strong>
            <span>{{
              t('One file, from the start to the end of the last clip, the tails after.')
            }}</span>
          </button>
          <button [class.on]="part() === 'loop'" [disabled]="!hasLoop()" (click)="part.set('loop')">
            <strong>{{ t('The loop') }}</strong>
            <span>{{
              hasLoop()
                ? t('One file of the part set on the ruler.')
                : t('Set a loop on the playlist’s ruler first.')
            }}</span>
          </button>
          <button
            [class.on]="part() === 'stems'"
            [disabled]="!hasTracks()"
            (click)="part.set('stems')"
          >
            <strong>{{ t('Each mixer track') }}</strong>
            <span>{{
              t('A file each, in a folder: to mix elsewhere, or to share the parts.')
            }}</span>
          </button>
        </div>
      </section>
      <section class="row">
        <label class="s-field">
          <span>{{ t('Quality') }}</span>
          <select [value]="depth()" (change)="depth.set($any($event.target).value)">
            <option value="16" [selected]="depth() === '16'">
              16 bits · {{ t('CD, the smallest') }}
            </option>
            <option value="24" [selected]="depth() === '24'">
              24 bits · {{ t('for mastering') }}
            </option>
            <option value="32" [selected]="depth() === '32'">
              32 bits float · {{ t('every bit kept') }}
            </option>
          </select>
        </label>
        <label class="s-field">
          <span>{{ t('Sample rate') }}</span>
          <select [value]="rate()" (change)="rate.set(+$any($event.target).value)">
            <option value="44100" [selected]="rate() === 44100">44.1 kHz</option>
            <option value="48000" [selected]="rate() === 48000">48 kHz</option>
            <option value="96000" [selected]="rate() === 96000">96 kHz</option>
          </select>
        </label>
      </section>
      <div class="actions">
        <button class="s-btn" (click)="closed.emit()">{{ t('Cancel') }}</button>
        <button class="s-btn s-btn--primary" (click)="run()">{{ t('Export…') }}</button>
      </div>
    </div>
  `,
  styles: `
    :host {
      position: fixed;
      inset: 0;
      z-index: 30;
      display: grid;
      place-items: center;
    }
    .backdrop {
      position: absolute;
      inset: 0;
      background: rgb(0 0 0 / 0.55);
    }
    .dialog {
      position: relative;
      width: 560px;
      padding: 22px 24px;
      border: 1px solid var(--s-line-strong);
      border-radius: 12px;
      background: var(--s-base);
      box-shadow: 0 24px 64px rgb(0 0 0 / 0.6);
    }
    h2 {
      margin: 0 0 12px;
      font-size: 17px;
    }
    h3 {
      margin: 0 0 8px;
      color: var(--s-text-2);
      font-size: 13px;
    }
    section {
      padding: 12px 0;
      border-top: 1px solid var(--s-line);
    }
    .row {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
    }
    .choices {
      display: flex;
      flex-direction: column;
      gap: 6px;
      button {
        display: flex;
        flex-direction: column;
        gap: 2px;
        padding: 10px 12px;
        border: 1px solid var(--s-line);
        border-radius: var(--s-radius);
        background: var(--s-raised);
        text-align: left;
        cursor: pointer;
        span {
          color: var(--s-text-3);
          font-size: 12px;
        }
        &.on {
          border-color: var(--s-accent);
          background: color-mix(in srgb, var(--s-accent) 10%, var(--s-raised));
        }
        &:disabled {
          opacity: 0.5;
          cursor: default;
        }
      }
    }
    .actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
      padding-top: 12px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class ExportDialog {
  protected readonly t = inject(I18n).t;
  private readonly studio = inject(Studio);
  private readonly files = inject(Files);
  readonly closed = output();

  protected readonly part = signal<Part>('song');
  protected readonly depth = signal('24');
  protected readonly rate = signal(48000);
  protected readonly hasLoop = computed(() => !!this.studio.song()?.loopRegion);
  protected readonly hasTracks = computed(() => (this.studio.song()?.inserts.length ?? 0) > 0);

  protected async run(): Promise<void> {
    const name = this.studio.song()?.name ?? 'Song';
    const part = this.part();
    const path =
      part === 'stems'
        ? ((await open({ directory: true, title: this.t('The folder the tracks go in') })) as
            string | null)
        : await save({
            title: this.t('Export the song as a WAV file'),
            defaultPath: `${name}${part === 'loop' ? ' (loop)' : ''}.wav`,
            filters: [{ name: 'WAV', extensions: ['wav'] }],
          });
    if (!path) return;
    this.closed.emit();
    await this.files.export(path, part, this.depth(), this.rate());
  }
}
