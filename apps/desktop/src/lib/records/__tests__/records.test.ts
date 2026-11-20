import { describe, expect, it } from 'vite-plus/test';

import type { AppConfig, SubtitleSegment } from '@/lib/session';

import { applyRecordSubtitleBatch, sortRecordSegments } from '../records';
import type { RecordSnapshot, RecordStats } from '../records.type';

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

function config(): AppConfig {
  return {
    version: 1,
    audio: {
      live: {
        kind: 'systemOutput',
        deviceId: null,
      },
      record: [
        { kind: 'systemOutput', id: 'device-a' },
        { kind: 'systemOutput', id: 'device-b' },
      ],
    },
    recognition: {
      mode: 'auto',
      modelId: 'asr',
      sourceLanguage: 'auto',
      maxSegmentSecs: 12,
      stableWaitMs: 1_000,
      vadThreshold: 0.5,
      minSilenceSecs: 0.4,
      minSpeechSecs: 0.2,
    },
    translation: {
      engine: 'bergamot',
      modelId: 'model',
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
      similarityThreshold: 0.7,
    },
    ui: {
      historyLimit: 400,
    },
  };
}

function segment(
  sourceRunId: string,
  localId: number,
  occurrenceMs: number,
  overrides: Partial<SubtitleSegment> = {},
): SubtitleSegment {
  return {
    id: `${sourceRunId}:${localId}`,
    sessionId: sourceRunId,
    recordId: 'record',
    sourceRunId,
    sourceKind: 'systemOutput',
    deviceId: sourceRunId,
    deviceName: sourceRunId,
    localId,
    startSecs: occurrenceMs / 1_000,
    endSecs: occurrenceMs / 1_000 + 1,
    deviceTimeSecs: occurrenceMs / 1_000,
    occurrenceMs,
    text: '',
    revision: 1,
    isFinal: true,
    speaker: null,
    speakerRevision: 0,
    translation: null,
    sourceLanguage: 'en',
    createdAtMs: occurrenceMs,
    updatedAtMs: occurrenceMs,
    ...overrides,
  };
}

function snapshot(overrides: Partial<RecordSnapshot> = {}): RecordSnapshot {
  return {
    summary: {
      id: 'record',
      name: 'Record',
      state: 'running',
      devices: [
        { kind: 'systemOutput', id: 'device-a' },
        { kind: 'systemOutput', id: 'device-b' },
      ],
      activeDevices: [
        { kind: 'systemOutput', id: 'device-a' },
        { kind: 'systemOutput', id: 'device-b' },
      ],
      hasActiveSource: true,
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
    ...overrides,
  };
}

describe('record subtitle cache', () => {
  it('keeps identical local ids from different source runs', () => {
    const withA = applyRecordSubtitleBatch(snapshot(), {
      recordId: 'record',
      sourceRunId: 'run-a',
      revision: 1,
      patches: [
        {
          type: 'finalize',
          segment: segment('run-a', 1, 1_000, { text: 'from A' }),
        },
      ],
    });
    const withB = applyRecordSubtitleBatch(withA, {
      recordId: 'record',
      sourceRunId: 'run-b',
      revision: 2,
      patches: [
        {
          type: 'finalize',
          segment: segment('run-b', 1, 2_000, { text: 'from B' }),
        },
      ],
    });

    expect(withB.segments.map((item) => item.id)).toEqual(['run-a:1', 'run-b:1']);
    expect(withB.totalSubtitleCount).toBe(2);
  });

  it('inserts a late result at its original occurrence position', () => {
    const withLater = applyRecordSubtitleBatch(snapshot(), {
      recordId: 'record',
      sourceRunId: 'run-a',
      revision: 1,
      patches: [
        {
          type: 'finalize',
          segment: segment('run-a', 1, 3_000, { text: 'later speech' }),
        },
      ],
    });
    const withEarlier = applyRecordSubtitleBatch(withLater, {
      recordId: 'record',
      sourceRunId: 'run-b',
      revision: 2,
      patches: [
        {
          type: 'finalize',
          segment: segment('run-b', 1, 1_000, { text: 'earlier speech' }),
        },
      ],
    });

    expect(withEarlier.segments.map((item) => item.text)).toEqual([
      'earlier speech',
      'later speech',
    ]);
  });

  it('does not let an older text revision replace newer text or speaker data', () => {
    const current = segment('run-a', 1, 1_000, {
      text: 'newer text',
      revision: 3,
      speaker: {
        id: 'S1',
        index: 1,
        confidence: 0.9,
        uncertain: false,
        overlap: false,
        source: 'embedding',
      },
      speakerRevision: 2,
    });
    const initial = snapshot({
      segments: [current],
      subtitleRevision: 1,
      totalSubtitleCount: 1,
    });
    const result = applyRecordSubtitleBatch(initial, {
      recordId: 'record',
      sourceRunId: 'run-a',
      revision: 2,
      patches: [
        {
          type: 'finalize',
          segment: segment('run-a', 1, 1_000, {
            text: 'older text',
            revision: 2,
            speaker: null,
            speakerRevision: 1,
          }),
        },
      ],
    });

    expect(result.segments[0]?.text).toBe('newer text');
    expect(result.segments[0]?.speaker?.id).toBe('S1');
  });

  it('rejects batches for another record or an older revision', () => {
    const initial = snapshot({ subtitleRevision: 4 });
    const otherRecord = applyRecordSubtitleBatch(initial, {
      recordId: 'other',
      sourceRunId: 'run-a',
      revision: 5,
      patches: [],
    });
    const olderRevision = applyRecordSubtitleBatch(initial, {
      recordId: 'record',
      sourceRunId: 'run-a',
      revision: 4,
      patches: [],
    });

    expect(otherRecord).toBe(initial);
    expect(olderRevision).toBe(initial);
  });

  it('sorts equal occurrence times by the stable segment id', () => {
    const result = sortRecordSegments([segment('run-b', 1, 1_000), segment('run-a', 1, 1_000)]);

    expect(result.map((item) => item.id)).toEqual(['run-a:1', 'run-b:1']);
  });
});
