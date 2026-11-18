import type { AppConfig, SubtitlePatch, SubtitleSegment } from '@/lib/session';

export type RecordState =
  | 'notStarted'
  | 'starting'
  | 'running'
  | 'paused'
  | 'waitingForDevice'
  | 'failed'
  | 'ended';

export type SourceRunState = 'starting' | 'running' | 'disconnected' | 'failed' | 'ended';

export type RecordSummary = {
  id: string;
  name: string;
  state: RecordState;
  deviceIds: string[];
  activeDeviceIds: string[];
  hasActiveSource: boolean;
  lastError: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  revision: number;
};

export type RecordSource = {
  sourceRunId: string;
  recordId: string;
  deviceId: string;
  deviceName: string;
  state: SourceRunState;
  elapsedBaseMs: number;
  elapsedMs: number;
  startedAtMs: number;
  endedAtMs: number | null;
  error: string | null;
};

export type RecordStats = {
  startedAtMs: number;
  activeSources: number;
  audioFrames: number;
  droppedFrames: number;
  partialUpdates: number;
  finalizedSubtitles: number;
  translationRequests: number;
  translationFailures: number;
  speakerUpdates: number;
  diarizationWindows: number;
  audioPeak: number;
  audioRms: number;
};

export type RecordNotice = {
  level: 'info' | 'warning' | 'error';
  message: string;
  atMs: number;
};

export type RecordSnapshot = {
  summary: RecordSummary;
  config: AppConfig;
  sources: RecordSource[];
  segments: SubtitleSegment[];
  stats: RecordStats;
  notices: RecordNotice[];
  summaryRevision: number;
  subtitleRevision: number;
  statsRevision: number;
  totalSubtitleCount: number;
  historyLimit: number;
};

export type RecordInput = {
  name: string;
  config: AppConfig;
};

export type RecordSummaryEvent = {
  recordId: string;
  revision: number;
  summary: RecordSummary;
};

export type RecordSubtitleBatch = {
  recordId: string;
  sourceRunId: string;
  revision: number;
  patches: SubtitlePatch[];
};

export type RecordStatsEvent = {
  recordId: string;
  revision: number;
  stats: RecordStats;
};

export type RecordNoticeEvent = {
  recordId: string;
  notice: RecordNotice;
};

export type RecordDevice = {
  id: string;
  name: string;
  isDefault: boolean;
  isAvailable: boolean;
  occupiedRecordId: string | null;
  occupiedRecordName: string | null;
};

export type SubtitleCursor = {
  occurrenceMs: number;
  id: string;
};

export type RecordSubtitlePage = {
  items: SubtitleSegment[];
  hasMore: boolean;
  nextBefore: SubtitleCursor | null;
};

export type RecordAction = 'start' | 'pause' | 'resume' | 'stop';
