import type { TranslationResource } from './resources/en-US';

export type Locale = 'en-US' | 'zh-CN';

export type LanguagePreference = 'system' | Locale;

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    returnNull: false;
    enableSelector: true;
    resources: {
      translation: TranslationResource;
    };
  }
}
