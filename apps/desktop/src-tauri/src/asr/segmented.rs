use sherpa_rs::{
    sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer},
    silero_vad::{SileroVad, SileroVadConfig},
};

use crate::asr::{AsrEvent, AsrFinal};

pub struct SegmentedConfig {
    pub vad_model: String,
    pub asr_model: String,
    pub tokens: String,
    pub language: String,
    pub max_segment_secs: f32,
    pub vad_threshold: f32,
    pub min_silence_secs: f32,
    pub min_speech_secs: f32,
}

/// A speech region produced by the voice activity detector.
///
/// `start` counts samples since the last reset of the detector, so the caller
/// adds the epoch of the current audio run to obtain an absolute timestamp.
#[derive(Debug, Clone)]
pub struct VadSegment {
    pub start: i32,
    pub samples: Vec<f32>,
}

/// The voice activity detector surface used by [`SegmentedEngine`].
///
/// `reset` must restart the internal sample counter. Clearing the finished
/// segments alone keeps the counter running, which would shift every
/// timestamp after an audio gap.
pub trait VadBackend {
    fn accept_waveform(&mut self, samples: &[f32]);
    fn flush(&mut self);
    fn drain(&mut self) -> Vec<VadSegment>;
    fn reset(&mut self) -> Result<(), String>;
}

/// The offline recognizer surface used by [`SegmentedEngine`].
pub trait RecognizerBackend {
    /// Returns the transcript and the detected language, if any.
    fn transcribe(&mut self, samples: &[f32]) -> (String, Option<String>);
}

struct SherpaVad {
    inner: SileroVad,
    model: String,
    min_silence_secs: f32,
    min_speech_secs: f32,
    max_segment_secs: f32,
    threshold: f32,
    buffer_size_secs: f32,
}

impl SherpaVad {
    fn new(config: &SegmentedConfig) -> Result<Self, String> {
        let inner = SileroVad::new(
            SileroVadConfig {
                model: config.vad_model.clone(),
                min_silence_duration: config.min_silence_secs,
                min_speech_duration: config.min_speech_secs,
                max_speech_duration: config.max_segment_secs,
                threshold: config.vad_threshold,
                sample_rate: 16_000,
                window_size: 512,
                provider: Some("cpu".to_owned()),
                num_threads: Some(1),
                debug: false,
            },
            90.0,
        )
        .map_err(|error| format!("Failed to initialize Silero VAD: {error}"))?;
        Ok(Self {
            inner,
            model: config.vad_model.clone(),
            min_silence_secs: config.min_silence_secs,
            min_speech_secs: config.min_speech_secs,
            max_segment_secs: config.max_segment_secs,
            threshold: config.vad_threshold,
            buffer_size_secs: 90.0,
        })
    }
}

impl VadBackend for SherpaVad {
    fn accept_waveform(&mut self, samples: &[f32]) {
        self.inner.accept_waveform(samples.to_vec());
    }

    fn flush(&mut self) {
        self.inner.flush();
    }

    fn drain(&mut self) -> Vec<VadSegment> {
        let mut segments = Vec::new();
        while !self.inner.is_empty() {
            let segment = self.inner.front();
            self.inner.pop();
            segments.push(VadSegment {
                start: segment.start,
                samples: segment.samples,
            });
        }
        segments
    }

    fn reset(&mut self) -> Result<(), String> {
        // `SileroVad::clear` only drops the finished segments; the sample
        // counter inside the detector keeps running. Recreating the detector
        // restarts that counter so the next segment offset is relative to the
        // audio that follows the gap.
        self.inner = SileroVad::new(
            SileroVadConfig {
                model: self.model.clone(),
                min_silence_duration: self.min_silence_secs,
                min_speech_duration: self.min_speech_secs,
                max_speech_duration: self.max_segment_secs,
                threshold: self.threshold,
                sample_rate: 16_000,
                window_size: 512,
                provider: Some("cpu".to_owned()),
                num_threads: Some(1),
                debug: false,
            },
            self.buffer_size_secs,
        )
        .map_err(|error| format!("Failed to reset the Silero VAD: {error}"))?;
        Ok(())
    }
}

struct SenseVoiceBackend {
    recognizer: SenseVoiceRecognizer,
}

impl SenseVoiceBackend {
    fn new(config: &SegmentedConfig) -> Result<Self, String> {
        let recognizer = SenseVoiceRecognizer::new(SenseVoiceConfig {
            model: config.asr_model.clone(),
            language: config.language.clone(),
            use_itn: true,
            provider: Some("cpu".to_owned()),
            num_threads: Some(2),
            debug: false,
            tokens: config.tokens.clone(),
        })
        .map_err(|error| format!("Failed to initialize SenseVoice: {error}"))?;
        Ok(Self { recognizer })
    }
}

impl RecognizerBackend for SenseVoiceBackend {
    fn transcribe(&mut self, samples: &[f32]) -> (String, Option<String>) {
        let result = self.recognizer.transcribe(16_000, samples);
        let text = sanitize_sense_voice_text(&result.text);
        let language = sanitize_language(&result.lang);
        let language = if language.is_empty() {
            None
        } else {
            Some(language)
        };
        (text, language)
    }
}

pub struct SegmentedEngine {
    vad: Box<dyn VadBackend + Send>,
    recognizer: Box<dyn RecognizerBackend + Send>,
    epoch_secs: Option<f64>,
    next_local_id: u64,
    max_segment_secs: f32,
}

impl SegmentedEngine {
    pub fn new(config: SegmentedConfig) -> Result<Self, String> {
        Ok(Self {
            vad: Box::new(SherpaVad::new(&config)?),
            recognizer: Box::new(SenseVoiceBackend::new(&config)?),
            epoch_secs: None,
            next_local_id: 1,
            max_segment_secs: config.max_segment_secs,
        })
    }

    #[cfg(test)]
    fn with_backends(
        vad: Box<dyn VadBackend + Send>,
        recognizer: Box<dyn RecognizerBackend + Send>,
        max_segment_secs: f32,
    ) -> Self {
        Self {
            vad,
            recognizer,
            epoch_secs: None,
            next_local_id: 1,
            max_segment_secs,
        }
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        start_frame_48k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        if self.epoch_secs.is_none() {
            self.epoch_secs = Some(start_frame_48k as f64 / 48_000.0);
        }
        self.vad.accept_waveform(samples);
        self.drain(emit);
        Ok(())
    }

    pub fn discontinuity(
        &mut self,
        at_secs: f64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        self.vad.flush();
        self.drain(emit);
        self.vad.reset()?;
        self.epoch_secs = Some(at_secs);
        Ok(())
    }

    pub fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
        self.vad.flush();
        self.drain(emit);
        Ok(())
    }

    fn drain(&mut self, emit: &mut dyn FnMut(AsrEvent)) {
        let epoch_secs = self.epoch_secs.unwrap_or(0.0);
        for segment in self.vad.drain() {
            let start_secs = epoch_secs + segment.start as f64 / 16_000.0;
            let end_secs = start_secs + segment.samples.len() as f64 / 16_000.0;
            if segment.samples.len() < (0.1 * 16_000.0) as usize {
                continue;
            }
            let (text, language) = self.recognizer.transcribe(&segment.samples);
            if text.is_empty() {
                continue;
            }
            let local_id = self.next_local_id;
            self.next_local_id += 1;
            emit(AsrEvent::Final(AsrFinal {
                local_id,
                text,
                start_secs,
                end_secs: end_secs.min(start_secs + self.max_segment_secs as f64 + 0.5),
                language,
                samples: segment.samples,
            }));
        }
    }
}

fn sanitize_sense_voice_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_tag = false;
    for character in text.chars() {
        if character == '<' {
            in_tag = true;
            continue;
        }
        if in_tag {
            if character == '>' {
                in_tag = false;
            }
            continue;
        }
        output.push(character);
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sanitize_language(language: &str) -> String {
    language
        .trim()
        .trim_start_matches("<|")
        .trim_end_matches("|>")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    /// A detector double that reports one segment per flushed chunk and
    /// counts the samples it received since the last reset.
    struct ScriptedVad {
        pending: Vec<VadSegment>,
        received: i32,
        resets: Arc<AtomicUsize>,
        segment_samples: usize,
    }

    impl ScriptedVad {
        fn new(segment_samples: usize) -> Self {
            Self {
                pending: Vec::new(),
                received: 0,
                resets: Arc::new(AtomicUsize::new(0)),
                segment_samples,
            }
        }
    }

    impl VadBackend for ScriptedVad {
        fn accept_waveform(&mut self, samples: &[f32]) {
            self.received += samples.len() as i32;
        }

        fn flush(&mut self) {
            if self.received <= 0 {
                return;
            }
            let start = self.received - self.segment_samples as i32;
            self.pending.push(VadSegment {
                start,
                samples: vec![0.0; self.segment_samples],
            });
            self.received = 0;
        }

        fn drain(&mut self) -> Vec<VadSegment> {
            std::mem::take(&mut self.pending)
        }

        fn reset(&mut self) -> Result<(), String> {
            self.resets.fetch_add(1, Ordering::SeqCst);
            self.received = 0;
            Ok(())
        }
    }

    struct FixedRecognizer {
        text: String,
        language: Option<String>,
    }

    impl RecognizerBackend for FixedRecognizer {
        fn transcribe(&mut self, _samples: &[f32]) -> (String, Option<String>) {
            (self.text.clone(), self.language.clone())
        }
    }

    fn finals(events: &[AsrEvent]) -> Vec<&AsrFinal> {
        events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Final(final_event) => Some(final_event),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn strips_sense_voice_metadata_tags() {
        assert_eq!(
            sanitize_sense_voice_text("<|en|><|NEUTRAL|> Hello   world <|Speech|>"),
            "Hello world"
        );
    }

    #[test]
    fn strips_angle_brackets_from_language_metadata() {
        assert_eq!(sanitize_language("<|zh|>"), "zh");
    }

    #[test]
    fn a_gap_resets_the_detector_and_keeps_absolute_timestamps() {
        let resets = Arc::new(AtomicUsize::new(0));
        let mut engine = SegmentedEngine::with_backends(
            Box::new(ScriptedVad {
                pending: Vec::new(),
                received: 0,
                resets: resets.clone(),
                segment_samples: 16_000,
            }),
            Box::new(FixedRecognizer {
                text: "hello".to_owned(),
                language: Some("en".to_owned()),
            }),
            12.0,
        );
        let mut events = Vec::new();

        // Two seconds of audio, then a gap that is reported at 10 seconds.
        engine
            .push(&[0.0; 32_000], 0, &mut |event| events.push(event))
            .expect("push succeeds");
        engine
            .discontinuity(10.0, &mut |event| events.push(event))
            .expect("discontinuity succeeds");
        engine
            .push(&[0.0; 32_000], 10 * 48_000, &mut |event| events.push(event))
            .expect("push succeeds");
        engine
            .flush(&mut |event| events.push(event))
            .expect("flush succeeds");

        let finals = finals(&events);
        assert_eq!(finals.len(), 2);
        assert_eq!(resets.load(Ordering::SeqCst), 1);
        // The first segment belongs to the first audio run.
        assert!((finals[0].start_secs - 1.0).abs() < 0.001);
        // The second segment starts at the epoch of the recovered audio run,
        // not at the epoch plus the samples of the run before the gap.
        assert!((finals[1].start_secs - 11.0).abs() < 0.001);
        assert!((finals[1].end_secs - 12.0).abs() < 0.001);
    }

    #[test]
    fn a_gap_flushes_the_speech_that_precedes_it() {
        let mut engine = SegmentedEngine::with_backends(
            Box::new(ScriptedVad::new(8_000)),
            Box::new(FixedRecognizer {
                text: "before gap".to_owned(),
                language: None,
            }),
            12.0,
        );
        let mut events = Vec::new();
        engine
            .push(&[0.0; 16_000], 0, &mut |event| events.push(event))
            .expect("push succeeds");
        engine
            .discontinuity(5.0, &mut |event| events.push(event))
            .expect("discontinuity succeeds");

        let finals = finals(&events);
        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0].text, "before gap");
        assert!((finals[0].start_secs - 0.5).abs() < 0.001);
    }

    #[test]
    fn short_segments_are_dropped() {
        let mut engine = SegmentedEngine::with_backends(
            Box::new(ScriptedVad::new(800)),
            Box::new(FixedRecognizer {
                text: "too short".to_owned(),
                language: None,
            }),
            12.0,
        );
        let mut events = Vec::new();
        engine
            .push(&[0.0; 800], 0, &mut |event| events.push(event))
            .expect("push succeeds");
        engine
            .flush(&mut |event| events.push(event))
            .expect("flush succeeds");
        assert!(finals(&events).is_empty());
    }
}
