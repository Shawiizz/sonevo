import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { I18n } from '../core/i18n';
import { Studio } from '../core/studio.service';
import { Icon } from '../shared/icon';
import { ChannelRack } from './channel-rack';
import { PianoRoll } from './piano-roll';

/**
 * A pattern opened to be composed, in the place of the playlist: its channels' steps above,
 * the piano roll of the channel chosen below, the pattern played over and over meanwhile.
 */
@Component({
  selector: 'app-pattern-editor',
  imports: [ChannelRack, Icon, PianoRoll],
  template: `
    @if (studio.pattern(); as pattern) {
      <div class="head">
        <button class="s-bar-btn back" (click)="studio.closePattern()">
          <s-icon name="back" /> {{ t('Back to the song') }}
        </button>
        <span class="dot" [style.background]="pattern.color"></span>
        <span class="title">{{ pattern.name }}</span>
        <span class="length">{{
          bars() === 1 ? t('1 bar') : t('{count} bars', { count: bars() })
        }}</span>
        <span class="grow"></span>
        <span class="hint">{{
          t('Space plays it over and over · Esc goes back to the song')
        }}</span>
        <button class="s-btn s-btn--primary" (click)="studio.togglePlay()">
          <s-icon [name]="studio.status().playing ? 'pause' : 'play'" />
          {{ studio.status().playing ? t('Pause') : t('Play it over and over') }}
        </button>
      </div>
    }
    <section class="steps" [style.flex-basis.%]="share()">
      <app-channel-rack [head]="false" />
    </section>
    <div class="splitter" (pointerdown)="split($event)"></div>
    <section class="roll">
      <app-piano-roll />
    </section>
  `,
  styles: `
    :host {
      flex: 1;
      min-height: 0;
      display: flex;
      flex-direction: column;
      background: var(--s-base);
    }
    .head {
      flex: none;
      display: flex;
      align-items: center;
      gap: 10px;
      height: var(--s-panel-head);
      padding: 0 8px;
      border-bottom: 1px solid var(--s-line);
    }
    .dot {
      width: 10px;
      height: 10px;
      border-radius: 3px;
    }
    .title {
      font-weight: 600;
    }
    .length,
    .hint {
      color: var(--s-text-3);
      font-size: 12px;
    }
    .grow {
      flex: 1;
    }
    .steps,
    .roll {
      min-height: 0;
      flex-shrink: 0;
      display: flex;
      flex-direction: column;
    }
    .roll {
      flex: 1;
    }
    .steps > *,
    .roll > * {
      flex: 1;
      min-height: 0;
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
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PatternEditor {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  /** The share of the height the steps have, the piano roll the rest. */
  protected readonly share = signal(42);
  protected readonly bars = computed(() => {
    const pattern = this.studio.pattern();
    return pattern ? Math.max(1, Math.round(pattern.length / this.studio.bar())) : 1;
  });

  protected split(event: PointerEvent): void {
    const splitter = event.currentTarget as HTMLElement;
    splitter.setPointerCapture(event.pointerId);
    const host = splitter.parentElement!.getBoundingClientRect();
    const move = (e: PointerEvent) => {
      const share = ((e.clientY - host.top) / host.height) * 100;
      this.share.set(Math.min(80, Math.max(15, share)));
    };
    const up = () => {
      splitter.removeEventListener('pointermove', move as EventListener);
      splitter.removeEventListener('pointerup', up);
    };
    splitter.addEventListener('pointermove', move as EventListener);
    splitter.addEventListener('pointerup', up);
  }
}
