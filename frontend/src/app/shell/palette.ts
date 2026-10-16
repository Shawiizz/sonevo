import {
  afterNextRender,
  ChangeDetectionStrategy,
  Component,
  computed,
  ElementRef,
  inject,
  input,
  output,
  signal,
  viewChild,
} from '@angular/core';
import { I18n } from '../core/i18n';

export interface Command {
  label: string;
  key?: string;
  run: () => void;
}

/** How well `query` matches `text`: its letters in order, sooner and closer together better. */
export function fuzzy(query: string, text: string): number | null {
  const q = query.toLowerCase();
  const s = text.toLowerCase();
  let score = 0;
  let at = -1;
  for (const c of q) {
    const found = s.indexOf(c, at + 1);
    if (found < 0) return null;
    score += found - at === 1 ? 3 : 1;
    if (found === 0 || s[found - 1] === ' ') score += 2;
    at = found;
  }
  return score - s.length * 0.01;
}

/** The command palette: every command, found by typing a few of its letters. */
@Component({
  selector: 'app-palette',
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="palette">
      <input
        #input
        [placeholder]="t('Type a command…')"
        (input)="query.set($any($event.target).value); active.set(0)"
        (keydown)="key($event)"
      />
      <div class="list">
        @for (c of found(); track c.label; let i = $index) {
          <button
            class="s-menu-item"
            [class.active]="i === active()"
            (click)="run(c)"
            (mouseenter)="active.set(i)"
          >
            {{ c.label }}
            @if (c.key) {
              <span class="s-key">{{ c.key }}</span>
            }
          </button>
        } @empty {
          <p class="s-empty">{{ t('No command matches.') }}</p>
        }
      </div>
    </div>
  `,
  styles: `
    :host {
      position: fixed;
      inset: 0;
      z-index: 40;
    }
    .backdrop {
      position: absolute;
      inset: 0;
      background: rgb(0 0 0 / 0.4);
    }
    .palette {
      position: absolute;
      top: 90px;
      left: 50%;
      width: 560px;
      transform: translateX(-50%);
      border: 1px solid var(--s-line-strong);
      border-radius: 10px;
      background: var(--s-raised);
      box-shadow: 0 24px 64px rgb(0 0 0 / 0.6);
      overflow: hidden;
    }
    input {
      width: 100%;
      height: 46px;
      padding: 0 16px;
      border: 0;
      border-bottom: 1px solid var(--s-line);
      background: transparent;
      color: var(--s-text);
      font: inherit;
      font-size: 14px;
      outline: none;
    }
    .list {
      max-height: 380px;
      overflow-y: auto;
      padding: 4px;
      display: flex;
      flex-direction: column;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Palette {
  protected readonly t = inject(I18n).t;
  readonly commands = input<Command[]>([]);
  readonly closed = output();
  protected readonly query = signal('');
  protected readonly active = signal(0);
  private readonly input = viewChild.required<ElementRef<HTMLInputElement>>('input');

  protected readonly found = computed(() => {
    const q = this.query().trim();
    if (!q) return this.commands();
    return this.commands()
      .map((c) => ({ c, score: fuzzy(q, c.label) }))
      .filter((x) => x.score !== null)
      .sort((a, b) => b.score! - a.score!)
      .map((x) => x.c);
  });

  constructor() {
    afterNextRender(() => this.input().nativeElement.focus());
  }

  protected key(event: KeyboardEvent): void {
    const found = this.found();
    if (event.key === 'ArrowDown') this.active.set(Math.min(found.length - 1, this.active() + 1));
    else if (event.key === 'ArrowUp') this.active.set(Math.max(0, this.active() - 1));
    else if (event.key === 'Enter' && found[this.active()]) this.run(found[this.active()]);
    else if (event.key === 'Escape') this.closed.emit();
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  protected run(command: Command): void {
    this.closed.emit();
    command.run();
  }
}
