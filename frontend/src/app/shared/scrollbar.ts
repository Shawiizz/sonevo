import { ChangeDetectionStrategy, Component, computed, input, output } from '@angular/core';

/** Where a view is along what it scrolls through, all in one unit (ticks, pixels, keys). */
export interface Extent {
  /** All there is to scroll through. */
  total: number;
  /** The part of it in view. */
  view: number;
  /** Where the view starts. */
  at: number;
}

/**
 * A thin bar along a canvas, its thumb as long as the part in view: dragged, or the track
 * clicked to go a page that way. Gone while everything is in view.
 */
@Component({
  selector: 's-scrollbar',
  template: `
    @if (thumb(); as thumb) {
      <div
        class="thumb"
        [style.left.%]="vertical() ? null : thumb.start"
        [style.width.%]="vertical() ? null : thumb.size"
        [style.top.%]="vertical() ? thumb.start : null"
        [style.height.%]="vertical() ? thumb.size : null"
        (pointerdown)="grab($event)"
      ></div>
    }
  `,
  styles: `
    :host {
      position: absolute;
      z-index: 2;
      display: block;
      &:hover .thumb,
      .thumb.held {
        background: var(--s-text-3);
      }
    }
    .thumb {
      position: absolute;
      inset: 2px;
      border-radius: 3px;
      background: var(--s-line-strong);
      cursor: default;
    }
    :host(:not(.vertical)) .thumb {
      min-width: 24px;
    }
    :host(.vertical) .thumb {
      min-height: 24px;
    }
  `,
  host: {
    '[class.vertical]': 'vertical()',
    '(pointerdown)': 'page($event)',
  },
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Scrollbar {
  readonly vertical = input(false);
  readonly extent = input.required<Extent>();
  /** Where the view should start now. */
  readonly moved = output<number>();

  /** The thumb's place and length along the bar, in percent; none when all is in view. */
  protected readonly thumb = computed(() => {
    const { total, view, at } = this.extent();
    if (total <= 0 || view >= total) return null;
    return { start: (Math.max(0, at) / total) * 100, size: (view / total) * 100 };
  });

  private clamp(at: number): number {
    const { total, view } = this.extent();
    return Math.max(0, Math.min(total - view, at));
  }

  private length(element: HTMLElement): number {
    const box = element.getBoundingClientRect();
    return Math.max(1, this.vertical() ? box.height : box.width);
  }

  /** The thumb dragged: the view following it. */
  protected grab(event: PointerEvent): void {
    event.stopPropagation();
    const thumb = event.currentTarget as HTMLElement;
    const bar = thumb.parentElement!;
    thumb.setPointerCapture(event.pointerId);
    thumb.classList.add('held');
    const from = this.vertical() ? event.clientY : event.clientX;
    const start = this.extent().at;
    const scale = this.extent().total / this.length(bar);
    const move = (e: PointerEvent) => {
      const now = this.vertical() ? e.clientY : e.clientX;
      this.moved.emit(this.clamp(start + (now - from) * scale));
    };
    const up = () => {
      thumb.classList.remove('held');
      thumb.removeEventListener('pointermove', move);
      thumb.removeEventListener('pointerup', up);
    };
    thumb.addEventListener('pointermove', move);
    thumb.addEventListener('pointerup', up);
  }

  /** The track clicked beside the thumb: a page towards where it was. */
  protected page(event: PointerEvent): void {
    event.stopPropagation();
    const bar = event.currentTarget as HTMLElement;
    const box = bar.getBoundingClientRect();
    const along = this.vertical() ? event.clientY - box.top : event.clientX - box.left;
    const { total, view, at } = this.extent();
    const clicked = (along / this.length(bar)) * total;
    this.moved.emit(this.clamp(clicked < at ? at - view : at + view));
  }
}
