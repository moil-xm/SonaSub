import { useSyncExternalStore } from 'react';
import {
  createI18n,
  getLocaleForPreference,
  isLanguagePreference,
  type LanguagePreference,
} from '@sonasub/i18n';
import { browser } from 'wxt/browser';

const LANGUAGE_STORAGE_KEY = 'sonasub-language';
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notify() {
  for (const listener of listeners) listener();
}

let currentLanguagePreference: LanguagePreference = 'system';
let i18n: ReturnType<typeof createI18n> | undefined;

export async function initializeI18n() {
  const stored = await browser.storage.local.get(LANGUAGE_STORAGE_KEY);
  const preference = stored[LANGUAGE_STORAGE_KEY];
  if (isLanguagePreference(preference)) currentLanguagePreference = preference;

  i18n = createI18n(getLocaleForPreference(currentLanguagePreference));
  window.addEventListener('languagechange', () => {
    if (currentLanguagePreference === 'system') {
      void i18n?.changeLanguage(getLocaleForPreference('system'));
    }
  });

  return i18n;
}

export function useLanguagePreference() {
  return useSyncExternalStore(subscribe, () => currentLanguagePreference);
}

export function setLanguagePreference(preference: LanguagePreference) {
  if (preference === currentLanguagePreference) return;

  currentLanguagePreference = preference;
  if (preference === 'system') void browser.storage.local.remove(LANGUAGE_STORAGE_KEY);
  else void browser.storage.local.set({ [LANGUAGE_STORAGE_KEY]: preference });

  notify();
  void i18n?.changeLanguage(getLocaleForPreference(preference));
}
