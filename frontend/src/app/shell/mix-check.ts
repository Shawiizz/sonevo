import { ChangeDetectionStrategy, Component, inject, output, signal } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';

interface Advice {
  good: boolean;
  text: string;
}

/** The mix looked at as a whole, and what to do about it, in plain words. */
@Component({
  selector: 'app-mix-check',
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog">
      <h2>{{ t('Mix check') }}</h2>
      @if (advice(); as advice) {
        <ul>
          @for (a of advice; track a.text) {
            <li [class.good]="a.good">{{ a.text }}</li>
          }
        </ul>
      } @else {
        <p class="s-empty">{{ t('Listening to the whole song…') }}</p>
      }
      <div class="actions">
        <button class="s-btn s-btn--primary" (click)="closed.emit()">{{ t('Close') }}</button>
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
      padding: 22px 24px;
      border: 1px solid var(--s-line-strong);
      border-radius: 12px;
      background: var(--s-base);
      box-shadow: 0 24px 64px rgb(0 0 0 / 0.6);
    }
    h2 {
      margin: 0 0 14px;
      font-size: 17px;
    }
    ul {
      display: flex;
      flex-direction: column;
      gap: 10px;
      margin: 0;
      padding: 0;
      list-style: none;
    }
    li {
      padding: 10px 12px;
      border-left: 2px solid var(--s-warn);
      border-radius: var(--s-radius);
      background: var(--s-raised);
      font-size: 13px;
      line-height: 1.45;
      &.good {
        border-left-color: var(--s-ok);
      }
    }
    .actions {
      display: flex;
      justify-content: flex-end;
      padding-top: 16px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class MixCheck {
  protected readonly t = inject(I18n).t;
  readonly closed = output();
  protected readonly advice = signal<Advice[] | null>(null);

  constructor() {
    void api.checkMix().then((found) => {
      const t = this.t;
      const said: Advice[] = [];
      const say = (text: string) => said.push({ good: false, text });
      const lufs = found.loudness;
      if (found.peak >= -0.1) {
        say(
          t(
            'The song clips: its loudest moments go past the top. Turn the master down a little, or put a limiter on it.',
          ),
        );
      }
      if (lufs === null) {
        say(t('Nothing plays yet: place patterns in the playlist first.'));
      } else if (lufs < -18) {
        say(
          t(
            'The song is quiet ({lufs} LUFS): streaming services play songs at about -14. A limiter on the master, pushed a little, brings it up.',
            { lufs: lufs.toFixed(1) },
          ),
        );
      } else if (lufs > -8) {
        say(
          t(
            'The song is very loud ({lufs} LUFS): streaming services will turn it down, and it may sound squashed. Ease the limiter off.',
            { lufs: lufs.toFixed(1) },
          ),
        );
      }
      if (!found.limited && lufs !== null && lufs > -16) {
        say(
          t('The master has no limiter: one at the end keeps the loudest moments from clipping.'),
        );
      }
      if (found.rumble > 0.3) {
        say(
          t(
            'Much of the sound is rumble under 40 Hz, felt more than heard: cut the lows of the parts that do not need them, with an EQ set to "Cut the lows".',
          ),
        );
      }
      if (lufs !== null && found.air < 0.005) {
        say(
          t(
            'The song sounds dull: little of it is over 8 kHz. Hats, or an EQ set "Brighter", let air in.',
          ),
        );
      } else if (found.air > 0.25) {
        say(
          t(
            'The song is bright, even harsh: much of it is over 8 kHz. Turn the highs of the brightest parts down.',
          ),
        );
      }
      if (lufs !== null && found.correlation < 0.2) {
        say(
          t(
            'The two sides cancel each other out in part: on a phone, in mono, the song may sound thin. Narrow the widest parts (chorus, unison, auto-pan).',
          ),
        );
      }
      if (!said.length) {
        said.push({
          good: true,
          text: t(
            'Nothing to worry about: the song is balanced, as loud as songs are ({lufs} LUFS).',
            {
              lufs: (lufs ?? 0).toFixed(1),
            },
          ),
        });
      }
      this.advice.set(said);
    });
  }
}
