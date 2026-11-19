use std::time::Instant;

use crossbeam_channel::TrySendError;

use super::Runner;
use crate::{
    asr::AsrInput,
    audio::{AudioChunk, CaptureEvent},
    session::state::add_notice,
};

impl Runner {
    pub(super) fn handle_capture(&mut self, event: CaptureEvent) {
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
            CaptureEvent::DeviceLost(message) => {
                self.begin_shutdown(Some(message));
            }
            CaptureEvent::Fatal(message) => {
                self.begin_shutdown(Some(message));
            }
        }
    }

    fn process_chunk(&mut self, chunk: AudioChunk) {
        let outcome = self.audio.process_chunk(&chunk);
        if outcome.discontinuity {
            self.last_diarization = Instant::now();
            self.add_dropped_frames(outcome.dropped_frames);
            if outcome.notify_gap {
                add_notice(
                    &self.inner,
                    "warning",
                    format!(
                        "System audio gap detected at {:.2}s; subtitle timing continues after the real gap.",
                        outcome.start_secs
                    ),
                );
            }
        }
        {
            let mut runtime = self.inner.runtime();
            runtime.metrics.add_audio(chunk.frames as u64, 0);
            runtime.metrics.set_level(chunk.peak, chunk.rms);
        }
        self.send_asr(
            AsrInput::Audio {
                samples: outcome.samples,
                start_frame_48k: outcome.start_frame_48k,
            },
            outcome.start_secs,
        );
    }

    fn send_asr(&mut self, input: AsrInput, start_secs: f64) {
        if let Some(at_secs) = self.audio.pending_discontinuity() {
            let Some(sender) = self.workers.asr_tx.as_ref() else {
                self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
                return;
            };
            match sender.try_send(AsrInput::Discontinuity { at_secs }) {
                Ok(()) => self.audio.clear_pending_discontinuity(),
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
        self.audio.mark_asr_dropped(start_secs);
        self.add_dropped_frames(960);
        add_notice(
            &self.inner,
            "warning",
            "ASR could not keep up with system audio. The dropped interval is excluded from the subtitle timeline."
                .to_owned(),
        );
    }

    fn add_dropped_frames(&mut self, frames: u64) {
        let mut runtime = self.inner.runtime();
        runtime.metrics.add_audio(0, frames);
    }
}
