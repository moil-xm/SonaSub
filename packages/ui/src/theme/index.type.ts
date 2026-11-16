export type Theme = 'light' | 'dark';

export type ThemeState = {
  theme: Theme;
};

export type ThemeActions = {
  toggleTheme: () => void;
};

export type ThemeStore = ThemeState & ThemeActions;
