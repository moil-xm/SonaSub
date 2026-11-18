use serde::Serialize;

use crate::audio::SystemOutput;

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
pub struct ModelDownloadSettings {
    pub directory: String,
    pub default_directory: String,
    pub models_dir: String,
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
    /// A stable identifier for the reason, so the interface can show a
    /// localized explanation instead of the raw backend text.
    pub reason_code: Option<String>,
}
