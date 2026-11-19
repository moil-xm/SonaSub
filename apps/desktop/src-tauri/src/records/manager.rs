use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::bounded;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{
    config::AppConfig,
    models::{validate_asr_config, ModelPaths, BERGAMOT_ENGINE_ID, TRANSLATION_MODELS},
};

use super::{
    devices::device_name,
    events::{handle_source_notice, run_event_loop, shutdown_event, RECORD_EVENT_QUEUE_DEPTH},
    persistence::RecordStore,
    runtime::{
        load_records, now_ms, record_snapshot, set_record_state, sync_summary, RecordManagerInner,
        RecordRuntime, RecordRuntimeState,
    },
    subtitles::RecordSubtitleStore,
    types::{
        RecordInput, RecordNotice, RecordSnapshot, RecordState, RecordStats, RecordSubtitlePage,
        RecordSummary, RecordSummaryEvent, StoredRecord, SubtitleCursor,
    },
};

#[derive(Clone)]
pub struct RecordManager {
    pub(super) inner: Arc<RecordManagerInner>,
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

    pub(super) fn set_error(&self, record_id: &str, error: &str) -> Result<(), String> {
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

    pub(super) fn record_start_failure(&self, record_id: &str, error: &str) -> Result<(), String> {
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

    pub(super) fn persist_current(&self, record_id: &str) -> Result<(), String> {
        let persisted = {
            let runtime = self.runtime();
            runtime.records.get(record_id).map(RecordRuntime::as_stored)
        };
        if let Some(record) = persisted {
            self.persist(&record)?;
        }
        Ok(())
    }

    pub(super) fn persist(&self, record: &StoredRecord) -> Result<(), String> {
        self.inner.store.save_record(record)?;
        Ok(())
    }

    pub(super) fn snapshot(&self, record_id: &str) -> Result<RecordSnapshot, String> {
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

    pub(super) fn ensure_segments_loaded(&self, record_id: &str) -> Result<(), String> {
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

    pub(super) fn emit_summary(&self, record_id: &str) {
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

    pub(super) fn runtime(&self) -> std::sync::MutexGuard<'_, RecordRuntimeState> {
        self.inner
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn operations(&self) -> std::sync::MutexGuard<'_, ()> {
        self.inner
            .operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(super) fn validate_record_config(config: &AppConfig) -> Result<(), String> {
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
    validate_asr_config(
        &config.recognition.model_id,
        config.recognition.mode,
        &config.recognition.source_language,
    )?;
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

#[cfg(test)]
mod tests {
    use super::validate_record_config;
    use crate::{
        config::{AppConfig, AsrMode},
        models::{ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID},
    };

    #[test]
    fn record_config_rejects_an_incompatible_translation_model() {
        let mut config = AppConfig::default();
        config.audio.device_ids = vec!["device".to_owned()];
        config.translation.target_language = "zh-Hans".to_owned();
        config.translation.model_id = "bergamotZhEn".to_owned();
        assert!(validate_record_config(&config).is_err());
    }

    #[test]
    fn record_config_accepts_qwen3_and_rejects_fake_streaming() {
        for model_id in [ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID] {
            let mut config = AppConfig::default();
            config.audio.device_ids = vec!["device".to_owned()];
            config.recognition.model_id = model_id.to_owned();
            config.recognition.mode = AsrMode::Segmented;
            validate_record_config(&config).unwrap();
            config.recognition.mode = AsrMode::Streaming;
            assert!(validate_record_config(&config).is_err());
        }
    }
}
