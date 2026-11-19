import type { AppConfig, Capabilities } from '../session';

import type { ModelReadiness, ReadinessIssue } from './model-readiness.type';

export function assessModelReadiness(
  capabilities: Capabilities,
  config: AppConfig,
): ModelReadiness {
  const selectedAsr =
    capabilities.asrModels.find((model) => model.id === config.recognition.modelId) ?? null;
  let selectedMode: 'streaming' | 'segmented' | null = null;
  if (selectedAsr) {
    if (config.recognition.mode === 'auto') selectedMode = selectedAsr.kind;
    else selectedMode = config.recognition.mode;
  }
  const selectedTranslationModels = capabilities.translationModels.filter(
    (model) => model.targetLanguage === config.translation.targetLanguage,
  );
  const issues: ReadinessIssue[] = [];

  if (!capabilities.audio.systemCapture) issues.push('unsupportedPlatform');
  if (!selectedAsr) issues.push('unknownAsrModel');
  else {
    if (!selectedAsr.available) issues.push('asrModel');
    if (selectedMode !== selectedAsr.kind) issues.push('asrMode');
    if (
      config.recognition.sourceLanguage !== 'auto' &&
      (!selectedAsr.supportsSourceLanguage ||
        !selectedAsr.sourceLanguages.includes(config.recognition.sourceLanguage))
    ) {
      issues.push('asrLanguage');
    }
    if (selectedMode === 'segmented' && !capabilities.vad.available) issues.push('vadModel');
  }
  if (!selectedTranslationModels.some((model) => model.available)) {
    issues.push('translationModel');
  }
  if (config.speakers.enabled && !capabilities.speakerEmbedding.available) {
    issues.push('speakerModel');
  }
  if (
    config.speakers.enabled &&
    config.speakers.diarizationEnabled &&
    !capabilities.diarization.available
  ) {
    issues.push('diarizationModel');
  }

  return {
    canStart: issues.length === 0,
    issues,
    selectedAsr,
    selectedMode,
    selectedTranslationModels,
  };
}
