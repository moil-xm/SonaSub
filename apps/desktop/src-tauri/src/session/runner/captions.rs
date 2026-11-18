use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::TrySendError;

use super::Runner;
use crate::{
    asr::{AsrEvent, AsrFinal, AsrPartial},
    pipeline::workers::{
        DiarizationInput, DiarizationResult, DiarizationTask, SpeakerInput, SpeakerResult,
        SpeakerTask,
    },
    session::state::add_notice,
    speaker::DiarizationWindow,
};

impl Runner {
    pub(super) fn handle_asr_event(&mut self, event: AsrEvent) {
        match event {
            AsrEvent::Partial(partial) => self.apply_partial(partial),
            AsrEvent::Final(final_event) => self.apply_final(final_event),
            AsrEvent::Warning(message) => add_notice(&self.inner, "warning", message),
            AsrEvent::Fatal(message) => self.begin_shutdown(Some(message)),
        }
    }

    fn apply_partial(&mut self, partial: AsrPartial) {
        let patches = {
            let mut runtime = self.inner.runtime();
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
            let mut runtime = self.inner.runtime();
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

    pub(super) fn handle_speaker_result(&mut self, result: SpeakerResult) {
        match result.result {
            Ok(Some(speaker)) => {
                let patch = {
                    let mut runtime = self.inner.runtime();
                    runtime.metrics.speaker(result.latency_ms);
                    runtime.store.set_speaker(&result.segment_id, speaker)
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
            Ok(None) => {
                let mut runtime = self.inner.runtime();
                runtime.metrics.speaker(result.latency_ms);
            }
            Err(error) => add_notice(&self.inner, "warning", error),
        }
    }

    pub(super) fn handle_diarization_result(&mut self, result: DiarizationResult) {
        match result.result {
            Ok(window) => self.apply_diarization(window),
            Err(error) => add_notice(&self.inner, "warning", error),
        }
    }

    fn apply_diarization(&mut self, window: DiarizationWindow) {
        let segments = {
            let runtime = self.inner.runtime();
            runtime.store.snapshot()
        };
        let mut patches = Vec::new();
        for segment in &segments {
            let Some(speaker) = window.speaker_for_segment(segment.start_secs, segment.end_secs)
            else {
                continue;
            };
            let patch = {
                let mut runtime = self.inner.runtime();
                runtime.store.set_speaker(&segment.id, speaker)
            };
            if let Some(patch) = patch {
                patches.push(patch);
            }
        }
        self.pending_patches.extend(patches);
        {
            let mut runtime = self.inner.runtime();
            runtime.metrics.diarization(window.latency_ms);
        }
    }

    pub(super) fn schedule_diarization(&mut self) {
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
}
