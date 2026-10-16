import { ChangeDetectionStrategy, Component, inject, output } from '@angular/core';
import { I18n } from '../core/i18n';

const GROUPS: { title: string; keys: [string, string][] }[] = [
  {
    title: 'Everywhere',
    keys: [
      ['Space', 'Play / pause'],
      ['Ctrl+Z / Ctrl+Y', 'Undo / redo'],
      ['Ctrl+S', 'Save'],
      ['Ctrl+K', 'Commands'],
      ['F6 · F7 · F9', 'Channel rack · piano roll · mixer'],
      ['Z to M, Q to P', 'Your keyboard as a piano'],
      ['Numpad + / −', 'Octave up / down'],
    ],
  },
  {
    title: 'Piano roll',
    keys: [
      ['Click · drag', 'Draw a note, as long as dragged'],
      ['Right-click', 'Delete'],
      ['Ctrl+drag', 'Choose several'],
      ['Alt+drag', 'Copy'],
      ['Ctrl+C · Ctrl+X · Ctrl+V', 'Copy · cut · paste'],
      ['Ctrl+D', 'Duplicate after'],
      ['↑ / ↓ (Shift: an octave)', 'Move the notes chosen'],
      ['Q', 'Put the notes on the grid'],
      ['Ctrl+wheel', 'Zoom'],
    ],
  },
  {
    title: 'Playlist',
    keys: [
      ['Click', 'Place the pattern chosen'],
      ['Right-click', 'Delete'],
      ['Drag along the ruler', 'Set the loop'],
      ['Right-click the ruler', 'Add a marker, clear the loop'],
      ['Click a marker', 'Play from there'],
      ['Ctrl+D', 'Duplicate after'],
      ['Alt+wheel', 'Taller or shorter tracks'],
    ],
  },
];

/** The keyboard and mouse shortcuts, all on one page. */
@Component({
  selector: 'app-shortcuts',
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="dialog" role="dialog">
      <h2>{{ t('Shortcuts') }}</h2>
      <div class="groups">
        @for (group of groups; track group.title) {
          <section>
            <h3>{{ t(group.title) }}</h3>
            @for (k of group.keys; track k[0]) {
              <div class="key">
                <span class="s-mono">{{ t(k[0]) }}</span>
                <span>{{ t(k[1]) }}</span>
              </div>
            }
          </section>
        }
      </div>
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
      width: 860px;
      max-height: 86vh;
      overflow-y: auto;
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
    h3 {
      margin: 0 0 8px;
      color: var(--s-text-2);
      font-size: 13px;
    }
    .groups {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 20px;
    }
    .key {
      display: flex;
      flex-direction: column;
      gap: 2px;
      padding: 6px 0;
      border-top: 1px solid var(--s-line);
      font-size: 12px;
      .s-mono {
        color: var(--s-accent);
      }
      span:last-child {
        color: var(--s-text-2);
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
export class Shortcuts {
  protected readonly t = inject(I18n).t;
  protected readonly groups = GROUPS;
  readonly closed = output();
}
