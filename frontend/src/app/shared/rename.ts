import {
  ChangeDetectionStrategy,
  Component,
  effect,
  ElementRef,
  inject,
  Injectable,
  signal,
  viewChild,
} from '@angular/core';
import { I18n } from '../core/i18n';

interface Asked {
  x: number;
  y: number;
  title: string;
  value: string;
  answer: (name: string | null) => void;
}

/** A name asked for in a small field where the click was, in place of the system's window. */
@Injectable({ providedIn: 'root' })
export class Rename {
  readonly asked = signal<Asked | null>(null);
  private x = 0;
  private y = 0;

  constructor() {
    // where a menu was last opened from: the field opens there
    window.addEventListener(
      'pointerdown',
      (e) => ((this.x = e.clientX), (this.y = e.clientY)),
      true,
    );
    window.addEventListener(
      'contextmenu',
      (e) => ((this.x = e.clientX), (this.y = e.clientY)),
      true,
    );
  }

  /** The name typed, trimmed; null if left. */
  ask(title: string, value: string): Promise<string | null> {
    this.asked()?.answer(null);
    return new Promise((answer) => this.asked.set({ x: this.x, y: this.y, title, value, answer }));
  }

  done(name: string | null): void {
    const asked = this.asked();
    this.asked.set(null);
    const trimmed = name?.trim();
    asked?.answer(trimmed ? trimmed : null);
  }
}

@Component({
  selector: 's-rename',
  template: `
    @if (rename.asked(); as asked) {
      <div class="catch" (pointerdown)="rename.done(null)"></div>
      <form
        class="s-popover"
        [style.left.px]="left(asked.x)"
        [style.top.px]="top(asked.y)"
        (submit)="$event.preventDefault(); rename.done(input.value)"
      >
        <span class="title">{{ asked.title }}</span>
        <input
          #input
          class="s-inline"
          [value]="asked.value"
          (keydown.escape)="rename.done(null)"
          (blur)="rename.done(input.value)"
        />
        <span class="hint">{{ t('Enter to keep, Esc to leave') }}</span>
      </form>
    }
  `,
  styles: `
    .catch {
      position: fixed;
      inset: 0;
      z-index: 24;
    }
    form {
      z-index: 25;
      gap: 6px;
      width: 260px;
      padding: 10px;
    }
    .title {
      color: var(--s-text-2);
      font-size: 12px;
    }
    input {
      width: 100%;
      height: 30px;
    }
    .hint {
      color: var(--s-text-3);
      font-size: 11px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RenameField {
  protected readonly rename = inject(Rename);
  protected readonly t = inject(I18n).t;
  private readonly input = viewChild<ElementRef<HTMLInputElement>>('input');

  constructor() {
    // the field focused and its text chosen, each time it opens
    effect(() => {
      const input = this.input()?.nativeElement;
      input?.focus();
      input?.select();
    });
  }

  protected left(x: number): number {
    return Math.min(x, window.innerWidth - 276);
  }

  protected top(y: number): number {
    return Math.min(y, window.innerHeight - 110);
  }
}
