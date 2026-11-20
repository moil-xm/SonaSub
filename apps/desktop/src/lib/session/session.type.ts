export type AsrMode = 'auto' | 'streaming' | 'segmented';

export type TranslationMode = 'auto' | 'streaming' | 'text';

export type TranslationTrigger = 'finalOnly' | 'stableSegment';

export type TranslationEngineId = 'bergamot' | 'openai-compatible';

export type AudioSourceKind = 'systemOutput' | 'microphone';

export type AudioDeviceRef = {
  kind: AudioSourceKind;
  id: string;
};

export type AudioDevice = AudioDeviceRef & {
  name: string;
  isDefault: boolean;
  captureSupported: boolean;
  captureReason: string | null;
  sampleRate: number | null;
  channels: number | null;
  sampleFormat: string | null;
};

export type CaptureInfo = {
  sourceKind: AudioSourceKind;
  deviceId: string;
  deviceName: string;
  sampleRate: number;
  channels: number;
  sampleFormat: string;
};

export type SessionAction = 'start' | 'stop' | 'restart';

export type SessionStatus =
  | { state: 'idle' }
  | { state: 'starting'; sessionId: string; stage: string }
  | { state: 'running'; info: SessionInfo }
  | { state: 'stopping'; sessionId: string; stage: string }
  | { state: 'failed'; error: string };

export type AsrInfo = {
  modelId: string;
  modelLabel: string;
  mode: 'streaming' | 'segmented';
  sourceLanguage: string;
  maxSegmentSecs: number;
};

export type TranslationInfo = {
  engine: TranslationEngineId;
  runtime: string;
  modelFamily: string;
  modelId: string;
  modelLabel: string;
  mode: 'text';
  targetLanguage: string;
  trigger: 'finalOnly' | 'stableSegment';
  streaming: false;
};

export type SpeakerInfoStatus = {
  enabled: boolean;
  diarization: boolean;
  embeddingModel: string;
  diarizationWindowSecs: number;
};

export type SessionInfo = {
  sessionId: string;
  startedAtMs: number;
  audioBackend: string;
  audio: CaptureInfo;
  modelsDir: string;
  asr: AsrInfo;
  translation: TranslationInfo;
  speakers: SpeakerInfoStatus;
  historyLimit: number;
  runBoundary: string;
};

export type ModelFileStatus = {
  path: string;
  present: boolean;
  bytes: number;
  expectedSha256: string | null;
};

export type ModelStatus = {
  id: string;
  label: string;
  available: boolean;
  missingFiles: string[];
  upstream: string;
  version: string;
  files: ModelFileStatus[];
};

export type AsrModelCapability = ModelStatus & {
  kind: 'streaming' | 'segmented';
  supportedModes: string[];
  sourceLanguages: string[];
  /** Whether a forced source language changes the recognition result. */
  supportsSourceLanguage: boolean;
  /** Why a forced source language is not available for this model. */
  sourceLanguageReason: string | null;
};

export type TranslationModelCapability = ModelStatus & {
  sourceLanguage: string;
  targetLanguage: string;
  engine: TranslationEngineId;
  supportedModes: string[];
};

export type TranslationEngineCapability = {
  id: TranslationEngineId;
  label: string;
  runtime: string;
  modelFamily: string;
  requiresLocalModel: boolean;
  targetLanguages: string[];
  modes: ModeCapability[];
};

export type ModeCapability = {
  mode: string;
  available: boolean;
  reason: string | null;
  reasonCode: string | null;
};

export type LanguageOption = {
  code: string;
  label: string;
};

export type AudioCapabilities = {
  backend: string;
  systemCapture: boolean;
  microphoneCapture: boolean;
  supportedPlatform: string;
  permission: string;
  defaultSystemOutput: AudioDeviceRef | null;
  defaultMicrophone: AudioDeviceRef | null;
  devices: AudioDevice[];
};

export type Capabilities = {
  modelsDir: string;
  platform: string;
  audio: AudioCapabilities;
  asrModels: AsrModelCapability[];
  translationModels: TranslationModelCapability[];
  translationTargets: LanguageOption[];
  translationEngines: TranslationEngineCapability[];
  diarization: ModelStatus;
  speakerEmbedding: ModelStatus;
  vad: ModelStatus;
};

export type AudioConfig = {
  live: {
    kind: AudioSourceKind;
    deviceId: string | null;
  };
  record: AudioDeviceRef[];
  saveAudio: boolean;
};

export type RecognitionConfig = {
  mode: AsrMode;
  modelId: string;
  sourceLanguage: string;
  maxSegmentSecs: number;
  stableWaitMs: number;
  vadThreshold: number;
  minSilenceSecs: number;
  minSpeechSecs: number;
};

export type TranslationConfig = {
  engine: TranslationEngineId;
  modelId: string;
  mode: TranslationMode;
  trigger: TranslationTrigger;
  targetLanguage: string;
  openaiCompatible: {
    baseUrl: string;
    model: string;
    apiKey: string;
  };
};

export type SpeakerConfig = {
  enabled: boolean;
  diarizationEnabled: boolean;
  windowSecs: number;
  maxSpeakers: number;
  similarityThreshold: number;
};

export type UiConfig = {
  historyLimit: number;
};

export type AppConfig = {
  version: number;
  audio: AudioConfig;
  recognition: RecognitionConfig;
  translation: TranslationConfig;
  speakers: SpeakerConfig;
  ui: UiConfig;
};

export type ConfigView = {
  config: AppConfig;
  revision: number;
  loadError: string | null;
};

export type SpeakerInfo = {
  id: string;
  index: number;
  confidence: number | null;
  uncertain: boolean;
  overlap: boolean;
  source: 'embedding' | 'diarization';
};

export type TranslationStatus =
  | 'pending'
  | 'ready'
  | 'error'
  | 'stale'
  | 'notNeeded'
  | 'unavailable';

export type TranslationState = {
  targetLanguage: string;
  text: string | null;
  status: TranslationStatus;
  revision: number;
  engine: TranslationEngineId;
  mode: 'text';
  modelId: string;
  error: string | null;
  updatedAtMs: number;
};

export type SubtitleSegment = {
  id: string;
  sessionId: string;
  recordId: string;
  sourceRunId: string;
  sourceKind: AudioSourceKind;
  deviceId: string;
  deviceName: string;
  localId: number;
  startSecs: number;
  endSecs: number | null;
  deviceTimeSecs: number;
  occurrenceMs: number;
  text: string;
  revision: number;
  isFinal: boolean;
  speaker: SpeakerInfo | null;
  /** Increments every time the speaker of this segment is assigned. */
  speakerRevision: number;
  translation: TranslationState | null;
  sourceLanguage: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

export type SubtitlePatch =
  | { type: 'upsert'; segment: SubtitleSegment }
  | { type: 'finalize'; segment: SubtitleSegment }
  | { type: 'speaker'; id: string; speaker: SpeakerInfo | null; revision: number }
  | { type: 'translation'; id: string; translation: TranslationState | null };

export type SubtitleBatch = {
  revision: number;
  sessionId: string;
  patches: SubtitlePatch[];
};

/** A status change published by the backend. */
export type StatusEvent = {
  revision: number;
  status: SessionStatus;
};

/** A statistics update published by the backend. */
export type StatsEvent = {
  revision: number;
  stats: SessionStats;
};

export type SessionStats = {
  startedAtMs: number;
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
  lastSubtitleLatencyMs: number;
  averageSubtitleLatencyMs: number;
  lastTranslationLatencyMs: number;
  averageTranslationLatencyMs: number;
  lastSpeakerLatencyMs: number;
  lastDiarizationLatencyMs: number;
  cpuPercent: number;
  rssMb: number;
};

export type SessionNotice = {
  level: 'info' | 'warning' | 'error';
  message: string;
  atMs: number;
};

export type SessionSnapshot = {
  /** Monotonic counter of every backend state change. */
  revision: number;
  /** The session the current state belongs to. */
  sessionId: string | null;
  status: SessionStatus;
  /** Revision of the status this snapshot carries. */
  statusRevision: number;
  /** Revision of the subtitle list this snapshot carries. */
  subtitleRevision: number;
  /** Revision of the statistics this snapshot carries. */
  statsRevision: number;
  segments: SubtitleSegment[];
  stats: SessionStats;
  notices: SessionNotice[];
  historyLimit: number;
};
