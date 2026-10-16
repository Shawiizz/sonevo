import { Injectable, signal } from '@angular/core';

export interface MenuItem {
  label: string;
  key?: string;
  disabled?: boolean;
  danger?: boolean;
  run?: () => void;
  separator?: boolean;
}

/** The one menu open at a time, where it was opened: a context menu or a button's. */
@Injectable({ providedIn: 'root' })
export class Menus {
  readonly open = signal<{ x: number; y: number; items: MenuItem[] } | null>(null);

  show(event: MouseEvent, items: MenuItem[]): void {
    event.preventDefault();
    event.stopPropagation();
    this.open.set({ x: event.clientX, y: event.clientY, items });
  }

  below(event: MouseEvent, items: MenuItem[]): void {
    event.stopPropagation();
    const box = (event.currentTarget as Element).getBoundingClientRect();
    this.open.set({ x: box.left, y: box.bottom + 4, items });
  }

  close(): void {
    this.open.set(null);
  }
}
