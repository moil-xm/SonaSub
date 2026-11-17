import type {
  SessionNotice,
  SessionSnapshot,
  SessionStats,
  StatsEvent,
  StatusEvent,
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

/**
 * Narrows an untyped IPC payload to a session snapshot.
 *
 * The snapshot crosses the Tauri boundary as an untyped value, so the
 * structural sharing callback needs a runtime guard before it can read the
 * revision fields.
 */
export function isSessionSnapshot(value: unknown): value is SessionSnapshot {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<SessionSnapshot>;
  return (
    typeof candidate.revision === 'number' &&
    typeof candidate.statusRevision === 'number' &&
    typeof candidate.subtitleRevision === 'number' &&
    typeof candidate.statsRevision === 'number' &&
    typeof candidate.historyLimit === 'number' &&
    Array.isArray(candidate.segments) &&
    Array.isArray(candidate.notices)
  );
}

function upsertSegment(segments: SubtitleSegment[], incoming: SubtitleSegment): SubtitleSegment[] {
  const index = segments.findIndex((segment) => segment.id === incoming.id);
  if (index === -1) return [...segments, incoming];
  const existing = segments[index];
  if (!existing) return segments;
  // A patch that carries an older text revision must not replace a newer one.
  if (incoming.revision < existing.revision) return segments;
  const merged =
    incoming.speakerRevision < existing.speakerRevision
      ? {
          ...incoming,
          speaker: existing.speaker,
          speakerRevision: existing.speakerRevision,
        }
      : incoming;
  return segments.map((segment, current) => {
    if (current === index) return merged;
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
        if (patch.revision <= segment.speakerRevision) return segment;
        return { ...segment, speaker: patch.speaker, speakerRevision: patch.revision };
      }
      // A translation belongs to the text revision it was produced from.
      if (patch.translation && patch.translation.revision !== segment.revision) return segment;
      return { ...segment, translation: patch.translation };
    });
  }
  return next;
}

export function activeSessionId(snapshot: SessionSnapshot): string | null {
  switch (snapshot.status.state) {
    case 'running':
      return snapshot.status.info.sessionId;
    case 'starting':
    case 'stopping':
      return snapshot.status.sessionId;
    default:
      return snapshot.sessionId;
  }
}

export function applySubtitleBatch(
  snapshot: SessionSnapshot,
  batch: SubtitleBatch,
): SessionSnapshot {
  // A batch that was already applied, or that arrives after a newer one, is
  // dropped instead of replaying an older subtitle state.
  if (batch.revision <= snapshot.subtitleRevision) return snapshot;
  const sessionId = activeSessionId(snapshot);
  if (sessionId !== null && sessionId !== batch.sessionId) return snapshot;
  return {
    ...snapshot,
    subtitleRevision: batch.revision,
    segments: trimSegments(
      applySubtitlePatches(snapshot.segments, batch.patches),
      snapshot.historyLimit,
    ),
  };
}

export function applySessionStatus(snapshot: SessionSnapshot, event: StatusEvent): SessionSnapshot {
  if (event.revision <= snapshot.statusRevision) return snapshot;
  const status = event.status;
  if (status.state === 'starting' && status.sessionId !== snapshot.sessionId) {
    // A new session starts with an empty subtitle history.
    return {
      ...snapshot,
      status,
      statusRevision: event.revision,
      sessionId: status.sessionId,
      segments: [],
      stats: emptySessionStats(),
    };
  }
  let sessionId = snapshot.sessionId;
  if (status.state === 'running') sessionId = status.info.sessionId;
  if (status.state === 'starting' || status.state === 'stopping') sessionId = status.sessionId;
  return {
    ...snapshot,
    status,
    statusRevision: event.revision,
    sessionId,
  };
}

export function applySessionStats(snapshot: SessionSnapshot, event: StatsEvent): SessionSnapshot {
  if (event.revision <= snapshot.statsRevision) return snapshot;
  return { ...snapshot, stats: event.stats, statsRevision: event.revision };
}

export function appendSessionNotice(
  snapshot: SessionSnapshot,
  notice: SessionNotice,
): SessionSnapshot {
  const notices = [...snapshot.notices, notice].slice(-NOTICE_LIMIT);
  return { ...snapshot, notices };
}

/**
 * Keeps the subtitle list within the configured history limit.
 *
 * The backend applies the same limit to its own store, so the cache and the
 * document never grow without bound.
 */
export function trimSegments(segments: SubtitleSegment[], historyLimit: number): SubtitleSegment[] {
  if (historyLimit <= 0 || segments.length <= historyLimit) return segments;
  return segments.slice(segments.length - historyLimit);
}

/**
 * Merges a snapshot that arrived from the backend into the cached state.
 *
 * Each channel carries its own revision, so a snapshot that was taken before
 * a newer event does not roll that channel back.
 */
export function mergeSnapshot(
  current: SessionSnapshot | undefined,
  incoming: SessionSnapshot,
): SessionSnapshot {
  if (!current) return incoming;
  const status =
    incoming.statusRevision >= current.statusRevision ? incoming.status : current.status;
  const segments =
    incoming.subtitleRevision >= current.subtitleRevision ? incoming.segments : current.segments;
  const stats = incoming.statsRevision >= current.statsRevision ? incoming.stats : current.stats;
  const notices = incoming.revision >= current.revision ? incoming.notices : current.notices;
  return {
    ...incoming,
    status,
    segments,
    stats,
    notices,
    sessionId: incoming.sessionId ?? current.sessionId,
    revision: Math.max(incoming.revision, current.revision),
    statusRevision: Math.max(incoming.statusRevision, current.statusRevision),
    subtitleRevision: Math.max(incoming.subtitleRevision, current.subtitleRevision),
    statsRevision: Math.max(incoming.statsRevision, current.statsRevision),
    historyLimit: incoming.historyLimit,
  };
}
