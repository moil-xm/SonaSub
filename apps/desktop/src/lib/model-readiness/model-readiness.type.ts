import type {
  AsrModelCapability,
  TranslationEngineCapability,
  TranslationModelCapability,
} from '../session';

export type ReadinessIssue =
  | 'unsupportedPlatform'
  | 'unknownAsrModel'
  | 'asrModel'
  | 'asrMode'
  | 'asrLanguage'
  | 'vadModel'
  | 'translationEngine'
  | 'translationTarget'
  | 'translationConfig'
  | 'translationModel'
  | 'speakerModel'
  | 'diarizationModel';

export type ModelReadiness = {
  canStart: boolean;
  issues: ReadinessIssue[];
  selectedAsr: AsrModelCapability | null;
  selectedMode: 'streaming' | 'segmented' | null;
  selectedTranslationModels: TranslationModelCapability[];
};

export type TranslationReadiness = {
  issues: ReadinessIssue[];
  selectedEngine: TranslationEngineCapability | null;
  selectedTranslationModels: TranslationModelCapability[];
};
