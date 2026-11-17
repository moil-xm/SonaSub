import { create } from 'zustand';
import { persist } from 'zustand/middleware';

import type { AppPreferences, AppPreferencesStore } from './app-preferences.type';

const DEFAULT_PREFERENCES: AppPreferences = {
  accentColor: 'graphite',
  captionTextSize: 'standard',
  colorblindMode: false,
  confirmBeforeQuit: true,
  reduceMotion: false,
};

function applyPreferences(preferences: AppPreferences) {
  const root = document.documentElement;
  root.dataset.accent = preferences.accentColor;
  root.dataset.captionSize = preferences.captionTextSize;
  root.dataset.colorblind = String(preferences.colorblindMode);
  root.dataset.reduceMotion = String(preferences.reduceMotion);
}

const preferencesStore = create<AppPreferencesStore>()(
  persist(
    (set) => ({
      ...DEFAULT_PREFERENCES,
      setAccentColor: (accentColor) => set({ accentColor }),
      setCaptionTextSize: (captionTextSize) => set({ captionTextSize }),
      setColorblindMode: (colorblindMode) => set({ colorblindMode }),
      setConfirmBeforeQuit: (confirmBeforeQuit) => set({ confirmBeforeQuit }),
      setReduceMotion: (reduceMotion) => set({ reduceMotion }),
    }),
    {
      name: 'sonasub-app-preferences',
      onRehydrateStorage: () => (state) => {
        if (state) applyPreferences(state);
      },
    },
  ),
);

export function useAccentColor() {
  return preferencesStore((state) => state.accentColor);
}

export function useCaptionTextSize() {
  return preferencesStore((state) => state.captionTextSize);
}

export function useColorblindMode() {
  return preferencesStore((state) => state.colorblindMode);
}

export function useConfirmBeforeQuit() {
  return preferencesStore((state) => state.confirmBeforeQuit);
}

export function useReduceMotion() {
  return preferencesStore((state) => state.reduceMotion);
}

export function useSetAccentColor() {
  return preferencesStore((state) => state.setAccentColor);
}

export function useSetCaptionTextSize() {
  return preferencesStore((state) => state.setCaptionTextSize);
}

export function useSetColorblindMode() {
  return preferencesStore((state) => state.setColorblindMode);
}

export function useSetConfirmBeforeQuit() {
  return preferencesStore((state) => state.setConfirmBeforeQuit);
}

export function useSetReduceMotion() {
  return preferencesStore((state) => state.setReduceMotion);
}

preferencesStore.subscribe((state) => applyPreferences(state));
applyPreferences(preferencesStore.getState());
