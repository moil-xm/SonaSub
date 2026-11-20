import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { RecordDevice, RecordSnapshot, RecordStats } from '@/lib/records';
import type { AppConfig, Capabilities, ModelStatus } from '@/lib/session';

import { RecordToolbar } from '../RecordToolbar';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const EMPTY_STATS: RecordStats = {
  startedAtMs: 0,
  activeSources: 0,
  audioFrames: 0,
  droppedFrames: 0,
  partialUpdates: 0,
  finalizedSubtitles: 0,
  translationRequests: 0,
  translationFailures: 0,
  speakerUpdates: 0,
  diarizationWindows: 0,
  audioPeak: 0,
  audioRms: 0,
};

function readyModel(id: string): ModelStatus {
  return {
    id,
    label: id,
    available: true,
    missingFiles: [],
    upstream: 'https://example.invalid',
    version: 'test',
    files: [],
  };
}

function capabilities(): Capabilities {
  return {
    modelsDir: '/tmp/models',
    platform: 'macos-aarch64',
    audio: {
      backend: 'CPAL / Core Audio',
      systemCapture: true,
      microphoneCapture: true,
      supportedPlatform: 'macOS 14.6 or later',
      permission: 'Screen & System Audio Recording',
      defaultSystemOutput: { kind: 'systemOutput', id: 'device-a' },
      defaultMicrophone: null,
      devices: [],
    },
    asrModels: [],
    translationModels: [],
    translationTargets: [],
    translationEngines: [],
    diarization: readyModel('diarization'),
    speakerEmbedding: readyModel('speaker-embedding'),
    vad: readyModel('vad'),
  };
}

function config(): AppConfig {
  return {
    version: 1,
    audio: {
      live: { kind: 'systemOutput', deviceId: null },
      record: [{ kind: 'systemOutput', id: 'device-a' }],
      saveAudio: true,
    },
    recognition: {
      mode: 'auto',
      modelId: 'asr',
      sourceLanguage: 'auto',
      maxSegmentSecs: 12,
      stableWaitMs: 800,
      vadThreshold: 0.5,
      minSilenceSecs: 0.45,
      minSpeechSecs: 0.25,
    },
    translation: {
      engine: 'bergamot',
      modelId: 'translation',
      mode: 'auto',
      trigger: 'finalOnly',
      targetLanguage: 'zh-Hans',
      openaiCompatible: {
        baseUrl: 'https://api.openai.com/v1',
        model: '',
        apiKey: '',
      },
    },
    speakers: {
      enabled: false,
      diarizationEnabled: false,
      windowSecs: 20,
      maxSpeakers: 8,
      similarityThreshold: 0.55,
    },
    ui: { historyLimit: 400 },
  };
}

function snapshot(): RecordSnapshot {
  return {
    summary: {
      id: 'current-record',
      name: 'Current record',
      state: 'notStarted',
      devices: [{ kind: 'systemOutput', id: 'device-a' }],
      activeDevices: [],
      hasActiveSource: false,
      lastError: null,
      createdAtMs: 1,
      updatedAtMs: 1,
      revision: 1,
    },
    config: config(),
    sources: [],
    segments: [],
    stats: EMPTY_STATS,
    notices: [],
    summaryRevision: 1,
    subtitleRevision: 0,
    statsRevision: 0,
    totalSubtitleCount: 0,
    historyLimit: 400,
  };
}

function recordDevice(available: boolean): RecordDevice {
  if (available) {
    return {
      kind: 'systemOutput',
      id: 'device-a',
      name: 'Device A',
      isDefault: true,
      captureSupported: true,
      captureReason: null,
      isAvailable: true,
      occupiedRecordId: null,
      occupiedRecordName: null,
    };
  }
  return {
    kind: 'systemOutput',
    id: 'device-a',
    name: 'Device A',
    isDefault: true,
    captureSupported: true,
    captureReason: null,
    isAvailable: false,
    occupiedRecordId: 'other-record',
    occupiedRecordName: 'Other record',
  };
}

function renderToolbar(device: RecordDevice) {
  const rootRoute = createRootRoute({ component: () => <Outlet /> });
  const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => (
      <RecordToolbar capabilities={capabilities()} devices={[device]} snapshot={snapshot()} />
    ),
  });
  const editRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/records/$recordId/edit',
    component: () => null,
  });
  const routeTree = rootRoute.addChildren([indexRoute, editRoute]);
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: ['/'] }),
  });
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  render(
    <I18nextProvider i18n={createI18n('en-US')}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

afterEach(() => {
  cleanup();
  invoke.mockReset();
});

describe('RecordToolbar', () => {
  it('disables Start when a selected device is occupied by another record', async () => {
    renderToolbar(recordDevice(false));

    const start = await screen.findByRole('button', { name: 'Start' });

    expect(start).toHaveProperty('disabled', true);
  });

  it('keeps Start enabled when selected devices are available', async () => {
    renderToolbar(recordDevice(true));

    const start = await screen.findByRole('button', { name: 'Start' });

    expect(start).toHaveProperty('disabled', false);
  });
});
