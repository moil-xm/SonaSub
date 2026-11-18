use std::sync::{mpsc, Arc};

use crossbeam_channel::Receiver;
use tauri::Emitter;

use crate::subtitles::SubtitlePatch;

use super::{
    runtime::{now_ms, set_record_state, sync_summary, RecordManagerInner, RecordRuntime},
    source::{SourceEvent, SourceOutcome},
    types::{
        RecordNotice, RecordNoticeEvent, RecordState, RecordStats, RecordStatsEvent,
        RecordSubtitleBatch, RecordSummaryEvent, SourceEndReason, SourceRunState,
    },
};

pub(super) const RECORD_EVENT_QUEUE_DEPTH: usize = 4_096;
const NOTICE_LIMIT: usize = 100;

pub(super) fn run_event_loop(inner: Arc<RecordManagerInner>, events: Receiver<SourceEvent>) {
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

pub(super) fn handle_source_notice(
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

pub(super) fn shutdown_event() -> SourceEvent {
    SourceEvent::Shutdown
}
