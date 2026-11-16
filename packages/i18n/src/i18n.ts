import { createInstance, type i18n as I18nInstance } from 'i18next';

import type { Locale } from './i18n.type';
import { FALLBACK_LOCALE, SUPPORTED_LOCALES } from './locale';
import { resources } from './resources';

function syncDocumentLanguage(language: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = language;
}

export function createI18n(locale: Locale): I18nInstance {
  const instance = createInstance();

  instance.on('languageChanged', syncDocumentLanguage);
  syncDocumentLanguage(locale);
  void instance.init({
    lng: locale,
    fallbackLng: FALLBACK_LOCALE,
    supportedLngs: SUPPORTED_LOCALES,
    load: 'currentOnly',
    resources,
    defaultNS: 'translation',
    ns: ['translation'],
    initAsync: false,
    enableSelector: true,
    returnNull: false,
    interpolation: { escapeValue: false },
  });

  return instance;
}
