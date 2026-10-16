import {
  ApplicationConfig,
  inject,
  provideAppInitializer,
  provideBrowserGlobalErrorListeners,
} from '@angular/core';
import { api } from './core/api';
import { I18n } from './core/i18n';
import { Studio } from './core/studio.service';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    // the song and the language before anything shows
    provideAppInitializer(() => {
      const studio = inject(Studio);
      const i18n = inject(I18n);
      return Promise.all([studio.start(), api.settings().then((s) => i18n.set(s.language))]);
    }),
  ],
};
