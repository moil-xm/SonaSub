use std::collections::VecDeque;

use super::ASR_RATE;

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
