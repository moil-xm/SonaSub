import type { AppConfig, Capabilities } from '../session';

import type { ModelReadiness, ReadinessIssue, TranslationReadiness } from './model-readiness.type';

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
  const translation = assessTranslationReadiness(capabilities, config);
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
  issues.push(...translation.issues);
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
    selectedTranslationModels: translation.selectedTranslationModels,
  };
}

export function assessTranslationReadiness(
  capabilities: Capabilities,
  config: AppConfig,
): TranslationReadiness {
  const selectedEngine =
    capabilities.translationEngines.find((engine) => engine.id === config.translation.engine) ??
    null;
  const selectedTranslationModels = capabilities.translationModels.filter(
    (model) =>
      model.engine === config.translation.engine &&
      model.targetLanguage === config.translation.targetLanguage,
  );
  const issues: ReadinessIssue[] = [];

  if (!selectedEngine) issues.push('translationEngine');
  if (selectedEngine && !selectedEngine.targetLanguages.includes(config.translation.targetLanguage))
    issues.push('translationTarget');
  if (selectedEngine?.requiresLocalModel) {
    if (!selectedTranslationModels.some((model) => model.available))
      issues.push('translationModel');
  }
  if (selectedEngine && !selectedEngine.requiresLocalModel) {
    const baseUrl = config.translation.openaiCompatible.baseUrl.trim();
    const model = config.translation.openaiCompatible.model.trim();
    if (!isHttpUrl(baseUrl) || !model) issues.push('translationConfig');
  }

  return {
    issues,
    selectedEngine,
    selectedTranslationModels,
  };
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}
