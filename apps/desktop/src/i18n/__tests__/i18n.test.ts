import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

const LANGUAGE_STORAGE_KEY = 'sonasub-language';

function createMemoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));

  return {
    getItem(key: string) {
      return values.get(key) ?? null;
    },
    setItem(key: string, value: string) {
      values.set(key, value);
    },
    removeItem(key: string) {
      values.delete(key);
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe('desktop language preference', () => {
  it('restores a saved language and persists or clears later choices', async () => {
    const storage = createMemoryStorage({ [LANGUAGE_STORAGE_KEY]: 'zh-CN' });
    vi.stubGlobal('localStorage', storage);
    vi.stubGlobal('window', { addEventListener: vi.fn() });
    vi.stubGlobal('document', { documentElement: { lang: '' } });
    vi.stubGlobal('navigator', { language: 'zh-CN' });

    const { i18n, setLanguagePreference } = await import('../i18n');

    expect(i18n.language).toBe('zh-CN');
    expect(document.documentElement.lang).toBe('zh-CN');

    setLanguagePreference('en-US');

    expect(storage.getItem(LANGUAGE_STORAGE_KEY)).toBe('en-US');

    setLanguagePreference('system');

    expect(storage.getItem(LANGUAGE_STORAGE_KEY)).toBeNull();
  });

  it('falls back to English when the system language is unsupported', async () => {
    vi.stubGlobal('localStorage', createMemoryStorage());
    vi.stubGlobal('window', { addEventListener: vi.fn() });
    vi.stubGlobal('document', { documentElement: { lang: '' } });
    vi.stubGlobal('navigator', { language: 'fr-FR' });

    const { i18n } = await import('../i18n');

    expect(i18n.language).toBe('en-US');
  });
});
