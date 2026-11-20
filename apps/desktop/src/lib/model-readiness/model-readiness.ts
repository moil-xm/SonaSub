import { audioDeviceKey } from '../session';
import type { AppConfig, AudioDeviceRef, Capabilities } from '../session';

import type { ModelReadiness, ReadinessIssue, TranslationReadiness } from './model-readiness.type';

export function assessModelReadiness(
  capabilities: Capabilities,
  config: AppConfig,
  providers: { name: string; baseUrl: string; apiKey: string; models: string[] }[] = [],
): ModelReadiness {
  const selectedAsr =
    capabilities.asrModels.find((model) => model.id === config.recognition.modelId) ?? null;
  let selectedMode: 'streaming' | 'segmented' | null = null;
  if (selectedAsr) {
    if (config.recognition.mode === 'auto') selectedMode = selectedAsr.kind;
    else selectedMode = config.recognition.mode;
  }
  const translation = assessTranslationReadiness(capabilities, config, providers);
  const issues: ReadinessIssue[] = [];

  const sourceKind = config.audio.live.kind;
  const sourceCapturable =
    sourceKind === 'systemOutput'
      ? capabilities.audio.systemCapture
      : capabilities.audio.microphoneCapture;
  if (!sourceCapturable) issues.push('unsupportedPlatform');
  let selectedDeviceRef: AudioDeviceRef | null = null;
  if (config.audio.live.deviceId) {
    selectedDeviceRef = { kind: sourceKind, id: config.audio.live.deviceId };
  } else if (sourceKind === 'systemOutput') {
    selectedDeviceRef = capabilities.audio.defaultSystemOutput;
  } else {
    selectedDeviceRef = capabilities.audio.defaultMicrophone;
  }
  const selectedDevice = selectedDeviceRef
    ? capabilities.audio.devices.find(
        (device) => audioDeviceKey(device) === audioDeviceKey(selectedDeviceRef),
      )
    : undefined;
  if (!selectedDevice?.captureSupported) issues.push('audioDevice');
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
  providers: { name: string; baseUrl: string; apiKey: string; models: string[] }[] = [],
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
    const apiKey = config.translation.openaiCompatible.apiKey.trim();
    const selectedProvider = providers.find(
      (provider) =>
        provider.baseUrl === baseUrl &&
        provider.apiKey === apiKey &&
        provider.models.includes(model),
    );
    if (!isHttpUrl(baseUrl) || !model || !selectedProvider) issues.push('translationConfig');
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
