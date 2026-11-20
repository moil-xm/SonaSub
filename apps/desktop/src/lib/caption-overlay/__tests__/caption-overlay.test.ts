import { describe, expect, it } from 'vite-plus/test';

import type { RecordSnapshot } from '@/lib/records';
import type { SubtitleSegment } from '@/lib/session';

import { selectOverlaySegment } from '../caption-overlay';

function segment(id: string, occurrenceMs: number): SubtitleSegment {
  return {
    id,
    sessionId: 'session',
    recordId: 'record',
    sourceRunId: 'source',
    sourceKind: 'systemOutput',
    deviceId: 'device',
    deviceName: 'Device',
    localId: 1,
    startSecs: 0,
    endSecs: 1,
    deviceTimeSecs: 0,
    occurrenceMs,
    text: `text-${id}`,
    revision: 1,
    isFinal: true,
    speaker: null,
    speakerRevision: 0,
    translation: null,
    sourceLanguage: null,
    createdAtMs: occurrenceMs,
    updatedAtMs: occurrenceMs,
  };
}

function snapshot(segments: SubtitleSegment[]): RecordSnapshot {
  return {
    summary: {
      id: 'record',
      name: 'Record',
      state: 'running',
      devices: [],
      activeDevices: [],
      hasActiveSource: true,
      lastError: null,
      createdAtMs: 0,
      updatedAtMs: 0,
      revision: 1,
    },
    config: {
      version: 1,
      audio: { live: { kind: 'systemOutput', deviceId: null }, record: [], saveAudio: false },
      recognition: {
        mode: 'auto',
        modelId: 'model',
        sourceLanguage: 'auto',
        maxSegmentSecs: 10,
        stableWaitMs: 500,
        vadThreshold: 0.5,
        minSilenceSecs: 0.3,
        minSpeechSecs: 0.2,
      },
      translation: {
        engine: 'bergamot',
        modelId: 'model',
        mode: 'auto',
        trigger: 'finalOnly',
        targetLanguage: 'zh-Hans',
        openaiCompatible: { baseUrl: '', model: '', apiKey: '' },
      },
      speakers: {
        enabled: false,
        diarizationEnabled: false,
        windowSecs: 20,
        maxSpeakers: 4,
        similarityThreshold: 0.5,
      },
      ui: { historyLimit: 100 },
    },
    sources: [],
    segments,
    stats: {
      startedAtMs: 0,
      activeSources: 1,
      audioFrames: 0,
      droppedFrames: 0,
      partialUpdates: 0,
      finalizedSubtitles: segments.length,
      translationRequests: 0,
      translationFailures: 0,
      speakerUpdates: 0,
      diarizationWindows: 0,
      audioPeak: 0,
      audioRms: 0,
    },
    notices: [],
    summaryRevision: 1,
    subtitleRevision: 1,
    statsRevision: 1,
    totalSubtitleCount: segments.length,
    historyLimit: 100,
  };
}

describe('selectOverlaySegment', () => {
  it('returns null when no subtitle is available', () => {
    expect(selectOverlaySegment(snapshot([]))).toBeNull();
  });

  it('returns the latest segment by occurrence time from an unsorted list', () => {
    const older = segment('a', 1_000);
    const latest = segment('b', 3_000);
    const middle = segment('c', 2_000);

    const selected = selectOverlaySegment(snapshot([latest, older, middle]));

    expect(selected).toBe(latest);
  });
});
