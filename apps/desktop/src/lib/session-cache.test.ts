import { describe, expect, it } from 'vite-plus/test';

import {
  applySessionStatus,
  applySubtitleBatch,
  applySubtitlePatches,
  emptySessionStats,
} from './session-cache';
import type {
  SessionSnapshot,
  SubtitlePatch,
  SubtitleSegment,
  TranslationState,
} from './session.type';

function segment(overrides: Partial<SubtitleSegment> = {}): SubtitleSegment {
  return {
    id: 'session:1',
    sessionId: 'session',
    startSecs: 1,
    endSecs: 2,
    text: 'hello',
    revision: 1,
    isFinal: false,
    speaker: null,
    translation: null,
    sourceLanguage: null,
    createdAtMs: 1,
    updatedAtMs: 1,
    ...overrides,
  };
}

function snapshot(): SessionSnapshot {
  return {
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
          modelLabel: 'Streaming',
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
    segments: [],
    stats: emptySessionStats(),
    notices: [],
  };
}

describe('subtitle cache patches', () => {
  it('updates one stable subtitle by id instead of appending revisions', () => {
    const patches: SubtitlePatch[] = [
      { type: 'upsert', segment: segment() },
      {
        type: 'upsert',
        segment: segment({ text: 'hello world', revision: 2, isFinal: true }),
      },
    ];

    const result = applySubtitlePatches([], patches);

    expect(result).toHaveLength(1);
    expect(result[0]?.text).toBe('hello world');
    expect(result[0]?.revision).toBe(2);
  });

  it('applies speaker and translation patches independently', () => {
    const translation: TranslationState = {
      targetLanguage: 'zh-Hans',
      text: '你好',
      status: 'ready',
      revision: 1,
      engine: 'bergamot',
      mode: 'text',
      modelId: 'en-zh',
      error: null,
      updatedAtMs: 2,
    };
    const result = applySubtitlePatches(
      [segment()],
      [
        {
          type: 'speaker',
          id: 'session:1',
          speaker: {
            id: 'S1',
            index: 1,
            confidence: 0.8,
            uncertain: false,
            overlap: false,
            source: 'diarization',
          },
        },
        { type: 'translation', id: 'session:1', translation },
      ],
    );

    expect(result[0]?.speaker?.id).toBe('S1');
    expect(result[0]?.translation?.text).toBe('你好');
    expect(result[0]?.text).toBe('hello');
  });

  it('ignores a batch from an older session while another session is running', () => {
    const current = snapshot();
    const next = applySubtitleBatch(current, {
      sessionId: 'older-session',
      patches: [{ type: 'upsert', segment: segment() }],
    });

    expect(next).toBe(current);
  });

  it('clears stale captions when a new session starts', () => {
    const current = {
      ...snapshot(),
      segments: [segment()],
      stats: { ...emptySessionStats(), startedAtMs: 10 },
    };
    const next = applySessionStatus(current, {
      state: 'starting',
      stage: 'Loading speech recognition model',
    });

    expect(next.segments).toHaveLength(0);
    expect(next.stats.startedAtMs).toBe(0);
    expect(next.status.state).toBe('starting');
  });
});
