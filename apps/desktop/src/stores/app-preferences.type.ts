export type AccentColor = 'graphite' | 'indigo' | 'blue' | 'teal' | 'green' | 'amber' | 'rose';

export type CaptionTextSize = 'compact' | 'standard' | 'large';

export type AppPreferences = {
  accentColor: AccentColor;
  captionTextSize: CaptionTextSize;
  colorblindMode: boolean;
  confirmBeforeQuit: boolean;
  reduceMotion: boolean;
};

export type AppPreferenceActions = {
  setAccentColor: (color: AccentColor) => void;
  setCaptionTextSize: (size: CaptionTextSize) => void;
  setColorblindMode: (enabled: boolean) => void;
  setConfirmBeforeQuit: (enabled: boolean) => void;
  setReduceMotion: (enabled: boolean) => void;
};

export type AppPreferencesStore = AppPreferences & AppPreferenceActions;
