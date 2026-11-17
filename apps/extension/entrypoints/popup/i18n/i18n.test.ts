import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const LANGUAGE_STORAGE_KEY = 'sonasub-language';
const storage = vi.hoisted(() => ({
  get: vi.fn(),
  set: vi.fn(),
  remove: vi.fn(),
}));

vi.mock('wxt/browser', () => ({
  browser: {
    storage: {
      local: storage,
    },
  },
}));

beforeEach(() => {
  vi.resetModules();
  storage.get.mockReset();
  storage.set.mockReset();
  storage.remove.mockReset();
  storage.get.mockResolvedValue({});
  storage.set.mockResolvedValue(undefined);
  storage.remove.mockResolvedValue(undefined);
  vi.stubGlobal('window', { addEventListener: vi.fn() });
  vi.stubGlobal('document', { documentElement: { lang: '' } });
  vi.stubGlobal('navigator', { language: 'zh-CN' });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('extension language preference', () => {
  it('restores a saved language and persists or clears later choices', async () => {
    storage.get.mockResolvedValue({ [LANGUAGE_STORAGE_KEY]: 'zh-CN' });
    const { initializeI18n, setLanguagePreference } = await import('./i18n');

    const instance = await initializeI18n();

    expect(instance.language).toBe('zh-CN');
    expect(document.documentElement.lang).toBe('zh-CN');

    setLanguagePreference('en-US');

    expect(storage.set).toHaveBeenCalledWith({ [LANGUAGE_STORAGE_KEY]: 'en-US' });

    setLanguagePreference('system');

    expect(storage.remove).toHaveBeenCalledWith(LANGUAGE_STORAGE_KEY);
  });

  it('falls back to English when the system language is unsupported', async () => {
    vi.stubGlobal('navigator', { language: 'fr-FR' });
    const { initializeI18n } = await import('./i18n');

    const instance = await initializeI18n();

    expect(instance.language).toBe('en-US');
  });
});
