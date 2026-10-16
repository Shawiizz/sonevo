import { ChangeDetectionStrategy, Component, inject, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { OutputInfo, Settings } from '../core/models';
import { Studio } from '../core/studio.service';

const BUFFERS = [64, 128, 256, 512, 1024, 2048];

/** The settings: the sound card and its latency, the language. */
@Component({
  selector: 'app-settings-dialog',
  imports: [FormsModule],
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog">
      <h2>{{ t('Settings') }}</h2>
      @if (settings(); as s) {
        <section>
          <h3>{{ t('Sound card') }}</h3>
          <label class="s-field">
            <span>{{ t('Driver') }}</span>
            <select [ngModel]="s.driver" (ngModelChange)="set({ driver: $event, device: null })">
              <option value="wasapi">WASAPI · {{ t('every card, a little more latency') }}</option>
              <option value="asio">
                ASIO · {{ t('the lowest latency, with the card maker’s driver') }}
              </option>
            </select>
          </label>
          <label class="s-field">
            <span>{{ t('Output') }}</span>
            <select [ngModel]="s.device ?? ''" (ngModelChange)="set({ device: $event || null })">
              <option value="">{{ t('The default output') }}</option>
              @for (o of outputsOf(s.driver); track o.name) {
                <option [value]="o.name">{{ o.name }}</option>
              }
            </select>
          </label>
          <label class="s-field">
            <span>{{ t('Buffer (frames): smaller is less latency, larger is safer') }}</span>
            <select [ngModel]="s.buffer ?? 0" (ngModelChange)="set({ buffer: +$event || null })">
              <option [value]="0">{{ t('The card’s own') }}</option>
              @for (b of buffers; track b) {
                <option [value]="b">{{ b }}</option>
              }
            </select>
          </label>
          <p class="status" [class.bad]="!!audio().error || audio().failed">{{ audioText() }}</p>
        </section>
        <section>
          <h3>{{ t('Language') }}</h3>
          <div class="s-seg" role="radiogroup">
            <button role="radio" [attr.aria-checked]="!s.language" (click)="setLanguage(null)">
              {{ t('The system’s') }}
            </button>
            <button
              role="radio"
              [attr.aria-checked]="s.language === 'en'"
              (click)="setLanguage('en')"
            >
              English
            </button>
            <button
              role="radio"
              [attr.aria-checked]="s.language === 'fr'"
              (click)="setLanguage('fr')"
            >
              Français
            </button>
          </div>
        </section>
      }
      <div class="actions">
        <button class="s-btn s-btn--primary" (click)="closed.emit()">{{ t('Done') }}</button>
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
      width: 520px;
      max-height: 86vh;
      overflow-y: auto;
      padding: 22px 24px;
      border: 1px solid var(--s-line-strong);
      border-radius: 12px;
      background: var(--s-base);
      box-shadow: 0 24px 64px rgb(0 0 0 / 0.6);
    }
    h2 {
      margin: 0 0 16px;
      font-size: 17px;
    }
    h3 {
      margin: 0 0 10px;
      color: var(--s-text-2);
      font-size: 13px;
    }
    section {
      display: flex;
      flex-direction: column;
      gap: 12px;
      padding: 14px 0;
      border-top: 1px solid var(--s-line);
    }
    .status {
      margin: 0;
      color: var(--s-text-3);
      font-size: 12px;
      &.bad {
        color: var(--s-warn);
      }
    }
    .actions {
      display: flex;
      justify-content: flex-end;
      padding-top: 12px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class SettingsDialog {
  protected readonly t = inject(I18n).t;
  private readonly i18n = inject(I18n);
  private readonly studio = inject(Studio);
  readonly closed = output();

  protected readonly buffers = BUFFERS;
  protected readonly settings = signal<Settings | null>(null);
  protected readonly outputs = signal<OutputInfo[]>([]);
  protected readonly audio = () => this.studio.status().audio;

  constructor() {
    void api.settings().then((s) => this.settings.set(s));
    void api.outputs().then((o) => this.outputs.set(o));
  }

  protected outputsOf(driver: string): OutputInfo[] {
    return this.outputs().filter((o) => o.driver === driver);
  }

  protected audioText(): string {
    const audio = this.audio();
    if (audio.error) return audio.error;
    if (audio.failed) return this.t('The sound card stopped: choose it again.');
    if (!audio.device) return '';
    return `${audio.device} · ${audio.sampleRate} Hz · ${audio.latencyMs.toFixed(1)} ms`;
  }

  protected async set(change: Partial<Settings>): Promise<void> {
    const current = this.settings();
    if (!current) return;
    const next = { ...current, ...change };
    this.settings.set(next);
    await api.setSettings(next);
  }

  protected async setLanguage(language: string | null): Promise<void> {
    this.i18n.set(language);
    await this.set({ language });
  }
}
