use std::collections::VecDeque;
use std::f64::consts::PI;

const PHASES: usize = 1024;
const HALF_TAPS: usize = 32;
const TAPS: usize = HALF_TAPS * 2;

/// Streams interleaved capture audio into mono at the target rate.
///
/// The converter keeps its interpolation history across callbacks. Output
/// sample `n` is calculated at source position `n * input_rate / output_rate`
/// with a windowed-sinc low-pass centred on that position. The first outputs
/// are withheld until enough future samples exist, which is what makes the
/// result continuous across arbitrary callback sizes.
pub(crate) struct StreamingResampler {
    input_rate: u32,
    output_rate: u32,
    channels: usize,
    phases: Vec<Vec<f32>>,
    input: VecDeque<f32>,
    buffer_start: u64,
    input_frames: u64,
    available_frames: u64,
    output_frames: u64,
}

impl StreamingResampler {
    pub(crate) fn new(input_rate: u32, output_rate: u32, channels: usize) -> Result<Self, String> {
        if input_rate == 0 || output_rate == 0 {
            return Err("Audio sample rates must be greater than zero.".to_owned());
        }
        if channels == 0 {
            return Err("Audio channel count must be greater than zero.".to_owned());
        }
        Ok(Self {
            input_rate,
            output_rate,
            channels,
            phases: design_phases(input_rate, output_rate),
            input: VecDeque::with_capacity(TAPS * 8),
            buffer_start: 0,
            input_frames: 0,
            available_frames: 0,
            output_frames: 0,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.input.clear();
        self.buffer_start = 0;
        self.input_frames = 0;
        self.available_frames = 0;
        self.output_frames = 0;
    }

    pub(crate) fn process(&mut self, interleaved: &[f32]) -> Vec<f32> {
        let frames = interleaved.len() / self.channels;
        if frames == 0 {
            return Vec::new();
        }
        self.input.reserve(frames + TAPS);
        for frame in interleaved[..frames * self.channels].chunks_exact(self.channels) {
            let mono = frame.iter().copied().sum::<f32>() / self.channels as f32;
            self.input.push_back(mono);
        }
        self.input_frames += frames as u64;
        self.available_frames += frames as u64;
        self.drain_outputs()
    }

    /// Emits the samples whose interpolation window extends past the last
    /// captured frame. Call once when the capture stream stops.
    pub(crate) fn finish(&mut self) -> Vec<f32> {
        if self.input_frames == 0 {
            return Vec::new();
        }
        let target = rounded_output_frames(self.input_frames, self.input_rate, self.output_rate);
        if self.output_frames >= target {
            return Vec::new();
        }
        self.input.extend(std::iter::repeat_n(0.0, TAPS));
        self.available_frames += TAPS as u64;
        let mut output = self.drain_until(target);
        self.drain_buffer();
        output.shrink_to_fit();
        output
    }

    fn drain_outputs(&mut self) -> Vec<f32> {
        self.drain_until(u64::MAX)
    }

    fn drain_until(&mut self, target: u64) -> Vec<f32> {
        let mut output = Vec::with_capacity(64);
        while self.output_frames < target {
            let position =
                self.output_frames as f64 * self.input_rate as f64 / self.output_rate as f64;
            let base = position.floor() as i64;
            let fraction = position - base as f64;
            let last_needed = base + HALF_TAPS as i64;
            if last_needed < 0 || last_needed as u64 >= self.available_frames {
                break;
            }
            let phase = ((fraction * PHASES as f64).round() as usize).min(PHASES - 1);
            let first_needed = base - (HALF_TAPS as i64 - 1);
            let mut sample = 0.0f32;
            let taps = &self.phases[phase];
            for (index, weight) in taps.iter().enumerate() {
                let source_index = first_needed + index as i64;
                if source_index < 0 || source_index as u64 >= self.available_frames {
                    continue;
                }
                let relative = source_index as u64 - self.buffer_start;
                if let Some(value) = self.input.get(relative as usize) {
                    sample += value * weight;
                }
            }
            output.push(sample);
            self.output_frames += 1;
            self.drain_buffer();
        }
        output
    }

    fn drain_buffer(&mut self) {
        let next_position =
            self.output_frames as f64 * self.input_rate as f64 / self.output_rate as f64;
        let next_base = next_position.floor() as i64;
        let first_needed = next_base - (HALF_TAPS as i64 - 1);
        let keep_from = first_needed.max(0) as u64;
        let drop = keep_from.saturating_sub(self.buffer_start);
        let drop = drop.min(self.input.len() as u64) as usize;
        if drop > 0 {
            self.input.drain(..drop);
            self.buffer_start += drop as u64;
        }
    }
}

fn rounded_output_frames(input_frames: u64, input_rate: u32, output_rate: u32) -> u64 {
    (input_frames as f64 * output_rate as f64 / input_rate as f64).round() as u64
}

fn design_phases(input_rate: u32, output_rate: u32) -> Vec<Vec<f32>> {
    let cutoff_hz = (output_rate as f64 * 0.45).min(7_200.0);
    let cutoff = cutoff_hz / input_rate as f64;
    let mut phases = Vec::with_capacity(PHASES);
    for phase in 0..PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let mut taps = Vec::with_capacity(TAPS);
        let mut sum = 0.0;
        for index in 0..TAPS {
            let distance = index as f64 - (HALF_TAPS - 1) as f64 - fraction;
            let ratio = distance / HALF_TAPS as f64;
            let window = if ratio.abs() >= 1.0 {
                0.0
            } else {
                0.42 + 0.5 * (PI * ratio).cos() + 0.08 * (2.0 * PI * ratio).cos()
            };
            let sinc_argument = 2.0 * cutoff * distance;
            let sinc = if sinc_argument.abs() < f64::EPSILON {
                1.0
            } else {
                (PI * sinc_argument).sin() / (PI * sinc_argument)
            };
            let value = 2.0 * cutoff * sinc * window;
            taps.push(value);
            sum += value;
        }
        let scale = if sum.abs() < f64::EPSILON {
            1.0
        } else {
            1.0 / sum
        };
        phases.push(
            taps.into_iter()
                .map(|value| (value * scale) as f32)
                .collect(),
        );
    }
    phases
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, frequency: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|index| (2.0 * PI * frequency * index as f64 / rate as f64).sin() as f32)
            .collect()
    }

    fn process_in_chunks(
        input_rate: u32,
        channels: usize,
        samples: &[f32],
        chunk_frames: &[usize],
    ) -> (Vec<f32>, StreamingResampler) {
        let mut resampler = StreamingResampler::new(input_rate, 16_000, channels).unwrap();
        let mut output = Vec::new();
        let mut offset = 0;
        let mut chunk_index = 0;
        while offset < samples.len() / channels {
            let frames = chunk_frames[chunk_index % chunk_frames.len()];
            let frames = frames.min(samples.len() / channels - offset);
            let end = (offset + frames) * channels;
            output.extend(resampler.process(&samples[offset * channels..end]));
            offset += frames;
            chunk_index += 1;
        }
        output.extend(resampler.finish());
        (output, resampler)
    }

    fn dominant_frequency(samples: &[f32], rate: u32) -> f64 {
        let start = samples.len() / 4;
        let samples = &samples[start..samples.len() - start];
        let mut best_bin = 0;
        let mut best_magnitude = 0.0;
        for bin in 1..2_000 {
            let mut real = 0.0;
            let mut imaginary = 0.0;
            for (index, sample) in samples.iter().enumerate() {
                let angle = 2.0 * PI * bin as f64 * index as f64 / samples.len() as f64;
                real += *sample as f64 * angle.cos();
                imaginary -= *sample as f64 * angle.sin();
            }
            let magnitude = real.hypot(imaginary);
            if magnitude > best_magnitude {
                best_magnitude = magnitude;
                best_bin = bin;
            }
        }
        best_bin as f64 * rate as f64 / samples.len() as f64
    }

    #[test]
    fn converts_44_1_khz_stereo_to_16_khz_mono_without_changing_the_tone() {
        let left = sine(44_100, 440.0, 44_100);
        let mut interleaved = Vec::with_capacity(left.len() * 2);
        for sample in &left {
            interleaved.push(*sample);
            interleaved.push(*sample);
        }
        let (output, _) = process_in_chunks(44_100, 2, &interleaved, &[441, 1_023, 317, 5_009]);
        assert!((output.len() as i64 - 16_000).abs() <= 2);
        assert!((dominant_frequency(&output, 16_000) - 440.0).abs() < 4.0);
    }

    #[test]
    fn converts_48_khz_to_16_khz_with_continuous_blocks() {
        let input = sine(48_000, 1_000.0, 48_000);
        let (output, _) = process_in_chunks(48_000, 1, &input, &[480, 96, 1_920, 37]);
        assert_eq!(output.len(), 16_000);
        assert!((dominant_frequency(&output, 16_000) - 1_000.0).abs() < 4.0);
    }

    #[test]
    fn downmixes_stereo_before_resampling() {
        let frames = 4_800;
        let mut interleaved = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            interleaved.push(1.0);
            interleaved.push(-1.0);
        }
        let mut resampler = StreamingResampler::new(48_000, 16_000, 2).unwrap();
        let mut output = resampler.process(&interleaved);
        output.extend(resampler.finish());
        assert_eq!(output.len(), 1_600);
        assert!(output.iter().all(|sample| sample.abs() < 1e-5));
    }

    #[test]
    fn reset_drops_history_and_starts_a_new_stream() {
        let input = sine(48_000, 440.0, 4_800);
        let mut resampler = StreamingResampler::new(48_000, 16_000, 1).unwrap();
        resampler.process(&input);
        resampler.reset();
        let mut output = resampler.process(&input);
        output.extend(resampler.finish());
        assert_eq!(output.len(), 1_600);
    }

    #[test]
    fn finish_emits_the_tail_after_short_input() {
        let mut resampler = StreamingResampler::new(44_100, 16_000, 1).unwrap();
        let output = resampler.process(&sine(44_100, 300.0, 500));
        let tail = resampler.finish();
        assert_eq!((output.len() + tail.len()) as i64, 181);
    }
}
