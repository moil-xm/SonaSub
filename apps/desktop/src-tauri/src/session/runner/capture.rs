use std::time::Instant;

use crossbeam_channel::TrySendError;

use super::Runner;
use crate::{
    asr::AsrInput,
    audio::{AudioChunk, CaptureEvent, CAPTURE_RATE},
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
        let expected = self.expected_frame_end.unwrap_or(chunk.frame_index);
        let gap = chunk.frame_index.saturating_sub(expected);
        let discontinuity =
            chunk.discontinuity || chunk.recovered || chunk.dropped_before > 0 || gap > 48;
        let start_secs = chunk.frame_index as f64 / CAPTURE_RATE as f64;
        if discontinuity {
            self.resampler.reset();
            self.history.clear();
            self.pending_discontinuity = Some(start_secs);
            self.last_diarization = Instant::now();
            let dropped = if gap > 0 {
                gap
            } else {
                u64::from(chunk.dropped_before) * 960
            };
            self.add_dropped_frames(dropped);
            if gap > 0 || chunk.dropped_before > 0 {
                add_notice(
                    &self.inner,
                    "warning",
                    format!(
                        "System audio gap detected at {:.2}s; subtitle timing continues after the real gap.",
                        start_secs
                    ),
                );
            }
        }
        let samples = self.resampler.process(&chunk.data, chunk.frames);
        self.history.push(start_secs, &samples);
        {
            let mut runtime = self.inner.runtime();
            runtime.metrics.add_audio(chunk.frames as u64, 0);
            runtime.metrics.set_level(chunk.peak, chunk.rms);
        }
        self.send_asr(
            AsrInput::Audio {
                samples,
                start_frame_48k: chunk.frame_index,
            },
            start_secs,
        );
        self.expected_frame_end = Some(chunk.frame_index.saturating_add(chunk.frames as u64));
        self.last_audio_secs =
            self.expected_frame_end.unwrap_or(chunk.frame_index) as f64 / CAPTURE_RATE as f64;
    }

    fn send_asr(&mut self, input: AsrInput, start_secs: f64) {
        if let Some(at_secs) = self.pending_discontinuity {
            let Some(sender) = self.workers.asr_tx.as_ref() else {
                self.begin_shutdown(Some("The ASR worker stopped unexpectedly.".to_owned()));
                return;
            };
            match sender.try_send(AsrInput::Discontinuity { at_secs }) {
                Ok(()) => self.pending_discontinuity = None,
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
        self.pending_discontinuity = Some(start_secs);
        self.history.clear();
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
