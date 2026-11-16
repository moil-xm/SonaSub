import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vite-plus/test';

import { emptySessionStats } from '@/lib/session-cache';
import { sessionSnapshotOptions } from '@/lib/session';
import type { SessionSnapshot } from '@/lib/session.type';

import { LiveSubtitles } from './live-subtitles';

const snapshot: SessionSnapshot = {
  status: {
    state: 'running',
    info: {
      sessionId: 'session',
      startedAtMs: 1,
      audioBackend: 'Core Audio process tap',
      audioDevice: null,
      modelsDir: '/models',
      asr: {
        modelId: 'streaming',
        modelLabel: 'Streaming Zipformer',
        mode: 'streaming',
        sourceLanguage: 'auto',
        maxSegmentSecs: 12,
      },
      translation: {
        engine: 'bergamot',
        modelId: 'en-zh',
        modelLabel: 'English to Chinese',
        mode: 'text',
        targetLanguage: 'zh-Hans',
        trigger: 'finalOnly',
        streaming: false,
      },
      speakers: {
        enabled: true,
        diarization: true,
        embeddingModel: 'campplus',
        diarizationWindowSecs: 20,
      },
      historyLimit: 400,
      runBoundary: 'Shared process',
    },
  },
  segments: [
    {
      id: 'session:1',
      sessionId: 'session',
      startSecs: 1.2,
      endSecs: 2.8,
      text: 'Hello from SonaSub.',
      revision: 2,
      isFinal: true,
      speaker: {
        id: 'S1',
        index: 1,
        confidence: 0.82,
        uncertain: false,
        overlap: false,
        source: 'embedding',
      },
      translation: {
        targetLanguage: 'zh-Hans',
        text: '你好，来自 SonaSub。',
        status: 'ready',
        revision: 2,
        engine: 'bergamot',
        mode: 'text',
        modelId: 'en-zh',
        error: null,
        updatedAtMs: 2,
      },
      sourceLanguage: 'en',
      createdAtMs: 1,
      updatedAtMs: 2,
    },
  ],
  stats: emptySessionStats(),
  notices: [],
};

afterEach(() => {
  cleanup();
});

describe('LiveSubtitles', () => {
  it('renders source text, translation and speaker from one subtitle id', () => {
    const i18n = createI18n('en-US');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { staleTime: Infinity, retry: false } },
    });
    queryClient.setQueryData(sessionSnapshotOptions.queryKey, snapshot);

    render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={queryClient}>
          <LiveSubtitles />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    expect(screen.getByText('Hello from SonaSub.')).toBeTruthy();
    expect(screen.getByText('你好，来自 SonaSub。')).toBeTruthy();
    expect(screen.getByText('Speaker 1')).toBeTruthy();
    expect(screen.getByText('1 caption')).toBeTruthy();
  });
});
