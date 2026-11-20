use super::{
    paths::ModelPaths,
    registry::{
        translation_files, AsrModelKind, ASR_MODELS, BERGAMOT_ENGINE_ID, DIARIZATION_FILES,
        OPENAI_COMPATIBLE_ENGINE_ID, SPEAKER_FILES, SUPPORTED_LANGUAGES, TRANSLATION_MODELS,
        VAD_FILES,
    },
    status::{
        AsrModelCapability, AudioCapabilities, Capabilities, LanguageOption, ModeCapability,
        TranslationEngineCapability, TranslationModelCapability,
    },
};

impl ModelPaths {
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
        let translation_targets = SUPPORTED_LANGUAGES
            .iter()
            .map(|(code, label)| LanguageOption {
                code: (*code).to_owned(),
                label: (*label).to_owned(),
            })
            .collect::<Vec<_>>();
        let bergamot_targets = SUPPORTED_LANGUAGES
            .iter()
            .filter(|(code, _)| {
                TRANSLATION_MODELS
                    .iter()
                    .any(|model| model.target_language == *code)
            })
            .map(|(code, _)| (*code).to_owned())
            .collect();
        let all_targets = SUPPORTED_LANGUAGES
            .iter()
            .map(|(code, _)| (*code).to_owned())
            .collect();
        Capabilities {
            models_dir: self.root.display().to_string(),
            platform: platform_label(),
            audio: audio_capabilities(),
            asr_models,
            translation_models,
            translation_targets,
            translation_engines: vec![
                TranslationEngineCapability {
                    id: BERGAMOT_ENGINE_ID.to_owned(),
                    label: "Bergamot (local)".to_owned(),
                    runtime: crate::translation::BERGAMOT_RUNTIME.to_owned(),
                    model_family: crate::translation::BERGAMOT_MODEL_FAMILY.to_owned(),
                    requires_local_model: true,
                    target_languages: bergamot_targets,
                    modes: translation_modes(Some(
                        "The fxtranslate runtime returns complete translated text for one submitted source string; it does not expose incremental output.",
                    )),
                },
                TranslationEngineCapability {
                    id: OPENAI_COMPATIBLE_ENGINE_ID.to_owned(),
                    label: "OpenAI-Compatible".to_owned(),
                    runtime: crate::translation::OPENAI_COMPATIBLE_RUNTIME.to_owned(),
                    model_family: crate::translation::OPENAI_COMPATIBLE_MODEL_FAMILY.to_owned(),
                    requires_local_model: false,
                    target_languages: all_targets,
                    modes: translation_modes(Some(
                        "The OpenAI-compatible adapter returns one complete translated response per caption; it does not expose incremental output.",
                    )),
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

fn translation_modes(streaming_reason: Option<&str>) -> Vec<ModeCapability> {
    vec![
        ModeCapability {
            mode: "auto".to_owned(),
            available: true,
            reason: None,
            reason_code: None,
        },
        ModeCapability {
            mode: "text".to_owned(),
            available: true,
            reason: None,
            reason_code: None,
        },
        ModeCapability {
            mode: "streaming".to_owned(),
            available: false,
            reason: streaming_reason.map(ToOwned::to_owned),
            reason_code: Some("complete-text-only".to_owned()),
        },
    ]
}

fn platform_label() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn audio_capabilities() -> AudioCapabilities {
    let mut devices = crate::audio::system_outputs().unwrap_or_default();
    devices.extend(crate::audio::microphones().unwrap_or_default());
    AudioCapabilities {
        backend: crate::audio::backend_label().to_owned(),
        system_capture: crate::audio::system_capture_supported(),
        microphone_capture: crate::audio::microphone_capture_supported(),
        supported_platform: crate::audio::supported_platform_label().to_owned(),
        permission: if cfg!(target_os = "macos") {
            "macOS asks for Screen & System Audio Recording for system audio and Microphone for microphone input."
                .to_owned()
        } else {
            "Windows applies the system microphone privacy setting to microphone input.".to_owned()
        },
        default_system_output: crate::audio::default_system_output().map(|id| {
            crate::audio::AudioDeviceRef::new(crate::audio::AudioSourceKind::SystemOutput, id)
        }),
        default_microphone: crate::audio::default_microphone().map(|id| {
            crate::audio::AudioDeviceRef::new(crate::audio::AudioSourceKind::Microphone, id)
        }),
        devices,
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{ModelPaths, ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID};

    #[test]
    fn a_missing_translation_route_carries_a_stable_reason_code() {
        let paths = ModelPaths::new(std::env::temp_dir());
        let capabilities = paths.capabilities();
        let streaming = capabilities
            .translation_engines
            .first()
            .expect("the first translation engine is reported")
            .modes
            .iter()
            .find(|mode| mode.mode == "streaming")
            .expect("the streaming route is reported");
        assert!(!streaming.available);
        assert_eq!(streaming.reason_code.as_deref(), Some("complete-text-only"));
        assert!(streaming.reason.is_some());
    }

    #[test]
    fn qwen3_capabilities_report_segmented_recognition_and_all_languages() {
        let paths = ModelPaths::new(std::env::temp_dir());
        let capabilities = paths.capabilities();
        for id in [ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID] {
            let model = capabilities
                .asr_models
                .iter()
                .find(|model| model.status.id == id)
                .expect("Qwen3 capability is exposed");
            assert_eq!(model.kind, "segmented");
            assert_eq!(model.supported_modes, vec!["segmented"]);
            assert!(!model.supports_source_language);
            assert_eq!(model.source_languages.len(), 30);
        }
    }

    #[test]
    fn translation_engines_declare_their_real_target_scope() {
        let paths = ModelPaths::new(std::env::temp_dir());
        let capabilities = paths.capabilities();
        let bergamot = capabilities
            .translation_engines
            .iter()
            .find(|engine| engine.id == "bergamot")
            .expect("Bergamot is reported");
        assert!(bergamot.requires_local_model);
        assert_eq!(bergamot.target_languages, vec!["zh-Hans", "en"]);

        let openai = capabilities
            .translation_engines
            .iter()
            .find(|engine| engine.id == "openai-compatible")
            .expect("OpenAI-compatible is reported");
        assert!(!openai.requires_local_model);
        assert_eq!(openai.target_languages.len(), 30);
    }
}
