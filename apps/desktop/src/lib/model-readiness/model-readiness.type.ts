import type { AsrModelCapability, TranslationModelCapability } from '../session';

export type ReadinessIssue =
  | 'unsupportedPlatform'
  | 'unknownAsrModel'
  | 'asrModel'
  | 'asrMode'
  | 'asrLanguage'
  | 'vadModel'
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
