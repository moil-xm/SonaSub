import { describe, expect, it } from 'vite-plus/test';

import type { AppConfig, AsrModelCapability, Capabilities, ModelStatus } from '../../session';
import { assessModelReadiness } from '../model-readiness';

function readyModel(id: string, label: string): ModelStatus {
  return {
    id,
    label,
    available: true,
    missingFiles: [],
    upstream: 'https://example.invalid',
    version: 'test',
    files: [],
  };
}

function qwenCapability(): AsrModelCapability {
  return {
    ...readyModel('qwen3-asr-0.6b-int8', 'Qwen3 ASR 0.6B INT8'),
    kind: 'segmented',
    supportedModes: ['segmented'],
    sourceLanguages: ['zh', 'en', 'ja'],
    supportsSourceLanguage: false,
    sourceLanguageReason:
      'Qwen3 detects the language of every utterance. sherpa-onnx 2.0.1 does not expose a per-stream language override.',
  };
}

function capabilities(qwen: AsrModelCapability): Capabilities {
  return {
    modelsDir: '/tmp/models',
    platform: 'macos-aarch64',
    audio: {
      backend: 'Core Audio process tap',
      systemCapture: true,
      supportedPlatform: 'macOS 14.4 or later (arm64)',
      permission: 'System Audio Recording',
      sampleRate: 16_000,
      channels: 1,
      defaultDevice: null,
      devices: [],
    },
    asrModels: [qwen],
    translationModels: [
      {
        ...readyModel('bergamot-en-zh-Hans', 'Bergamot English to Chinese'),
        sourceLanguage: 'en',
        targetLanguage: 'zh-Hans',
        engine: 'bergamot',
        supportedModes: ['text'],
      },
    ],
    translationTargets: [{ code: 'zh-Hans', label: 'Simplified Chinese' }],
    translationEngines: [
      {
        id: 'bergamot',
        label: 'Bergamot (local)',
        runtime: 'fxtranslate',
        modelFamily: 'bergamot-firefox',
        requiresLocalModel: true,
        targetLanguages: ['zh-Hans', 'en'],
        modes: [],
      },
      {
        id: 'openai-compatible',
        label: 'OpenAI-Compatible',
        runtime: 'adk-model',
        modelFamily: 'openai-compatible',
        requiresLocalModel: false,
        targetLanguages: ['zh-Hans', 'en', 'ja'],
        modes: [],
      },
    ],
    diarization: readyModel('pyannote', 'Pyannote'),
    speakerEmbedding: readyModel('campplus', 'CAM++'),
    vad: readyModel('silero-vad', 'Silero VAD'),
  };
}

function config(): AppConfig {
  return {
    version: 1,
    audio: { deviceId: null, deviceIds: [] },
    recognition: {
      mode: 'auto',
      modelId: 'qwen3-asr-0.6b-int8',
      sourceLanguage: 'auto',
      maxSegmentSecs: 12,
      stableWaitMs: 800,
      vadThreshold: 0.5,
      minSilenceSecs: 0.45,
      minSpeechSecs: 0.25,
    },
    translation: {
      engine: 'bergamot',
      modelId: 'bergamot-en-zh-Hans',
      mode: 'text',
      trigger: 'finalOnly',
      targetLanguage: 'zh-Hans',
      openaiCompatible: {
        baseUrl: 'https://api.openai.com/v1',
        model: '',
        apiKey: '',
      },
    },
    speakers: {
      enabled: false,
      diarizationEnabled: false,
      windowSecs: 20,
      maxSpeakers: 8,
      similarityThreshold: 0.55,
    },
    ui: { historyLimit: 400 },
  };
}

describe('Qwen3 model readiness', () => {
  it('resolves auto to segmented and does not require native partials', () => {
    const result = assessModelReadiness(capabilities(qwenCapability()), config());
    expect(result.selectedMode).toBe('segmented');
    expect(result.canStart).toBe(true);
    expect(result.issues).toEqual([]);
  });

  it('rejects an explicit streaming selection for Qwen3', () => {
    const value = config();
    value.recognition.mode = 'streaming';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('asrMode');
  });

  it('rejects a forced language the selected model cannot apply', () => {
    const value = config();
    value.recognition.sourceLanguage = 'en';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('asrLanguage');
  });

  it('accepts a blank API key for a configured OpenAI-compatible endpoint', () => {
    const value = config();
    value.translation.engine = 'openai-compatible';
    value.translation.targetLanguage = 'ja';
    value.translation.openaiCompatible.model = 'translation-model';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(true);
    expect(result.issues).toEqual([]);
  });

  it('does not require a local Bergamot model for OpenAI-compatible translation', () => {
    const value = config();
    value.translation.engine = 'openai-compatible';
    value.translation.targetLanguage = 'ja';
    value.translation.openaiCompatible.model = 'translation-model';
    const capabilitiesWithMissingBergamot = capabilities(qwenCapability());
    capabilitiesWithMissingBergamot.translationModels[0]!.available = false;
    const result = assessModelReadiness(capabilitiesWithMissingBergamot, value);
    expect(result.canStart).toBe(true);
    expect(result.issues).not.toContain('translationModel');
  });

  it('requires a valid base URL for OpenAI-compatible translation', () => {
    const value = config();
    value.translation.engine = 'openai-compatible';
    value.translation.targetLanguage = 'ja';
    value.translation.openaiCompatible.model = 'translation-model';
    value.translation.openaiCompatible.baseUrl = 'api.openai.com/v1';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('translationConfig');
  });

  it('requires a model name for OpenAI-compatible translation', () => {
    const value = config();
    value.translation.engine = 'openai-compatible';
    value.translation.targetLanguage = 'ja';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('translationConfig');
  });

  it('requires an installed local model for Bergamot translation', () => {
    const capabilitiesWithMissingBergamot = capabilities(qwenCapability());
    capabilitiesWithMissingBergamot.translationModels[0]!.available = false;
    const result = assessModelReadiness(capabilitiesWithMissingBergamot, config());
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('translationModel');
  });

  it('rejects a target outside the selected engine capability', () => {
    const value = config();
    value.translation.targetLanguage = 'ja';
    const result = assessModelReadiness(capabilities(qwenCapability()), value);
    expect(result.canStart).toBe(false);
    expect(result.issues).toContain('translationTarget');
  });
});
