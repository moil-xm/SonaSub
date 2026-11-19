use crate::audio::{
    dsp::{AudioHistory, DownmixResampler},
    AudioChunk, CAPTURE_RATE,
};

/// Owns the audio state shared by live sessions and recorded sources.
///
/// Capture delivery, subtitle scheduling and runner-specific notices stay in
/// the caller; this type only turns capture chunks into a continuous 16 kHz
/// stream and tracks gaps that must not be bridged.
pub(crate) struct AudioFlow {
    history: AudioHistory,
    resampler: DownmixResampler,
    expected_frame_end: Option<u64>,
    last_audio_secs: f64,
    pending_discontinuity: Option<f64>,
}

pub(crate) struct AudioChunkOutcome {
    pub(crate) start_secs: f64,
    pub(crate) samples: Vec<f32>,
    pub(crate) start_frame_48k: u64,
    pub(crate) discontinuity: bool,
    pub(crate) dropped_frames: u64,
    pub(crate) notify_gap: bool,
}

impl AudioFlow {
    pub(crate) fn new(history_secs: f64) -> Self {
        Self {
            history: AudioHistory::new(history_secs),
            resampler: DownmixResampler::new(2),
            expected_frame_end: None,
            last_audio_secs: 0.0,
            pending_discontinuity: None,
        }
    }

    pub(crate) fn process_chunk(&mut self, chunk: &AudioChunk) -> AudioChunkOutcome {
        let expected = self.expected_frame_end.unwrap_or(chunk.frame_index);
        let gap = chunk.frame_index.saturating_sub(expected);
        let discontinuity =
            chunk.discontinuity || chunk.recovered || chunk.dropped_before > 0 || gap > 48;
        let start_secs = chunk.frame_index as f64 / CAPTURE_RATE as f64;
        let dropped_frames = if discontinuity {
            self.resampler.reset();
            self.history.clear();
            self.pending_discontinuity = Some(start_secs);
            if gap > 0 {
                gap
            } else {
                u64::from(chunk.dropped_before) * 960
            }
        } else {
            0
        };
        let samples = self.resampler.process(&chunk.data, chunk.frames);
        self.history.push(start_secs, &samples);
        self.expected_frame_end = Some(chunk.frame_index.saturating_add(chunk.frames as u64));
        self.last_audio_secs =
            self.expected_frame_end.unwrap_or(chunk.frame_index) as f64 / CAPTURE_RATE as f64;
        AudioChunkOutcome {
            start_secs,
            samples,
            start_frame_48k: chunk.frame_index,
            discontinuity,
            dropped_frames,
            notify_gap: discontinuity && (gap > 0 || chunk.dropped_before > 0),
        }
    }

    pub(crate) fn pending_discontinuity(&self) -> Option<f64> {
        self.pending_discontinuity
    }

    pub(crate) fn clear_pending_discontinuity(&mut self) {
        self.pending_discontinuity = None;
    }

    pub(crate) fn mark_asr_dropped(&mut self, start_secs: f64) {
        self.pending_discontinuity = Some(start_secs);
        self.history.clear();
    }

    pub(crate) fn last_audio_secs(&self) -> f64 {
        self.last_audio_secs
    }

    pub(crate) fn available_secs(&self) -> f64 {
        self.history.available_secs()
    }

    pub(crate) fn snapshot(&self, seconds: f64) -> Option<(f64, Vec<f32>)> {
        self.history.snapshot(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(frame_index: u64, frames: usize) -> AudioChunk {
        AudioChunk {
            data: vec![0.25; frames * 2],
            frames,
            channels: 2,
            frame_index,
            pts_ns: 0,
            discontinuity: false,
            recovered: false,
            dropped_before: 0,
            peak: 0.25,
            rms: 0.25,
        }
    }

    #[test]
    fn a_large_frame_gap_starts_a_new_continuity_run() {
        let mut flow = AudioFlow::new(5.0);
        let first = flow.process_chunk(&chunk(0, 960));
        assert!(!first.discontinuity);
        assert_eq!(first.dropped_frames, 0);
        assert!(flow.available_secs() > 0.0);

        let second = flow.process_chunk(&chunk(2_960, 960));
        assert!(second.discontinuity);
        assert_eq!(second.dropped_frames, 2_000);
        assert!(second.notify_gap);
        assert_eq!(flow.pending_discontinuity(), Some(second.start_secs));
    }

    #[test]
    fn dropping_asr_input_clears_the_diarization_history() {
        let mut flow = AudioFlow::new(5.0);
        flow.process_chunk(&chunk(0, 960));
        assert!(flow.available_secs() > 0.0);

        flow.mark_asr_dropped(0.5);
        assert_eq!(flow.pending_discontinuity(), Some(0.5));
        assert_eq!(flow.available_secs(), 0.0);
    }
}
