import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { api } from '../core/api';
import { I18n } from '../core/i18n';
import { formatPosition, newId, Pattern } from '../core/models';
import { Studio } from '../core/studio.service';
import { Icon } from '../shared/icon';
import { Menus } from '../shared/menu';
import { Meter } from '../shared/meter';
import { Rename } from '../shared/rename';

/**
 * The transport: play and stop, the pattern played alone or the whole song, the tempo, where
 * playing is, the metronome; the master's level and the sound card's latency.
 */
@Component({
  selector: 'app-transport',
  imports: [Icon, Meter],
  template: `
    <div class="group">
      <button
        class="play"
        [class.on]="studio.status().playing"
        (click)="studio.togglePlay()"
        [title]="t('Play') + ' / ' + t('Pause') + ' (Space)'"
      >
        <s-icon
          [name]="studio.status().playing ? 'pause' : 'play'"
          [size]="16"
          [filled]="!studio.status().playing"
        />
      </button>
      <button class="s-bar-btn" (click)="stop()" [title]="t('Stop')">
        <s-icon name="stop" [filled]="true" />
      </button>
      <button
        class="record"
        [class.on]="recording()"
        (click)="studio.toggleRecord()"
        [title]="recording() ? t('Stop recording') : t('Record the microphone, the song playing')"
      >
        <s-icon name="record" [filled]="true" [size]="12" />
      </button>
      @if (recording()) {
        <s-meter class="input" [peak]="studio.status().recording ?? 0" />
      }
    </div>

    <div class="s-seg" role="radiogroup">
      <button
        role="radio"
        [attr.aria-checked]="patternMode()"
        (click)="setMode(true)"
        [title]="t('Play the pattern chosen, over and over')"
      >
        {{ t('Pattern') }}
      </button>
      <button
        role="radio"
        [attr.aria-checked]="!patternMode()"
        (click)="setMode(false)"
        [title]="t('Play the whole song')"
      >
        {{ t('Song') }}
      </button>
    </div>

    <div class="readout">
      <span class="label">{{ t('Tempo') }}</span>
      @if (editingTempo()) {
        <input
          class="s-inline s-mono tempo-input"
          [value]="tempo()"
          (keydown.enter)="setTempo($any($event.target).value)"
          (keydown.escape)="editingTempo.set(false)"
          (blur)="setTempo($any($event.target).value)"
          autofocus
        />
      } @else {
        <span
          class="value s-mono drag"
          (pointerdown)="dragTempo($event)"
          (dblclick)="editingTempo.set(true)"
          [title]="t('Drag up or down, or double-click to type')"
          >{{ tempoShown() }}</span
        >
      }
      <span class="unit">BPM</span>
      <button
        class="tap"
        (click)="tap()"
        [title]="t('Tap in time, a few times: the tempo follows')"
      >
        {{ t('Tap') }}
      </button>
      <button
        class="signature s-mono"
        (click)="signatureMenu($event)"
        [title]="t('Beats to a bar')"
      >
        {{ studio.song()?.signature?.beats }}/{{ studio.song()?.signature?.unit }}
      </button>
    </div>

    <div class="readout position">
      <span class="label">{{ t('Position') }}</span>
      <span class="value s-mono">{{ position() }}</span>
    </div>

    <button
      class="s-bar-btn"
      [class.on]="studio.metronome()"
      (click)="toggleMetronome()"
      [title]="t('Metronome')"
    >
      <s-icon name="metronome" [size]="16" />
    </button>

    <div class="pattern">
      <button
        class="s-bar-btn pattern-pick"
        (click)="patternMenu($event)"
        [title]="t('The pattern edited')"
      >
        <span class="swatch" [style.background]="studio.pattern()?.color"></span>
        {{ studio.pattern()?.name }}
        <s-icon name="down" [size]="12" />
      </button>
      <button class="s-bar-btn" (click)="studio.addPattern()" [title]="t('New pattern')">
        <s-icon name="plus" />
      </button>
    </div>

    <div class="grow"></div>

    <div class="audio" [class.bad]="!!audioProblem()" [title]="audioTitle()">
      @if (audioProblem(); as problem) {
        {{ problem }}
      } @else {
        <span class="s-mono">{{ latency() }}</span>
      }
    </div>
    <div class="master">
      <s-meter [peak]="studio.status().master" />
      <s-meter [peak]="studio.status().master" />
    </div>
  `,
  styles: `
    :host {
      flex: none;
      height: var(--s-transport);
      display: flex;
      align-items: center;
      gap: 14px;
      padding: 0 12px;
      background: var(--s-base);
      border-bottom: 1px solid var(--s-line);
    }
    .group {
      display: flex;
      align-items: center;
      gap: 4px;
    }
    .record {
      width: 30px;
      height: 30px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 0;
      border-radius: var(--s-radius);
      background: transparent;
      color: var(--s-record);
      cursor: pointer;
      &:hover {
        background: var(--s-hover);
      }
      &.on {
        background: var(--s-record);
        color: #1a0605;
        animation: pulse 1s ease-in-out infinite alternate;
      }
    }
    @keyframes pulse {
      to {
        background: color-mix(in srgb, var(--s-record) 60%, transparent);
      }
    }
    .input {
      height: 28px;
    }
    .play {
      width: 38px;
      height: 34px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 0;
      border-radius: var(--s-radius);
      background: var(--s-raised);
      color: var(--s-text);
      cursor: pointer;
      &:hover {
        background: var(--s-hover);
      }
      &.on {
        background: var(--s-play);
        color: #06140e;
      }
    }
    .readout {
      display: flex;
      align-items: baseline;
      gap: 6px;
      height: 34px;
      padding: 0 12px;
      border-radius: var(--s-radius);
      background: var(--s-void);
      line-height: 34px;
      .label {
        color: var(--s-text-3);
        font-size: 11px;
      }
      .value {
        min-width: 44px;
        color: var(--s-text);
        font-size: 15px;
        text-align: right;
      }
      .unit {
        color: var(--s-text-3);
        font-size: 11px;
      }
      .drag {
        cursor: ns-resize;
      }
    }
    .tap,
    .signature {
      height: 24px;
      padding: 0 8px;
      border: 0;
      border-radius: 4px;
      background: var(--s-hover);
      color: var(--s-text-2);
      font-size: 11px;
      cursor: pointer;
      &:hover {
        color: var(--s-text);
      }
    }
    .position .value {
      min-width: 64px;
    }
    .tempo-input {
      width: 64px;
      margin-top: 4px;
    }
    .pattern {
      display: flex;
      align-items: center;
      gap: 2px;
    }
    .pattern-pick {
      min-width: 140px;
      justify-content: flex-start;
      color: var(--s-text);
    }
    .swatch {
      width: 10px;
      height: 10px;
      border-radius: 3px;
    }
    .grow {
      flex: 1;
    }
    .audio {
      color: var(--s-text-3);
      font-size: 12px;
      &.bad {
        color: var(--s-warn);
      }
    }
    .master {
      display: flex;
      gap: 3px;
      height: 32px;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Transport {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly menus = inject(Menus);
  private readonly rename = inject(Rename);

  protected readonly editingTempo = signal(false);
  private readonly draggingTempo = signal<number | null>(null);
  protected readonly tempo = computed(() => this.studio.song()?.tempo ?? 120);
  protected readonly tempoShown = computed(() => (this.draggingTempo() ?? this.tempo()).toFixed(1));
  protected readonly patternMode = computed(() => !!this.studio.view()?.patternMode);
  protected readonly recording = computed(() => this.studio.status().recording !== null);

  protected readonly position = computed(() => {
    const song = this.studio.song();
    return song ? formatPosition(this.studio.status().position, song.signature) : '1.1.1';
  });

  protected readonly latency = computed(() => {
    const audio = this.studio.status().audio;
    return audio.device
      ? `${audio.latencyMs.toFixed(0)} ms · ${(audio.sampleRate / 1000).toFixed(1)} kHz`
      : '';
  });

  protected readonly audioProblem = computed(() => {
    const audio = this.studio.status().audio;
    if (audio.failed) return this.t('The sound card stopped');
    if (!audio.device) return audio.error ? this.t('No sound card') : '';
    return '';
  });

  protected readonly audioTitle = computed(() => {
    const audio = this.studio.status().audio;
    return audio.error ?? audio.device ?? '';
  });

  protected stop(): void {
    if (this.recording()) void this.studio.stopRecording().then(() => api.stop());
    else void api.stop();
  }

  protected setMode(pattern: boolean): void {
    void this.studio.setMode(pattern ? (this.studio.pattern()?.id ?? null) : null);
  }

  protected toggleMetronome(): void {
    const on = !this.studio.metronome();
    this.studio.metronome.set(on);
    void api.metronome(on);
  }

  protected setTempo(text: string): void {
    this.editingTempo.set(false);
    const tempo = Number(String(text).replace(',', '.'));
    if (Number.isFinite(tempo) && tempo >= 10 && tempo <= 999) this.commitTempo(tempo);
  }

  private commitTempo(tempo: number): void {
    if (tempo !== this.studio.song()?.tempo) void this.studio.setSong({ tempo });
  }

  /** The times tapped lately: their spacing is the tempo. */
  private taps: number[] = [];

  protected tap(): void {
    const now = performance.now();
    // a pause of two seconds starts again
    this.taps = [...this.taps.filter((t) => now - t < 2000), now].slice(-6);
    if (this.taps.length < 3) return;
    const spacing = (this.taps[this.taps.length - 1] - this.taps[0]) / (this.taps.length - 1);
    this.commitTempo(Math.round((60000 / spacing) * 10) / 10);
  }

  protected signatureMenu(event: MouseEvent): void {
    const signatures = [
      [4, 4],
      [3, 4],
      [2, 4],
      [5, 4],
      [6, 8],
      [7, 8],
      [12, 8],
    ];
    this.menus.below(
      event,
      signatures.map(([beats, unit]) => ({
        label: `${beats}/${unit}`,
        run: () => this.studio.setSong({ signature: { beats, unit } }),
      })),
    );
  }

  protected dragTempo(event: PointerEvent): void {
    if (event.button !== 0) return;
    const target = event.currentTarget as Element;
    target.setPointerCapture(event.pointerId);
    const start = this.tempo();
    const y = event.clientY;
    const move = (e: PointerEvent) => {
      const step = e.shiftKey ? 0.1 : 1;
      const tempo = Math.min(999, Math.max(10, start + Math.round((y - e.clientY) / 3) * step));
      this.draggingTempo.set(Math.round(tempo * 10) / 10);
    };
    const up = () => {
      target.removeEventListener('pointermove', move as EventListener);
      target.removeEventListener('pointerup', up);
      const tempo = this.draggingTempo();
      this.draggingTempo.set(null);
      if (tempo !== null) this.commitTempo(tempo);
    };
    target.addEventListener('pointermove', move as EventListener);
    target.addEventListener('pointerup', up);
  }

  /** A copy of `pattern` and its notes, to change one while keeping the other. */
  private async duplicate(pattern: Pattern): Promise<void> {
    const copy: Pattern = { ...pattern, id: newId(), name: `${pattern.name} (2)` };
    const notes = this.studio.view()?.notes[pattern.id] ?? {};
    const song = this.studio.song();
    const index = song ? song.patterns.findIndex((p) => p.id === pattern.id) + 1 : null;
    const done = await this.studio.edit({
      type: 'batch',
      edits: [
        { type: 'putPattern', pattern: copy, index },
        ...Object.entries(notes).map(([channel, list]) => ({
          type: 'setNotes' as const,
          pattern: copy.id,
          channel,
          notes: list,
        })),
      ],
    });
    if (done) this.studio.selectPattern(copy.id);
  }

  protected patternMenu(event: MouseEvent): void {
    const song = this.studio.song();
    if (!song) return;
    const t = this.t;
    const current = this.studio.pattern();
    this.menus.below(event, [
      ...song.patterns.map((p) => ({ label: p.name, run: () => this.studio.selectPattern(p.id) })),
      { separator: true, label: '' },
      { label: t('New pattern'), run: () => this.studio.addPattern() },
      {
        label: t('Rename…'),
        disabled: !current,
        run: () => {
          if (!current) return;
          void this.rename.ask(t('Name of the pattern'), current.name).then((name) => {
            if (name)
              void this.studio.edit({
                type: 'putPattern',
                pattern: { ...current, name },
                index: null,
              });
          });
        },
      },
      {
        label: t('Duplicate, as a variant'),
        disabled: !current,
        run: () => current && this.duplicate(current),
      },
      {
        label: t('Delete the pattern'),
        disabled: !current || song.patterns.length < 2,
        danger: true,
        run: () => current && this.studio.edit({ type: 'removePattern', id: current.id }),
      },
    ]);
  }
}
