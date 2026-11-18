pub mod types;
pub mod workers;

use std::{
    collections::VecDeque,
    mem,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{bounded, tick, Receiver, Sender, TrySendError};
use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::{
    asr::{AsrControl, AsrEngine, AsrEvent, AsrFinal, AsrInput, AsrPartial},
    audio::{
        self,
        dsp::{AudioHistory, DownmixResampler},
        AudioChunk, CaptureEvent, CaptureOptions, CAPTURE_RATE,
    },
    config::{AppConfig, AsrMode, ConfigStore, TranslationMode, TranslationTrigger},
    models::{
        detect_translation_source, resolve_asr_mode, resolve_translation_mode, ModelPaths,
        ASR_MODELS, BERGAMOT_ENGINE_ID, DIARIZATION_FILES, SPEAKER_FILES, SUPPORTED_LANGUAGES,
        TRANSLATION_MODELS,
    },
    speaker::{DiarizationWindow, SpeakerEngine, SpeakerInfo, SpeakerRegistry},
    subtitles::{StableCandidate, SubtitleBatch, SubtitlePatch, SubtitleStore},
    translation::{PushOutcome, TranslationRequest, TranslationResult},
};

use self::types::{
    AsrInfo, SessionInfo, SessionMetrics, SessionNotice, SessionSnapshot, SessionStatus,
    SpeakerInfoStatus, StatsEvent, StatusEvent, TranslationInfo,
};
use self::workers::{
    DiarizationInput, DiarizationResult, DiarizationTask, ShutdownState, SpeakerInput,
    SpeakerResult, SpeakerTask, WorkerEvent, WorkerKind, WorkerRuntime,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const UI_BATCH_INTERVAL: Duration = Duration::from_millis(80);
const STATS_INTERVAL: Duration = Duration::from_millis(750);
const NOTICE_LIMIT: usize = 100;
/// How often the session reports that it is still waiting for native work
/// that cannot be interrupted.
const NATIVE_WAIT_NOTICE_INTERVAL: Duration = Duration::from_secs(10);
const RUN_BOUNDARY: &str =
    "Recognition, diarization and translation share this process through worker threads; a native crash cannot be isolated from the desktop app.";

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    app: AppHandle,
    config_store: ConfigStore,
    models: ModelPaths,
    runtime: Mutex<RuntimeState>,
    control: Mutex<ControlState>,
    operations: Mutex<()>,
    config_error: Mutex<Option<String>>,
    config_revision: AtomicU64,
}

struct RuntimeState {
    status: SessionStatus,
    session_id: Option<String>,
    store: SubtitleStore,
    metrics: SessionMetrics,
    notices: VecDeque<SessionNotice>,
    config: AppConfig,
    /// Monotonic counter of every published state change.
    revision: u64,
    /// Revision of the current status.
    status_revision: u64,
    /// Revision of the current subtitle list.
    subtitle_revision: u64,
    /// Revision of the current statistics.
    stats_revision: u64,
}

struct ControlState {
    stop: Option<Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
    session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub config: AppConfig,
    pub revision: u64,
    pub load_error: Option<String>,
}

impl SessionManager {
    pub fn model_root(&self) -> PathBuf {
        self.inner.models.root.clone()
    }

    pub fn model_paths(&self) -> &crate::models::ModelPaths {
        &self.inner.models
    }

    pub fn config_dir(&self) -> PathBuf {
        self.inner
            .app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| self.inner.models.root.clone())
    }

    pub fn new(app: AppHandle) -> Result<Self, String> {
        let config_dir = app.path().app_config_dir().map_err(|error| {
            format!("Failed to resolve the application config directory: {error}")
        })?;
        let data_dir = app.path().app_data_dir().map_err(|error| {
            format!("Failed to resolve the application data directory: {error}")
        })?;
        let model_root = match std::env::var_os("SONASUB_MODELS_DIR") {
            Some(value) => {
                let path = PathBuf::from(value);
                if !path.is_absolute() {
                    return Err("SONASUB_MODELS_DIR must be an absolute path.".to_owned());
                }
                path
            }
            None => data_dir.join("models"),
        };
        std::fs::create_dir_all(&model_root)
            .map_err(|error| format!("failed to create {}: {error}", model_root.display()))?;
        let config_store = ConfigStore::new(config_dir.join("config.json"));
        let (config, config_error) = config_store.load();
        let history_limit = config.ui.history_limit;
        let mut notices = VecDeque::new();
        if let Some(error) = &config_error {
            push_notice(&mut notices, "error", error.clone());
        }
        Ok(Self {
            inner: Arc::new(ManagerInner {
                app,
                config_store,
                models: ModelPaths::new(model_root),
                runtime: Mutex::new(RuntimeState {
                    status: SessionStatus::Idle,
                    session_id: None,
                    store: SubtitleStore::new(String::new(), history_limit),
                    metrics: SessionMetrics::new(now_ms()),
                    notices,
                    config,
                    revision: 0,
                    status_revision: 0,
                    subtitle_revision: 0,
                    stats_revision: 0,
                }),
                control: Mutex::new(ControlState {
                    stop: None,
                    handle: None,
                    session_id: None,
                }),
                operations: Mutex::new(()),
                config_error: Mutex::new(config_error),
                config_revision: AtomicU64::new(1),
            }),
        })
    }

    pub fn capabilities(&self) -> crate::models::Capabilities {
        self.inner.models.capabilities()
    }

    pub fn config(&self) -> ConfigView {
        let runtime = self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ConfigView {
            config: runtime.config.clone(),
            revision: self.inner.config_revision.load(Ordering::SeqCst),
            load_error: self
                .inner
                .config_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        snapshot(&self.inner)
    }

    pub fn save_config(&self, config: AppConfig) -> Result<ConfigView, String> {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !matches!(
                runtime.status,
                SessionStatus::Idle | SessionStatus::Failed { .. }
            ) {
                return Err(
                    "Stop the current session before changing configuration. A restart applies the saved configuration."
                        .to_owned(),
                );
            }
        }
        validate_configuration(&config)?;
        self.inner.config_store.save(&config)?;
        let revision = self
            .inner
            .config_revision
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.config = config;
            let history_limit = runtime.config.ui.history_limit;
            runtime.store.set_history_limit(history_limit);
        }
        *self
            .inner
            .config_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        Ok(ConfigView {
            config: self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .config
                .clone(),
            revision,
            load_error: None,
        })
    }

    pub fn start(&self) -> Result<SessionSnapshot, String> {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.start_locked()
    }

    pub fn stop(&self) -> Result<SessionSnapshot, String> {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.stop_locked()
    }

    pub fn restart(&self) -> Result<SessionSnapshot, String> {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.stop_locked()?;
        self.start_locked()
    }

    pub fn shutdown(&self) {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = self.stop_locked();
    }

    fn start_locked(&self) -> Result<SessionSnapshot, String> {
        self.cleanup_finished();
        let status = {
            self.inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .status
                .clone()
        };
        match status {
            SessionStatus::Running { .. } | SessionStatus::Starting { .. } => {
                return Ok(self.snapshot());
            }
            SessionStatus::Stopping { .. } => {
                return Err("The previous session is still stopping.".to_owned());
            }
            SessionStatus::Idle | SessionStatus::Failed { .. } => {}
        }

        let (config, config_revision, history_limit) = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                runtime.config.clone(),
                self.inner.config_revision.load(Ordering::SeqCst),
                runtime.config.ui.history_limit,
            )
        };
        validate_configuration(&config)?;
        if !audio::is_supported() {
            return Err(audio::unsupported_capture_error());
        }

        let session_id = Uuid::new_v4().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let started_at_ms = now_ms();
        {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.status = SessionStatus::Starting {
                session_id: session_id.clone(),
                stage: "Validating models and audio capture".to_owned(),
            };
            runtime.session_id = Some(session_id.clone());
            runtime.store = SubtitleStore::new(session_id.clone(), history_limit);
            runtime.metrics = SessionMetrics::new(started_at_ms);
            runtime.notices.clear();
        }
        publish_status(
            &self.inner,
            SessionStatus::Starting {
                session_id: session_id.clone(),
                stage: "Validating models and audio capture".to_owned(),
            },
        );

        let context = RunnerContext {
            inner: self.inner.clone(),
            session_id: session_id.clone(),
            config,
            config_revision,
            stop: stop.clone(),
            started_at_ms,
            history_limit,
        };
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let handle = thread::Builder::new()
            .name("sonasub-session".to_owned())
            .spawn(move || session_thread(context, ready_tx))
            .map_err(|error| {
                let message = format!("Failed to start the session thread: {error}");
                publish_failed(&self.inner, &message);
                message
            })?;
        {
            let mut control = self
                .inner
                .control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            control.stop = Some(stop.clone());
            control.handle = Some(handle);
            control.session_id = Some(session_id.clone());
        }

        match ready_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(())) => Ok(self.snapshot()),
            Ok(Err(error)) => {
                self.join_current();
                Err(error)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                stop.store(true, Ordering::SeqCst);
                // The worker thread keeps running until it observes the stop
                // flag. The handle stays registered so a later stop can join
                // it; the state reports the pending cancellation instead of
                // pretending the cleanup already finished.
                publish_status(
                    &self.inner,
                    SessionStatus::Stopping {
                        session_id: session_id.clone(),
                        stage:
                            "Cancelling the session start; the worker is still loading native models"
                                .to_owned(),
                    },
                );
                Err(format!(
                    "The session did not become ready within {} seconds. It is being cancelled.",
                    STARTUP_TIMEOUT.as_secs()
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.join_current();
                Err("The session worker exited before reporting readiness.".to_owned())
            }
        }
    }

    fn stop_locked(&self) -> Result<SessionSnapshot, String> {
        let (stop, handle) = {
            let mut control = self
                .inner
                .control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let stop = control.stop.clone();
            let handle = control.handle.take();
            if let Some(stop) = &stop {
                stop.store(true, Ordering::SeqCst);
            }
            (stop, handle)
        };
        let Some(handle) = handle else {
            let status = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .status
                .clone();
            if matches!(
                status,
                SessionStatus::Running { .. }
                    | SessionStatus::Starting { .. }
                    | SessionStatus::Stopping { .. }
            ) {
                let message = "The session is marked active but no worker handle exists.";
                publish_failed(&self.inner, message);
                return Err(message.to_owned());
            }
            return Ok(self.snapshot());
        };
        if let Some(stop) = stop {
            stop.store(true, Ordering::SeqCst);
        }
        let status = self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone();
        if !matches!(status, SessionStatus::Stopping { .. }) {
            let session_id = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .session_id
                .clone()
                .unwrap_or_default();
            publish_status(
                &self.inner,
                SessionStatus::Stopping {
                    session_id,
                    stage: "Flushing recognition and waiting for native inference".to_owned(),
                },
            );
        }
        let joined = handle.join();
        {
            let mut control = self
                .inner
                .control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            control.stop = None;
            control.session_id = None;
        }
        if joined.is_err() {
            publish_failed(&self.inner, "The session worker panicked while stopping.");
            return Err("The session worker panicked while stopping.".to_owned());
        }
        let status = self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone();
        if matches!(status, SessionStatus::Stopping { .. }) {
            publish_failed(
                &self.inner,
                "The session stopped without reporting a final state.",
            );
        }
        Ok(self.snapshot())
    }

    fn cleanup_finished(&self) {
        let status = self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone();
        let terminal = matches!(status, SessionStatus::Idle | SessionStatus::Failed { .. });
        let active = matches!(
            status,
            SessionStatus::Running { .. } | SessionStatus::Starting { .. }
        );
        let was_finished;
        let handle = {
            let mut control = self
                .inner
                .control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            was_finished = control.handle.as_ref().is_some_and(JoinHandle::is_finished);
            if terminal || was_finished {
                let handle = control.handle.take();
                control.stop = None;
                control.session_id = None;
                handle
            } else {
                None
            }
        };
        if let Some(handle) = handle {
            if handle.join().is_err() {
                publish_failed(&self.inner, "The previous session worker panicked.");
            } else if active && was_finished {
                publish_failed(
                    &self.inner,
                    "The session worker exited without a final state.",
                );
            }
        }
    }

    fn join_current(&self) {
        let handle = {
            let mut control = self
                .inner
                .control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let handle = control.handle.take();
            control.stop = None;
            control.session_id = None;
            handle
        };
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

fn validate_configuration(config: &AppConfig) -> Result<(), String> {
    config.validate()?;
    if config.translation.engine != BERGAMOT_ENGINE_ID {
        return Err(format!(
            "Unknown translation engine `{}`. The only compiled adapter is `{BERGAMOT_ENGINE_ID}`.",
            config.translation.engine
        ));
    }
    let asr = ASR_MODELS
        .iter()
        .find(|model| model.id == config.recognition.model_id)
        .ok_or_else(|| format!("Unknown ASR model `{}`.", config.recognition.model_id))?;
    resolve_asr_mode(asr, config.recognition.mode)?;
    if config.recognition.source_language != "auto"
        && !asr
            .source_languages
            .iter()
            .any(|language| *language == config.recognition.source_language)
    {
        return Err(format!(
            "ASR model `{}` does not support source language `{}`.",
            asr.label, config.recognition.source_language
        ));
    }
    if config.recognition.source_language != "auto" && !asr.supports_source_language {
        return Err(format!(
            "ASR model `{}` detects the language of every utterance, so a forced source language has no effect. Use `auto` instead.",
            asr.label
        ));
    }
    if !SUPPORTED_LANGUAGES
        .iter()
        .any(|(code, _)| *code == config.translation.target_language)
    {
        return Err(format!(
            "Unsupported translation target `{}`.",
            config.translation.target_language
        ));
    }
    if !TRANSLATION_MODELS
        .iter()
        .any(|model| model.target_language == config.translation.target_language)
    {
        return Err(format!(
            "No Bergamot model targets `{}`.",
            config.translation.target_language
        ));
    }
    resolve_translation_mode(config.translation.mode)?;
    Ok(())
}

#[derive(Clone)]
struct RunnerContext {
    inner: Arc<ManagerInner>,
    session_id: String,
    config: AppConfig,
    config_revision: u64,
    stop: Arc<AtomicBool>,
    started_at_ms: u64,
    history_limit: usize,
}

enum StartupError {
    Cancelled,
    Failed(String),
}

impl From<String> for StartupError {
    fn from(value: String) -> Self {
        Self::Failed(value)
    }
}

fn session_thread(context: RunnerContext, ready: mpsc::Sender<Result<(), String>>) {
    match Runner::initialize(context.clone()) {
        Ok(runner) => {
            if ready.send(Ok(())).is_err() {
                context.stop.store(true, Ordering::SeqCst);
            }
            let failure = runner.run();
            finish_session(&context.inner, failure.as_deref());
        }
        Err(StartupError::Cancelled) => {
            publish_stopped(&context.inner, Some("Session start was cancelled."));
            let _ = ready.send(Err("Session start was cancelled.".to_owned()));
        }
        Err(StartupError::Failed(error)) => {
            publish_failed(&context.inner, &error);
            let _ = ready.send(Err(error));
        }
    }
}

struct Runner {
    inner: Arc<ManagerInner>,
    config: AppConfig,
    config_revision: u64,
    session_id: String,
    stop: Arc<AtomicBool>,
    info: SessionInfo,
    capture_rx: Receiver<CaptureEvent>,
    _capture_keepalive: Option<Sender<CaptureEvent>>,
    capture_handle: Option<JoinHandle<()>>,
    workers: WorkerRuntime,
    history: AudioHistory,
    resampler: DownmixResampler,
    expected_frame_end: Option<u64>,
    last_audio_secs: f64,
    pending_discontinuity: Option<f64>,
    pending_patches: Vec<SubtitlePatch>,
    last_patch_emit: Instant,
    last_stats_emit: Instant,
    last_diarization: Instant,
    shutdown_started: Option<Instant>,
    last_wait_notice: Instant,
    ending: bool,
    failure: Option<String>,
    shutdown: ShutdownState,
    resources: ResourceMonitor,
}

impl Runner {
    fn initialize(context: RunnerContext) -> Result<Self, StartupError> {
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

    fn run(mut self) -> Option<String> {
        let ticker = tick(Duration::from_millis(50));
        loop {
            crossbeam_channel::select! {
                recv(self.capture_rx) -> message => {
                    match message {
                        Ok(event) => self.handle_capture(event),
                        Err(_) => {
                            if !self.ending {
                                if self.stop.load(Ordering::SeqCst) {
                                    self.begin_shutdown(None);
                                } else {
                                    self.begin_shutdown(Some(
                                        "System audio capture ended unexpectedly.".to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                }
                recv(self.workers.events) -> message => {
                    match message {
                        Ok(event) => self.handle_worker_event(event),
                        Err(_) => {
                            if !self.ending {
                                self.failure = Some(
                                    "All native workers exited before the session stopped.".to_owned(),
                                );
                            }
                            break;
                        }
                    }
                }
                recv(ticker) -> _ => self.handle_tick(),
            }
            // The translation queue may only close once the recognition
            // worker stopped and every subtitle it produced was consumed.
            // Closing it earlier would drop the translation of the last
            // subtitle.
            if self.ending && self.shutdown.may_close_translation(&self.workers.events) {
                self.workers.queue.close();
                self.shutdown.mark_translation_closed();
            }
            if self.ending && self.shutdown.all_finished() && self.workers.events.is_empty() {
                break;
            }
        }
        self.flush_patches(true);
        self.publish_stats();
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }
        let failure = self.failure.clone();
        let workers = mem::replace(&mut self.workers, WorkerRuntime::empty());
        workers.join();
        failure
    }

    fn handle_capture(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Chunk(chunk) => {
                if self.ending {
                    return;
                }
                self.process_chunk(chunk);
            }
            CaptureEvent::Warning(message) => {
                add_notice(&self.inner, "warning", message);
            }
            CaptureEvent::Fatal(message) => {
                self.begin_shutdown(Some(message));
            }
        }
    }

    fn process_chunk(&mut self, chunk: AudioChunk) {
        let expected = self.expected_frame_end.unwrap_or(chunk.frame_index);
        let gap = chunk.frame_index.saturating_sub(expected);
        let discontinuity =
            chunk.discontinuity || chunk.recovered || chunk.dropped_before > 0 || gap > 48;
        let start_secs = chunk.frame_index as f64 / CAPTURE_RATE as f64;
        if discontinuity {
            self.resampler.reset();
            self.history.clear();
            self.pending_discontinuity = Some(start_secs);
            self.last_diarization = Instant::now();
            let dropped = if gap > 0 {
                gap
            } else {
                u64::from(chunk.dropped_before) * 960
            };
            self.add_dropped_frames(dropped);
            if gap > 0 || chunk.dropped_before > 0 {
                add_notice(
                    &self.inner,
                    "warning",
                    format!(
                        "System audio gap detected at {:.2}s; subtitle timing continues after the real gap.",
                        start_secs
                    ),
                );
            }
        }
        let samples = self.resampler.process(&chunk.data, chunk.frames);
        self.history.push(start_secs, &samples);
        {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.add_audio(chunk.frames as u64, 0);
            runtime.metrics.set_level(chunk.peak, chunk.rms);
        }
        self.send_asr(
            AsrInput::Audio {
                samples,
                start_frame_48k: chunk.frame_index,
            },
            start_secs,
        );
        self.expected_frame_end = Some(chunk.frame_index.saturating_add(chunk.frames as u64));
        self.last_audio_secs =
            self.expected_frame_end.unwrap_or(chunk.frame_index) as f64 / CAPTURE_RATE as f64;
    }

    fn send_asr(&mut self, input: AsrInput, start_secs: f64) {
        if let Some(at_secs) = self.pending_discontinuity {
            let Some(sender) = self.workers.asr_tx.as_ref() else {
                self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
                return;
            };
            match sender.try_send(AsrInput::Discontinuity { at_secs }) {
                Ok(()) => self.pending_discontinuity = None,
                Err(TrySendError::Full(_)) => {
                    self.drop_asr_input(start_secs);
                    return;
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
                    return;
                }
            }
        }
        let Some(sender) = self.workers.asr_tx.as_ref() else {
            self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
            return;
        };
        match sender.try_send(input) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.drop_asr_input(start_secs),
            Err(TrySendError::Disconnected(_)) => {
                self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
            }
        }
    }

    fn drop_asr_input(&mut self, start_secs: f64) {
        self.pending_discontinuity = Some(start_secs);
        self.history.clear();
        self.add_dropped_frames(960);
        add_notice(
            &self.inner,
            "warning",
            "ASR could not keep up with system audio. The dropped interval is excluded from the subtitle timeline."
                .to_owned(),
        );
    }

    fn add_dropped_frames(&mut self, frames: u64) {
        let mut runtime = self
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.metrics.add_audio(0, frames);
    }

    fn handle_worker_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Asr(event) => self.handle_asr_event(event),
            WorkerEvent::Speaker(result) => self.handle_speaker_result(result),
            WorkerEvent::Diarization(result) => self.handle_diarization_result(result),
            WorkerEvent::Translation(result) => self.handle_translation_result(result),
            WorkerEvent::Done(kind) => self.mark_worker_done(kind),
        }
    }

    fn mark_worker_done(&mut self, kind: WorkerKind) {
        match kind {
            WorkerKind::Asr => {}
            WorkerKind::Speaker => {
                self.workers.speaker_tx = None;
            }
            WorkerKind::Diarization => {
                self.workers.diarization_tx = None;
            }
            WorkerKind::Translation => {}
        }
        self.shutdown.worker_done(kind);
    }

    fn handle_asr_event(&mut self, event: AsrEvent) {
        match event {
            AsrEvent::Partial(partial) => self.apply_partial(partial),
            AsrEvent::Final(final_event) => self.apply_final(final_event),
            AsrEvent::Warning(message) => add_notice(&self.inner, "warning", message),
            AsrEvent::Fatal(message) => self.begin_shutdown(Some(message)),
        }
    }

    fn apply_partial(&mut self, partial: AsrPartial) {
        let patches = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.partial();
            runtime.store.apply_partial(partial)
        };
        self.pending_patches.extend(patches);
    }

    fn apply_final(&mut self, final_event: AsrFinal) {
        let segment_id = format!("{}:{}", self.session_id, final_event.local_id);
        let samples = Arc::new(final_event.samples.clone());
        let latency_ms = ((self.last_audio_secs - final_event.end_secs).max(0.0) * 1_000.0) as u64;
        let patches = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.final_subtitle(latency_ms);
            runtime.store.apply_final(final_event.clone())
        };
        self.pending_patches.extend(patches);
        if !self.ending && self.config.speakers.enabled && samples.len() >= 4_000 {
            let task = SpeakerTask {
                segment_id: segment_id.clone(),
                samples,
                queued_at: Instant::now(),
            };
            if let Some(sender) = self.workers.speaker_tx.as_ref() {
                match sender.try_send(SpeakerInput::Task(task)) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => add_notice(
                        &self.inner,
                        "warning",
                        "Speaker analysis is behind; one final subtitle is marked as an unknown speaker."
                            .to_owned(),
                    ),
                    Err(TrySendError::Disconnected(_)) => add_notice(
                        &self.inner,
                        "warning",
                        "The speaker worker stopped; speaker labels are unavailable.".to_owned(),
                    ),
                }
            }
        }
        self.queue_translation(&segment_id, final_event.text, &final_event.language);
    }

    fn handle_speaker_result(&mut self, result: SpeakerResult) {
        match result.result {
            Ok(Some(speaker)) => {
                let patch = {
                    let mut runtime = self
                        .inner
                        .runtime
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    runtime.metrics.speaker(result.latency_ms);
                    runtime.store.set_speaker(&result.segment_id, speaker)
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
            Ok(None) => {
                let mut runtime = self
                    .inner
                    .runtime
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime.metrics.speaker(result.latency_ms);
            }
            Err(error) => add_notice(&self.inner, "warning", error),
        }
    }

    fn handle_diarization_result(&mut self, result: DiarizationResult) {
        match result.result {
            Ok(window) => self.apply_diarization(window),
            Err(error) => add_notice(&self.inner, "warning", error),
        }
    }

    fn apply_diarization(&mut self, window: DiarizationWindow) {
        let segments = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.store.snapshot()
        };
        let mut patches = Vec::new();
        for segment in &segments {
            let end = segment.end_secs.unwrap_or(window.window_end);
            let mut durations: Vec<(String, f64, Option<f32>, bool)> = Vec::new();
            for interval in &window.intervals {
                let overlap =
                    end.min(interval.end_secs) - segment.start_secs.max(interval.start_secs);
                if overlap < 0.2 {
                    continue;
                }
                let Some(id) = &interval.speaker_id else {
                    continue;
                };
                if let Some(existing) = durations.iter_mut().find(|(speaker, ..)| speaker == id) {
                    existing.1 += overlap;
                    if interval.confidence.unwrap_or(0.0) > existing.2.unwrap_or(0.0) {
                        existing.2 = interval.confidence;
                    }
                    existing.3 |= interval.uncertain;
                } else {
                    durations.push((id.clone(), overlap, interval.confidence, interval.uncertain));
                }
            }
            if durations.is_empty() {
                continue;
            }
            durations.sort_by(|left, right| right.1.total_cmp(&left.1));
            let (id, _, confidence, uncertain_cluster) = &durations[0];
            let speaker_index = id
                .strip_prefix('S')
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let speaker_overlap = window
                .overlap_regions
                .iter()
                .any(|(start, finish)| end.min(*finish) - segment.start_secs.max(*start) > 0.15);
            let speaker = SpeakerInfo {
                id: id.clone(),
                index: speaker_index,
                confidence: *confidence,
                uncertain: *uncertain_cluster || durations.len() > 1,
                overlap: speaker_overlap,
                source: "diarization".to_owned(),
            };
            let patch = {
                let mut runtime = self
                    .inner
                    .runtime
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime.store.set_speaker(&segment.id, speaker)
            };
            if let Some(patch) = patch {
                patches.push(patch);
            }
        }
        self.pending_patches.extend(patches);
        {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.diarization(window.latency_ms);
        }
    }

    fn handle_translation_result(&mut self, result: TranslationResult) {
        let stale = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime
                .store
                .is_stale(&result.request, self.config_revision)
        };
        if stale {
            return;
        }
        let failed = result.result.is_err();
        let patch = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.translation(result.latency_ms, failed);
            runtime.store.set_translation(
                &result.request.segment_id,
                result.request.text_revision,
                result.result,
                &result.request.target_language,
                BERGAMOT_ENGINE_ID,
                &result.request.model_id,
            )
        };
        if let Some(patch) = patch {
            self.pending_patches.push(patch);
        }
    }

    fn queue_translation(&mut self, segment_id: &str, text: String, language: &Option<String>) {
        let target = self.config.translation.target_language.clone();
        let needs = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.store.needs_translation(segment_id, &target)
        };
        if !needs {
            return;
        }
        let segment_revision = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime
                .store
                .segment(segment_id)
                .map(|segment| segment.revision)
        };
        let Some(text_revision) = segment_revision else {
            return;
        };
        let source_language = translation_source(&text, language.as_deref());
        if source_language == target {
            let patch = {
                let mut runtime = self
                    .inner
                    .runtime
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime
                    .store
                    .mark_same_language(segment_id, &target, BERGAMOT_ENGINE_ID)
            };
            if let Some(patch) = patch {
                self.pending_patches.push(patch);
            }
            return;
        }
        let model = TRANSLATION_MODELS.iter().find(|model| {
            model.target_language == target && model.source_language == source_language
        });
        let Some(model) = model else {
            let patch = {
                let mut runtime = self
                    .inner
                    .runtime
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime.store.set_translation_unavailable(
                    segment_id,
                    format!(
                        "No local Bergamot direction for {source_language} to {target}; this segment remains source-only."
                    ),
                    &target,
                    BERGAMOT_ENGINE_ID,
                )
            };
            if let Some(patch) = patch {
                self.pending_patches.push(patch);
            }
            return;
        };
        let request = TranslationRequest {
            session_id: self.session_id.clone(),
            segment_id: segment_id.to_owned(),
            text_revision,
            config_revision: self.config_revision,
            text,
            source_language: source_language.to_owned(),
            target_language: target.clone(),
            model_id: model.id.to_owned(),
        };
        let patch = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.store.mark_translation_pending(
                segment_id,
                &target,
                BERGAMOT_ENGINE_ID,
                "text",
                model.id,
            )
        };
        if let Some(patch) = patch {
            self.pending_patches.push(patch);
        }
        match self.workers.queue.push(request) {
            PushOutcome::Queued => {}
            PushOutcome::Displaced(evicted) => {
                let patch = {
                    let mut runtime = self
                        .inner
                        .runtime
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    runtime.store.set_translation_error(
                        &evicted.segment_id,
                        evicted.text_revision,
                        &evicted.target_language,
                        BERGAMOT_ENGINE_ID,
                        &evicted.model_id,
                        "Translation queue was full; a newer subtitle superseded this request."
                            .to_owned(),
                    )
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
            PushOutcome::Closed(rejected) => {
                let patch = {
                    let mut runtime = self
                        .inner
                        .runtime
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    runtime.store.set_translation_unavailable(
                        &rejected.segment_id,
                        "The translation worker already stopped, so this subtitle stays source-only."
                            .to_owned(),
                        &rejected.target_language,
                        BERGAMOT_ENGINE_ID,
                    )
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
        }
    }

    fn handle_tick(&mut self) {
        for kind in self.workers.finished_workers() {
            // A worker that exited without reporting its completion left a
            // pending event in the queue that explains why. The failure is
            // only reported once the queue is empty.
            if !self.ending && self.workers.events.is_empty() {
                self.begin_shutdown(Some(format!(
                    "The {} worker exited unexpectedly.",
                    kind.label()
                )));
            }
            self.mark_worker_done(kind);
        }
        if self.stop.load(Ordering::SeqCst) && !self.ending {
            self.begin_shutdown(None);
        }
        self.report_native_wait();
        if !self.ending {
            self.schedule_stable_translations();
            self.schedule_diarization();
            self.update_resource_stats();
        }
        self.flush_patches(false);
        if self.last_stats_emit.elapsed() >= STATS_INTERVAL {
            self.publish_stats();
        }
    }

    /// Reports that the session is still waiting for native work.
    ///
    /// A native inference call cannot be interrupted, so the session keeps
    /// waiting instead of dropping the worker handle and pretending the
    /// cleanup finished.
    fn report_native_wait(&mut self) {
        let Some(started) = self.shutdown_started else {
            return;
        };
        if started.elapsed() < NATIVE_WAIT_NOTICE_INTERVAL
            || self.last_wait_notice.elapsed() < NATIVE_WAIT_NOTICE_INTERVAL
        {
            return;
        }
        self.last_wait_notice = Instant::now();
        add_notice(
            &self.inner,
            "info",
            "Still waiting for the native speech runtime to return. Native inference cannot be interrupted, so the session keeps the worker until it finishes."
                .to_owned(),
        );
    }

    fn schedule_stable_translations(&mut self) {
        if self.config.translation.trigger != TranslationTrigger::StableSegment {
            return;
        }
        let candidates = {
            let runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime
                .store
                .stable_candidates(self.config.recognition.stable_wait_ms)
        };
        for candidate in candidates {
            self.queue_stable_candidate(candidate);
        }
    }

    fn queue_stable_candidate(&mut self, candidate: StableCandidate) {
        if self.ending {
            return;
        }
        self.queue_translation(
            &candidate.segment_id,
            candidate.text,
            &candidate.source_language,
        );
    }

    fn schedule_diarization(&mut self) {
        if !self.info.speakers.diarization {
            return;
        }
        let window_secs = self.config.speakers.window_secs as f64;
        let interval = Duration::from_secs_f64((window_secs / 2.0).max(5.0));
        if self.last_diarization.elapsed() < interval
            || self.history.available_secs() + 0.001 < window_secs
        {
            return;
        }
        let Some((start_secs, samples)) = self.history.snapshot(window_secs) else {
            return;
        };
        let Some(sender) = self.workers.diarization_tx.as_ref() else {
            return;
        };
        let task = DiarizationTask {
            start_secs,
            samples: Arc::new(samples),
        };
        match sender.try_send(DiarizationInput::Task(task)) {
            Ok(()) => self.last_diarization = Instant::now(),
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                self.workers.diarization_tx = None;
            }
        }
    }

    fn update_resource_stats(&mut self) {
        if self.last_stats_emit.elapsed() < STATS_INTERVAL {
            return;
        }
        if let Some((cpu, rss)) = self.resources.sample() {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.metrics.set_resources(cpu, rss);
        }
    }

    fn begin_shutdown(&mut self, failure: Option<String>) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.failure = failure;
        self.shutdown_started = Some(Instant::now());
        self.stop.store(true, Ordering::SeqCst);
        publish_status(
            &self.inner,
            SessionStatus::Stopping {
                session_id: self.session_id.clone(),
                stage: if self.failure.is_some() {
                    "Stopping after a pipeline failure".to_owned()
                } else {
                    "Flushing recognition and waiting for native inference".to_owned()
                },
            },
        );
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }
        let (keepalive, receiver) = bounded(1);
        self.capture_rx = receiver;
        self._capture_keepalive = Some(keepalive);
        // The stop signal travels on the control channel so it is never
        // blocked behind a full audio backlog. A busy recognition worker
        // therefore always reaches its flush and reports the last subtitle.
        if let Some(sender) = self.workers.asr_control_tx.as_ref() {
            if let Err(error) = sender.try_send(AsrControl::Stop) {
                if error.is_disconnected() {
                    self.workers.asr_control_tx = None;
                }
            }
        }
        if let Some(sender) = self.workers.speaker_tx.as_ref() {
            if sender.try_send(SpeakerInput::Stop).is_err() {
                self.workers.speaker_tx = None;
            }
        }
        if let Some(sender) = self.workers.diarization_tx.as_ref() {
            if sender.try_send(DiarizationInput::Stop).is_err() {
                self.workers.diarization_tx = None;
            }
        }
    }

    fn flush_patches(&mut self, force: bool) {
        if self.pending_patches.is_empty() {
            return;
        }
        if !force && self.last_patch_emit.elapsed() < UI_BATCH_INTERVAL {
            return;
        }
        let patches = mem::take(&mut self.pending_patches);
        let revision = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.revision = runtime.revision.saturating_add(1);
            runtime.subtitle_revision = runtime.revision;
            runtime.revision
        };
        let batch = SubtitleBatch {
            revision,
            session_id: self.session_id.clone(),
            patches,
        };
        let _ = self.inner.app.emit("session://subtitles", batch);
        self.last_patch_emit = Instant::now();
    }

    fn publish_stats(&mut self) {
        let (revision, stats) = {
            let mut runtime = self
                .inner
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.revision = runtime.revision.saturating_add(1);
            runtime.stats_revision = runtime.revision;
            (runtime.revision, runtime.metrics.snapshot())
        };
        let _ = self
            .inner
            .app
            .emit("session://stats", StatsEvent { revision, stats });
        self.last_stats_emit = Instant::now();
    }
}

fn finish_session(inner: &Arc<ManagerInner>, failure: Option<&str>) {
    if let Some(error) = failure {
        publish_failed(inner, error);
    } else {
        publish_stopped(inner, Some("Session stopped."));
    }
}

fn publish_stopped(inner: &Arc<ManagerInner>, notice: Option<&str>) {
    publish_status(inner, SessionStatus::Idle);
    if let Some(notice) = notice {
        add_notice(inner, "info", notice.to_owned());
    }
}

fn publish_failed(inner: &Arc<ManagerInner>, error: &str) {
    publish_status(
        inner,
        SessionStatus::Failed {
            error: error.to_owned(),
        },
    );
    add_notice(inner, "error", error.to_owned());
}

fn publish_status(inner: &Arc<ManagerInner>, status: SessionStatus) {
    let revision = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.status = status.clone();
        runtime.revision = runtime.revision.saturating_add(1);
        runtime.status_revision = runtime.revision;
        runtime.revision
    };
    let _ = inner
        .app
        .emit("session://status", StatusEvent { revision, status });
}

fn add_notice(inner: &Arc<ManagerInner>, level: &str, message: String) {
    let notice = SessionNotice {
        level: level.to_owned(),
        message,
        at_ms: now_ms(),
    };
    let emit = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let duplicate = runtime.notices.back().is_some_and(|last| {
            last.level == notice.level
                && last.message == notice.message
                && notice.at_ms.saturating_sub(last.at_ms) < 2_000
        });
        if duplicate {
            false
        } else {
            runtime.notices.push_back(notice.clone());
            while runtime.notices.len() > NOTICE_LIMIT {
                runtime.notices.pop_front();
            }
            true
        }
    };
    if emit {
        let _ = inner.app.emit("session://notice", notice);
    }
}

fn push_notice(notices: &mut VecDeque<SessionNotice>, level: &str, message: String) {
    notices.push_back(SessionNotice {
        level: level.to_owned(),
        message,
        at_ms: now_ms(),
    });
}

fn snapshot(inner: &Arc<ManagerInner>) -> SessionSnapshot {
    let runtime = inner
        .runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    SessionSnapshot {
        revision: runtime.revision,
        session_id: runtime.session_id.clone(),
        status: runtime.status.clone(),
        status_revision: runtime.status_revision,
        subtitle_revision: runtime.subtitle_revision,
        stats_revision: runtime.stats_revision,
        segments: runtime.store.snapshot(),
        stats: runtime.metrics.snapshot(),
        notices: runtime.notices.iter().cloned().collect(),
        history_limit: runtime.store.history_limit(),
    }
}

fn stage(context: &RunnerContext, label: &str) -> Result<(), StartupError> {
    if context.stop.load(Ordering::SeqCst) {
        return Err(StartupError::Cancelled);
    }
    let (revision, status) = {
        let mut runtime = context
            .inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if context.stop.load(Ordering::SeqCst) {
            return Err(StartupError::Cancelled);
        }
        if !matches!(runtime.status, SessionStatus::Starting { .. }) {
            return Err(StartupError::Cancelled);
        }
        runtime.status = SessionStatus::Starting {
            session_id: context.session_id.clone(),
            stage: label.to_owned(),
        };
        runtime.revision = runtime.revision.saturating_add(1);
        runtime.status_revision = runtime.revision;
        (runtime.revision, runtime.status.clone())
    };
    let _ = context
        .inner
        .app
        .emit("session://status", StatusEvent { revision, status });
    Ok(())
}

fn translation_source(text: &str, language: Option<&str>) -> String {
    if let Some(language) = language {
        let normalized = normalize_language(language);
        if !normalized.is_empty() && normalized != "auto" {
            return normalized.to_owned();
        }
    }
    detect_translation_source(text).to_owned()
}

fn normalize_language(language: &str) -> &str {
    let language = language
        .trim()
        .trim_start_matches("<|")
        .trim_end_matches("|>");
    if language.starts_with("zh") || language == "yue" {
        "zh-Hans"
    } else if language.starts_with("en") {
        "en"
    } else {
        language
    }
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

struct ResourceMonitor {
    system: Option<System>,
    pid: Pid,
    last_sample: Instant,
}

impl ResourceMonitor {
    fn new() -> Self {
        Self {
            system: System::new().ok(),
            pid: Pid::from_u32(std::process::id()),
            last_sample: Instant::now() - Duration::from_secs(10),
        }
    }

    fn sample(&mut self) -> Option<(f32, f64)> {
        if self.last_sample.elapsed() < Duration::from_secs(1) {
            return None;
        }
        self.last_sample = Instant::now();
        let system = self.system.as_mut()?;
        system.refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
        let process = system.process(self.pid)?;
        Some((process.cpu_usage(), process.memory() as f64 / 1_048_576.0))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_capabilities(
    state: State<'_, SessionManager>,
) -> Result<crate::models::Capabilities, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.capabilities())
        .await
        .map_err(|error| format!("Capability query failed: {error}"))
}

#[tauri::command]
pub fn get_config(state: State<'_, SessionManager>) -> ConfigView {
    state.inner().config()
}

#[tauri::command]
pub async fn save_config(
    state: State<'_, SessionManager>,
    config: AppConfig,
) -> Result<ConfigView, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.save_config(config))
        .await
        .map_err(|error| format!("Configuration save failed: {error}"))?
}

#[tauri::command]
pub fn session_snapshot(state: State<'_, SessionManager>) -> SessionSnapshot {
    state.inner().snapshot()
}

#[tauri::command]
pub async fn start_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.start())
        .await
        .map_err(|error| format!("Session start failed: {error}"))?
}

#[tauri::command]
pub async fn stop_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop())
        .await
        .map_err(|error| format!("Session stop failed: {error}"))?
}

#[tauri::command]
pub async fn restart_session(state: State<'_, SessionManager>) -> Result<SessionSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.restart())
        .await
        .map_err(|error| format!("Session restart failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_source_uses_the_written_script() {
        assert_eq!(translation_source("今天天气很好。", None), "zh-Hans");
        assert_eq!(translation_source("The weather is nice.", None), "en");
        assert_eq!(translation_source("123", Some("<|zh|>")), "zh-Hans");
        assert_eq!(translation_source("こんにちは", Some("ja")), "ja");
    }

    #[test]
    fn mode_labels_match_the_wire_contract() {
        assert_eq!(asr_mode_string(AsrMode::Streaming), "streaming");
        assert_eq!(
            translation_trigger_string(TranslationTrigger::FinalOnly),
            "finalOnly"
        );
    }
}
