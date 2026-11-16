export { I18nextProvider, useTranslation } from 'react-i18next';

export { createI18n } from './i18n';
export type { LanguagePreference, Locale } from './i18n.type';
export {
  FALLBACK_LOCALE,
  getLocaleForPreference,
  getSystemLanguageTag,
  isLanguagePreference,
  resolveLocale,
  SUPPORTED_LOCALES,
} from './locale';
