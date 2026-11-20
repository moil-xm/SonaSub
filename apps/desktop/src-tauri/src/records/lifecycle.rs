use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use uuid::Uuid;

use crate::{
    audio::{self, AudioDeviceRef, AudioSourceKind},
    config::AppConfig,
};

use super::{
    manager::{validate_record_config, RecordManager},
    runtime::{
        mark_source_failed, now_ms, record_snapshot, rollback_added_device, set_record_state,
        sync_summary, ActiveRecordSource,
    },
    source::{self, SourceContext},
    types::{RecordSnapshot, RecordSource, RecordState, SourceRunState},
};

pub(super) const SOURCE_STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const SOURCE_STOP_TIMEOUT: Duration = Duration::from_secs(300);

type SourceSpawn = (
    Arc<AtomicBool>,
    mpsc::Receiver<()>,
    JoinHandle<()>,
    mpsc::Receiver<Result<(), String>>,
);

struct SourceStop {
    record_id: String,
    source_run_id: String,
    device: AudioDeviceRef,
    stop: Arc<AtomicBool>,
    completion: Option<mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
}

impl RecordManager {
    pub(super) fn start_record_locked(&self, record_id: &str) -> Result<RecordSnapshot, String> {
        self.ensure_segments_loaded(record_id)?;
        let mut runtime = self.runtime();
        let (record_config, devices) = {
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
            validate_record_config(&mut record.config)?;
            if record.config.audio.record.is_empty() {
                return Err("Select at least one audio source for the record.".to_owned());
            }
            (record.config.clone(), record.config.audio.record.clone())
        };
        let available = enumerate_devices();
        let device_names = available
            .iter()
            .map(|device| (device.device_ref(), device.name.clone()))
            .collect::<HashMap<_, _>>();
        let capture_support = available
            .iter()
            .map(|device| {
                (
                    device.device_ref(),
                    (device.capture_supported, device.capture_reason.clone()),
                )
            })
            .collect::<HashMap<_, _>>();
        for device in &devices {
            if !device_names.contains_key(device) {
                return Err(format!(
                    "The selected {} device `{}` is unavailable.",
                    device.kind.label().to_lowercase(),
                    device.id
                ));
            }
            if let Some((false, reason)) = capture_support.get(device) {
                return Err(reason.clone().unwrap_or_else(|| {
                    format!(
                        "The selected {} device `{}` cannot be captured by this build.",
                        device.kind.label().to_lowercase(),
                        device.id
                    )
                }));
            }
            if runtime
                .device_claims
                .get(device)
                .is_some_and(|owner| owner != record_id)
            {
                return Err(format!(
                    "The {} `{}` is already in use by another record.",
                    device.kind.label().to_lowercase(),
                    device_names
                        .get(device)
                        .cloned()
                        .unwrap_or_else(|| device.id.clone())
                ));
            }
        }
        for device in &devices {
            runtime
                .device_claims
                .insert(device.clone(), record_id.to_owned());
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
            for device in &devices {
                let source_run_id = Uuid::new_v4().to_string();
                let device_name = device_names
                    .get(device)
                    .cloned()
                    .unwrap_or_else(|| device.id.clone());
                let elapsed_base_ms = self
                    .inner
                    .store
                    .device_elapsed_ms(record_id, device)
                    .unwrap_or(0);
                let source = RecordSource {
                    source_run_id,
                    record_id: record_id.to_owned(),
                    source_kind: device.kind,
                    device_id: device.id.clone(),
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
            if record.active_sources.len() == record.config.audio.record.len()
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
            source_kind: source.source_kind,
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

    pub(super) fn stop_record_sources(&self, record_id: &str) -> Result<(), String> {
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
                    device: source.source.device_ref(),
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

    pub(super) fn stop_sources_for_device(
        &self,
        record_id: &str,
        device: &AudioDeviceRef,
    ) -> Result<(), String> {
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
                .filter(|(_, source)| source.source.device_ref() == *device)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            for source_run_id in source_ids {
                if let Some(source) = record.active_sources.get_mut(&source_run_id) {
                    source.stop.store(true, Ordering::SeqCst);
                    stops.push(SourceStop {
                        source_run_id,
                        device: device.clone(),
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
            runtime.device_claims.remove(&stop.device);
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
                    device: source.source.device_ref(),
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
            runtime.device_claims.remove(&stop.device);
            let _ = self.inner.store.finish_source_run(
                &stop.source_run_id,
                SourceRunState::Failed,
                now_ms(),
                0,
                Some(error),
            );
        }
    }

    pub(super) fn cleanup_source_after_start_failure(
        &self,
        record_id: &str,
        source_run_id: &str,
        device: &AudioDeviceRef,
        error: &str,
    ) {
        let source = {
            let mut runtime = self.runtime();
            runtime.device_claims.remove(device);
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
                rollback_added_device(record, device);
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

    pub(super) fn release_record_claims(&self, record_id: &str) {
        let mut runtime = self.runtime();
        runtime.device_claims.retain(|_, owner| owner != record_id);
    }
}

fn enumerate_devices() -> Vec<audio::AudioDevice> {
    let mut devices = Vec::new();
    for kind in [AudioSourceKind::SystemOutput, AudioSourceKind::Microphone] {
        devices.extend(audio::devices(kind).unwrap_or_default());
    }
    devices
}
