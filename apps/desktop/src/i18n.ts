import { useSyncExternalStore } from 'react';
import {
  createI18n,
  getLocaleForPreference,
  isLanguagePreference,
  type LanguagePreference,
} from '@sonasub/i18n';

const LANGUAGE_STORAGE_KEY = 'sonasub-language';
const listeners = new Set<() => void>();

function readStoredPreference(): LanguagePreference {
  const stored: unknown = localStorage.getItem(LANGUAGE_STORAGE_KEY);
  if (isLanguagePreference(stored)) return stored;
  return 'system';
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notify() {
  for (const listener of listeners) listener();
}

let currentLanguagePreference = readStoredPreference();

export const i18n = createI18n(getLocaleForPreference(currentLanguagePreference));

export function useLanguagePreference() {
  return useSyncExternalStore(subscribe, () => currentLanguagePreference);
}

export function setLanguagePreference(preference: LanguagePreference) {
  if (preference === currentLanguagePreference) return;

  currentLanguagePreference = preference;
  if (preference === 'system') localStorage.removeItem(LANGUAGE_STORAGE_KEY);
  else localStorage.setItem(LANGUAGE_STORAGE_KEY, preference);

  notify();
  void i18n.changeLanguage(getLocaleForPreference(preference));
}

window.addEventListener('languagechange', () => {
  if (currentLanguagePreference === 'system') {
    void i18n.changeLanguage(getLocaleForPreference('system'));
  }
});
