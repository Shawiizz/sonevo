import { ChangeDetectionStrategy, Component, inject, output } from '@angular/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { I18n } from '../core/i18n';
import { Studio } from '../core/studio.service';
import { Icon } from '../shared/icon';
import { Menus } from '../shared/menu';
import { Rename } from '../shared/rename';
import { Files } from './files.service';

@Component({
  selector: 'app-titlebar',
  imports: [Icon],
  template: `
    <div class="brand" data-tauri-drag-region>
      <img src="favicon.png" alt="" />
      <button class="s-bar-btn" (click)="fileMenu($event)">{{ t('File') }}</button>
      <button class="s-bar-btn" (click)="editMenu($event)">{{ t('Edit') }}</button>
      <button class="s-bar-btn" (click)="settings.emit()">{{ t('Settings') }}</button>
      <button class="s-bar-btn" (click)="guide.emit()">{{ t('First steps') }}</button>
    </div>
    <div class="title" data-tauri-drag-region>
      <button class="name" (click)="rename()" [title]="t('Rename the song')">
        {{ studio.song()?.name }}
      </button>
      @if (studio.view()?.dirty) {
        <span class="dirty" [title]="t('Changes not saved')"></span>
      }
      @if (!studio.view()?.folder) {
        <span class="draft" data-tauri-drag-region>{{ t('not saved yet') }}</span>
      }
    </div>
    <div class="window">
      <button class="s-bar-btn" (click)="palette.emit()" [title]="t('Commands') + ' (Ctrl+K)'">
        <s-icon name="search" />
      </button>
      <button class="win" (click)="minimize()"><s-icon name="minimize" /></button>
      <button class="win" (click)="maximize()"><s-icon name="maximize" [size]="12" /></button>
      <button class="win close" (click)="close()"><s-icon name="close" /></button>
    </div>
  `,
  styles: `
    :host {
      flex: none;
      height: var(--s-titlebar);
      display: grid;
      grid-template-columns: 1fr auto 1fr;
      align-items: center;
      background: var(--s-void);
      border-bottom: 1px solid var(--s-line);
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 2px;
      height: 100%;
      padding-left: 10px;
      img {
        width: 20px;
        height: 20px;
        margin-right: 8px;
      }
    }
    .title {
      display: flex;
      align-items: center;
      gap: 8px;
      height: 100%;
      font-weight: 600;
    }
    .name {
      padding: 2px 8px;
      border: 0;
      border-radius: var(--s-radius);
      background: transparent;
      font-weight: 600;
      cursor: pointer;
      &:hover {
        background: var(--s-hover);
      }
    }
    .dirty {
      width: 7px;
      height: 7px;
      border-radius: 50%;
      background: var(--s-warn);
    }
    .draft {
      color: var(--s-text-3);
      font-size: 12px;
      font-weight: 400;
    }
    .window {
      display: flex;
      justify-content: flex-end;
      align-items: center;
      height: 100%;
    }
    .win {
      width: 46px;
      height: 100%;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 0;
      background: transparent;
      color: var(--s-text-2);
      cursor: pointer;
      &:hover {
        background: var(--s-hover);
        color: var(--s-text);
      }
      &.close:hover {
        background: #c42b1c;
        color: #fff;
      }
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Titlebar {
  protected readonly studio = inject(Studio);
  protected readonly t = inject(I18n).t;
  private readonly files = inject(Files);
  private readonly menus = inject(Menus);
  private readonly renaming = inject(Rename);
  readonly settings = output();
  readonly palette = output();
  readonly guide = output();

  protected fileMenu(event: MouseEvent): void {
    const t = this.t;
    this.menus.below(event, [
      { label: t('New song'), key: 'Ctrl+N', run: () => this.files.newSong() },
      { label: t('Open the demo'), run: () => this.files.newSong(true) },
      { label: t('Open…'), key: 'Ctrl+O', run: () => this.files.open() },
      { separator: true, label: '' },
      { label: t('Save'), key: 'Ctrl+S', run: () => this.files.save() },
      { label: t('Save as…'), key: 'Ctrl+Shift+S', run: () => this.files.saveAs() },
      { separator: true, label: '' },
      { label: t('Export as WAV…'), key: 'Ctrl+E', run: () => this.files.exportWav() },
    ]);
  }

  protected editMenu(event: MouseEvent): void {
    const t = this.t;
    const view = this.studio.view();
    this.menus.below(event, [
      { label: t('Undo'), key: 'Ctrl+Z', disabled: !view?.canUndo, run: () => this.studio.undo() },
      { label: t('Redo'), key: 'Ctrl+Y', disabled: !view?.canRedo, run: () => this.studio.redo() },
    ]);
  }

  protected rename(): void {
    const song = this.studio.song();
    if (!song) return;
    void this.renaming.ask(this.t('Name of the song'), song.name).then((name) => {
      if (name) void this.studio.setSong({ name });
    });
  }

  protected minimize(): void {
    void getCurrentWindow().minimize();
  }

  protected maximize(): void {
    void getCurrentWindow().toggleMaximize();
  }

  protected async close(): Promise<void> {
    if (await this.files.leave()) void getCurrentWindow().close();
  }
}
