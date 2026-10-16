import {
  ApplicationConfig,
  inject,
  provideAppInitializer,
  provideBrowserGlobalErrorListeners,
} from '@angular/core';
import { api } from './core/api';
import { I18n } from './core/i18n';
import { Studio } from './core/studio.service';
import { Themes } from './core/theme';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    // the song, the language and the theme before anything shows
    provideAppInitializer(() => {
      const studio = inject(Studio);
      const i18n = inject(I18n);
      const themes = inject(Themes);
      const settings = api.settings().then((s) => {
        i18n.set(s.language);
        themes.set(s.theme);
        themes.setAccent(s.accent);
      });
      return Promise.all([studio.start(), settings]);
    }),
  ],
};
