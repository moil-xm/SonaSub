export const enUs = {
  navigation: {
    overview: 'Overview',
    about: 'About',
  },
  theme: {
    switchToDark: 'Switch to dark',
    switchToLight: 'Switch to light',
  },
  language: {
    label: 'Language',
    followSystem: 'Follow system',
    english: 'English',
    chinese: '简体中文',
  },
  overview: {
    title: 'Runtime overview',
    description: 'Manage local engine startup, connection and shutdown.',
  },
  engine: {
    title: 'Local engine',
    description:
      'The desktop app manages the engine and exposes real runtime information over a loopback connection. Audio, recognition and translation are not connected yet.',
    states: {
      stopped: 'Stopped',
      starting: 'Starting',
      running: 'Running',
      stopping: 'Stopping',
      failed: 'Failed to start or run',
    },
    status: {
      checking: 'Checking',
      queryFailed: 'Status query failed',
    },
    actions: {
      start: 'Start',
      stop: 'Stop',
      restart: 'Restart',
    },
    errors: {
      failed: 'Engine failed',
      operationFailed: 'Engine operation failed',
      statusQueryFailed: 'Engine status query failed',
      unknown: 'Unknown engine error',
    },
    details: {
      version: 'Engine version',
      startedAt: 'Started at',
    },
    diagnostics: {
      title: 'Development diagnostics',
    },
  },
  extension: {
    description: 'The extension entry point is ready; subtitling is not connected yet.',
    roadmap: 'Multi-tab handling, capture and recognition flows are left to later tasks.',
  },
  about: {
    description:
      'SonaSub is an open-source real-time subtitling project. This page was rendered at {{time}}.',
    moduleCount_one: '{{count}} module',
    moduleCount_other: '{{count}} modules',
    modules: {
      desktop: 'Desktop frontend and the src-tauri native backend',
      engine: 'Loopback WebSocket engine with real runtime information',
      extension: 'Chrome extension',
      ui: 'Shared components, theme, icons and styles',
      protocol: 'Desktop-backend and engine communication contract',
      i18n: 'Shared interface translations and type-safe i18next setup',
    },
  },
} as const;

export type TranslationResource = typeof enUs;
