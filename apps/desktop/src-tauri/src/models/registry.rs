use std::{fs, path::Path};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::{AsrMode, TranslationMode};

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

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("zh-Hans", "Simplified Chinese"), ("en", "English")];

pub const LANGUAGE_LABEL_ZH: &str = "简体中文";

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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::TranslationMode;
    use crate::models::detect_translation_source;

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
