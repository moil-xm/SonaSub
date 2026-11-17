import type {
  SessionNotice,
  SessionSnapshot,
  SessionStats,
  SessionStatus,
  SubtitleBatch,
  SubtitlePatch,
  SubtitleSegment,
} from '../session';

const NOTICE_LIMIT = 100;

export function emptySessionStats(): SessionStats {
  return {
    startedAtMs: 0,
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
    lastSubtitleLatencyMs: 0,
    averageSubtitleLatencyMs: 0,
    lastTranslationLatencyMs: 0,
    averageTranslationLatencyMs: 0,
    lastSpeakerLatencyMs: 0,
    lastDiarizationLatencyMs: 0,
    cpuPercent: 0,
    rssMb: 0,
  };
}

function upsertSegment(segments: SubtitleSegment[], incoming: SubtitleSegment): SubtitleSegment[] {
  const index = segments.findIndex((segment) => segment.id === incoming.id);
  if (index === -1) return [...segments, incoming];
  return segments.map((segment, current) => {
    if (current === index) return incoming;
    return segment;
  });
}

export function applySubtitlePatches(
  segments: SubtitleSegment[],
  patches: SubtitlePatch[],
): SubtitleSegment[] {
  let next = segments;
  for (const patch of patches) {
    if (patch.type === 'upsert' || patch.type === 'finalize') {
      next = upsertSegment(next, patch.segment);
      continue;
    }
    next = next.map((segment) => {
      if (segment.id !== patch.id) return segment;
      if (patch.type === 'speaker') {
        return { ...segment, speaker: patch.speaker };
      }
      return { ...segment, translation: patch.translation };
    });
  }
  return next;
}

export function activeSessionId(snapshot: SessionSnapshot): string | null {
  if (snapshot.status.state === 'running') return snapshot.status.info.sessionId;
  return null;
}

export function applySubtitleBatch(
  snapshot: SessionSnapshot,
  batch: SubtitleBatch,
): SessionSnapshot {
  const sessionId = activeSessionId(snapshot);
  if (sessionId !== null && sessionId !== batch.sessionId) return snapshot;
  return {
    ...snapshot,
    segments: applySubtitlePatches(snapshot.segments, batch.patches),
  };
}

export function applySessionStatus(
  snapshot: SessionSnapshot,
  status: SessionStatus,
): SessionSnapshot {
  if (status.state === 'starting' && snapshot.status.state !== 'starting') {
    return {
      ...snapshot,
      status,
      segments: [],
      stats: emptySessionStats(),
    };
  }
  return { ...snapshot, status };
}

export function applySessionStats(snapshot: SessionSnapshot, stats: SessionStats): SessionSnapshot {
  return { ...snapshot, stats };
}

export function appendSessionNotice(
  snapshot: SessionSnapshot,
  notice: SessionNotice,
): SessionSnapshot {
  const notices = [...snapshot.notices, notice].slice(-NOTICE_LIMIT);
  return { ...snapshot, notices };
}
