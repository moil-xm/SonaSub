use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::{
    runner::{session_thread, RunnerContext},
    state::{now_ms, publish_failed, publish_status, snapshot, RuntimeState},
    types::{SessionSnapshot, SessionStatus},
};
use crate::{
    audio,
    config::{AppConfig, ConfigStore},
    models::{validate_asr_config, ModelPaths},
    pipeline::metrics::SessionMetrics,
    subtitles::SubtitleStore,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<ManagerInner>,
}

pub(super) struct ManagerInner {
    pub(super) app: AppHandle,
    config_store: ConfigStore,
    pub(super) models: ModelPaths,
    runtime: Mutex<RuntimeState>,
    control: Mutex<ControlState>,
    operations: Mutex<()>,
    config_error: Mutex<Option<String>>,
    config_revision: AtomicU64,
}

struct ControlState {
    stop: Option<Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
    session_id: Option<String>,
}

impl ManagerInner {
    pub(super) fn runtime(&self) -> MutexGuard<'_, RuntimeState> {
        self.runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
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
        let runtime = RuntimeState::new(config, config_error.as_deref());
        Ok(Self {
            inner: Arc::new(ManagerInner {
                app,
                config_store,
                models: ModelPaths::new(model_root),
                runtime: Mutex::new(runtime),
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
        let runtime = self.inner.runtime();
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

    pub fn save_config(&self, mut config: AppConfig) -> Result<ConfigView, String> {
        let _operation = self
            .inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        {
            let runtime = self.inner.runtime();
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
        validate_configuration(&mut config)?;
        self.inner.config_store.save(&mut config)?;
        let revision = self
            .inner
            .config_revision
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        {
            let mut runtime = self.inner.runtime();
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
            config: self.inner.runtime().config.clone(),
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
        let status = { self.inner.runtime().status.clone() };
        match status {
            SessionStatus::Running { .. } | SessionStatus::Starting { .. } => {
                return Ok(self.snapshot());
            }
            SessionStatus::Stopping { .. } => {
                return Err("The previous session is still stopping.".to_owned());
            }
            SessionStatus::Idle | SessionStatus::Failed { .. } => {}
        }

        let (mut config, config_revision, history_limit) = {
            let runtime = self.inner.runtime();
            (
                runtime.config.clone(),
                self.inner.config_revision.load(Ordering::SeqCst),
                runtime.config.ui.history_limit,
            )
        };
        validate_configuration(&mut config)?;
        if !audio::is_supported() {
            return Err(audio::unsupported_capture_error());
        }

        let session_id = Uuid::new_v4().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let started_at_ms = now_ms();
        {
            let mut runtime = self.inner.runtime();
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
            let status = self.inner.runtime().status.clone();
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
        let status = self.inner.runtime().status.clone();
        if !matches!(status, SessionStatus::Stopping { .. }) {
            let session_id = self.inner.runtime().session_id.clone().unwrap_or_default();
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
        let status = self.inner.runtime().status.clone();
        if matches!(status, SessionStatus::Stopping { .. }) {
            publish_failed(
                &self.inner,
                "The session stopped without reporting a final state.",
            );
        }
        Ok(self.snapshot())
    }

    fn cleanup_finished(&self) {
        let status = self.inner.runtime().status.clone();
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

fn validate_configuration(config: &mut AppConfig) -> Result<(), String> {
    config.normalize()?;
    config.validate()?;
    validate_asr_config(
        &config.recognition.model_id,
        config.recognition.mode,
        &config.recognition.source_language,
    )?;
    crate::translation::resolve_translation_config(&config.translation)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_configuration;
    use crate::{
        config::{AppConfig, AsrMode},
        models::{ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID},
    };

    #[test]
    fn live_session_accepts_qwen3_segmented_and_rejects_streaming() {
        for model_id in [ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID] {
            let mut config = AppConfig::default();
            config.recognition.model_id = model_id.to_owned();
            config.recognition.mode = AsrMode::Segmented;
            validate_configuration(&mut config).unwrap();
            config.recognition.mode = AsrMode::Streaming;
            assert!(validate_configuration(&mut config).is_err());
        }
    }
}
