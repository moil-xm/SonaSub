use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};

use crossbeam_channel::bounded;
use tauri::Emitter;

use super::{ResourceMonitor, Runner, RunnerContext};
use crate::{
    asr::AsrEngine,
    audio::{
        self,
        dsp::{AudioHistory, DownmixResampler},
        CaptureOptions,
    },
    config::{AsrMode, TranslationMode, TranslationTrigger},
    models::{
        resolve_asr_mode, resolve_translation_mode, ASR_MODELS, BERGAMOT_ENGINE_ID,
        DIARIZATION_FILES, SPEAKER_FILES,
    },
    pipeline::workers::{ShutdownState, WorkerRuntime},
    session::{
        state::publish_status,
        types::{AsrInfo, SessionInfo, SessionStatus, SpeakerInfoStatus, TranslationInfo},
    },
    speaker::{SpeakerEngine, SpeakerRegistry},
};

const RUN_BOUNDARY: &str =
    "Recognition, diarization and translation share this process through worker threads; a native crash cannot be isolated from the desktop app.";

pub(super) enum StartupError {
    Cancelled,
    Failed(String),
}

impl From<String> for StartupError {
    fn from(value: String) -> Self {
        Self::Failed(value)
    }
}

impl Runner {
    pub(super) fn initialize(context: RunnerContext) -> Result<Self, StartupError> {
        let selected_asr = ASR_MODELS
            .iter()
            .find(|model| model.id == context.config.recognition.model_id)
            .ok_or_else(|| {
                StartupError::Failed(format!(
                    "Unknown ASR model `{}`.",
                    context.config.recognition.model_id
                ))
            })?;
        let asr_mode = resolve_asr_mode(selected_asr, context.config.recognition.mode)
            .map_err(StartupError::Failed)?;
        let translation_mode = resolve_translation_mode(context.config.translation.mode)
            .map_err(StartupError::Failed)?;
        let translation_spec = context
            .inner
            .models
            .translation_for_target(&context.config.translation.target_language)
            .map_err(StartupError::Failed)?;
        let missing = context.inner.models.missing(selected_asr.files);
        if !missing.is_empty() {
            return Err(StartupError::Failed(format!(
                "Missing ASR model files in {}: {}. See docs/models.md.",
                context.inner.models.root.display(),
                missing.join(", ")
            )));
        }
        let translation_files = crate::models::translation_files(translation_spec);
        let missing = context.inner.models.missing(&translation_files);
        if !missing.is_empty() {
            return Err(StartupError::Failed(format!(
                "Missing Bergamot model files in {}: {}. See docs/models.md.",
                context.inner.models.root.display(),
                missing.join(", ")
            )));
        }
        let speakers_enabled = context.config.speakers.enabled;
        let diarization_enabled = speakers_enabled && context.config.speakers.diarization_enabled;
        if speakers_enabled {
            let missing = context.inner.models.missing(SPEAKER_FILES);
            if !missing.is_empty() {
                return Err(StartupError::Failed(format!(
                    "Missing speaker embedding model files in {}: {}.",
                    context.inner.models.root.display(),
                    missing.join(", ")
                )));
            }
        }
        if diarization_enabled {
            let missing = context.inner.models.missing(DIARIZATION_FILES);
            if !missing.is_empty() {
                return Err(StartupError::Failed(format!(
                    "Missing diarization model files in {}: {}.",
                    context.inner.models.root.display(),
                    missing.join(", ")
                )));
            }
        }

        stage(&context, "Loading speech recognition model")?;
        let asr_engine = AsrEngine::load(
            asr_mode,
            &context.inner.models,
            &context.config.recognition,
            selected_asr.id,
        )
        .map_err(StartupError::Failed)?;
        let mut speaker_engine = None;
        let mut diarization_engine = None;
        let mut registry = None;
        if speakers_enabled {
            stage(&context, "Loading speaker embedding model")?;
            let model = context
                .inner
                .models
                .path("sherpa/speaker/campplus-zh-en.onnx");
            speaker_engine = Some(SpeakerEngine::new(&model).map_err(StartupError::Failed)?);
            registry = Some(Arc::new(Mutex::new(SpeakerRegistry::new(
                context.config.speakers.max_speakers,
                context.config.speakers.similarity_threshold,
            ))));
        }
        if diarization_enabled {
            stage(&context, "Loading windowed diarization model")?;
            let segmentation = context
                .inner
                .models
                .path("sherpa/diarization/pyannote-segmentation-3.0.onnx");
            let embedding = context
                .inner
                .models
                .path("sherpa/speaker/campplus-zh-en.onnx");
            diarization_engine = Some(
                crate::speaker::DiarizationEngine::new(&segmentation, &embedding)
                    .map_err(StartupError::Failed)?,
            );
        }

        stage(&context, "Loading Bergamot translation model")?;
        let workers = WorkerRuntime::spawn(
            Box::new(asr_engine),
            context.inner.models.clone(),
            translation_spec,
            speaker_engine,
            diarization_engine,
            registry,
        )
        .map_err(StartupError::Failed)?;
        // Every failure after the workers exist has to release them, otherwise
        // the recognition and translation threads stay blocked on their
        // queues for the lifetime of the process.
        if let Err(error) = stage(&context, "Starting macOS system audio capture") {
            workers.shutdown();
            return Err(error);
        }
        let (capture_tx, capture_rx) = bounded(250);
        let capture_handle = match audio::start(
            CaptureOptions {
                device_id: context.config.audio.device_id.clone(),
            },
            capture_tx,
            context.stop.clone(),
        ) {
            Ok(handle) => handle,
            Err(error) => {
                workers.shutdown();
                return Err(StartupError::Failed(error));
            }
        };
        if context.stop.load(Ordering::SeqCst) {
            let _ = capture_handle.join();
            workers.shutdown();
            return Err(StartupError::Cancelled);
        }
        let asr_info = AsrInfo {
            model_id: selected_asr.id.to_owned(),
            model_label: selected_asr.label.to_owned(),
            mode: asr_mode_string(asr_mode).to_owned(),
            // A model that detects the language per utterance ignores a
            // forced source language. The session reports the language that
            // is actually in effect.
            source_language: if selected_asr.supports_source_language {
                context.config.recognition.source_language.clone()
            } else {
                "auto".to_owned()
            },
            max_segment_secs: context.config.recognition.max_segment_secs,
        };
        let translation_capabilities =
            crate::translation::adapter_capabilities(BERGAMOT_ENGINE_ID)?;
        let translation_info = TranslationInfo {
            engine: translation_capabilities.engine.clone(),
            runtime: translation_capabilities.runtime.clone(),
            model_family: translation_capabilities.model_family.clone(),
            model_id: translation_spec.id.to_owned(),
            model_label: translation_spec.label.to_owned(),
            mode: translation_mode_string(translation_mode).to_owned(),
            target_language: context.config.translation.target_language.clone(),
            trigger: translation_trigger_string(context.config.translation.trigger).to_owned(),
            streaming: translation_capabilities.streaming,
        };
        let info = SessionInfo {
            session_id: context.session_id.clone(),
            started_at_ms: context.started_at_ms,
            audio_backend: "Core Audio process tap".to_owned(),
            audio_device: context.config.audio.device_id.clone(),
            models_dir: context.inner.models.root.display().to_string(),
            asr: asr_info,
            translation: translation_info,
            speakers: SpeakerInfoStatus {
                enabled: speakers_enabled,
                diarization: diarization_enabled,
                embedding_model: if speakers_enabled {
                    "campplus-zh-en".to_owned()
                } else {
                    String::new()
                },
                diarization_window_secs: context.config.speakers.window_secs,
            },
            history_limit: context.history_limit,
            run_boundary: RUN_BOUNDARY.to_owned(),
        };
        let history =
            AudioHistory::new(f64::from(context.config.speakers.window_secs.max(30.0)) + 2.0);
        let now = Instant::now();
        let runner = Self {
            inner: context.inner.clone(),
            config: context.config,
            config_revision: context.config_revision,
            session_id: context.session_id,
            stop: context.stop.clone(),
            info: info.clone(),
            capture_rx,
            _capture_keepalive: None,
            capture_handle: Some(capture_handle),
            workers,
            history,
            resampler: DownmixResampler::new(2),
            expected_frame_end: None,
            last_audio_secs: 0.0,
            pending_discontinuity: None,
            pending_patches: Vec::new(),
            last_patch_emit: now,
            last_stats_emit: now,
            last_diarization: now,
            shutdown_started: None,
            last_wait_notice: now,
            ending: false,
            failure: None,
            shutdown: ShutdownState::new(speakers_enabled, diarization_enabled),
            resources: ResourceMonitor::new(),
        };
        publish_status(
            &context.inner,
            SessionStatus::Running {
                info: Box::new(info),
            },
        );
        Ok(runner)
    }
}

fn stage(context: &RunnerContext, label: &str) -> Result<(), StartupError> {
    if context.stop.load(Ordering::SeqCst) {
        return Err(StartupError::Cancelled);
    }
    let event = {
        let mut runtime = context.inner.runtime();
        if context.stop.load(Ordering::SeqCst) {
            return Err(StartupError::Cancelled);
        }
        if !matches!(runtime.status, SessionStatus::Starting { .. }) {
            return Err(StartupError::Cancelled);
        }
        runtime.set_status(SessionStatus::Starting {
            session_id: context.session_id.clone(),
            stage: label.to_owned(),
        })
    };
    let _ = context.inner.app.emit("session://status", event);
    Ok(())
}

fn asr_mode_string(mode: AsrMode) -> &'static str {
    match mode {
        AsrMode::Auto => "auto",
        AsrMode::Streaming => "streaming",
        AsrMode::Segmented => "segmented",
    }
}

fn translation_mode_string(mode: TranslationMode) -> &'static str {
    match mode {
        TranslationMode::Auto => "auto",
        TranslationMode::Streaming => "streaming",
        TranslationMode::Text => "text",
    }
}

fn translation_trigger_string(trigger: TranslationTrigger) -> &'static str {
    match trigger {
        TranslationTrigger::FinalOnly => "finalOnly",
        TranslationTrigger::StableSegment => "stableSegment",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_labels_match_the_wire_contract() {
        assert_eq!(asr_mode_string(AsrMode::Streaming), "streaming");
        assert_eq!(
            translation_trigger_string(TranslationTrigger::FinalOnly),
            "finalOnly"
        );
    }
}
