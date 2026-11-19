use std::{fs, path::Path};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::{AsrMode, TranslationMode};

pub const ASR_STREAMING_ID: &str = "zipformer-bilingual-streaming";

pub const ASR_SEGMENTED_ID: &str = "sense-voice-int8";

pub const ASR_QWEN3_06B_ID: &str = "qwen3-asr-0.6b-int8";

pub const ASR_QWEN3_17B_ID: &str = "qwen3-asr-1.7b-int8";

pub const BERGAMOT_ENGINE_ID: &str = "bergamot";

pub const OPENAI_COMPATIBLE_ENGINE_ID: &str = "openai-compatible";

const QWEN3_ASR_LANGUAGES: &[&str] = &[
    "zh", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it", "ko", "ru", "th", "vi", "ja",
    "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs", "fil", "fa", "el", "hu", "mk", "ro",
];

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
    pub download_bytes: u64,
    pub source_name: Option<&'static str>,
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
        download_bytes: 511_274_346,
        source_name: None,
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
        download_bytes: 163_002_883,
        source_name: None,
    },
    AsrModelSpec {
        id: ASR_QWEN3_06B_ID,
        label: "Qwen3 ASR 0.6B INT8 (segmented)",
        kind: AsrModelKind::Segmented,
        source_languages: QWEN3_ASR_LANGUAGES,
        supports_source_language: false,
        source_language_note: Some(
            "Qwen3 detects the language of every utterance. sherpa-onnx 2.0.1 does not expose a per-stream language override.",
        ),
        upstream: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        version: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
        files: &[
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/conv_frontend.onnx",
                min_bytes: 44_148_281,
                sha256: Some("d22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/encoder.int8.onnx",
                min_bytes: 182_491_662,
                sha256: Some("60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/decoder.int8.onnx",
                min_bytes: 755_914_231,
                sha256: Some("4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/tokenizer/merges.txt",
                min_bytes: 1_671_853,
                sha256: Some("8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/tokenizer/tokenizer_config.json",
                min_bytes: 12_487,
                sha256: Some("4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-0.6b-int8/tokenizer/vocab.json",
                min_bytes: 2_776_833,
                sha256: Some("ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910"),
            },
        ],
        download_bytes: 878_702_423,
        source_name: Some("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2"),
    },
    AsrModelSpec {
        id: ASR_QWEN3_17B_ID,
        label: "Qwen3 ASR 1.7B INT8 (segmented)",
        kind: AsrModelKind::Segmented,
        source_languages: QWEN3_ASR_LANGUAGES,
        supports_source_language: false,
        source_language_note: Some(
            "Qwen3 detects the language of every utterance. sherpa-onnx 2.0.1 does not expose a per-stream language override.",
        ),
        upstream: "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx",
        version: "9c182309f7bb075f241424441add9e16c5086dfb",
        files: &[
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/conv_frontend.onnx",
                min_bytes: 48_080_441,
                sha256: Some("fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/encoder.int8.onnx",
                min_bytes: 314_222_162,
                sha256: Some("436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/decoder.int8.onnx",
                min_bytes: 2_037_458_645,
                sha256: Some("c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/tokenizer/merges.txt",
                min_bytes: 1_671_853,
                sha256: Some("8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/tokenizer/tokenizer_config.json",
                min_bytes: 12_487,
                sha256: Some("4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c"),
            },
            ModelFile {
                relative: "sherpa/qwen3-asr-1.7b-int8/tokenizer/vocab.json",
                min_bytes: 2_776_833,
                sha256: Some("ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910"),
            },
        ],
        download_bytes: 2_404_222_421,
        source_name: None,
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

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("zh-Hans", "Simplified Chinese"),
    ("en", "English"),
    ("yue", "Cantonese"),
    ("ar", "Arabic"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("pt", "Portuguese"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ko", "Korean"),
    ("ru", "Russian"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("ja", "Japanese"),
    ("tr", "Turkish"),
    ("hi", "Hindi"),
    ("ms", "Malay"),
    ("nl", "Dutch"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("fi", "Finnish"),
    ("pl", "Polish"),
    ("cs", "Czech"),
    ("fil", "Filipino"),
    ("fa", "Persian"),
    ("el", "Greek"),
    ("hu", "Hungarian"),
    ("mk", "Macedonian"),
    ("ro", "Romanian"),
];

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

pub fn validate_asr_config(
    model_id: &str,
    requested_mode: AsrMode,
    source_language: &str,
) -> Result<(&'static AsrModelSpec, AsrMode), String> {
    let spec = ASR_MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("Unknown ASR model `{model_id}`."))?;
    if source_language != "auto" && !spec.source_languages.contains(&source_language) {
        return Err(format!(
            "ASR model `{}` does not support source language `{source_language}`.",
            spec.label
        ));
    }
    if source_language != "auto" && !spec.supports_source_language {
        return Err(format!(
            "ASR model `{}` detects the language of every utterance, so a forced source language has no effect. Use `auto` instead.",
            spec.label
        ));
    }
    let resolved_mode = resolve_asr_mode(spec, requested_mode)?;
    Ok((spec, resolved_mode))
}

pub fn resolve_translation_mode(requested: TranslationMode) -> Result<TranslationMode, String> {
    match requested {
        TranslationMode::Auto | TranslationMode::Text => Ok(TranslationMode::Text),
        TranslationMode::Streaming => Err(
            "Translation adapters support complete-text translation only; incremental translation is not available."
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
    fn qwen3_models_are_independently_selectable_segmented_models() {
        for id in [ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID] {
            let model = ASR_MODELS
                .iter()
                .find(|model| model.id == id)
                .expect("Qwen3 model is registered");
            assert_eq!(model.kind, AsrModelKind::Segmented);
            assert!(!model.supports_source_language);
            assert_eq!(model.source_languages.len(), 30);
            assert_eq!(
                resolve_asr_mode(model, AsrMode::Auto).unwrap(),
                AsrMode::Segmented
            );
            assert!(resolve_asr_mode(model, AsrMode::Streaming).is_err());
            assert!(validate_asr_config(id, AsrMode::Auto, "en").is_err());
        }
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

    #[test]
    fn supported_targets_cover_the_qwen3_language_set() {
        assert_eq!(SUPPORTED_LANGUAGES.len(), 30);
        assert!(SUPPORTED_LANGUAGES.iter().any(|(code, _)| *code == "yue"));
        assert!(SUPPORTED_LANGUAGES.iter().any(|(code, _)| *code == "ja"));
    }
}
