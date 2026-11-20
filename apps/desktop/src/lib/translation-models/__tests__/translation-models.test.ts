import { describe, expect, it } from 'vite-plus/test';

import type { Capabilities, TranslationConfig } from '@/lib/session';
import type { ProviderConfig } from '@/lib/providers';

import {
  findTranslationModelChoice,
  listTranslationModelChoices,
  selectedTranslationModelId,
} from '../translation-models';

const capabilities: Capabilities = {
  modelsDir: '/tmp/models',
  platform: 'test',
  audio: {
    backend: 'test',
    systemCapture: true,
    microphoneCapture: true,
    supportedPlatform: 'test',
    permission: 'test',
    defaultSystemOutput: null,
    defaultMicrophone: null,
    devices: [],
  },
  asrModels: [],
  translationModels: [
    {
      id: 'bergamot-en-zh-Hans',
      label: 'Bergamot en → zh-Hans',
      available: true,
      missingFiles: [],
      upstream: 'https://example.invalid',
      version: 'test',
      files: [],
      sourceLanguage: 'en',
      targetLanguage: 'zh-Hans',
      engine: 'bergamot',
      supportedModes: ['text'],
    },
  ],
  translationTargets: [],
  translationEngines: [],
  diarization: {
    id: 'diarization',
    label: 'Diarization',
    available: true,
    missingFiles: [],
    upstream: 'https://example.invalid',
    version: 'test',
    files: [],
  },
  speakerEmbedding: {
    id: 'speaker',
    label: 'Speaker',
    available: true,
    missingFiles: [],
    upstream: 'https://example.invalid',
    version: 'test',
    files: [],
  },
  vad: {
    id: 'vad',
    label: 'VAD',
    available: true,
    missingFiles: [],
    upstream: 'https://example.invalid',
    version: 'test',
    files: [],
  },
};

const provider: ProviderConfig = {
  name: 'OpenAI',
  baseUrl: 'https://api.openai.com/v1',
  apiKey: 'secret',
  icon: { name: '', dataUrl: '' },
  models: ['gpt-4o-mini'],
  concurrency: 1,
};

describe('translation model choices', () => {
  it('offers installed local directions and provider models', () => {
    const choices = listTranslationModelChoices(capabilities, [provider]);

    expect(choices.map((choice) => choice.id)).toEqual([
      'local:bergamot-en-zh-Hans',
      'OpenAI\u001fgpt-4o-mini',
    ]);
    expect(choices[1]?.baseUrl).toBe(provider.baseUrl);
    expect(choices[1]?.apiKey).toBe(provider.apiKey);
  });

  it('keeps a saved provider model selected after the provider key changes', () => {
    const translation: TranslationConfig = {
      engine: 'openai-compatible',
      modelId: 'bergamot-en-zh-Hans',
      mode: 'text',
      trigger: 'finalOnly',
      targetLanguage: 'ja',
      openaiCompatible: {
        baseUrl: provider.baseUrl,
        apiKey: 'old-key',
        model: 'gpt-4o-mini',
      },
    };
    const choices = listTranslationModelChoices(capabilities, [provider]);

    expect(selectedTranslationModelId(translation)).not.toBe(choices[1]?.id);
    expect(findTranslationModelChoice(choices, translation)?.id).toBe(choices[1]?.id);
  });
});
