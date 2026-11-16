export type AsrMode = 'auto' | 'streaming' | 'segmented';

export type TranslationMode = 'auto' | 'streaming' | 'text';

export type TranslationTrigger = 'finalOnly' | 'stableSegment';

export type SessionAction = 'start' | 'stop' | 'restart';

export type SessionStatus =
  | { state: 'idle' }
  | { state: 'starting'; stage: string }
  | { state: 'running'; info: SessionInfo }
  | { state: 'stopping'; stage: string }
  | { state: 'failed'; error: string };

export type AsrInfo = {
  modelId: string;
  modelLabel: string;
  mode: 'streaming' | 'segmented';
  sourceLanguage: string;
  maxSegmentSecs: number;
};

export type TranslationInfo = {
  engine: string;
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
  audioDevice: string | null;
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
};

export type TranslationModelCapability = ModelStatus & {
  sourceLanguage: string;
  targetLanguage: string;
  engine: string;
  supportedModes: string[];
};

export type ModeCapability = {
  mode: string;
  available: boolean;
  reason: string | null;
};

export type LanguageOption = {
  code: string;
  label: string;
};

export type AudioOutput = {
  id: string;
  name: string;
  isDefault: boolean;
};

export type AudioCapabilities = {
  backend: string;
  systemCapture: boolean;
  supportedPlatform: string;
  permission: string;
  sampleRate: number;
  channels: number;
  defaultDevice: string | null;
  devices: AudioOutput[];
};

export type Capabilities = {
  modelsDir: string;
  platform: string;
  audio: AudioCapabilities;
  asrModels: AsrModelCapability[];
  translationModels: TranslationModelCapability[];
  translationTargets: LanguageOption[];
  translationEngine: string;
  translationModes: ModeCapability[];
  diarization: ModelStatus;
  speakerEmbedding: ModelStatus;
  vad: ModelStatus;
};

export type AudioConfig = {
  deviceId: string | null;
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
  engine: string;
  mode: TranslationMode;
  trigger: TranslationTrigger;
  targetLanguage: string;
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
  engine: string;
  mode: 'text';
  modelId: string;
  error: string | null;
  updatedAtMs: number;
};

export type SubtitleSegment = {
  id: string;
  sessionId: string;
  startSecs: number;
  endSecs: number | null;
  text: string;
  revision: number;
  isFinal: boolean;
  speaker: SpeakerInfo | null;
  translation: TranslationState | null;
  sourceLanguage: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

export type SubtitlePatch =
  | { type: 'upsert'; segment: SubtitleSegment }
  | { type: 'finalize'; segment: SubtitleSegment }
  | { type: 'speaker'; id: string; speaker: SpeakerInfo | null }
  | { type: 'translation'; id: string; translation: TranslationState | null };

export type SubtitleBatch = {
  sessionId: string;
  patches: SubtitlePatch[];
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
  status: SessionStatus;
  segments: SubtitleSegment[];
  stats: SessionStats;
  notices: SessionNotice[];
};
