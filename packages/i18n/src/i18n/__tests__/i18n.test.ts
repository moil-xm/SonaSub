import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { FALLBACK_LOCALE, isLanguagePreference, resolveLocale } from '../../locale';
import { enUs, zhCn } from '../../resources';

import { createI18n } from '../i18n';

function flattenTranslations(value: object, prefix = ''): Map<string, string> {
  const translations = new Map<string, string>();

  for (const key of Object.keys(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    const child: unknown = Reflect.get(value, key);
    if (typeof child === 'string') {
      translations.set(path, child);
    } else if (typeof child === 'object' && child !== null) {
      for (const [nestedPath, translation] of flattenTranslations(child, path)) {
        translations.set(nestedPath, translation);
      }
    }
  }

  return translations;
}

function interpolationVariables(value: string): string[] {
  return [...value.matchAll(/\{\{(\w+)\}\}/g)].map((match) => match[1] ?? '');
}

function assertUnknownKeyIsRejected(): void {
  const instance = createI18n('en-US');
  // @ts-expect-error The selector must reference a key from the shared resources.
  instance.t(($) => $.missing);
}

void assertUnknownKeyIsRejected;

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('locale resolution', () => {
  it('maps Chinese and English system languages to the supported locales', () => {
    expect(resolveLocale('zh-CN')).toBe('zh-CN');
    expect(resolveLocale('zh-Hant-TW')).toBe('zh-CN');
    expect(resolveLocale('en-US')).toBe('en-US');
    expect(resolveLocale('en-GB')).toBe('en-US');
  });

  it('falls back to English for unsupported or missing system languages', () => {
    expect(resolveLocale('fr-FR')).toBe(FALLBACK_LOCALE);
    expect(resolveLocale(undefined)).toBe(FALLBACK_LOCALE);
    expect(FALLBACK_LOCALE).toBe('en-US');
  });

  it('recognizes only the supported persisted language preferences', () => {
    expect(isLanguagePreference('system')).toBe(true);
    expect(isLanguagePreference('en-US')).toBe(true);
    expect(isLanguagePreference('zh-CN')).toBe(true);
    expect(isLanguagePreference('fr-FR')).toBe(false);
    expect(isLanguagePreference(null)).toBe(false);
  });
});

describe('translation resources', () => {
  it('contains the same keys and interpolation variables in every locale', () => {
    const english = flattenTranslations(enUs);
    const chinese = flattenTranslations(zhCn);

    expect([...chinese.keys()].toSorted()).toEqual([...english.keys()].toSorted());

    for (const [key, englishText] of english) {
      const chineseText = chinese.get(key);
      expect(englishText.trim()).not.toBe('');
      expect(chineseText?.trim()).not.toBe('');
      expect(interpolationVariables(chineseText ?? '')).toEqual(
        interpolationVariables(englishText),
      );
    }
  });

  it('resolves selectors, interpolation and plurals synchronously in both locales', async () => {
    const instance = createI18n('en-US');

    expect(instance.t(($) => $.navigation.live)).toBe('Live captions');
    expect(instance.t(($) => $.about.description, { time: '2026-11-16 12:00' })).toContain(
      '2026-11-16 12:00',
    );
    expect(instance.t(($) => $.about.moduleCount, { count: 1 })).toBe('1 module');
    expect(instance.t(($) => $.about.moduleCount, { count: 6 })).toBe('6 modules');

    await instance.changeLanguage('zh-CN');

    expect(instance.t(($) => $.navigation.live)).toBe('实时字幕');
    expect(instance.t(($) => $.about.description, { time: '2026-11-16 12:00' })).toContain(
      '2026-11-16 12:00',
    );
    expect(instance.t(($) => $.about.moduleCount, { count: 6 })).toBe('6 个模块');
  });

  it('keeps the document language in sync with the active locale', async () => {
    vi.stubGlobal('document', { documentElement: { lang: '' } });
    const instance = createI18n('en-US');

    expect(document.documentElement.lang).toBe('en-US');

    await instance.changeLanguage('zh-CN');

    expect(document.documentElement.lang).toBe('zh-CN');
  });
});
