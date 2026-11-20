import type { AppConfig, Capabilities, TranslationConfig } from '@/lib/session';
import type { ProviderConfig } from '@/lib/providers';

import type { TranslationModelChoice } from './translation-models.type';

export const LOCAL_TRANSLATION_MODEL_PREFIX = 'local:';
export const PROVIDER_TRANSLATION_MODEL_SEPARATOR = '\u001f';

export function listTranslationModelChoices(
  capabilities: Capabilities,
  providers: ProviderConfig[],
): TranslationModelChoice[] {
  const localModels = capabilities.translationModels.map((model) => ({
    id: `${LOCAL_TRANSLATION_MODEL_PREFIX}${model.id}`,
    label: model.label,
    detail: 'Bergamot',
    baseUrl: '',
    model: '',
    apiKey: '',
    isLocal: true,
    isAvailable: model.available,
  }));
  const providerModels = providers.flatMap((provider) =>
    provider.models.map((model) => ({
      id: `${provider.name}${PROVIDER_TRANSLATION_MODEL_SEPARATOR}${model}`,
      label: model,
      detail: provider.name,
      baseUrl: provider.baseUrl,
      model,
      apiKey: provider.apiKey,
      isLocal: false,
      isAvailable: true,
    })),
  );
  return [...localModels, ...providerModels];
}

export function selectedTranslationModelId(translation: TranslationConfig): string {
  if (translation.engine === 'bergamot') {
    return `${LOCAL_TRANSLATION_MODEL_PREFIX}${translation.modelId}`;
  }
  const { baseUrl, model, apiKey } = translation.openaiCompatible;
  return `${baseUrl}${PROVIDER_TRANSLATION_MODEL_SEPARATOR}${apiKey}${PROVIDER_TRANSLATION_MODEL_SEPARATOR}${model}`;
}

export function findTranslationModelChoice(
  choices: TranslationModelChoice[],
  translation: TranslationConfig,
): TranslationModelChoice | null {
  const selectedId = selectedTranslationModelId(translation);
  const selected = choices.find((choice) => choice.id === selectedId);
  if (selected || translation.engine === 'bergamot') {
    return selected ?? null;
  }
  const { baseUrl, model } = translation.openaiCompatible;
  return (
    choices.find(
      (choice) => !choice.isLocal && choice.baseUrl === baseUrl && choice.model === model,
    ) ?? null
  );
}

export function translationChoiceLabel(choice: TranslationModelChoice): string {
  return `${choice.detail} · ${choice.label}`;
}

export function normalizeTranslationConfig(
  config: AppConfig,
  capabilities: Capabilities,
  choices: TranslationModelChoice[],
): AppConfig {
  const normalized = structuredClone(config);
  let choice = findTranslationModelChoice(choices, normalized.translation);
  if (!choice) {
    choice =
      choices.find((candidate) => candidate.isLocal && candidate.isAvailable) ??
      choices.find((candidate) => candidate.isLocal) ??
      null;
  }
  if (!choice) return normalized;
  if (choice.isLocal) {
    const modelId = choice.id.slice(LOCAL_TRANSLATION_MODEL_PREFIX.length);
    const model = capabilities.translationModels.find((candidate) => candidate.id === modelId);
    normalized.translation.engine = 'bergamot';
    normalized.translation.modelId = modelId;
    if (model) normalized.translation.targetLanguage = model.targetLanguage;
    normalized.translation.openaiCompatible.baseUrl = '';
    normalized.translation.openaiCompatible.model = '';
    normalized.translation.openaiCompatible.apiKey = '';
    return normalized;
  }
  normalized.translation.engine = 'openai-compatible';
  normalized.translation.openaiCompatible.baseUrl = choice.baseUrl;
  normalized.translation.openaiCompatible.model = choice.model;
  normalized.translation.openaiCompatible.apiKey = choice.apiKey;
  return normalized;
}
