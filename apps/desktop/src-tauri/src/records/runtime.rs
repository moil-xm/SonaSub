use std::{
    collections::{HashMap, VecDeque},
    sync::{atomic::AtomicBool, mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::{SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::Sender;
use tauri::AppHandle;

use crate::{config::AppConfig, models::ModelPaths};

use super::{
    source::SourceEvent,
    store::RecordStore,
    subtitles::RecordSubtitleStore,
    types::{
        RecordNotice, RecordSnapshot, RecordSource, RecordState, RecordStats, RecordSummary,
        SourceRunState, StoredRecord,
    },
};

pub(super) struct RecordManagerInner {
    pub(super) app: AppHandle,
    pub(super) models: ModelPaths,
    pub(super) store: RecordStore,
    pub(super) runtime: Mutex<RecordRuntimeState>,
    pub(super) operations: Mutex<()>,
    pub(super) event_tx: Sender<SourceEvent>,
    pub(super) event_handle: Mutex<Option<JoinHandle<()>>>,
}

pub(super) struct RecordRuntimeState {
    pub(super) records: HashMap<String, RecordRuntime>,
    pub(super) device_claims: HashMap<String, String>,
}

pub(super) struct RecordRuntime {
    pub(super) summary: RecordSummary,
    pub(super) config: AppConfig,
    pub(super) sources: Vec<RecordSource>,
    pub(super) active_sources: HashMap<String, ActiveRecordSource>,
    pub(super) segments: RecordSubtitleStore,
    pub(super) segments_loaded: bool,
    pub(super) stats: RecordStats,
    pub(super) source_stats: HashMap<String, RecordStats>,
    pub(super) notices: VecDeque<RecordNotice>,
    pub(super) started_at_ms: Option<u64>,
    pub(super) ended_at_ms: Option<u64>,
    pub(super) summary_revision: u64,
    pub(super) subtitle_revision: u64,
    pub(super) stats_revision: u64,
}

pub(super) struct ActiveRecordSource {
    pub(super) source: RecordSource,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) completion: Option<mpsc::Receiver<()>>,
    pub(super) handle: Option<JoinHandle<()>>,
}

impl RecordRuntime {
    pub(super) fn as_stored(&self) -> StoredRecord {
        StoredRecord {
            summary: self.summary.clone(),
            config: self.config.clone(),
            started_at_ms: self.started_at_ms,
            ended_at_ms: self.ended_at_ms,
        }
    }
}

pub(super) fn load_records(store: &RecordStore) -> Result<HashMap<String, RecordRuntime>, String> {
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

pub(super) fn record_snapshot(record: &RecordRuntime) -> RecordSnapshot {
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

pub(super) fn mark_source_failed(record: &mut RecordRuntime, source_run_id: &str, error: &str) {
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

pub(super) fn rollback_added_device(record: &mut RecordRuntime, device_id: &str) {
    record.config.audio.device_ids.retain(|id| id != device_id);
    record.summary.device_ids = record.config.audio.device_ids.clone();
    record.summary.updated_at_ms = now_ms();
    sync_summary(record);
}

pub(super) fn sync_summary(record: &mut RecordRuntime) {
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

pub(super) fn set_record_state(record: &mut RecordRuntime, state: RecordState) {
    record.summary.state = state;
    record.summary_revision = record.summary_revision.saturating_add(1);
    sync_summary(record);
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
