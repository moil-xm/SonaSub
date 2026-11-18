use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_ASR_MODEL: &str = "zipformer-bilingual-streaming";
pub const DEFAULT_TRANSLATION_ENGINE: &str = "bergamot";
pub const DEFAULT_TARGET_LANGUAGE: &str = "zh-Hans";

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioConfig {
    /// Output endpoint id. Empty selects the current macOS default output.
    pub device_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TranslationConfig {
    pub engine: String,
    pub mode: TranslationMode,
    pub trigger: TranslationTrigger,
    /// Empty asks the backend to use the first available Bergamot target.
    pub target_language: String,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            engine: DEFAULT_TRANSLATION_ENGINE.to_owned(),
            mode: TranslationMode::Auto,
            trigger: TranslationTrigger::FinalOnly,
            target_language: DEFAULT_TARGET_LANGUAGE.to_owned(),
        }
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
            enabled: true,
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
            Ok(config) => match config.validate() {
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

    pub fn save(&self, config: &AppConfig) -> Result<(), String> {
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
        store.save(&config).unwrap();
        let (loaded, error) = store.load();
        assert!(error.is_none());
        assert_eq!(loaded.translation.target_language, "en");
        fs::remove_dir_all(directory).unwrap();
    }
}
