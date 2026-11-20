import { describe, expect, it } from 'vite-plus/test';

import {
  applySessionStatus,
  applySubtitleBatch,
  applySubtitlePatches,
  emptySessionStats,
  mergeSnapshot,
} from '../session-cache';
import type {
  SessionSnapshot,
  SubtitlePatch,
  SubtitleSegment,
  TranslationState,
} from '../../session';

function segment(overrides: Partial<SubtitleSegment> = {}): SubtitleSegment {
  return {
    id: 'session:1',
    sessionId: 'session',
    recordId: 'session',
    sourceRunId: 'run:session',
    sourceKind: 'systemOutput',
    deviceId: 'default',
    deviceName: 'Default output',
    localId: 1,
    startSecs: 1,
    endSecs: 2,
    deviceTimeSecs: 1,
    occurrenceMs: 1,
    text: 'hello',
    revision: 1,
    isFinal: false,
    speaker: null,
    speakerRevision: 0,
    translation: null,
    sourceLanguage: null,
    createdAtMs: 1,
    updatedAtMs: 1,
    ...overrides,
  };
}

function snapshot(overrides: Partial<SessionSnapshot> = {}): SessionSnapshot {
  return {
    revision: 10,
    sessionId: 'session',
    status: {
      state: 'running',
      info: {
        sessionId: 'session',
        startedAtMs: 1,
        audioBackend: 'CPAL / Core Audio',
        audio: {
          sourceKind: 'systemOutput',
          deviceId: 'speaker',
          deviceName: 'Built-in Output',
          sampleRate: 48_000,
          channels: 2,
          sampleFormat: 'f32',
        },
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
          runtime: 'fxtranslate',
          modelFamily: 'bergamot-firefox',
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
    statusRevision: 10,
    subtitleRevision: 10,
    statsRevision: 10,
    segments: [],
    stats: emptySessionStats(),
    notices: [],
    historyLimit: 400,
    ...overrides,
  };
}

function translation(revision: number, text: string): TranslationState {
  return {
    targetLanguage: 'zh-Hans',
    text,
    status: 'ready',
    revision,
    engine: 'bergamot',
    mode: 'text',
    modelId: 'en-zh',
    error: null,
    updatedAtMs: 2,
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
    const result = applySubtitlePatches(
      [segment()],
      [
        {
          type: 'speaker',
          id: 'session:1',
          revision: 1,
          speaker: {
            id: 'S1',
            index: 1,
            confidence: 0.8,
            uncertain: false,
            overlap: false,
            source: 'diarization',
          },
        },
        { type: 'translation', id: 'session:1', translation: translation(1, '你好') },
      ],
    );

    expect(result[0]?.speaker?.id).toBe('S1');
    expect(result[0]?.translation?.text).toBe('你好');
    expect(result[0]?.text).toBe('hello');
  });

  it('ignores a batch from an older session while another session is running', () => {
    const current = snapshot();
    const next = applySubtitleBatch(current, {
      revision: 20,
      sessionId: 'older-session',
      patches: [{ type: 'upsert', segment: segment() }],
    });

    expect(next).toBe(current);
  });

  it('ignores a batch that arrives after a newer one', () => {
    const current = snapshot({ subtitleRevision: 30 });
    const next = applySubtitleBatch(current, {
      revision: 20,
      sessionId: 'session',
      patches: [{ type: 'upsert', segment: segment({ text: 'late' }) }],
    });

    expect(next).toBe(current);
  });

  it('keeps the subtitle list within the configured history limit', () => {
    let current = snapshot({ historyLimit: 400 });
    for (let index = 0; index < 1_000; index += 1) {
      current = applySubtitleBatch(current, {
        revision: 100 + index,
        sessionId: 'session',
        patches: [
          {
            type: 'upsert',
            segment: segment({ id: `session:${index}`, text: `line ${index}` }),
          },
        ],
      });
    }

    expect(current.segments).toHaveLength(400);
    expect(current.segments[0]?.id).toBe('session:600');
    expect(current.segments.at(-1)?.id).toBe('session:999');
  });

  it('rejects an older text revision after a newer subtitle was applied', () => {
    const result = applySubtitlePatches(
      [segment({ text: 'newer', revision: 3 })],
      [{ type: 'upsert', segment: segment({ text: 'older', revision: 2 }) }],
    );

    expect(result[0]?.text).toBe('newer');
    expect(result[0]?.revision).toBe(3);
  });

  it('rejects a translation that belongs to an older text revision', () => {
    const result = applySubtitlePatches(
      [segment({ revision: 3 })],
      [{ type: 'translation', id: 'session:1', translation: translation(2, '旧译文') }],
    );

    expect(result[0]?.translation).toBeNull();
  });

  it('rejects a speaker patch that is older than the applied one', () => {
    const result = applySubtitlePatches(
      [segment({ speakerRevision: 2 })],
      [
        {
          type: 'speaker',
          id: 'session:1',
          revision: 1,
          speaker: {
            id: 'S9',
            index: 9,
            confidence: 0.1,
            uncertain: false,
            overlap: false,
            source: 'embedding',
          },
        },
      ],
    );

    expect(result[0]?.speaker).toBeNull();
    expect(result[0]?.speakerRevision).toBe(2);
  });

  it('clears stale captions when a new session starts', () => {
    const current = {
      ...snapshot(),
      segments: [segment()],
      stats: { ...emptySessionStats(), startedAtMs: 10 },
    };
    const next = applySessionStatus(current, {
      revision: 40,
      status: {
        state: 'starting',
        sessionId: 'next-session',
        stage: 'Loading speech recognition model',
      },
    });

    expect(next.segments).toHaveLength(0);
    expect(next.stats.startedAtMs).toBe(0);
    expect(next.status.state).toBe('starting');
    expect(next.sessionId).toBe('next-session');
  });

  it('ignores a subtitle of the previous session while a new one starts', () => {
    const current = applySessionStatus(snapshot(), {
      revision: 40,
      status: {
        state: 'starting',
        sessionId: 'next-session',
        stage: 'Loading speech recognition model',
      },
    });
    const next = applySubtitleBatch(current, {
      revision: 41,
      sessionId: 'session',
      patches: [{ type: 'upsert', segment: segment({ text: 'from the old session' }) }],
    });

    expect(next).toBe(current);
    expect(next.segments).toHaveLength(0);
  });

  it('ignores a status event that arrives after a newer one', () => {
    const current = snapshot({ statusRevision: 50 });
    const next = applySessionStatus(current, {
      revision: 45,
      status: { state: 'idle' },
    });

    expect(next).toBe(current);
  });

  it('keeps the newer channels when a late snapshot arrives', () => {
    const current = snapshot({
      revision: 60,
      statusRevision: 60,
      subtitleRevision: 60,
      statsRevision: 60,
      segments: [segment({ text: 'newer' })],
    });
    const late = snapshot({
      revision: 55,
      statusRevision: 55,
      subtitleRevision: 55,
      statsRevision: 55,
      segments: [segment({ text: 'older' })],
    });

    const merged = mergeSnapshot(current, late);

    expect(merged.segments[0]?.text).toBe('newer');
    expect(merged.revision).toBe(60);
    expect(merged.subtitleRevision).toBe(60);
  });

  it('takes a newer snapshot over the cached state', () => {
    const current = snapshot({ revision: 55, subtitleRevision: 55 });
    const newer = snapshot({
      revision: 70,
      subtitleRevision: 70,
      segments: [segment({ text: 'newer' })],
    });

    const merged = mergeSnapshot(current, newer);

    expect(merged.segments[0]?.text).toBe('newer');
    expect(merged.subtitleRevision).toBe(70);
  });
});
