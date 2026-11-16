use std::collections::VecDeque;

pub const CANONICAL_RATE: u32 = 48_000;
pub const ASR_RATE: u32 = 16_000;
const DECIMATION: usize = 3;
const TAPS: usize = 63;

/// Converts the interleaved 48 kHz capture chunks to the 16 kHz mono stream
/// consumed by sherpa-onnx. The filter is a windowed-sinc low-pass followed by
/// 3:1 decimation; the phase is stable across chunks so sample positions map
/// linearly back to the capture timeline.
pub struct DownmixResampler {
    channels: usize,
    taps: Vec<f32>,
    history: VecDeque<f32>,
    input_index: u64,
}

impl DownmixResampler {
    pub fn new(channels: usize) -> Self {
        let mut resampler = Self {
            channels,
            taps: design_low_pass(),
            history: VecDeque::with_capacity(TAPS),
            input_index: 0,
        };
        resampler.reset();
        resampler
    }

    pub fn reset(&mut self) {
        self.history.clear();
        // Causal form of the centered FIR: the group delay is 31 samples
        // (0.65 ms at 48 kHz), far below subtitle timing precision.
        for _ in 0..TAPS - 1 {
            self.history.push_back(0.0);
        }
        self.input_index = 0;
    }

    /// Returns 16 kHz mono samples and the number of 16 kHz frames produced.
    pub fn process(&mut self, interleaved: &[f32], frames: usize) -> Vec<f32> {
        let channels = self.channels.max(1);
        let frames = frames.min(interleaved.len() / channels);
        let mut output = Vec::with_capacity(frames / DECIMATION + 1);
        for frame in 0..frames {
            let base = frame * channels;
            let mono = if channels == 1 {
                interleaved[base]
            } else {
                let mut sum = 0.0;
                for channel in 0..channels {
                    sum += interleaved[base + channel];
                }
                sum / channels as f32
            };
            self.history.push_back(mono);
            while self.history.len() > TAPS {
                self.history.pop_front();
            }
            if self.input_index.is_multiple_of(DECIMATION as u64) {
                let mut filtered = 0.0f32;
                for (sample, tap) in self.history.iter().zip(self.taps.iter()) {
                    filtered += sample * tap;
                }
                output.push(filtered);
            }
            self.input_index += 1;
        }
        output
    }
}

fn design_low_pass() -> Vec<f32> {
    let cutoff = 7_200.0 / CANONICAL_RATE as f64;
    let center = (TAPS - 1) as f64 / 2.0;
    let mut taps = Vec::with_capacity(TAPS);
    let mut sum = 0.0;
    for index in 0..TAPS {
        let offset = index as f64 - center;
        let sinc = if offset.abs() < f64::EPSILON {
            2.0 * cutoff
        } else {
            (2.0 * std::f64::consts::PI * cutoff * offset).sin() / (std::f64::consts::PI * offset)
        };
        let window =
            0.54 - 0.46 * (2.0 * std::f64::consts::PI * index as f64 / (TAPS - 1) as f64).cos();
        let value = sinc * window;
        taps.push(value);
        sum += value;
    }
    taps.into_iter().map(|value| (value / sum) as f32).collect()
}

/// Bounded rolling history of 16 kHz audio in absolute session time. Used by
/// the windowed diarization worker; gaps start a new contiguous run instead of
/// pretending the missing audio was silence.
pub struct AudioHistory {
    capacity: usize,
    start_secs: f64,
    samples: VecDeque<f32>,
}

impl AudioHistory {
    pub fn new(seconds: f64) -> Self {
        Self {
            capacity: (seconds * ASR_RATE as f64) as usize,
            start_secs: 0.0,
            samples: VecDeque::new(),
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn push(&mut self, start_secs: f64, samples: &[f32]) {
        let expected_start = self.start_secs + self.samples.len() as f64 / ASR_RATE as f64;
        if !self.samples.is_empty() && (start_secs - expected_start).abs() > 0.05 {
            self.samples.clear();
        }
        if self.samples.is_empty() {
            self.start_secs = start_secs;
        }
        self.samples.extend(samples.iter().copied());
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
            self.start_secs += 1.0 / ASR_RATE as f64;
        }
    }

    pub fn snapshot(&self, seconds: f64) -> Option<(f64, Vec<f32>)> {
        if self.samples.is_empty() {
            return None;
        }
        let wanted = (seconds * ASR_RATE as f64).round() as usize;
        let start = self.samples.len().saturating_sub(wanted);
        let offset_secs = start as f64 / ASR_RATE as f64;
        Some((
            self.start_secs + offset_secs,
            self.samples.iter().skip(start).copied().collect(),
        ))
    }

    pub fn available_secs(&self) -> f64 {
        self.samples.len() as f64 / ASR_RATE as f64
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimates_stereo_to_mono_at_one_third() {
        let mut resampler = DownmixResampler::new(2);
        let mut input = Vec::new();
        for index in 0..4_800 {
            let value = (index as f32 * 0.01).sin() * 0.25;
            input.push(value);
            input.push(value);
        }
        let output = resampler.process(&input, 4_800);
        assert_eq!(output.len(), 1_600);
        assert!(output.iter().all(|sample| sample.is_finite()));
        assert!(output.iter().any(|sample| sample.abs() > 0.01));
    }

    #[test]
    fn history_keeps_a_bounded_tail() {
        let mut history = AudioHistory::new(1.0);
        let samples = vec![0.25f32; ASR_RATE as usize];
        history.push(3.0, &samples);
        let (start, tail) = history.snapshot(1.0).unwrap();
        assert!((start - 3.0).abs() < 0.001);
        assert_eq!(tail.len(), ASR_RATE as usize);
    }

    #[test]
    fn history_discards_a_discontinuous_run() {
        let mut history = AudioHistory::new(5.0);
        history.push(1.0, &vec![0.1; 1_600]);
        history.push(9.0, &vec![0.2; 1_600]);
        let (start, _) = history.snapshot(5.0).unwrap();
        assert!((start - 9.0).abs() < 0.001);
    }
}
