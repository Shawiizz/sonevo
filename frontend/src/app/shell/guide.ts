import { ChangeDetectionStrategy, Component, inject, output, signal } from '@angular/core';
import { I18n } from '../core/i18n';
import { Studio } from '../core/studio.service';

interface Step {
  title: string;
  text: string;
  show?: { label: string; open?: 'pattern' | 'mixer'; play?: boolean };
}

const STEPS: Step[] = [
  {
    title: 'Listen first',
    text: 'The song open is a demo. Press Space, or the play button at the top left, and watch the line move across the playlist: each coloured block is a pattern, a short loop of notes, placed in time.',
    show: { label: 'Play the demo', play: true },
  },
  {
    title: 'A beat, step by step',
    text: 'Double-click a block of the playlist: its pattern opens in a tab of its own, played over and over. Its instruments are a row each, and their squares the steps: sixteen to a bar. Click one, and the instrument plays there. Click it again, and it is gone. Write a beat for me starts one in a style.',
    show: { label: 'Open the pattern chosen', open: 'pattern' },
  },
  {
    title: 'A melody, note by note',
    text: 'In a pattern’s tab, click an instrument’s name: its notes are below, in the piano roll. Time goes across, the keys go up. Click to draw a note, drag its end to make it longer, right-click to take it out. Choose a scale, and the notes that go well together light up. Tools writes chord progressions and melodies for you.',
    show: { label: 'Open the pattern chosen', open: 'pattern' },
  },
  {
    title: 'The song, part by part',
    text: 'In the Song tab, a click places the pattern chosen at the top; drag a block to move it, its end to make it longer. Space plays the whole song there, and only the pattern in its own tab.',
  },
  {
    title: 'The mix',
    text: 'Each instrument goes to a mixer track: its level, its place from left to right, its effects. A reverb makes a room around the sound, a delay echoes it. Right-click any knob to make it move by itself over the song. Check the mix, under the spectrum, says what to fix.',
    show: { label: 'Show the mixer', open: 'mixer' },
  },
  {
    title: 'Sounds ready made',
    text: 'Above each instrument and effect, Presets offers sounds already made: an 808 that glides, a soft pad, a big hall. And File, New song from a template, starts a whole song in a style, trap, lo-fi or techno, to be made your own.',
  },
  {
    title: 'Nothing is lost',
    text: 'Ctrl+S saves the song as a folder. Every save is kept as a version: under Versions, at the right, listen to an older one or go back to it. Ctrl+Z undoes, and should Sonevo close, your changes are offered back.',
  },
];

/** A guide for a first song, a step at a time, for those who never made music before. */
@Component({
  selector: 'app-guide',
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <div class="guide" role="dialog">
      <div class="dots">
        @for (s of steps; track $index) {
          <button class="dot" [class.on]="$index === at()" (click)="at.set($index)"></button>
        }
      </div>
      <h2>{{ t(step().title) }}</h2>
      <p>{{ t(step().text) }}</p>
      @if (step().show; as show) {
        <button class="s-btn" (click)="act(show)">{{ t(show.label) }}</button>
      }
      <div class="actions">
        <button class="s-btn" (click)="closed.emit()">{{ t('Close') }}</button>
        <span class="grow"></span>
        <button class="s-btn" [disabled]="at() === 0" (click)="at.set(at() - 1)">
          {{ t('Back') }}
        </button>
        @if (at() < steps.length - 1) {
          <button class="s-btn s-btn--primary" (click)="at.set(at() + 1)">{{ t('Next') }}</button>
        } @else {
          <button class="s-btn s-btn--primary" (click)="closed.emit()">{{ t('Let’s go') }}</button>
        }
      </div>
    </div>
  `,
  styles: `
    :host {
      position: fixed;
      inset: 0;
      z-index: 30;
    }
    .backdrop {
      position: absolute;
      inset: 0;
      background: rgb(0 0 0 / 0.35);
    }
    .guide {
      position: absolute;
      right: 24px;
      bottom: 24px;
      width: 440px;
      padding: 20px 22px;
      border: 1px solid var(--s-line-strong);
      border-radius: 12px;
      background: var(--s-base);
      box-shadow: 0 24px 64px rgb(0 0 0 / 0.6);
    }
    .dots {
      display: flex;
      gap: 6px;
      margin-bottom: 14px;
    }
    .dot {
      width: 8px;
      height: 8px;
      padding: 0;
      border: 0;
      border-radius: 50%;
      background: var(--s-line-strong);
      cursor: pointer;
      &.on {
        background: var(--s-accent);
      }
    }
    h2 {
      margin: 0 0 8px;
      font-size: 17px;
    }
    p {
      margin: 0 0 14px;
      color: var(--s-text-2);
      line-height: 1.6;
    }
    .actions {
      display: flex;
      gap: 8px;
      margin-top: 18px;
    }
    .grow {
      flex: 1;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Guide {
  protected readonly t = inject(I18n).t;
  private readonly studio = inject(Studio);
  readonly closed = output();

  protected readonly steps = STEPS;
  protected readonly at = signal(0);
  protected readonly step = () => STEPS[this.at()];

  protected act(show: NonNullable<Step['show']>): void {
    const pattern = this.studio.pattern();
    if (show.open === 'pattern' && pattern) void this.studio.openPattern(pattern.id);
    if (show.open === 'mixer') void this.studio.showTab('mixer');
    if (show.play && !this.studio.status().playing) this.studio.togglePlay();
  }
}
