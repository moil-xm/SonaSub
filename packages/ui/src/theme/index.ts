import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Theme = 'light' | 'dark';

interface ThemeState {
  theme: Theme;
  toggleTheme: () => void;
}

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark');
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: 'light',
      toggleTheme: () => set((state) => ({ theme: state.theme === 'dark' ? 'light' : 'dark' })),
    }),
    {
      name: 'sonasub-theme',
      onRehydrateStorage: () => (state) => {
        if (state) applyTheme(state.theme);
      },
    },
  ),
);

useThemeStore.subscribe((state) => applyTheme(state.theme));
applyTheme(useThemeStore.getState().theme);
