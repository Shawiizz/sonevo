import { Injectable, signal } from '@angular/core';
import { FR } from './fr';

export type Language = 'en' | 'fr';

/**
 * The interface's words: English as written in the code, French from fr.ts. A text with
 * {name} in it takes the value of `name`.
 */
@Injectable({ providedIn: 'root' })
export class I18n {
  readonly language = signal<Language>(navigator.language.startsWith('fr') ? 'fr' : 'en');

  readonly t = (text: string, values?: Record<string, string | number>): string => {
    const said = this.language() === 'fr' ? (FR[text] ?? text) : text;
    if (!values) return said;
    return said.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''));
  };

  set(language: string | null): void {
    if (language === 'fr' || language === 'en') this.language.set(language);
    else this.language.set(navigator.language.startsWith('fr') ? 'fr' : 'en');
  }
}
