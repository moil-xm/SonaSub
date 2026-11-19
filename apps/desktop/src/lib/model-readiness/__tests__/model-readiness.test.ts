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
    translationEngine: 'bergamot',
    translationRuntime: 'fxtranslate',
    translationModelFamily: 'bergamot-firefox',
    translationModes: [],
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
});
