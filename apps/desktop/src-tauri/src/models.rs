use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    audio::SystemOutput,
    config::{AsrMode, TranslationMode},
};

pub const ASR_STREAMING_ID: &str = "zipformer-bilingual-streaming";
pub const ASR_SEGMENTED_ID: &str = "sense-voice-int8";
pub const BERGAMOT_ENGINE_ID: &str = "bergamot";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AsrModelKind {
    Streaming,
    Segmented,
}

impl AsrModelKind {
    pub fn mode(self) -> AsrMode {
        match self {
            Self::Streaming => AsrMode::Streaming,
            Self::Segmented => AsrMode::Segmented,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelFile {
    pub relative: &'static str,
    pub min_bytes: u64,
    pub sha256: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct AsrModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: AsrModelKind,
    pub source_languages: &'static [&'static str],
    /// Whether a forced source language changes the recognition result.
    pub supports_source_language: bool,
    /// Why a forced source language is not available for this model.
    pub source_language_note: Option<&'static str>,
    pub upstream: &'static str,
    pub version: &'static str,
    pub files: &'static [ModelFile],
}

#[derive(Debug, Clone)]
pub struct TranslationModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub source_language: &'static str,
    pub target_language: &'static str,
    pub upstream: &'static str,
    pub version: &'static str,
    pub model: ModelFile,
    pub source_vocab: ModelFile,
    pub target_vocab: Option<ModelFile>,
    pub shortlist: Option<ModelFile>,
}

pub const ASR_MODELS: &[AsrModelSpec] = &[
    AsrModelSpec {
        id: ASR_STREAMING_ID,
        label: "Zipformer bilingual zh-en (streaming)",
        kind: AsrModelKind::Streaming,
        source_languages: &["zh", "en"],
        supports_source_language: false,
        source_language_note: Some(
            "The bilingual streaming model detects the language of every utterance. A forced source language would not change the result.",
        ),
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        version: "sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20",
        files: &[
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx",
                min_bytes: 1_000_000,
                sha256: Some("8fa764187a261844f859d7143ebaa563af5d10adfece4c18a8f414c88cba2a9b"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx",
                min_bytes: 100_000,
                sha256: Some("1a70c593d71e53f023f5f55b0b4cfff5055abb786ee3992e5f63dc2e273cc4fa"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx",
                min_bytes: 100_000,
                sha256: Some("1ed689c5ed19dbaa725d9d191bb4822b5f4855a39e1ffd28cbc1f340d25b2ee0"),
            },
            ModelFile {
                relative: "sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt",
                min_bytes: 10_000,
                sha256: Some("a8e0e4ec53810e433789b54a5c0134a7eaa2ffca595a6334d54c00da858841d3"),
            },
        ],
    },
    AsrModelSpec {
        id: ASR_SEGMENTED_ID,
        label: "SenseVoice multilingual (segmented)",
        kind: AsrModelKind::Segmented,
        source_languages: &["zh", "en", "ja", "ko", "yue"],
        supports_source_language: true,
        source_language_note: None,
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        version: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
        files: &[
            ModelFile {
                relative: "sherpa/sense-voice-int8/model.onnx",
                min_bytes: 100_000_000,
                sha256: Some("c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51"),
            },
            ModelFile {
                relative: "sherpa/sense-voice-int8/tokens.txt",
                min_bytes: 100_000,
                sha256: Some("f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc"),
            },
        ],
    },
];

pub const TRANSLATION_MODELS: &[TranslationModelSpec] = &[
    TranslationModelSpec {
        id: "bergamot-en-zh-Hans",
        label: "Bergamot English to Simplified Chinese",
        source_language: "en",
        target_language: "zh-Hans",
        upstream: "https://firefox.settings.services.mozilla.com/v1/buckets/main/collections/translations-models-v2/records",
        version: "Bergamot v3.0",
        model: ModelFile {
            relative: "bergamot/en-zh-Hans/model.bin",
            min_bytes: 10_000_000,
            sha256: Some("4e5accc141373565ddc8fa1565bceaa8d0c3482a82cab8131c719ebcc6c2157c"),
        },
        source_vocab: ModelFile {
            relative: "bergamot/en-zh-Hans/src.spm",
            min_bytes: 100_000,
            sha256: Some("bd9b65504acc6d9726dd281f7defc2adb7c2c22d0688fe2f84697de25197c8c5"),
        },
        target_vocab: Some(ModelFile {
            relative: "bergamot/en-zh-Hans/trg.spm",
            min_bytes: 100_000,
            sha256: Some("aded6993c36e440284d11cec3f6b8aef9c0e43188a772d80be342a713adf223d"),
        }),
        shortlist: Some(ModelFile {
            relative: "bergamot/en-zh-Hans/lex.bin",
            min_bytes: 100_000,
            sha256: Some("4a5e5827788060f1d718a8132b69440929387514a045796e9b77f935db68c055"),
        }),
    },
    TranslationModelSpec {
        id: "bergamot-zh-Hans-en",
        label: "Bergamot Simplified Chinese to English",
        source_language: "zh-Hans",
        target_language: "en",
        upstream: "https://firefox.settings.services.mozilla.com/v1/buckets/main/collections/translations-models-v2/records",
        version: "Bergamot v3.1",
        model: ModelFile {
            relative: "bergamot/zh-Hans-en/model.bin",
            min_bytes: 10_000_000,
            sha256: Some("5cd149601802fc8a18124a1c1306144dbbedc058630c4ddb2d53aa76fa9c7c06"),
        },
        source_vocab: ModelFile {
            relative: "bergamot/zh-Hans-en/vocab.spm",
            min_bytes: 100_000,
            sha256: Some("dff594318ab7d8b7b60b844ab98ebe6b932ae8045fab15235404c787715965b3"),
        },
        target_vocab: None,
        shortlist: Some(ModelFile {
            relative: "bergamot/zh-Hans-en/lex.bin",
            min_bytes: 100_000,
            sha256: Some("8524dd4c93ffd7f0ae7be32f77a7c14fea7cdbbebe34ea6ad0ec368d5ddd8b13"),
        }),
    },
];

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("zh-Hans", "Simplified Chinese"), ("en", "English")];

pub const LANGUAGE_LABEL_ZH: &str = "简体中文";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub missing_files: Vec<String>,
    pub upstream: String,
    pub version: String,
    pub files: Vec<ModelFileStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileStatus {
    pub path: String,
    pub present: bool,
    pub bytes: u64,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrModelCapability {
    #[serde(flatten)]
    pub status: ModelStatus,
    pub kind: String,
    #[serde(rename = "supportedModes")]
    pub supported_modes: Vec<String>,
    pub source_languages: Vec<String>,
    /// Whether a forced source language changes the recognition result.
    pub supports_source_language: bool,
    /// Why a forced source language is not available for this model.
    pub source_language_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelCapability {
    #[serde(flatten)]
    pub status: ModelStatus,
    pub source_language: String,
    pub target_language: String,
    pub engine: String,
    pub supported_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOption {
    pub code: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCapabilities {
    pub backend: String,
    pub system_capture: bool,
    pub supported_platform: String,
    pub permission: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub default_device: Option<String>,
    pub devices: Vec<SystemOutput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub models_dir: String,
    pub platform: String,
    pub audio: AudioCapabilities,
    pub asr_models: Vec<AsrModelCapability>,
    pub translation_models: Vec<TranslationModelCapability>,
    pub translation_targets: Vec<LanguageOption>,
    pub translation_engine: String,
    /// The native runtime that executes the translation models.
    pub translation_runtime: String,
    /// The model family the compiled adapter reads.
    pub translation_model_family: String,
    pub translation_modes: Vec<ModeCapability>,
    pub diarization: ModelStatus,
    pub speaker_embedding: ModelStatus,
    pub vad: ModelStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeCapability {
    pub mode: String,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub root: PathBuf,
}

impl ModelPaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn asr(&self, id: &str) -> Result<&'static AsrModelSpec, String> {
        ASR_MODELS
            .iter()
            .find(|model| model.id == id)
            .ok_or_else(|| format!("Unknown ASR model id `{id}`."))
    }

    pub fn translation(
        &self,
        source: &str,
        target: &str,
    ) -> Result<&'static TranslationModelSpec, String> {
        TRANSLATION_MODELS
            .iter()
            .find(|model| {
                model.source_language == source && model.target_language == target
            })
            .ok_or_else(|| {
                format!(
                    "No local Bergamot model for {source} to {target}. Available directions: en to zh-Hans, zh-Hans to en."
                )
            })
    }

    pub fn translation_for_target(
        &self,
        target: &str,
    ) -> Result<&'static TranslationModelSpec, String> {
        TRANSLATION_MODELS
            .iter()
            .find(|model| model.target_language == target)
            .ok_or_else(|| {
                format!(
                    "No local Bergamot model targets `{target}`. Available targets: zh-Hans, en."
                )
            })
    }

    pub fn missing(&self, files: &[ModelFile]) -> Vec<String> {
        files
            .iter()
            .filter(|file| {
                let path = self.path(file.relative);
                match fs::metadata(&path) {
                    Ok(metadata) => !metadata.is_file() || metadata.len() < file.min_bytes,
                    Err(_) => true,
                }
            })
            .map(|file| file.relative.to_owned())
            .collect()
    }

    pub fn verify_file(&self, file: &ModelFile) -> Result<String, String> {
        let path = self.path(file.relative);
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn model_status(
        &self,
        id: &str,
        label: &str,
        upstream: &str,
        version: &str,
        files: &[ModelFile],
    ) -> ModelStatus {
        let mut statuses = Vec::new();
        let mut missing = Vec::new();
        for file in files {
            let path = self.path(file.relative);
            let metadata = fs::metadata(&path).ok();
            let present = metadata
                .as_ref()
                .map(|metadata| metadata.is_file() && metadata.len() >= file.min_bytes)
                .unwrap_or(false);
            if !present {
                missing.push(file.relative.to_owned());
            }
            statuses.push(ModelFileStatus {
                path: path.display().to_string(),
                present,
                bytes: metadata.map(|metadata| metadata.len()).unwrap_or(0),
                expected_sha256: file.sha256.map(ToOwned::to_owned),
            });
        }
        ModelStatus {
            id: id.to_owned(),
            label: label.to_owned(),
            available: missing.is_empty(),
            missing_files: missing,
            upstream: upstream.to_owned(),
            version: version.to_owned(),
            files: statuses,
        }
    }

    pub fn capabilities(&self) -> Capabilities {
        let asr_models = ASR_MODELS
            .iter()
            .map(|model| {
                let kind = match model.kind {
                    AsrModelKind::Streaming => "streaming",
                    AsrModelKind::Segmented => "segmented",
                };
                AsrModelCapability {
                    status: self.model_status(
                        model.id,
                        model.label,
                        model.upstream,
                        model.version,
                        model.files,
                    ),
                    kind: kind.to_owned(),
                    supported_modes: vec![kind.to_owned()],
                    source_languages: model
                        .source_languages
                        .iter()
                        .map(|language| (*language).to_owned())
                        .collect(),
                    supports_source_language: model.supports_source_language,
                    source_language_reason: model.source_language_note.map(ToOwned::to_owned),
                }
            })
            .collect();
        let translation_models = TRANSLATION_MODELS
            .iter()
            .map(|model| TranslationModelCapability {
                status: self.model_status(
                    model.id,
                    model.label,
                    model.upstream,
                    model.version,
                    &translation_files(model),
                ),
                source_language: model.source_language.to_owned(),
                target_language: model.target_language.to_owned(),
                engine: BERGAMOT_ENGINE_ID.to_owned(),
                supported_modes: vec!["text".to_owned()],
            })
            .collect();
        Capabilities {
            models_dir: self.root.display().to_string(),
            platform: platform_label(),
            audio: audio_capabilities(),
            asr_models,
            translation_models,
            translation_targets: SUPPORTED_LANGUAGES
                .iter()
                .map(|(code, label)| LanguageOption {
                    code: (*code).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            translation_engine: BERGAMOT_ENGINE_ID.to_owned(),
            translation_runtime: crate::translation::TRANSLATION_RUNTIME.to_owned(),
            translation_model_family: crate::translation::TRANSLATION_MODEL_FAMILY.to_owned(),
            translation_modes: vec![
                ModeCapability {
                    mode: "auto".to_owned(),
                    available: true,
                    reason: None,
                },
                ModeCapability {
                    mode: "text".to_owned(),
                    available: true,
                    reason: None,
                },
                ModeCapability {
                    mode: "streaming".to_owned(),
                    available: false,
                    reason: Some(
                        "The fxtranslate runtime returns complete translated text for one submitted source string; it does not expose incremental output."
                            .to_owned(),
                    ),
                },
            ],
            diarization: self.model_status(
                "pyannote-segmentation-3.0",
                "Pyannote segmentation 3.0",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-segmentation-models",
                "sherpa-onnx-pyannote-segmentation-3-0",
                DIARIZATION_FILES,
            ),
            speaker_embedding: self.model_status(
                "campplus-zh-en",
                "3D-Speaker CAM++ zh-en",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models",
                "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
                SPEAKER_FILES,
            ),
            vad: self.model_status(
                "silero-vad",
                "Silero VAD",
                "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
                "silero_vad.onnx",
                VAD_FILES,
            ),
        }
    }
}

pub const VAD_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/silero_vad.onnx",
    min_bytes: 100_000,
    sha256: Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"),
}];

pub const SPEAKER_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/speaker/campplus-zh-en.onnx",
    min_bytes: 10_000_000,
    sha256: Some("aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2"),
}];

pub const DIARIZATION_FILES: &[ModelFile] = &[ModelFile {
    relative: "sherpa/diarization/pyannote-segmentation-3.0.onnx",
    min_bytes: 1_000_000,
    sha256: Some("d582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d"),
}];

pub fn translation_files(model: &TranslationModelSpec) -> Vec<ModelFile> {
    let mut files = vec![model.model.clone(), model.source_vocab.clone()];
    if let Some(target) = &model.target_vocab {
        files.push(target.clone());
    }
    if let Some(shortlist) = &model.shortlist {
        files.push(shortlist.clone());
    }
    files
}

pub fn resolve_asr_mode(spec: &AsrModelSpec, requested: AsrMode) -> Result<AsrMode, String> {
    let actual = match requested {
        AsrMode::Auto => spec.kind.mode(),
        mode => mode,
    };
    if actual != spec.kind.mode() {
        let requested_label = mode_label(actual);
        return Err(format!(
            "ASR model `{}` does not support {requested_label}. Its capability is {}.",
            spec.label,
            mode_label(spec.kind.mode())
        ));
    }
    Ok(actual)
}

pub fn resolve_translation_mode(requested: TranslationMode) -> Result<TranslationMode, String> {
    match requested {
        TranslationMode::Auto | TranslationMode::Text => Ok(TranslationMode::Text),
        TranslationMode::Streaming => Err(
            "The Bergamot adapter supports complete-text translation only; incremental translation is not available."
                .to_owned(),
        ),
    }
}

pub fn mode_label(mode: AsrMode) -> &'static str {
    match mode {
        AsrMode::Auto => "auto",
        AsrMode::Streaming => "streaming recognition",
        AsrMode::Segmented => "segmented recognition",
    }
}

pub fn language_label(code: &str) -> &str {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, label)| *label)
        .unwrap_or(code)
}

pub fn detect_translation_source(text: &str) -> &'static str {
    let mut han = 0usize;
    let mut latin = 0usize;
    for character in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&character) {
            han += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if han > 0 && han * 2 >= latin {
        "zh-Hans"
    } else {
        "en"
    }
}

pub fn verify_files(root: &Path, files: &[ModelFile]) -> Vec<(String, bool, Option<String>)> {
    files
        .iter()
        .map(|file| {
            let path = root.join(file.relative);
            let actual = match fs::read(&path) {
                Ok(bytes) => Some(format!("{:x}", Sha256::digest(&bytes))),
                Err(_) => None,
            };
            let matches = match (&file.sha256, &actual) {
                (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
                (None, Some(_)) => true,
                (_, None) => false,
            };
            (file.relative.to_owned(), matches, actual)
        })
        .collect()
}

fn platform_label() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn audio_capabilities() -> AudioCapabilities {
    let devices = crate::audio::system_outputs().unwrap_or_default();
    let supported = crate::audio::is_supported();
    AudioCapabilities {
        backend: if supported {
            "Core Audio process tap".to_owned()
        } else {
            "Unsupported on this platform".to_owned()
        },
        system_capture: supported,
        supported_platform: "macOS 14.4 or later (arm64)".to_owned(),
        permission: "macOS System Audio Recording".to_owned(),
        sample_rate: crate::audio::CAPTURE_RATE,
        channels: crate::audio::CAPTURE_CHANNELS,
        default_device: crate::audio::default_system_output(),
        devices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_model_capability() {
        let streaming = ASR_MODELS
            .iter()
            .find(|model| model.id == ASR_STREAMING_ID)
            .unwrap();
        assert_eq!(
            resolve_asr_mode(streaming, AsrMode::Auto).unwrap(),
            AsrMode::Streaming
        );
        assert!(resolve_asr_mode(streaming, AsrMode::Segmented).is_err());
    }

    #[test]
    fn bergamot_never_claims_streaming() {
        assert_eq!(
            resolve_translation_mode(TranslationMode::Auto).unwrap(),
            TranslationMode::Text
        );
        assert!(resolve_translation_mode(TranslationMode::Streaming).is_err());
    }

    #[test]
    fn language_detection_handles_mixed_scripts() {
        assert_eq!(detect_translation_source("今天天气很好。"), "zh-Hans");
        assert_eq!(detect_translation_source("The weather is nice."), "en");
    }
}
