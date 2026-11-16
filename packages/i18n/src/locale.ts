import type { LanguagePreference, Locale } from './i18n.type';

export const FALLBACK_LOCALE: Locale = 'en-US';

export const SUPPORTED_LOCALES: readonly Locale[] = ['en-US', 'zh-CN'];

export function getSystemLanguageTag(): string {
  if (typeof navigator === 'undefined') return '';
  return navigator.language;
}

export function resolveLocale(languageTag: string | undefined): Locale {
  const normalized = languageTag?.toLowerCase() ?? '';
  if (normalized.startsWith('zh')) return 'zh-CN';
  if (normalized.startsWith('en')) return 'en-US';
  return FALLBACK_LOCALE;
}

export function getLocaleForPreference(preference: LanguagePreference): Locale {
  if (preference === 'system') return resolveLocale(getSystemLanguageTag());
  return preference;
}

export function isLanguagePreference(value: unknown): value is LanguagePreference {
  return value === 'system' || value === 'en-US' || value === 'zh-CN';
}
