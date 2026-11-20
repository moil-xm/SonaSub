use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use uuid::Uuid;

use crate::audio::{self, AudioDeviceRef, AudioSourceKind};

use super::{
    lifecycle::SOURCE_STARTUP_TIMEOUT,
    manager::RecordManager,
    runtime::{now_ms, record_snapshot, rollback_added_device, sync_summary, ActiveRecordSource},
    source::{self, SourceContext},
    types::{RecordDevice, RecordSnapshot, RecordSource, RecordState, SourceRunState},
};

impl RecordManager {
    pub fn list_devices(&self) -> Vec<RecordDevice> {
        let mut devices = Vec::new();
        for kind in [AudioSourceKind::SystemOutput, AudioSourceKind::Microphone] {
            let Ok(entries) = audio::devices(kind) else {
                continue;
            };
            devices.extend(entries);
        }
        let runtime = self.runtime();
        devices
            .into_iter()
            .map(|device| {
                let device_ref = device.device_ref();
                let occupied_record_id = runtime.claim(&device_ref).map(str::to_owned);
                let occupied_record_name = occupied_record_id
                    .as_ref()
                    .and_then(|id| runtime.records.get(id))
                    .map(|record| record.summary.name.clone());
                RecordDevice {
                    kind: device.kind,
                    id: device.id,
                    name: device.name,
                    is_default: device.is_default,
                    capture_supported: device.capture_supported,
                    capture_reason: device.capture_reason,
                    is_available: occupied_record_id.is_none(),
                    occupied_record_id,
                    occupied_record_name,
                }
            })
            .collect()
    }

    pub fn add_device(
        &self,
        record_id: &str,
        device: AudioDeviceRef,
    ) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        let is_active = {
            let runtime = self.runtime();
            let record = runtime
                .records
                .get(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            if record.summary.state == RecordState::Ended {
                return Err("An ended record cannot be changed.".to_owned());
            }
            if record.config.audio.record.contains(&device) {
                return Ok(record_snapshot(record));
            }
            record.summary.state.is_active()
        };
        // Device enumeration can take a while on Core Audio; keep it outside
        // the runtime lock so record events are not blocked meanwhile.
        let name = selectable_device_name(&device)?;
        let mut runtime = self.runtime();
        if is_active {
            if let Some(owner) = runtime.claim(&device) {
                if owner != record_id {
                    return Err(format!(
                        "The {} `{}` is already in use by another record.",
                        device.kind.label().to_lowercase(),
                        name
                    ));
                }
            }
            let (config, history_limit) = {
                let record = runtime
                    .records
                    .get_mut(record_id)
                    .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
                record.config.audio.record.push(device.clone());
                sync_summary(record);
                let config = record.config.clone();
                let history_limit = config.ui.history_limit;
                (config, history_limit)
            };
            let elapsed_base_ms = self
                .inner
                .store
                .device_elapsed_ms(record_id, &device)
                .unwrap_or(0);
            let source = RecordSource {
                source_run_id: Uuid::new_v4().to_string(),
                record_id: record_id.to_owned(),
                source_kind: device.kind,
                device_id: device.id.clone(),
                device_name: name.clone(),
                state: SourceRunState::Starting,
                elapsed_base_ms,
                elapsed_ms: 0,
                started_at_ms: now_ms(),
                ended_at_ms: None,
                error: None,
            };
            if let Err(error) = self.inner.store.insert_source_run(&source) {
                if let Some(record) = runtime.records.get_mut(record_id) {
                    rollback_added_device(record, &device);
                }
                drop(runtime);
                self.record_start_failure(record_id, &error)?;
                return Err(error);
            }
            let source_run_id = source.source_run_id.clone();
            let (completion_tx, completion_rx) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let (ready_tx, ready_rx) = mpsc::channel();
            let context = SourceContext {
                record_id: record_id.to_owned(),
                source_run_id: source_run_id.clone(),
                source_kind: device.kind,
                device_id: device.id.clone(),
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
                    if let Some(record) = runtime.records.get_mut(record_id) {
                        let mut failed = source.clone();
                        failed.state = SourceRunState::Failed;
                        failed.ended_at_ms = Some(now_ms());
                        failed.error = Some(error.clone());
                        record.sources.push(failed);
                        rollback_added_device(record, &device);
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
            record.sources.push(source.clone());
            record.active_sources.insert(
                source_run_id.clone(),
                ActiveRecordSource {
                    source,
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
                        &device,
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
                        &device,
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
        record.config.audio.record.push(device);
        record.summary.updated_at_ms = now_ms();
        sync_summary(record);
        let persisted = record.as_stored();
        drop(runtime);
        self.persist(&persisted)?;
        self.emit_summary(record_id);
        self.snapshot(record_id)
    }

    pub fn remove_device(
        &self,
        record_id: &str,
        device: &AudioDeviceRef,
    ) -> Result<RecordSnapshot, String> {
        let _operation = self.operations();
        self.stop_sources_for_device(record_id, device)?;
        let persisted = {
            let mut runtime = self.runtime();
            let record = runtime
                .records
                .get_mut(record_id)
                .ok_or_else(|| format!("Unknown record `{record_id}`."))?;
            record
                .config
                .audio
                .record
                .retain(|selected| selected != device);
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

fn find_device(device: &AudioDeviceRef) -> Result<Option<audio::AudioDevice>, String> {
    Ok(audio::devices(device.kind)?
        .into_iter()
        .find(|candidate| candidate.id == device.id))
}

/// Resolves the display name for an already selected device without requiring
/// the device to be capturable; records keep their stored selection visible
/// even when the device is temporarily unplugged.
pub(super) fn device_name(device: &AudioDeviceRef) -> String {
    find_device(device)
        .ok()
        .flatten()
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| device.id.clone())
}

/// Resolves the display name of a device that can actually be opened. Adding a
/// source to a running record must fail here rather than after the record has
/// already claimed the device and persisted it.
fn selectable_device_name(device: &AudioDeviceRef) -> Result<String, String> {
    let entry = find_device(device)?.ok_or_else(|| {
        format!(
            "The selected {} device `{}` is unavailable.",
            device.kind.label().to_lowercase(),
            device.id
        )
    })?;
    if !entry.capture_supported {
        return Err(entry.capture_reason.unwrap_or_else(|| {
            format!(
                "The selected {} device `{}` cannot be captured by this build.",
                device.kind.label().to_lowercase(),
                device.id
            )
        }));
    }
    Ok(if entry.name.is_empty() {
        device.id.clone()
    } else {
        entry.name
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use crate::{
        audio::{AudioDeviceRef, AudioSourceKind},
        config::AppConfig,
    };

    use super::super::{
        runtime::{mark_source_failed, rollback_added_device, RecordRuntime},
        subtitles::RecordSubtitleStore,
        types::{RecordSource, RecordState, RecordStats, RecordSummary, SourceRunState},
    };

    fn device(id: &str) -> AudioDeviceRef {
        AudioDeviceRef::new(AudioSourceKind::SystemOutput, id)
    }

    fn runtime_with_added_device() -> RecordRuntime {
        let mut config = AppConfig::default();
        config.audio.record = vec![device("device-a"), device("device-b")];
        let source = RecordSource {
            source_run_id: "run-b".to_owned(),
            record_id: "record".to_owned(),
            source_kind: AudioSourceKind::SystemOutput,
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
                devices: config.audio.record.clone(),
                active_devices: vec![device("device-a")],
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
        rollback_added_device(&mut record, &device("device-b"));

        assert_eq!(record.sources[0].state, SourceRunState::Failed);
        assert_eq!(record.sources[0].error.as_deref(), Some("capture failed"));
        assert_eq!(record.config.audio.record, vec![device("device-a")]);
        assert_eq!(record.summary.devices, vec![device("device-a")]);
    }
}
