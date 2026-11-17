import { create } from 'zustand';
import { persist } from 'zustand/middleware';

import type { Theme, ThemeStore } from './theme.type';

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark');
}

const themeStore = create<ThemeStore>()(
  persist(
    (set) => ({
      theme: 'light',
      setTheme: (theme) => set({ theme }),
      toggleTheme: () =>
        set((state) => {
          if (state.theme === 'dark') return { theme: 'light' };
          return { theme: 'dark' };
        }),
    }),
    {
      name: 'sonasub-theme',
      onRehydrateStorage: () => (state) => {
        if (state) applyTheme(state.theme);
      },
    },
  ),
);

export function useTheme() {
  return themeStore((state) => state.theme);
}

export function useToggleTheme() {
  return themeStore((state) => state.toggleTheme);
}

export function useSetTheme() {
  return themeStore((state) => state.setTheme);
}

themeStore.subscribe((state) => applyTheme(state.theme));
applyTheme(themeStore.getState().theme);
