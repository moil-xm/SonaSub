pub mod source;
pub mod store;
pub mod subtitles;
pub mod types;

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{bounded, Receiver, Sender};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::{
    audio,
    config::AppConfig,
    models::{ModelPaths, BERGAMOT_ENGINE_ID, TRANSLATION_MODELS},
    subtitles::SubtitlePatch,
};

use self::{
    source::{SourceContext, SourceEvent, SourceOutcome},
    store::RecordStore,
    subtitles::RecordSubtitleStore,
    types::{
        RecordDevice, RecordInput, RecordNotice, RecordNoticeEvent, RecordSnapshot, RecordSource,
        RecordState, RecordStats, RecordStatsEvent, RecordSubtitleBatch, RecordSubtitlePage,
        RecordSummary, RecordSummaryEvent, SourceEndReason, SourceRunState, StoredRecord,
        SubtitleCursor,
    },
};

const RECORD_EVENT_QUEUE_DEPTH: usize = 4_096;
const NOTICE_LIMIT: usize = 100;
const SOURCE_STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const SOURCE_STOP_TIMEOUT: Duration = Duration::from_secs(300);

type SourceSpawn = (
    Arc<AtomicBool>,
    mpsc::Receiver<()>,
    JoinHandle<()>,
    mpsc::Receiver<Result<(), String>>,
);

#[derive(Clone)]
pub struct RecordManager {
    inner: Arc<RecordManagerInner>,
}

struct RecordManagerInner {
    app: AppHandle,
    models: ModelPaths,
    store: RecordStore,
    runtime: Mutex<RecordRuntimeState>,
    operations: Mutex<()>,
    event_tx: Sender<SourceEvent>,
    event_handle: Mutex<Option<JoinHandle<()>>>,
}

struct RecordRuntimeState {
    records: HashMap<String, RecordRuntime>,
    device_claims: HashMap<String, String>,
}

struct RecordRuntime {
    summary: RecordSummary,
    config: AppConfig,
    sources: Vec<RecordSource>,
    active_sources: HashMap<String, ActiveRecordSource>,
    segments: RecordSubtitleStore,
    segments_loaded: bool,
    stats: RecordStats,
    source_stats: HashMap<String, RecordStats>,
    notices: VecDeque<RecordNotice>,
    started_at_ms: Option<u64>,
    ended_at_ms: Option<u64>,
    summary_revision: u64,
    subtitle_revision: u64,
    stats_revision: u64,
}

struct ActiveRecordSource {
    source: RecordSource,
    stop: Arc<AtomicBool>,
    completion: Option<mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
}

struct SourceStop {
    record_id: String,
    source_run_id: String,
    device_id: String,
    stop: Arc<AtomicBool>,
    completion: Option<mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
}

impl RecordManager {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let data_dir = app.path().app_data_dir().map_err(|error| {
            format!("Failed to resolve the application data directory: {error}")
        })?;
        let model_root = match std::env::var_os("SONASUB_MODELS_DIR") {
            Some(value) => {
                let path = std::path::PathBuf::from(value);
                if !path.is_absolute() {
                    return Err("SONASUB_MODELS_DIR must be an absolute path.".to_owned());
                }
                path
            }
            None => data_dir.join("models"),
        };
        let store = RecordStore::open(data_dir.join("records.sqlite"))?;
        store.recover_active_records(now_ms())?;
        let records = load_records(&store)?;
        let (event_tx, event_rx) = bounded(RECORD_EVENT_QUEUE_DEPTH);
        let inner = Arc::new(RecordManagerInner {
            app,
            models: ModelPaths::new(model_root),
            store,
            runtime: Mutex::new(RecordRuntimeState {
                records,
                device_claims: HashMap::new(),
            }),
            operations: Mutex::new(()),
            event_tx,
            event_handle: Mutex::new(None),
        });
        let event_inner = inner.clone();
        let event_handle = thread::Builder::new()
            .name("sonasub-record-events".to_owned())
            .spawn(move || run_event_loop(event_inner, event_rx))
            .map_err(|error| format!("Failed to start the record event loop: {error}"))?;
        *inner
            .event_handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(event_handle);
        Ok(Self { inner })
    }

    pub fn list_records(&self) -> Vec<RecordSummary> {
        let runtime = self.runtime();
        let mut records = runtime
            .records
            .values()
            .map(|record| record.summary.clone())
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.created_at_ms, record.id.clone()));
        records
    }

    pub fn list_devices(&self) -> Vec<RecordDevice> {
        let outputs = audio::system_outputs().unwrap_or_default();
        let runtime = self.runtime();
        outputs
            .into_iter()
            .map(|output| {
                let occupied_record_id = runtime.device_claims.get(&output.id).cloned();
                let occupied_record_name = occupied_record_id
                    .as_ref()
                    .and_then(|id| runtime.records.get(id))
                    .map(|record| record.summary.name.clone());
                RecordDevice {
                    id: output.id,
                    name: output.name,
                    is_default: output.is_default,
                    is_available: occupied_record_id.is_none(),
                    occupied_record_id,
                    occupied_record_name,
                }
            })
            .collect()
    }

    pub fn get_record(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        self.ensure_segments_loaded(record_id)?;
        self.snapshot(record_id)
    }

    pub fn create_record(&self, input: RecordInput) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        validate_record_config(&input.config)?;
        let id = Uuid::new_v4().to_string();
        let now = now_ms();
        let name = resolved_name(&input.name, &input.config);
        let history_limit = input.config.ui.history_limit;
        let summary = RecordSummary {
            id: id.clone(),
            name,
            state: RecordState::NotStarted,
            device_ids: input.config.audio.device_ids.clone(),
            active_device_ids: Vec::new(),
            has_active_source: false,
            last_error: None,
            created_at_ms: now,
            updated_at_ms: now,
            revision: 1,
        };
        let record = RecordRuntime {
            summary,
            config: input.config,
            sources: Vec::new(),
            active_sources: HashMap::new(),
            segments: RecordSubtitleStore::new(history_limit),
            segments_loaded: true,
            stats: RecordStats::default(),
            source_stats: HashMap::new(),
            notices: VecDeque::new(),
            started_at_ms: None,
            ended_at_ms: None,
            summary_revision: 1,
            subtitle_revision: 0,
            stats_revision: 0,
        };
        self.persist(&record.as_stored())?;
        {
            let mut runtime = self.runtime();
            runtime.records.insert(id.clone(), record);
        }
        self.emit_summary(&id);
        self.snapshot(&id)
    }

    pub fn update_record(
        &self,
        record_id: &str,
        input: RecordInput,
    ) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        validate_record_config(&input.config)?;
        let mut runtime = self.runtime();
        let record = runtime
            .records
            .get_mut(record_id)
            .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
        if record.summary.state.is_active() || record.summary.state == RecordState::Ended {
            return Err(
                "An active or ended record cannot be edited. Stop it first or create a new record."
                    .to_owned(),
            );
        }
        let name = resolved_name(&input.name, &input.config);
        record.summary.name = name;
        record.summary.device_ids = input.config.audio.device_ids.clone();
        record.config = input.config;
        record.summary.updated_at_ms = now_ms();
        record.summary.revision = record.summary.revision.saturating_add(1);
        record.summary_revision = record.summary_revision.saturating_add(1);
        let record = record.as_stored();
        drop(runtime);
        self.persist(&record)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn start_record(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.start_record_locked(record_id)
    }

    pub fn resume_record(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.start_record_locked(record_id)
    }

    pub fn pause_record(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.stop_record_sources(record_id)?;
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.summary.state == RecordState::Ended {
                return Ok(record_snapshot(record));
            }
            set_record_state(record, RecordState::Paused);
            record.summary.updated_at_ms = now_ms();
            record.as_stored()
        };
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn stop_record(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.stop_record_sources(record_id)?;
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.summary.state == RecordState::Ended {
                return Ok(record_snapshot(record));
            }
            set_record_state(record, RecordState::Ended);
            record.ended_at_ms = Some(now_ms());
            record.summary.updated_at_ms = now_ms();
            record.as_stored()
        };
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn add_device(&self, record_id: &str, device_id: &str) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        let name = device_name(device_id);
        let mut runtime = self.runtime();
        let is_active = {
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.summary.state == RecordState::Ended {
                return Err("An ended record cannot be changed.".to_owned());
            }
            if record
                .config
                .audio
                .device_ids
                .iter()
                .any(|id| id == device_id)
            {
                return Ok(record_snapshot(record));
            }
            record.summary.state.is_active()
        };
        if is_active {
            if let Some(owner) = runtime.device_claims.get(device_id) {
                if owner != record_id {
                    return Err(format!(
                        "The device `{}` is already in use by another record.",
                        name
                    ));
                }
            }
            runtime
                .device_claims
                .insert(device_id.to_owned(), record_id.to_owned());
            let (config, history_limit) = {
                let record = runtime
                    .records
                    .get_mut(record_id)
                    .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
                record.config.audio.device_ids.push(device_id.to_owned());
                record.summary.device_ids = record.config.audio.device_ids.clone();
                let config = record.config.clone();
                let history_limit = config.ui.history_limit;
                (config, history_limit)
            };
            let elapsed_base_ms = self
                .inner
                .store
                .device_elapsed_ms(record_id, device_id)
                .unwrap_or(0);
            let device = RecordSource {
                source_run_id: Uuid::new_v4().to_string(),
                record_id: record_id.to_owned(),
                device_id: device_id.to_owned(),
                device_name: name.clone(),
                state: SourceRunState::Starting,
                elapsed_base_ms,
                elapsed_ms: 0,
                started_at_ms: now_ms(),
                ended_at_ms: None,
                error: None,
            };
            if let Err(error) = self.inner.store.insert_source_run(&device) {
                runtime.device_claims.remove(device_id);
                if let Some(record) = runtime.records.get_mut(record_id) {
                    rollback_added_device(record, device_id);
                }
                drop(runtime);
                self.record_start_failure(record_id, &error)?;
                return Err(error);
            }
            let source_run_id = device.source_run_id.clone();
            let (completion_tx, completion_rx) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let (ready_tx, ready_rx) = mpsc::channel();
            let context = SourceContext {
                record_id: record_id.to_owned(),
                source_run_id: source_run_id.clone(),
                device_id: device_id.to_owned(),
                device_name: name.clone(),
                config,
                models: self.inner.models.clone(),
                elapsed_base_ms,
                occurrence_base_ms: now_ms(),
                history_limit,
                stop: stop.clone(),
                completion: completion_tx,
                events: self.inner.event_tx.clone(),
            };
            let handle = match source::spawn(context, ready_tx) {
                Ok(handle) => handle,
                Err(error) => {
                    runtime.device_claims.remove(device_id);
                    if let Some(record) = runtime.records.get_mut(record_id) {
                        let mut failed = device.clone();
                        failed.state = SourceRunState::Failed;
                        failed.ended_at_ms = Some(now_ms());
                        failed.error = Some(error.clone());
                        record.sources.push(failed);
                        rollback_added_device(record, device_id);
                    }
                    let _ = self.inner.store.finish_source_run(
                        &source_run_id,
                        SourceRunState::Failed,
                        now_ms(),
                        0,
                        Some(&error),
                    );
                    drop(runtime);
                    self.record_start_failure(record_id, &error)?;
                    return Err(error);
                }
            };
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record.sources.push(device.clone());
            record.active_sources.insert(
                source_run_id.clone(),
                ActiveRecordSource {
                    source: device,
                    stop: stop.clone(),
                    completion: Some(completion_rx),
                    handle: Some(handle),
                },
            );
            drop(runtime);
            match ready_rx.recv_timeout(SOURCE_STARTUP_TIMEOUT) {
                Ok(Ok(())) => {
                    let mut runtime = self.runtime();
                    if let Some(record) = runtime.records.get_mut(record_id) {
                        if let Some(source) = record.active_sources.get_mut(&source_run_id) {
                            source.source.state = SourceRunState::Running;
                        }
                        record.summary.state = RecordState::Running;
                        record.summary.updated_at_ms = now_ms();
                        sync_summary(record);
                    }
                    drop(runtime);
                    let _ = self.inner.store.update_source_state(
                        &source_run_id,
                        SourceRunState::Running,
                        None,
                    );
                    self.persist_current(record_id)?;
                    self.emit_summary(record_id);
                    return self.snapshot(record_id);
                }
                Ok(Err(error)) => {
                    self.cleanup_source_after_start_failure(
                        record_id,
                        &source_run_id,
                        device_id,
                        &error,
                    );
                    self.record_start_failure(record_id, &error)?;
                    return Err(error);
                }
                Err(_) => {
                    stop.store(true, Ordering::SeqCst);
                    let error = "The source did not become ready within 180 seconds.".to_owned();
                    self.cleanup_source_after_start_failure(
                        record_id,
                        &source_run_id,
                        device_id,
                        &error,
                    );
                    self.record_start_failure(record_id, &error)?;
                    return Err(error);
                }
            }
        }
        let record = runtime
            .records
            .get_mut(record_id)
            .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
        record.config.audio.device_ids.push(device_id.to_owned());
        record.summary.device_ids = record.config.audio.device_ids.clone();
        record.summary.updated_at_ms = now_ms();
        let persisted = record.as_stored();
        drop(runtime);
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn remove_device(
        &self,
        record_id: &str,
        device_id: &str,
    ) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.stop_sources_for_device(record_id, device_id)?;
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record.config.audio.device_ids.retain(|id| id != device_id);
            record.summary.device_ids = record.config.audio.device_ids.clone();
            record.summary.updated_at_ms = now_ms();
            if record.summary.state == RecordState::Running && record.active_sources.is_empty() {
                record.summary.state = RecordState::WaitingForDevice;
            }
            sync_summary(record);
            record.as_stored()
        };
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn subtitles(
        &self,
        record_id: &str,
        before: Option<SubtitleCursor>,
        limit: usize,
    ) -> Result<RecordSubtitlePage, String> {
        self.inner
            .store
            .list_subtitles(record_id, before.as_ref(), limit)
    }

    pub fn shutdown(&self) {
        let _operation = self.operations();
        let ids = {
            let runtime = self.runtime();
            runtime
                .records
                .values()
                .filter(|record| record.summary.state.is_active())
                .map(|record| record.summary.id.clone())
                .collect::<Vec<_>>()
        };
        for id in ids {
            let _ = self.stop_record_sources(&id);
            let persisted = {
                let mut runtime = self.runtime();
                runtime.records.get_mut(&id).map(|record| {
                    set_record_state(record, RecordState::Paused);
                    record.as_stored()
                })
            };
            if let Some(record) = persisted {
                let _ = self.persist(&record);
                self.emit_summary(&id);
            }
        }
        let _ = self.inner.event_tx.send(shutdown_event());
        let handle = self
            .inner
            .event_handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    fn start_record_locked(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        self.ensure_segments_loaded(record_id)?;
        let mut runtime = self.runtime();
        let (record_config, device_ids) = {
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            match record.summary.state {
                RecordState::Running | RecordState::Starting => {
                    return Ok(record_snapshot(record));
                }
                RecordState::Ended => {
                    return Err(
                        "This record has ended. Create a new record with the same settings."
                            .to_owned(),
                    );
                }
                RecordState::NotStarted
                | RecordState::Paused
                | RecordState::WaitingForDevice
                | RecordState::Failed => {}
            }
            validate_record_config(&record.config)?;
            if record.config.audio.device_ids.is_empty() {
                return Err("Select at least one system audio device.".to_owned());
            }
            (
                record.config.clone(),
                record.config.audio.device_ids.clone(),
            )
        };
        let devices = audio::system_outputs().map_err(|error| error.to_string())?;
        let device_names = devices
            .iter()
            .map(|device| (device.id.clone(), device.name.clone()))
            .collect::<HashMap<_, _>>();
        for device_id in &device_ids {
            if !device_names.contains_key(device_id) {
                return Err(format!(
                    "The selected audio device `{device_id}` is unavailable."
                ));
            }
            if runtime
                .device_claims
                .get(device_id)
                .is_some_and(|owner| owner != record_id)
            {
                return Err(format!(
                    "The device `{}` is already in use by another record.",
                    device_names
                        .get(device_id)
                        .cloned()
                        .unwrap_or_else(|| device_id.clone())
                ));
            }
        }
        for device_id in &device_ids {
            runtime
                .device_claims
                .insert(device_id.clone(), record_id.to_owned());
        }
        let now = now_ms();
        let history_limit = record_config.ui.history_limit;
        let (persisted, sources, spawn_failure) = {
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record.started_at_ms = record.started_at_ms.or(Some(now));
            record.ended_at_ms = None;
            record.summary.last_error = None;
            set_record_state(record, RecordState::Starting);
            record.summary.updated_at_ms = now;
            let mut sources = Vec::new();
            let mut spawn_failure = None;
            for device_id in &device_ids {
                let source_run_id = Uuid::new_v4().to_string();
                let device_name = device_names
                    .get(device_id)
                    .cloned()
                    .unwrap_or_else(|| device_id.clone());
                let elapsed_base_ms = self
                    .inner
                    .store
                    .device_elapsed_ms(record_id, device_id)
                    .unwrap_or(0);
                let source = RecordSource {
                    source_run_id,
                    record_id: record_id.to_owned(),
                    device_id: device_id.clone(),
                    device_name,
                    state: SourceRunState::Starting,
                    elapsed_base_ms,
                    elapsed_ms: 0,
                    started_at_ms: now,
                    ended_at_ms: None,
                    error: None,
                };
                if let Err(error) = self.inner.store.insert_source_run(&source) {
                    spawn_failure = Some(error);
                    break;
                }
                match self.spawn_source(record_id, &record_config, &source, history_limit) {
                    Ok((stop, completion, handle, ready_rx)) => {
                        record.sources.push(source.clone());
                        record.active_sources.insert(
                            source.source_run_id.clone(),
                            ActiveRecordSource {
                                source: source.clone(),
                                stop,
                                completion: Some(completion),
                                handle: Some(handle),
                            },
                        );
                        sources.push((source.source_run_id, ready_rx));
                    }
                    Err(error) => {
                        let _ = self.inner.store.finish_source_run(
                            &source.source_run_id,
                            SourceRunState::Failed,
                            now_ms(),
                            0,
                            Some(&error),
                        );
                        let mut failed = source.clone();
                        failed.state = SourceRunState::Failed;
                        failed.ended_at_ms = Some(now_ms());
                        failed.error = Some(error.clone());
                        record.sources.push(failed);
                        spawn_failure = Some(error);
                        break;
                    }
                }
            }
            (record.as_stored(), sources, spawn_failure)
        };
        if let Some(error) = spawn_failure {
            self.cleanup_record_sources(record_id, &error);
            self.release_record_claims(record_id);
            self.set_error(record_id, &error)?;
            return Err(error);
        }
        drop(runtime);
        self.persist(&persisted)?;
        self.emit_summary(record_id);

        for (source_run_id, ready_rx) in sources {
            match ready_rx.recv_timeout(SOURCE_STARTUP_TIMEOUT) {
                Ok(Ok(())) => {
                    let _ = self.inner.store.update_source_state(
                        &source_run_id,
                        SourceRunState::Running,
                        None,
                    );
                    let mut runtime = self.runtime();
                    if let Some(record) = runtime.records.get_mut(record_id) {
                        if let Some(source) = record.active_sources.get_mut(&source_run_id) {
                            source.source.state = SourceRunState::Running;
                        }
                    }
                }
                Ok(Err(error)) => {
                    self.cleanup_record_sources(record_id, &error);
                    self.release_record_claims(record_id);
                    self.set_error(record_id, &error)?;
                    return Err(error);
                }
                Err(_) => {
                    let error = "A source did not become ready within 180 seconds.".to_owned();
                    self.cleanup_record_sources(record_id, &error);
                    self.release_record_claims(record_id);
                    self.set_error(record_id, &error)?;
                    return Err(error);
                }
            }
        }
        let startup_error = {
            let runtime = self.runtime();
            let record = runtime
                .records
                .get(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.active_sources.len() == record.config.audio.device_ids.len()
                && record
                    .active_sources
                    .values()
                    .all(|source| source.source.state == SourceRunState::Running)
            {
                None
            } else {
                Some("One or more selected devices stopped during startup.".to_owned())
            }
        };
        if let Some(error) = startup_error {
            self.cleanup_record_sources(record_id, &error);
            self.release_record_claims(record_id);
            self.set_error(record_id, &error)?;
            return Err(error);
        }
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            set_record_state(record, RecordState::Running);
            record.summary.updated_at_ms = now_ms();
            sync_summary(record);
            record.as_stored()
        };
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    fn spawn_source(
        &self,
        record_id: &str,
        config: &AppConfig,
        source: &RecordSource,
        history_limit: usize,
    ) -> Result<SourceSpawn, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (completion_tx, completion_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let context = SourceContext {
            record_id: record_id.to_owned(),
            source_run_id: source.source_run_id.clone(),
            device_id: source.device_id.clone(),
            device_name: source.device_name.clone(),
            config: config.clone(),
            models: self.inner.models.clone(),
            elapsed_base_ms: source.elapsed_base_ms,
            occurrence_base_ms: source.started_at_ms,
            history_limit,
            stop: stop.clone(),
            completion: completion_tx,
            events: self.inner.event_tx.clone(),
        };
        let handle = source::spawn(context, ready_tx)?;
        Ok((stop, completion_rx, handle, ready_rx))
    }

    fn stop_record_sources(&self, record_id: &str) -> Result<(), String> {
        let stops = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            let mut stops = Vec::new();
            for source in record.active_sources.values_mut() {
                source.stop.store(true, Ordering::SeqCst);
                stops.push(SourceStop {
                    source_run_id: source.source.source_run_id.clone(),
                    device_id: source.source.device_id.clone(),
                    stop: source.stop.clone(),
                    record_id: record_id.to_owned(),
                    completion: source.completion.take(),
                    handle: source.handle.take(),
                });
            }
            stops
        };
        self.wait_for_stops(stops)?;
        Ok(())
    }

    fn stop_sources_for_device(&self, record_id: &str, device_id: &str) -> Result<(), String> {
        let stops = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            let mut stops = Vec::new();
            let source_ids = record
                .active_sources
                .iter()
                .filter(|(_, source)| source.source.device_id == device_id)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            for source_run_id in source_ids {
                if let Some(source) = record.active_sources.get_mut(&source_run_id) {
                    source.stop.store(true, Ordering::SeqCst);
                    stops.push(SourceStop {
                        source_run_id,
                        device_id: device_id.to_owned(),
                        stop: source.stop.clone(),
                        record_id: record_id.to_owned(),
                        completion: source.completion.take(),
                        handle: source.handle.take(),
                    });
                }
            }
            stops
        };
        self.wait_for_stops(stops)
    }

    fn wait_for_stops(&self, stops: Vec<SourceStop>) -> Result<(), String> {
        for mut stop in stops {
            stop.stop.store(true, Ordering::SeqCst);
            if let Some(completion) = stop.completion {
                let _ = completion.recv_timeout(SOURCE_STOP_TIMEOUT);
            }
            if let Some(handle) = stop.handle.take() {
                let _ = handle.join();
            }
            let mut runtime = self.runtime();
            if let Some(record) = runtime.records.get_mut(&stop.record_id) {
                if let Some(source) = record.active_sources.get_mut(&stop.source_run_id) {
                    source.handle = None;
                }
            }
            runtime.device_claims.remove(&stop.device_id);
        }
        Ok(())
    }

    fn cleanup_record_sources(&self, record_id: &str, error: &str) {
        let stops = {
            let mut runtime = self.runtime();
            let Some(record) = runtime.records.get_mut(record_id) else {
                return;
            };
            let mut stops = Vec::new();
            for source in record.active_sources.values_mut() {
                source.stop.store(true, Ordering::SeqCst);
                stops.push(SourceStop {
                    source_run_id: source.source.source_run_id.clone(),
                    device_id: source.source.device_id.clone(),
                    stop: source.stop.clone(),
                    record_id: record_id.to_owned(),
                    completion: source.completion.take(),
                    handle: source.handle.take(),
                });
            }
            stops
        };
        for mut stop in stops {
            stop.stop.store(true, Ordering::SeqCst);
            if let Some(completion) = stop.completion {
                let _ = completion.recv_timeout(Duration::from_secs(5));
            }
            if let Some(handle) = stop.handle.take() {
                let _ = handle.join();
            }
            let mut runtime = self.runtime();
            if let Some(record) = runtime.records.get_mut(&stop.record_id) {
                mark_source_failed(record, &stop.source_run_id, error);
                record.active_sources.remove(&stop.source_run_id);
            }
            runtime.device_claims.remove(&stop.device_id);
            let _ = self.inner.store.finish_source_run(
                &stop.source_run_id,
                SourceRunState::Failed,
                now_ms(),
                0,
                Some(error),
            );
        }
    }

    fn cleanup_source_after_start_failure(
        &self,
        record_id: &str,
        source_run_id: &str,
        device_id: &str,
        error: &str,
    ) {
        let source = {
            let mut runtime = self.runtime();
            runtime.device_claims.remove(device_id);
            runtime
                .records
                .get_mut(record_id)
                .and_then(|record| record.active_sources.remove(source_run_id))
        };
        if let Some(source) = source {
            source.stop.store(true, Ordering::SeqCst);
            if let Some(completion) = source.completion {
                let _ = completion.recv_timeout(Duration::from_secs(5));
            }
            if let Some(handle) = source.handle {
                let _ = handle.join();
            }
        }
        {
            let mut runtime = self.runtime();
            if let Some(record) = runtime.records.get_mut(record_id) {
                mark_source_failed(record, source_run_id, error);
                rollback_added_device(record, device_id);
            }
        }
        let _ = self.inner.store.finish_source_run(
            source_run_id,
            SourceRunState::Failed,
            now_ms(),
            0,
            Some(error),
        );
    }

    fn release_record_claims(&self, record_id: &str) {
        let mut runtime = self.runtime();
        runtime.device_claims.retain(|_, owner| owner != record_id);
    }

    fn set_error(&self, record_id: &str, error: &str) -> Result<(), String> {
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record.summary.state = RecordState::Failed;
            record.summary.last_error = Some(error.to_owned());
            record.summary.updated_at_ms = now_ms();
            sync_summary(record);
            record.as_stored()
        };
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        Ok(())
    }

    fn record_start_failure(&self, record_id: &str, error: &str) -> Result<(), String> {
        let has_active_source = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record.summary.last_error = Some(error.to_owned());
            record.summary.updated_at_ms = now_ms();
            record.summary_revision = record.summary_revision.saturating_add(1);
            sync_summary(record);
            !record.active_sources.is_empty()
        };
        if has_active_source {
            self.persist_current(record_id)?;
            self.emit_summary(record_id);
            handle_source_notice(
                &self.inner,
                record_id,
                "",
                RecordNotice {
                    level: "error".to_owned(),
                    message: error.to_owned(),
                    at_ms: now_ms(),
                },
            );
        } else {
            self.set_error(record_id, error)?;
        }
        Ok(())
    }

    fn persist_current(&self, record_id: &str) -> Result<(), String> {
        let persisted = {
            let runtime = self.runtime();
            runtime.records.get(record_id).map(RecordRuntime::as_stored)
        };
        if let Some(record) = persisted {
            self.persist(&record)?;
        }
        Ok(())
    }

    fn persist(&self, record: &StoredRecord) -> Result<(), String> {
        self.inner.store.save_record(record)?;
        Ok(())
    }

    fn snapshot(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        let mut snapshot = {
            let runtime = self.runtime();
            let record = runtime
                .records
                .get(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record_snapshot(record)
        };
        snapshot.total_subtitle_count = self.inner.store.subtitle_count(record_id)?;
        Ok(snapshot)
    }

    fn ensure_segments_loaded(&self, record_id: &str) -> Result<(), String> {
        let limit = {
            let runtime = self.runtime();
            let record = runtime
                .records
                .get(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.segments_loaded {
                return Ok(());
            }
            record.config.ui.history_limit
        };
        let segments = self.inner.store.list_recent_segments(record_id, limit)?;
        let mut runtime = self.runtime();
        if let Some(record) = runtime.records.get_mut(record_id) {
            record.segments.replace(segments);
            record.segments_loaded = true;
        }
        Ok(())
    }

    fn emit_summary(&self, record_id: &str) {
        let event = {
            let runtime = self.runtime();
            runtime.records.get(record_id).map(|record| {
                let mut summary = record.summary.clone();
                summary.revision = record.summary_revision;
                RecordSummaryEvent {
                    record_id: record_id.to_owned(),
                    revision: record.summary_revision,
                    summary,
                }
            })
        };
        if let Some(event) = event {
            let _ = self.inner.app.emit("record://summary", event);
        }
    }

    fn runtime(&self) -> std::sync::MutexGuard<'_, RecordRuntimeState> {
        self.inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn operations(&self) -> std::sync::MutexGuard<'_, ()> {
        self.inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl RecordRuntime {
    fn as_stored(&self) -> StoredRecord {
        StoredRecord {
            summary: self.summary.clone(),
            config: self.config.clone(),
            started_at_ms: self.started_at_ms,
            ended_at_ms: self.ended_at_ms,
        }
    }
}

fn load_records(store: &RecordStore) -> Result<HashMap<String, RecordRuntime>, String> {
    let stored = store.list_records()?;
    let mut records = HashMap::new();
    for stored_record in stored {
        let summary = stored_record.summary.clone();
        let sources = store.list_source_runs(&summary.id)?;
        let history_limit = stored_record.config.ui.history_limit;
        records.insert(
            summary.id.clone(),
            RecordRuntime {
                summary,
                config: stored_record.config,
                sources,
                active_sources: HashMap::new(),
                segments: RecordSubtitleStore::new(history_limit),
                segments_loaded: false,
                stats: RecordStats::default(),
                source_stats: HashMap::new(),
                notices: VecDeque::new(),
                started_at_ms: stored_record.started_at_ms,
                ended_at_ms: stored_record.ended_at_ms,
                summary_revision: 1,
                subtitle_revision: 0,
                stats_revision: 0,
            },
        );
    }
    Ok(records)
}

fn record_snapshot(record: &RecordRuntime) -> RecordSnapshot {
    RecordSnapshot {
        summary: record.summary.clone(),
        config: record.config.clone(),
        sources: merged_sources(record),
        segments: record.segments.snapshot(),
        stats: record.stats.clone(),
        notices: record.notices.iter().cloned().collect(),
        summary_revision: record.summary_revision,
        subtitle_revision: record.subtitle_revision,
        stats_revision: record.stats_revision,
        total_subtitle_count: record.segments.len() as u64,
        history_limit: record.config.ui.history_limit,
    }
}

fn merged_sources(record: &RecordRuntime) -> Vec<RecordSource> {
    let mut sources = record.sources.clone();
    for active in record.active_sources.values() {
        if let Some(source) = sources
            .iter_mut()
            .find(|source| source.source_run_id == active.source.source_run_id)
        {
            *source = active.source.clone();
        } else {
            sources.push(active.source.clone());
        }
    }
    sources
}

fn mark_source_failed(record: &mut RecordRuntime, source_run_id: &str, error: &str) {
    if let Some(source) = record
        .sources
        .iter_mut()
        .find(|source| source.source_run_id == source_run_id)
    {
        source.state = SourceRunState::Failed;
        source.ended_at_ms = Some(now_ms());
        source.error = Some(error.to_owned());
    }
}

fn rollback_added_device(record: &mut RecordRuntime, device_id: &str) {
    record.config.audio.device_ids.retain(|id| id != device_id);
    record.summary.device_ids = record.config.audio.device_ids.clone();
    record.summary.updated_at_ms = now_ms();
    sync_summary(record);
}

fn sync_summary(record: &mut RecordRuntime) {
    record.summary.device_ids = record.config.audio.device_ids.clone();
    record.summary.active_device_ids = record
        .active_sources
        .values()
        .filter(|source| source.source.state == SourceRunState::Running)
        .map(|source| source.source.device_id.clone())
        .collect();
    record.summary.has_active_source = !record.summary.active_device_ids.is_empty();
    record.summary.revision = record.summary_revision;
}

fn set_record_state(record: &mut RecordRuntime, state: RecordState) {
    record.summary.state = state;
    record.summary_revision = record.summary_revision.saturating_add(1);
    sync_summary(record);
}

fn run_event_loop(inner: Arc<RecordManagerInner>, events: Receiver<SourceEvent>) {
    while let Ok(event) = events.recv() {
        match event {
            SourceEvent::Subtitle {
                record_id,
                source_run_id,
                patches,
            } => handle_source_subtitles(&inner, &record_id, &source_run_id, patches),
            SourceEvent::Stats {
                record_id,
                source_run_id,
                stats,
            } => handle_source_stats(&inner, &record_id, &source_run_id, stats),
            SourceEvent::Notice {
                record_id,
                source_run_id,
                notice,
            } => handle_source_notice(&inner, &record_id, &source_run_id, notice),
            SourceEvent::Finished {
                record_id,
                source_run_id,
                device_id: _,
                outcome,
                ack,
            } => handle_source_finished(&inner, &record_id, &source_run_id, outcome, ack),
            SourceEvent::Shutdown => break,
        }
    }
}

fn handle_source_subtitles(
    inner: &Arc<RecordManagerInner>,
    record_id: &str,
    source_run_id: &str,
    patches: Vec<SubtitlePatch>,
) {
    let (event, changed, elapsed_ms) = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(record) = runtime.records.get_mut(record_id) else {
            return;
        };
        let changed = record.segments.apply(patches);
        if changed.is_empty() {
            return;
        }
        record.subtitle_revision = record.subtitle_revision.saturating_add(1);
        let mut elapsed_ms = 0;
        if let Some(source) = record.active_sources.get_mut(source_run_id) {
            for segment in &changed {
                let local_elapsed_ms = ((segment.device_time_secs * 1_000.0)
                    - source.source.elapsed_base_ms as f64)
                    .max(0.0)
                    .min(u64::MAX as f64) as u64;
                elapsed_ms = elapsed_ms.max(local_elapsed_ms);
            }
            source.source.elapsed_ms = source.source.elapsed_ms.max(elapsed_ms);
        }
        let event = RecordSubtitleBatch {
            record_id: record_id.to_owned(),
            source_run_id: source_run_id.to_owned(),
            revision: record.subtitle_revision,
            patches: changed
                .iter()
                .map(|segment| SubtitlePatch::Finalize {
                    segment: segment.clone(),
                })
                .collect(),
        };
        (event, changed, elapsed_ms)
    };
    for segment in &changed {
        if let Err(error) = inner.store.upsert_subtitle(segment) {
            push_record_error(inner, record_id, error);
        }
    }
    if elapsed_ms > 0 {
        let _ = inner.store.touch_source_elapsed(source_run_id, elapsed_ms);
    }
    let _ = inner.app.emit("record://subtitles", event);
}

fn handle_source_stats(
    inner: &Arc<RecordManagerInner>,
    record_id: &str,
    source_run_id: &str,
    stats: RecordStats,
) {
    let event = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(record) = runtime.records.get_mut(record_id) else {
            return;
        };
        record.source_stats.insert(source_run_id.to_owned(), stats);
        record.stats = aggregate_stats(record);
        record.stats_revision = record.stats_revision.saturating_add(1);
        RecordStatsEvent {
            record_id: record_id.to_owned(),
            revision: record.stats_revision,
            stats: record.stats.clone(),
        }
    };
    let _ = inner.app.emit("record://stats", event);
}

fn handle_source_notice(
    inner: &Arc<RecordManagerInner>,
    record_id: &str,
    _source_run_id: &str,
    notice: RecordNotice,
) {
    let event = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(record) = runtime.records.get_mut(record_id) else {
            return;
        };
        let duplicate = record.notices.back().is_some_and(|last| {
            last.level == notice.level
                && last.message == notice.message
                && notice.at_ms.saturating_sub(last.at_ms) < 2_000
        });
        if duplicate {
            return;
        }
        record.notices.push_back(notice.clone());
        while record.notices.len() > NOTICE_LIMIT {
            record.notices.pop_front();
        }
        RecordNoticeEvent {
            record_id: record_id.to_owned(),
            notice,
        }
    };
    let _ = inner.app.emit("record://notice", event);
}

fn handle_source_finished(
    inner: &Arc<RecordManagerInner>,
    record_id: &str,
    source_run_id: &str,
    outcome: SourceOutcome,
    ack: mpsc::Sender<()>,
) {
    let (persisted, event, source_state, error, handle) = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let claimed_device_id = runtime
            .records
            .get(record_id)
            .and_then(|record| record.active_sources.get(source_run_id))
            .map(|source| source.source.device_id.clone());
        if let Some(device_id) = &claimed_device_id {
            if runtime
                .device_claims
                .get(device_id)
                .is_some_and(|owner| owner == record_id)
            {
                runtime.device_claims.remove(device_id);
            }
        }
        let Some(record) = runtime.records.get_mut(record_id) else {
            let _ = ack.send(());
            return;
        };
        let source_state = match outcome.reason {
            SourceEndReason::DeviceLost => SourceRunState::Disconnected,
            SourceEndReason::PipelineFailure => SourceRunState::Failed,
            _ => SourceRunState::Ended,
        };
        let now = now_ms();
        let error = outcome.failure.clone();
        let source = record.active_sources.remove(source_run_id);
        let (handle, active_elapsed_ms) = match source {
            Some(source) => (source.handle, source.source.elapsed_ms),
            None => (None, 0),
        };
        if let Some(source) = record
            .sources
            .iter_mut()
            .find(|source| source.source_run_id == source_run_id)
        {
            source.state = source_state;
            source.ended_at_ms = Some(now);
            source.elapsed_ms = source
                .elapsed_ms
                .max(active_elapsed_ms)
                .max(outcome.captured_ms);
            source.error = error.clone();
        }
        if record.summary.state != RecordState::Paused && record.summary.state != RecordState::Ended
        {
            if error.is_some() {
                record.summary.last_error = error.clone();
            }
            if record.active_sources.is_empty() {
                let next_state = if outcome.reason == SourceEndReason::PipelineFailure {
                    RecordState::Failed
                } else {
                    RecordState::WaitingForDevice
                };
                if record.summary.state != next_state {
                    set_record_state(record, next_state);
                }
            } else {
                if record.summary.state == RecordState::WaitingForDevice
                    || record.summary.state == RecordState::Failed
                {
                    set_record_state(record, RecordState::Running);
                }
            }
        }
        sync_summary(record);
        let persisted = record.as_stored();
        let event = RecordSummaryEvent {
            record_id: record_id.to_owned(),
            revision: record.summary_revision,
            summary: record.summary.clone(),
        };
        (persisted, event, source_state, error, handle)
    };
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    let _ = inner.store.save_record(&persisted);
    let _ = inner.store.finish_source_run(
        source_run_id,
        source_state,
        now_ms(),
        outcome.captured_ms,
        error.as_deref(),
    );
    let _ = inner.app.emit("record://summary", event);
    let _ = ack.send(());
}

fn aggregate_stats(record: &RecordRuntime) -> RecordStats {
    let mut aggregate = RecordStats {
        started_at_ms: record.started_at_ms.unwrap_or(0),
        active_sources: record.summary.active_device_ids.len(),
        ..RecordStats::default()
    };
    for stats in record.source_stats.values() {
        aggregate.audio_frames = aggregate.audio_frames.saturating_add(stats.audio_frames);
        aggregate.dropped_frames = aggregate
            .dropped_frames
            .saturating_add(stats.dropped_frames);
        aggregate.partial_updates = aggregate
            .partial_updates
            .saturating_add(stats.partial_updates);
        aggregate.finalized_subtitles = aggregate
            .finalized_subtitles
            .saturating_add(stats.finalized_subtitles);
        aggregate.translation_requests = aggregate
            .translation_requests
            .saturating_add(stats.translation_requests);
        aggregate.translation_failures = aggregate
            .translation_failures
            .saturating_add(stats.translation_failures);
        aggregate.speaker_updates = aggregate
            .speaker_updates
            .saturating_add(stats.speaker_updates);
        aggregate.diarization_windows = aggregate
            .diarization_windows
            .saturating_add(stats.diarization_windows);
        aggregate.audio_peak = aggregate.audio_peak.max(stats.audio_peak);
        aggregate.audio_rms = aggregate.audio_rms.max(stats.audio_rms);
    }
    aggregate
}

fn push_record_error(inner: &Arc<RecordManagerInner>, record_id: &str, error: String) {
    let event = {
        let mut runtime = inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(record) = runtime.records.get_mut(record_id) else {
            return;
        };
        record.summary.last_error = Some(error.clone());
        let notice = RecordNotice {
            level: "error".to_owned(),
            message: error,
            at_ms: now_ms(),
        };
        record.notices.push_back(notice.clone());
        RecordNoticeEvent {
            record_id: record_id.to_owned(),
            notice,
        }
    };
    let _ = inner.app.emit("record://notice", event);
}

fn validate_record_config(config: &AppConfig) -> Result<(), String> {
    config.validate()?;
    if config.audio.device_ids.is_empty() {
        return Err("Select at least one system audio device.".to_owned());
    }
    let mut ids = std::collections::HashSet::new();
    for id in &config.audio.device_ids {
        if !ids.insert(id) {
            return Err(format!("Device `{id}` is selected more than once."));
        }
    }
    let model = TRANSLATION_MODELS
        .iter()
        .find(|model| model.id == config.translation.model_id)
        .ok_or_else(|| {
            format!(
                "Unknown translation model `{}`.",
                config.translation.model_id
            )
        })?;
    if config.translation.engine != BERGAMOT_ENGINE_ID {
        return Err(format!(
            "Unknown translation engine `{}`. The only compiled adapter is `{BERGAMOT_ENGINE_ID}`.",
            config.translation.engine
        ));
    }
    if model.target_language != config.translation.target_language {
        return Err(format!(
            "Translation model `{}` targets `{}`, not `{}`.",
            model.label, model.target_language, config.translation.target_language
        ));
    }
    Ok(())
}

fn resolved_name(name: &str, config: &AppConfig) -> String {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_owned();
    }
    config
        .audio
        .device_ids
        .first()
        .map(|id| device_name(id))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Untitled record".to_owned())
}

fn device_name(device_id: &str) -> String {
    audio::system_outputs()
        .ok()
        .and_then(|devices| {
            devices
                .into_iter()
                .find(|device| device.id == device_id)
                .map(|device| device.name)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| device_id.to_owned())
}

fn shutdown_event() -> SourceEvent {
    SourceEvent::Shutdown
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn list_records(state: State<'_, RecordManager>) -> Vec<RecordSummary> {
    state.list_records()
}

#[tauri::command]
pub fn list_record_devices(state: State<'_, RecordManager>) -> Vec<RecordDevice> {
    state.list_devices()
}

#[tauri::command]
pub fn get_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    state.get_record(&record_id)
}

#[tauri::command]
pub fn create_record(
    state: State<'_, RecordManager>,
    input: RecordInput,
) -> Result<RecordSnapshot, String> {
    state.create_record(input)
}

#[tauri::command]
pub fn update_record(
    state: State<'_, RecordManager>,
    record_id: String,
    input: RecordInput,
) -> Result<RecordSnapshot, String> {
    state.update_record(&record_id, input)
}

#[tauri::command]
pub async fn start_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.start_record(&record_id))
        .await
        .map_err(|error| format!("Record start failed: {error}"))?
}

#[tauri::command]
pub async fn pause_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.pause_record(&record_id))
        .await
        .map_err(|error| format!("Record pause failed: {error}"))?
}

#[tauri::command]
pub async fn resume_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.resume_record(&record_id))
        .await
        .map_err(|error| format!("Record resume failed: {error}"))?
}

#[tauri::command]
pub async fn stop_record(
    state: State<'_, RecordManager>,
    record_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop_record(&record_id))
        .await
        .map_err(|error| format!("Record stop failed: {error}"))?
}

#[tauri::command]
pub async fn add_record_device(
    state: State<'_, RecordManager>,
    record_id: String,
    device_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.add_device(&record_id, &device_id))
        .await
        .map_err(|error| format!("Record device add failed: {error}"))?
}

#[tauri::command]
pub async fn remove_record_device(
    state: State<'_, RecordManager>,
    record_id: String,
    device_id: String,
) -> Result<RecordSnapshot, String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.remove_device(&record_id, &device_id))
        .await
        .map_err(|error| format!("Record device removal failed: {error}"))?
}

#[tauri::command]
pub fn list_record_subtitles(
    state: State<'_, RecordManager>,
    record_id: String,
    before: Option<SubtitleCursor>,
    limit: usize,
) -> Result<RecordSubtitlePage, String> {
    state.subtitles(&record_id, before, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_with_added_device() -> RecordRuntime {
        let mut config = AppConfig::default();
        config.audio.device_ids = vec!["device-a".to_owned(), "device-b".to_owned()];
        let source = RecordSource {
            source_run_id: "run-b".to_owned(),
            record_id: "record".to_owned(),
            device_id: "device-b".to_owned(),
            device_name: "Device B".to_owned(),
            state: SourceRunState::Starting,
            elapsed_base_ms: 0,
            elapsed_ms: 0,
            started_at_ms: 1,
            ended_at_ms: None,
            error: None,
        };
        RecordRuntime {
            summary: RecordSummary {
                id: "record".to_owned(),
                name: "Record".to_owned(),
                state: RecordState::Running,
                device_ids: config.audio.device_ids.clone(),
                active_device_ids: vec!["device-a".to_owned()],
                has_active_source: true,
                last_error: None,
                created_at_ms: 1,
                updated_at_ms: 1,
                revision: 1,
            },
            config,
            sources: vec![source],
            active_sources: HashMap::new(),
            segments: RecordSubtitleStore::new(400),
            segments_loaded: true,
            stats: RecordStats::default(),
            source_stats: HashMap::new(),
            notices: VecDeque::new(),
            started_at_ms: Some(1),
            ended_at_ms: None,
            summary_revision: 1,
            subtitle_revision: 0,
            stats_revision: 0,
        }
    }

    #[test]
    fn record_config_rejects_an_incompatible_translation_model() {
        let mut config = AppConfig::default();
        config.audio.device_ids = vec!["device".to_owned()];
        config.translation.target_language = "zh-Hans".to_owned();
        config.translation.model_id = "bergamotZhEn".to_owned();
        assert!(validate_record_config(&config).is_err());
    }

    #[test]
    fn a_failed_added_device_is_kept_in_history_but_removed_from_the_selection() {
        let mut record = runtime_with_added_device();

        mark_source_failed(&mut record, "run-b", "capture failed");
        rollback_added_device(&mut record, "device-b");

        assert_eq!(record.sources[0].state, SourceRunState::Failed);
        assert_eq!(record.sources[0].error.as_deref(), Some("capture failed"));
        assert_eq!(record.config.audio.device_ids, vec!["device-a"]);
        assert_eq!(record.summary.device_ids, vec!["device-a"]);
    }
}
