use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use uuid::Uuid;

use crate::audio;

use super::{
    lifecycle::SOURCE_STARTUP_TIMEOUT,
    manager::RecordManager,
    runtime::{now_ms, record_snapshot, rollback_added_device, sync_summary, ActiveRecordSource},
    source::{self, SourceContext},
    types::{RecordDevice, RecordSnapshot, RecordSource, RecordState, SourceRunState},
};

impl RecordManager {
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
}

pub(super) fn device_name(device_id: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use crate::config::AppConfig;

    use super::super::{
        runtime::{mark_source_failed, rollback_added_device, RecordRuntime},
        subtitles::RecordSubtitleStore,
        types::{RecordSource, RecordState, RecordStats, RecordSummary, SourceRunState},
    };

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
