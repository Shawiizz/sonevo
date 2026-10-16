import { ChangeDetectionStrategy, Component, inject, input, output } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { Kept } from '../core/models';
import { Studio } from '../core/studio.service';

/** Songs an earlier run kept and never saved: Sonevo ended before they were. */
@Component({
  selector: 'app-recovery-dialog',
  template: `
    <div class="backdrop"></div>
    <div class="dialog" role="dialog">
      <h2>{{ t('Sonevo closed before these songs were saved') }}</h2>
      <p>
        {{ t('Their changes were kept as they were made. Open one to pick up where you were.') }}
      </p>
      @for (k of kept(); track k.path) {
        <div class="song">
          <div class="text">
            <span class="name">{{ k.name }}</span>
            <span class="when">{{ when(k.time) }} · {{ k.origin ?? t('never saved') }}</span>
          </div>
          <button class="s-btn" (click)="forget(k)">{{ t('Forget') }}</button>
          <button class="s-btn s-btn--primary" (click)="open(k)">{{ t('Open') }}</button>
        </div>
      }
      <div class="actions">
        <button class="s-btn" (click)="closed.emit()">{{ t('Later') }}</button>
      </div>
    </div>
  `,
  styles: `
    :host {
      position: fixed;
      inset: 0;
      z-index: 35;
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
      margin: 0 0 8px;
      font-size: 17px;
    }
    p {
      margin: 0 0 16px;
      color: var(--s-text-2);
      line-height: 1.5;
    }
    .song {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 0;
      border-top: 1px solid var(--s-line);
    }
    .text {
      flex: 1;
      min-width: 0;
      display: flex;
      flex-direction: column;
    }
    .name {
      font-weight: 600;
    }
    .when {
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
      color: var(--s-text-3);
      font-size: 12px;
    }
    .actions {
      display: flex;
      justify-content: flex-end;
      padding-top: 12px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RecoveryDialog {
  private readonly studio = inject(Studio);
  private readonly i18n = inject(I18n);
  protected readonly t = this.i18n.t;
  readonly kept = input<Kept[]>([]);
  readonly closed = output();
  readonly done = output<Kept>();

  protected when(seconds: number): string {
    const locale = this.i18n.language() === 'fr' ? 'fr-FR' : 'en-GB';
    return new Date(seconds * 1000).toLocaleString(locale, {
      dateStyle: 'medium',
      timeStyle: 'short',
    });
  }

  protected async open(k: Kept): Promise<void> {
    try {
      this.studio.show(await api.recover(k.path, true));
      this.closed.emit();
    } catch (e) {
      this.studio.fail(e);
    }
  }

  protected async forget(k: Kept): Promise<void> {
    this.studio.show(await api.recover(k.path, false));
    this.done.emit(k);
  }
}
