import { ChangeDetectionStrategy, Component, input } from '@angular/core';

/** The interface's icons, drawn in lines a pixel and a half wide. */
const PATHS: Record<string, string> = {
  play: 'M5 3.5v9l7-4.5z',
  pause: 'M5 3.5v9M11 3.5v9',
  stop: 'M4 4h8v8H4z',
  metronome: 'M6 2.5h4l2.5 11h-9zM8 10l3.5-6',
  loop: 'M3 7a4 4 0 0 1 4-4h5l-2-2M13 9a4 4 0 0 1-4 4H4l2 2',
  plus: 'M8 3v10M3 8h10',
  close: 'M4 4l8 8M12 4l-8 8',
  minimize: 'M3.5 8h9',
  maximize: 'M3.5 3.5h9v9h-9z',
  menu: 'M2.5 4.5h11M2.5 8h11M2.5 11.5h11',
  folder: 'M2 4.5a1 1 0 0 1 1-1h3.5l1.5 1.5h5a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z',
  sound: 'M2 8h1.5M5 5v6M8 3v10M11 5.5v5M14 8h-1',
  synth: 'M2.5 3.5h11v9h-11zM5 3.5v5M8 3.5v5M11 3.5v5',
  drum: 'M2.5 6c0-1.4 2.5-2.5 5.5-2.5s5.5 1.1 5.5 2.5-2.5 2.5-5.5 2.5S2.5 7.4 2.5 6zM2.5 6v4c0 1.4 2.5 2.5 5.5 2.5s5.5-1.1 5.5-2.5V6',
  effect: 'M3 4h6M11 4h2M3 8h2M7 8h6M3 12h8M13 12h0M9 2.5v3M5 6.5v3M11 10.5v3',
  trash: 'M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 8.5h5.6l.7-8.5',
  undo: 'M4.5 6.5H10a3 3 0 0 1 0 6H6M4.5 6.5 7 4M4.5 6.5 7 9',
  redo: 'M11.5 6.5H6a3 3 0 0 0 0 6h4M11.5 6.5 9 4M11.5 6.5 9 9',
  save: 'M3 3h8l2 2v8H3zM5.5 3v3h4V3M5 13V9.5h6V13',
  settings:
    'M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4',
  search: 'M7 12A5 5 0 1 0 7 2a5 5 0 0 0 0 10zM14 14l-3.5-3.5',
  record: 'M8 3a5 5 0 1 0 0 10A5 5 0 0 0 8 3z',
  cut: 'M5 5.5a2 2 0 1 0 0-.01M5 13.5a2 2 0 1 0 0-.01M6.5 6.5l7 6M6.5 12.5l7-6',
  pencil: 'M10.5 2.5l3 3-8 8h-3v-3z',
  select: 'M3 2.5v11l3-3 2.5 4.5 1.5-1-2.5-4.5h4z',
  chevron: 'M6 4l4 4-4 4',
  back: 'M10 4L6 8l4 4',
  down: 'M4 6l4 4 4-4',
  power: 'M8 2v5M4.5 4a5 5 0 1 0 7 0',
  piano: 'M2.5 3h11v10h-11zM6 3v6M10 3v6M6 9v4M10 9v4',
  mixer: 'M4 2.5v11M8 2.5v11M12 2.5v11M2.5 10h3M6.5 5h3M10.5 8h3',
  rack: 'M2.5 3.5h11M2.5 8h11M2.5 12.5h11M5 2.5v2M9 7v2M11 11.5v2',
  playlist: 'M2.5 3.5h5M9 3.5h4.5M2.5 8h2.5M6.5 8h7M2.5 12.5h8M12 12.5h1.5',
  export: 'M8 2v8M5 5l3-3 3 3M3 10v3h10v-3',
  open: 'M2 4.5a1 1 0 0 1 1-1h3.5l1.5 1.5h5a1 1 0 0 1 1 1V7M2 12.5l2-5h11l-2 5z',
  new: 'M4 2h5l3 3v9H4zM9 2v3h3',
  keyboard: 'M2 4.5h12v7H2zM4.5 7h0M7 7h0M9.5 7h0M12 7h0M5 9.5h6',
  dice: 'M3 3h10v10H3zM6 6h0M10 10h0M10 6h0M6 10h0',
  copy: 'M5.5 5.5h7v7h-7zM3.5 10.5v-7h7',
  sample: 'M2 8h2l1.5-4 2 8 2-6 1.5 4 1-2h2',
  automation: 'M2 12c2 0 3-8 5-8s3 6 5 6 1.5-2 2-2',
};

@Component({
  selector: 's-icon',
  template: `<svg
    class="s-icon"
    viewBox="0 0 16 16"
    [style.width.px]="size()"
    [style.height.px]="size()"
  >
    <path [attr.d]="path()" [attr.fill]="filled() ? 'currentColor' : 'none'" />
  </svg>`,
  styles: `
    :host {
      display: inline-flex;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Icon {
  readonly name = input.required<string>();
  readonly size = input(14);
  readonly filled = input(false);
  protected readonly path = () => PATHS[this.name()] ?? '';
}
