use crate::audio::{dsp::AudioHistory, resample::StreamingResampler, AudioChunk, ASR_RATE};

/// Owns the audio state shared by live sessions and recorded sources.
///
/// Capture delivery, subtitle scheduling and runner-specific notices stay in
/// the caller; this type turns chunks from any native sample rate and channel
/// layout into a continuous 16 kHz mono stream and tracks gaps that must not
/// be bridged.
pub(crate) struct AudioFlow {
    history: AudioHistory,
    resampler: StreamingResampler,
    sample_rate: u32,
    expected_frame_end: Option<u64>,
    last_audio_secs: f64,
    pending_discontinuity: Option<f64>,
}

pub(crate) struct AudioChunkOutcome {
    pub(crate) start_secs: f64,
    pub(crate) samples: Vec<f32>,
    pub(crate) start_frame_16k: u64,
    pub(crate) native_frames: u64,
    pub(crate) discontinuity: bool,
    pub(crate) dropped_frames: u64,
    pub(crate) notify_gap: bool,
}

impl AudioFlow {
    pub(crate) fn new(history_secs: f64, sample_rate: u32, channels: u16) -> Result<Self, String> {
        Ok(Self {
            history: AudioHistory::new(history_secs),
            resampler: StreamingResampler::new(sample_rate, ASR_RATE, channels as usize)?,
            sample_rate,
            expected_frame_end: None,
            last_audio_secs: 0.0,
            pending_discontinuity: None,
        })
    }

    pub(crate) fn process_chunk(&mut self, chunk: &AudioChunk) -> AudioChunkOutcome {
        let expected = self.expected_frame_end.unwrap_or(chunk.frame_index);
        let gap = chunk.frame_index.saturating_sub(expected);
        let dropped_frames = gap.max(chunk.dropped_frames_before);
        let discontinuity = chunk.discontinuity || chunk.recovered || dropped_frames > 0;
        let start_secs = chunk.frame_index as f64 / self.sample_rate as f64;
        if discontinuity || self.expected_frame_end.is_none() {
            self.resampler.reset();
            self.history.clear();
            if discontinuity {
                self.pending_discontinuity = Some(start_secs);
            }
        }
        let start_frame_16k = secs_to_16k_frame(start_secs);
        let samples = self.resampler.process(&chunk.data);
        if !samples.is_empty() {
            self.history.push(start_secs, &samples);
        }
        self.expected_frame_end = Some(chunk.frame_index.saturating_add(chunk.frames as u64));
        self.last_audio_secs =
            self.expected_frame_end.unwrap_or(chunk.frame_index) as f64 / self.sample_rate as f64;
        AudioChunkOutcome {
            start_secs,
            samples,
            start_frame_16k,
            native_frames: chunk.frames as u64,
            discontinuity,
            dropped_frames: if discontinuity { dropped_frames } else { 0 },
            notify_gap: discontinuity && dropped_frames > 0,
        }
    }

    /// Emits the resampler tail after capture has stopped. The tail belongs to
    /// the end of the last captured chunk and carries the same absolute time
    /// contract as normal chunks.
    pub(crate) fn finish(&mut self) -> Option<AudioChunkOutcome> {
        let samples = self.resampler.finish();
        if samples.is_empty() {
            return None;
        }
        let start_frame_16k =
            secs_to_16k_frame(self.last_audio_secs).saturating_sub(samples.len() as u64);
        let start_secs = start_frame_16k as f64 / ASR_RATE as f64;
        self.history.push(start_secs, &samples);
        Some(AudioChunkOutcome {
            start_secs,
            start_frame_16k,
            native_frames: 0,
            samples,
            discontinuity: false,
            dropped_frames: 0,
            notify_gap: false,
        })
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

fn secs_to_16k_frame(secs: f64) -> u64 {
    (secs.max(0.0) * ASR_RATE as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(frame_index: u64, frames: usize, channels: u16, sample_rate: u32) -> AudioChunk {
        AudioChunk {
            data: vec![0.25; frames * channels as usize],
            frames,
            channels,
            sample_rate,
            frame_index,
            pts_ns: 0,
            discontinuity: false,
            recovered: false,
            dropped_frames_before: 0,
            peak: 0.25,
            rms: 0.25,
        }
    }

    #[test]
    fn a_large_native_frame_gap_starts_a_new_continuity_run() {
        let mut flow = AudioFlow::new(5.0, 48_000, 2).unwrap();
        let first = flow.process_chunk(&chunk(0, 960, 2, 48_000));
        assert!(!first.discontinuity);
        assert_eq!(first.dropped_frames, 0);
        assert!(flow.available_secs() > 0.0);

        let second = flow.process_chunk(&chunk(2_960, 960, 2, 48_000));
        assert!(second.discontinuity);
        assert_eq!(second.dropped_frames, 2_000);
        assert!(second.notify_gap);
        assert!((second.start_secs - 2_960.0 / 48_000.0).abs() < 1e-9);
        assert_eq!(flow.pending_discontinuity(), Some(second.start_secs));
    }

    #[test]
    fn a_44_1_khz_chunk_uses_its_real_rate_for_timing() {
        let mut flow = AudioFlow::new(5.0, 44_100, 2).unwrap();
        let first = flow.process_chunk(&chunk(0, 441, 2, 44_100));
        assert_eq!(first.start_frame_16k, 0);
        let second = flow.process_chunk(&chunk(441, 441, 2, 44_100));
        assert!(!second.discontinuity);
        assert!((second.start_secs - 0.01).abs() < 1e-9);
        assert_eq!(second.start_frame_16k, 160);
    }

    #[test]
    fn asr_drop_clears_the_diarization_history() {
        let mut flow = AudioFlow::new(5.0, 48_000, 2).unwrap();
        flow.process_chunk(&chunk(0, 960, 2, 48_000));
        assert!(flow.available_secs() > 0.0);

        flow.mark_asr_dropped(0.5);
        assert_eq!(flow.pending_discontinuity(), Some(0.5));
        assert_eq!(flow.available_secs(), 0.0);
    }

    #[test]
    fn finish_emits_the_remaining_resampler_tail() {
        let mut flow = AudioFlow::new(5.0, 44_100, 1).unwrap();
        flow.process_chunk(&chunk(0, 4_410, 1, 44_100));
        let tail = flow.finish().expect("tail is emitted");
        assert!(!tail.samples.is_empty());
        assert!(tail.start_secs <= flow.last_audio_secs());
    }
}
