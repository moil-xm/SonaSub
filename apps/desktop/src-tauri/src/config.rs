use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::audio::{AudioDeviceRef, AudioSourceKind};

pub const DEFAULT_ASR_MODEL: &str = "zipformer-bilingual-streaming";
pub const DEFAULT_TRANSLATION_ENGINE: &str = "bergamot";
pub const DEFAULT_TRANSLATION_MODEL: &str = "bergamot-en-zh-Hans";
pub const DEFAULT_TARGET_LANGUAGE: &str = "zh-Hans";
pub const DEFAULT_OPENAI_COMPATIBLE_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AsrMode {
    #[default]
    Auto,
    Streaming,
    Segmented,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationMode {
    #[default]
    Auto,
    Streaming,
    Text,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationTrigger {
    #[default]
    FinalOnly,
    StableSegment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioSourceSelection {
    pub kind: AudioSourceKind,
    /// `None` follows the current platform default for `kind`.
    pub device_id: Option<String>,
}

impl Default for AudioSourceSelection {
    fn default() -> Self {
        Self {
            kind: AudioSourceKind::SystemOutput,
            device_id: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioConfig {
    /// The single source used by the live subtitle session.
    pub live: AudioSourceSelection,
    /// Capture sources used by a record. Each entry runs as an independent
    /// source with its own ASR stream and timeline.
    pub record: Vec<AudioDeviceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecognitionConfig {
    pub mode: AsrMode,
    pub model_id: String,
    /// `auto` follows the model's language capability. The bilingual sherpa
    /// models accept both `zh` and `en`; translation still detects the script of
    /// each finalized segment before selecting a model direction.
    pub source_language: String,
    pub max_segment_secs: f32,
    pub stable_wait_ms: u64,
    pub vad_threshold: f32,
    pub min_silence_secs: f32,
    pub min_speech_secs: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            mode: AsrMode::Auto,
            model_id: DEFAULT_ASR_MODEL.to_owned(),
            source_language: "auto".to_owned(),
            max_segment_secs: 12.0,
            stable_wait_ms: 800,
            vad_threshold: 0.5,
            min_silence_secs: 0.45,
            min_speech_secs: 0.25,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl Default for OpenAiCompatibleConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_owned(),
            model: String::new(),
            api_key: String::new(),
        }
    }
}

impl std::fmt::Debug for OpenAiCompatibleConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenAiCompatibleConfig")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

pub fn normalize_openai_base_url(base_url: &str) -> Result<String, String> {
    if base_url.trim().is_empty() {
        return Err("OpenAI-compatible base URL must not be empty.".to_owned());
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    let trimmed = trimmed
        .strip_suffix("/chat/completions")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    let url = Url::parse(trimmed)
        .map_err(|error| format!("OpenAI-compatible base URL is invalid: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("OpenAI-compatible base URL must use http:// or https://.".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            "OpenAI-compatible base URL must not contain credentials; use the API key field instead."
                .to_owned(),
        );
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(
            "OpenAI-compatible base URL must not contain a query string or fragment.".to_owned(),
        );
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

pub fn normalize_target_language(target_language: &str) -> &str {
    match target_language.trim() {
        "zh" => DEFAULT_TARGET_LANGUAGE,
        other => other,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TranslationConfig {
    pub engine: String,
    /// The explicit local translation model selected for the record.
    pub model_id: String,
    pub mode: TranslationMode,
    pub trigger: TranslationTrigger,
    /// Empty asks the backend to use the first available Bergamot target.
    pub target_language: String,
    pub openai_compatible: OpenAiCompatibleConfig,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            engine: DEFAULT_TRANSLATION_ENGINE.to_owned(),
            model_id: DEFAULT_TRANSLATION_MODEL.to_owned(),
            mode: TranslationMode::Auto,
            trigger: TranslationTrigger::FinalOnly,
            target_language: DEFAULT_TARGET_LANGUAGE.to_owned(),
            openai_compatible: OpenAiCompatibleConfig::default(),
        }
    }
}

impl TranslationConfig {
    pub fn normalize(&mut self) -> Result<(), String> {
        self.engine = self.engine.trim().to_owned();
        self.model_id = self.model_id.trim().to_owned();
        self.target_language = normalize_target_language(&self.target_language).to_owned();
        self.openai_compatible.model = self.openai_compatible.model.trim().to_owned();
        self.openai_compatible.base_url = self.openai_compatible.base_url.trim().to_owned();
        if !self.openai_compatible.base_url.is_empty() {
            self.openai_compatible.base_url =
                normalize_openai_base_url(&self.openai_compatible.base_url)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SpeakerConfig {
    pub enabled: bool,
    pub diarization_enabled: bool,
    pub window_secs: f32,
    pub max_speakers: usize,
    pub similarity_threshold: f32,
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            diarization_enabled: true,
            window_secs: 20.0,
            max_speakers: 8,
            similarity_threshold: 0.55,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiConfig {
    pub history_limit: usize,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { history_limit: 400 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub audio: AudioConfig,
    pub recognition: RecognitionConfig,
    pub translation: TranslationConfig,
    pub speakers: SpeakerConfig,
    pub ui: UiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            audio: AudioConfig::default(),
            recognition: RecognitionConfig::default(),
            translation: TranslationConfig::default(),
            speakers: SpeakerConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn normalize(&mut self) -> Result<(), String> {
        self.translation.normalize()
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(1.0..=30.0).contains(&self.recognition.max_segment_secs) {
            return Err("Maximum segment length must be between 1 and 30 seconds.".to_owned());
        }
        if self.recognition.stable_wait_ms < 200 || self.recognition.stable_wait_ms > 10_000 {
            return Err("Stable wait must be between 200 ms and 10 seconds.".to_owned());
        }
        if !(0.05..=0.95).contains(&self.recognition.vad_threshold) {
            return Err("VAD threshold must be between 0.05 and 0.95.".to_owned());
        }
        if !(0.1..=2.0).contains(&self.recognition.min_silence_secs) {
            return Err("Minimum silence must be between 0.1 and 2 seconds.".to_owned());
        }
        if !(0.1..=2.0).contains(&self.recognition.min_speech_secs) {
            return Err("Minimum speech must be between 0.1 and 2 seconds.".to_owned());
        }
        if !(5.0..=60.0).contains(&self.speakers.window_secs) {
            return Err("Diarization window must be between 5 and 60 seconds.".to_owned());
        }
        if !(0.3..=0.9).contains(&self.speakers.similarity_threshold) {
            return Err("Speaker similarity threshold must be between 0.3 and 0.9.".to_owned());
        }
        if self.speakers.max_speakers == 0 || self.speakers.max_speakers > 16 {
            return Err("Maximum speaker count must be between 1 and 16.".to_owned());
        }
        if self.ui.history_limit < 50 || self.ui.history_limit > 2000 {
            return Err("Subtitle history limit must be between 50 and 2000.".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_raw(&self) -> Option<String> {
        fs::read_to_string(&self.path).ok()
    }

    pub fn save_raw(&self, value: &str) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "The configuration path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create the configuration directory {}: {error}",
                parent.display()
            )
        })?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, value)
            .map_err(|error| format!("Failed to write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &self.path)
            .map_err(|error| format!("Failed to replace {}: {error}", self.path.display()))?;
        Ok(())
    }

    /// Loads the configuration. Corrupt files are reported instead of being
    /// silently treated as a first run; the caller can keep the default values
    /// and show the error until the next successful save.
    pub fn load(&self) -> (AppConfig, Option<String>) {
        if !self.path.exists() {
            return (AppConfig::default(), None);
        }
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) => {
                return (
                    AppConfig::default(),
                    Some(format!(
                        "Failed to read configuration at {}: {error}",
                        self.path.display()
                    )),
                );
            }
        };
        match serde_json::from_str::<AppConfig>(&text) {
            Ok(mut config) => match config.normalize().and_then(|()| config.validate()) {
                Ok(()) => (config, None),
                Err(error) => (AppConfig::default(), Some(error)),
            },
            Err(error) => (
                AppConfig::default(),
                Some(format!(
                    "Failed to parse configuration at {}: {error}",
                    self.path.display()
                )),
            ),
        }
    }

    pub fn save(&self, config: &mut AppConfig) -> Result<(), String> {
        config.normalize()?;
        config.validate()?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "The configuration path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create the configuration directory {}: {error}",
                parent.display()
            )
        })?;
        let temporary = self.path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(config)
            .map_err(|error| format!("Failed to serialize configuration: {error}"))?;
        fs::write(&temporary, json).map_err(|error| {
            format!(
                "Failed to write the temporary configuration {}: {error}",
                temporary.display()
            )
        })?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            format!(
                "Failed to replace the configuration {}: {error}",
                self.path.display()
            )
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        AppConfig::default().validate().unwrap();
    }

    #[test]
    fn rejects_invalid_segment_length() {
        let mut config = AppConfig::default();
        config.recognition.max_segment_secs = 0.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn persists_and_reloads() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-config-{}", uuid::Uuid::new_v4()));
        let store = ConfigStore::new(directory.join("config.json"));
        let mut config = AppConfig::default();
        config.translation.target_language = "en".to_owned();
        store.save(&mut config).unwrap();
        let (loaded, error) = store.load();
        assert!(error.is_none());
        assert_eq!(loaded.translation.target_language, "en");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn persists_openai_compatible_settings_including_the_key() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-config-{}", uuid::Uuid::new_v4()));
        let store = ConfigStore::new(directory.join("config.json"));
        let mut config = AppConfig::default();
        config.translation.engine = "openai-compatible".to_owned();
        config.translation.target_language = "ja".to_owned();
        config.translation.openai_compatible.base_url =
            "https://example.test/v1/chat/completions/".to_owned();
        config.translation.openai_compatible.model = "remote-model".to_owned();
        let api_key = ["fixture", "value"].concat();
        config.translation.openai_compatible.api_key = api_key.to_owned();
        store.save(&mut config).unwrap();

        let (loaded, error) = store.load();

        assert!(error.is_none());
        assert_eq!(loaded.translation.engine, "openai-compatible");
        assert_eq!(loaded.translation.target_language, "ja");
        assert_eq!(
            loaded.translation.openai_compatible.base_url,
            "https://example.test/v1"
        );
        assert_eq!(loaded.translation.openai_compatible.model, "remote-model");
        assert_eq!(loaded.translation.openai_compatible.api_key, api_key);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_empty_api_key_can_be_saved() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-config-{}", uuid::Uuid::new_v4()));
        let store = ConfigStore::new(directory.join("config.json"));
        let mut config = AppConfig::default();
        config.translation.engine = "openai-compatible".to_owned();
        config.translation.openai_compatible.model = "local-model".to_owned();
        store.save(&mut config).unwrap();

        let (loaded, error) = store.load();

        assert!(error.is_none());
        assert!(loaded.translation.openai_compatible.api_key.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn api_key_is_redacted_from_debug_output() {
        let api_key = ["fixture", "value"].concat();
        let mut config = AppConfig::default();
        config.translation.openai_compatible.api_key = api_key.to_owned();
        let debug = format!("{config:?}");
        assert!(!debug.contains(api_key.as_str()));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn normalizes_openai_base_urls() {
        let credential = ["fixture", "value"].concat();
        assert_eq!(
            normalize_openai_base_url(" https://example.test/v1/chat/completions/ ").unwrap(),
            "https://example.test/v1"
        );
        assert!(normalize_openai_base_url("ftp://example.test/v1").is_err());
        assert!(
            normalize_openai_base_url(&format!("https://user:{credential}@example.test/v1"))
                .is_err()
        );
        assert!(normalize_openai_base_url("  ").is_err());
    }

    #[test]
    fn normalizes_target_language_aliases() {
        let mut config = TranslationConfig {
            target_language: " zh ".to_owned(),
            ..TranslationConfig::default()
        };
        config.normalize().unwrap();
        assert_eq!(config.target_language, "zh-Hans");

        config.target_language = "yue".to_owned();
        config.normalize().unwrap();
        assert_eq!(config.target_language, "yue");
    }

    #[test]
    fn saving_a_config_normalizes_the_target_and_base_url() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-config-{}", uuid::Uuid::new_v4()));
        let store = ConfigStore::new(directory.join("config.json"));
        let mut config = AppConfig::default();
        config.translation.target_language = "zh".to_owned();
        config.translation.openai_compatible.base_url =
            " https://example.test/v1/chat/completions/ ".to_owned();
        store.save(&mut config).unwrap();

        assert_eq!(config.translation.target_language, "zh-Hans");
        assert_eq!(
            config.translation.openai_compatible.base_url,
            "https://example.test/v1"
        );
        let (loaded, error) = store.load();
        assert!(error.is_none());
        assert_eq!(loaded.translation.target_language, "zh-Hans");
        assert_eq!(
            loaded.translation.openai_compatible.base_url,
            "https://example.test/v1"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn old_config_defaults_the_openai_compatible_section() {
        let config: AppConfig = serde_json::from_str(
            r#"{
                "translation": {
                    "engine": "bergamot",
                    "modelId": "bergamot-en-zh-Hans",
                    "mode": "auto",
                    "trigger": "finalOnly",
                    "targetLanguage": "zh-Hans"
                }
            }"#,
        )
        .unwrap();
        assert_eq!(
            config.translation.openai_compatible.base_url,
            DEFAULT_OPENAI_COMPATIBLE_BASE_URL
        );
        assert!(config.translation.openai_compatible.model.is_empty());
    }
}
