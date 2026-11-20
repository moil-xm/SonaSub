use sherpa_onnx::{OnlineRecognizer as OfficialOnlineRecognizer, OnlineRecognizerConfig};

use crate::asr::{AsrEvent, AsrFinal, AsrPartial};

pub struct OnlineRecognizer {
    inner: OfficialOnlineRecognizer,
}

pub struct StreamingConfig {
    pub encoder: String,
    pub decoder: String,
    pub joiner: String,
    pub tokens: String,
    pub max_segment_secs: f32,
    pub num_threads: i32,
}

impl OnlineRecognizer {
    pub fn new(source: &StreamingConfig) -> Result<Self, String> {
        let mut config = OnlineRecognizerConfig {
            decoding_method: Some("modified_beam_search".to_owned()),
            max_active_paths: 4,
            enable_endpoint: true,
            rule1_min_trailing_silence: 0.4,
            rule2_min_trailing_silence: 0.6,
            rule3_min_utterance_length: source.max_segment_secs.max(2.0),
            ..OnlineRecognizerConfig::default()
        };
        config.model_config.transducer.encoder = Some(source.encoder.clone());
        config.model_config.transducer.decoder = Some(source.decoder.clone());
        config.model_config.transducer.joiner = Some(source.joiner.clone());
        config.model_config.tokens = Some(source.tokens.clone());
        config.model_config.num_threads = source.num_threads.max(1);
        config.model_config.provider = Some("cpu".to_owned());
        let inner = OfficialOnlineRecognizer::create(&config).ok_or_else(|| {
            "sherpa-onnx could not create the streaming recognizer. Check the model files and architecture.".to_owned()
        })?;
        Ok(Self { inner })
    }

    pub fn create_stream(&self) -> Result<OnlineStream, String> {
        Ok(OnlineStream {
            inner: self.inner.create_stream(),
        })
    }
}

pub struct OnlineStream {
    inner: sherpa_onnx::OnlineStream,
}

#[derive(Debug, Clone)]
pub struct StreamingResult {
    pub text: String,
    pub timestamps: Vec<f32>,
}

/// The streaming recognizer surface used by [`StreamingEngine`].
///
/// The production backend wraps the sherpa-onnx online recognizer. Tests
/// substitute a scripted backend so the subtitle lifecycle can be verified
/// without the acoustic models.
pub trait StreamingBackend {
    fn accept(&mut self, samples: &[f32]);
    fn decode(&mut self);
    fn result(&mut self) -> Option<StreamingResult>;
    fn is_endpoint(&self) -> bool;
    fn reset(&mut self);
    fn finish(&mut self);
}

struct OnlineBackend {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
}

impl OnlineBackend {
    fn new(config: &StreamingConfig) -> Result<Self, String> {
        let recognizer = OnlineRecognizer::new(config)?;
        let stream = recognizer.create_stream()?;
        Ok(Self { recognizer, stream })
    }
}

impl StreamingBackend for OnlineBackend {
    fn accept(&mut self, samples: &[f32]) {
        self.stream.inner.accept_waveform(16_000, samples);
    }

    fn decode(&mut self) {
        while self.recognizer.inner.is_ready(&self.stream.inner) {
            self.recognizer.inner.decode(&self.stream.inner);
        }
    }

    fn result(&mut self) -> Option<StreamingResult> {
        let result = self.recognizer.inner.get_result(&self.stream.inner)?;
        Some(StreamingResult {
            text: result.text.trim().to_owned(),
            timestamps: result.timestamps.unwrap_or_default(),
        })
    }

    fn is_endpoint(&self) -> bool {
        self.recognizer.inner.is_endpoint(&self.stream.inner)
    }

    fn reset(&mut self) {
        self.recognizer.inner.reset(&self.stream.inner);
    }

    fn finish(&mut self) {
        self.stream.inner.input_finished();
    }
}

pub struct StreamingEngine {
    backend: Box<dyn StreamingBackend + Send>,
    segment_start_secs: f64,
    audio_position_secs: f64,
    segment_audio: Vec<f32>,
    current: Option<CurrentSegment>,
    next_local_id: u64,
}

struct CurrentSegment {
    local_id: u64,
    revision: u32,
    text: String,
    start_secs: f64,
}

impl StreamingEngine {
    pub fn new(config: StreamingConfig) -> Result<Self, String> {
        Ok(Self::with_backend(Box::new(OnlineBackend::new(&config)?)))
    }

    fn with_backend(backend: Box<dyn StreamingBackend + Send>) -> Self {
        Self {
            backend,
            segment_start_secs: 0.0,
            audio_position_secs: 0.0,
            segment_audio: Vec::new(),
            current: None,
            next_local_id: 1,
        }
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        start_frame_16k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        if self.segment_audio.is_empty() {
            self.segment_start_secs = start_frame_16k as f64 / 16_000.0;
            self.audio_position_secs = self.segment_start_secs;
        }
        self.backend.accept(samples);
        self.segment_audio.extend_from_slice(samples);
        self.audio_position_secs += samples.len() as f64 / 16_000.0;
        self.backend.decode();
        let result = self.backend.result();
        if let Some(result) = &result {
            self.emit_partial(result, emit);
        }
        if self.backend.is_endpoint() {
            if let Some(result) = result {
                self.finalize(&result, emit);
            } else {
                self.finalize_current(emit);
            }
            self.backend.reset();
            self.segment_start_secs = self.audio_position_secs;
            self.segment_audio.clear();
            self.current = None;
        }
        Ok(())
    }

    pub fn discontinuity(
        &mut self,
        at_secs: f64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        self.finalize_current(emit);
        self.backend.reset();
        self.segment_start_secs = at_secs;
        self.audio_position_secs = at_secs;
        self.segment_audio.clear();
        self.current = None;
        Ok(())
    }

    pub fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
        self.backend.finish();
        self.backend.decode();
        if let Some(result) = self.backend.result() {
            self.finalize(&result, emit);
        } else {
            self.finalize_current(emit);
        }
        Ok(())
    }

    fn emit_partial(&mut self, result: &StreamingResult, emit: &mut dyn FnMut(AsrEvent)) {
        let text = result.text.trim();
        if text.is_empty() {
            return;
        }
        let (start_secs, end_secs) = self.result_times(result);
        match &mut self.current {
            Some(current) if current.text == text => {}
            Some(current) => {
                current.revision = current.revision.saturating_add(1);
                current.text = text.to_owned();
                let start = current.start_secs;
                emit(AsrEvent::Partial(AsrPartial {
                    local_id: current.local_id,
                    revision: current.revision,
                    text: text.to_owned(),
                    start_secs: start,
                    end_secs,
                }));
            }
            None => {
                let local_id = self.next_local_id;
                self.next_local_id += 1;
                self.current = Some(CurrentSegment {
                    local_id,
                    revision: 1,
                    text: text.to_owned(),
                    start_secs,
                });
                emit(AsrEvent::Partial(AsrPartial {
                    local_id,
                    revision: 1,
                    text: text.to_owned(),
                    start_secs,
                    end_secs,
                }));
            }
        }
    }

    fn finalize(&mut self, result: &StreamingResult, emit: &mut dyn FnMut(AsrEvent)) {
        let text = result.text.trim();
        if text.is_empty() {
            self.finalize_current(emit);
            return;
        }
        let (start_secs, end_secs) = self.result_times(result);
        // A final result may be the first text the recognizer produces, for
        // example when the utterance only becomes decodable while flushing.
        // The subtitle still needs its own identity.
        let local_id = match self.current.take() {
            Some(current) => current.local_id,
            None => {
                let local_id = self.next_local_id;
                self.next_local_id += 1;
                local_id
            }
        };
        let start_offset = ((start_secs - self.segment_start_secs) * 16_000.0)
            .round()
            .max(0.0) as usize;
        let end_offset = ((end_secs - self.segment_start_secs) * 16_000.0)
            .round()
            .max(0.0) as usize;
        let start_offset = start_offset.min(self.segment_audio.len());
        let end_offset = end_offset.min(self.segment_audio.len()).max(start_offset);
        let samples = self.segment_audio[start_offset..end_offset].to_vec();
        emit(AsrEvent::Final(AsrFinal {
            local_id,
            text: text.to_owned(),
            start_secs,
            end_secs,
            language: None,
            samples,
        }));
    }

    fn finalize_current(&mut self, emit: &mut dyn FnMut(AsrEvent)) {
        let Some(current) = self.current.take() else {
            return;
        };
        if current.text.trim().is_empty() {
            return;
        }
        let end_secs = self.audio_position_secs;
        let start_offset = ((current.start_secs - self.segment_start_secs) * 16_000.0)
            .round()
            .max(0.0) as usize;
        let start_offset = start_offset.min(self.segment_audio.len());
        emit(AsrEvent::Final(AsrFinal {
            local_id: current.local_id,
            text: current.text,
            start_secs: current.start_secs,
            end_secs,
            language: None,
            samples: self.segment_audio[start_offset..].to_vec(),
        }));
    }

    fn result_times(&self, result: &StreamingResult) -> (f64, f64) {
        let first = result.timestamps.first().copied().unwrap_or(0.0) as f64;
        let last = result
            .timestamps
            .last()
            .copied()
            .unwrap_or((self.audio_position_secs - self.segment_start_secs) as f32)
            as f64;
        let start = (self.segment_start_secs + first).max(self.segment_start_secs);
        let end = (self.segment_start_secs + last + 0.2)
            .min(self.audio_position_secs)
            .max(start);
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend that replays a fixed sequence of recognizer results so the
    /// subtitle lifecycle can be checked without the acoustic models.
    struct ScriptedBackend {
        results: Vec<Option<StreamingResult>>,
        endpoint_after: Option<usize>,
        pushes: usize,
        finished: bool,
    }

    impl ScriptedBackend {
        fn new(results: Vec<Option<StreamingResult>>) -> Self {
            Self {
                results,
                endpoint_after: None,
                pushes: 0,
                finished: false,
            }
        }
    }

    impl StreamingBackend for ScriptedBackend {
        fn accept(&mut self, _samples: &[f32]) {
            self.pushes += 1;
        }

        fn decode(&mut self) {}

        fn result(&mut self) -> Option<StreamingResult> {
            if self.results.is_empty() {
                return None;
            }
            self.results.remove(0)
        }

        fn is_endpoint(&self) -> bool {
            self.endpoint_after == Some(self.pushes)
        }

        fn reset(&mut self) {}

        fn finish(&mut self) {
            self.finished = true;
        }
    }

    fn result(text: &str, timestamps: &[f32]) -> Option<StreamingResult> {
        Some(StreamingResult {
            text: text.to_owned(),
            timestamps: timestamps.to_vec(),
        })
    }

    fn engine(results: Vec<Option<StreamingResult>>) -> StreamingEngine {
        StreamingEngine::with_backend(Box::new(ScriptedBackend::new(results)))
    }

    fn collect(engine: &mut StreamingEngine) -> Vec<AsrEvent> {
        let mut events = Vec::new();
        engine
            .push(&[0.0; 16_000], 0, &mut |event| events.push(event))
            .expect("push succeeds");
        events
    }

    #[test]
    fn a_final_result_without_a_partial_still_becomes_a_subtitle() {
        let mut engine = engine(vec![None, result("hello world", &[0.0, 0.5])]);
        let mut events = collect(&mut engine);
        assert!(events
            .iter()
            .all(|event| matches!(event, AsrEvent::Partial(_))));

        engine
            .flush(&mut |event| events.push(event))
            .expect("flush succeeds");

        let finals: Vec<&AsrFinal> = events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Final(final_event) => Some(final_event),
                _ => None,
            })
            .collect();
        assert_eq!(finals.len(), 1);
        let final_event = finals[0];
        assert_eq!(final_event.text, "hello world");
        assert_eq!(final_event.local_id, 1);
        assert_eq!(final_event.start_secs, 0.0);
        assert!((final_event.end_secs - 0.7).abs() < 0.001);
    }

    #[test]
    fn an_empty_final_result_keeps_the_partial_text() {
        let mut engine = engine(vec![result("hello", &[0.0]), result("   ", &[0.0])]);
        let mut events = collect(&mut engine);
        engine
            .flush(&mut |event| events.push(event))
            .expect("flush succeeds");

        let finals: Vec<&AsrFinal> = events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Final(final_event) => Some(final_event),
                _ => None,
            })
            .collect();
        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0].text, "hello");
        assert_eq!(finals[0].local_id, 1);
    }

    #[test]
    fn a_final_result_reuses_the_partial_identity() {
        let mut engine = engine(vec![
            result("hello", &[0.0]),
            result("hello world", &[0.0, 0.4]),
        ]);
        let mut events = collect(&mut engine);
        engine
            .flush(&mut |event| events.push(event))
            .expect("flush succeeds");

        let partials: Vec<&AsrPartial> = events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Partial(partial) => Some(partial),
                _ => None,
            })
            .collect();
        let finals: Vec<&AsrFinal> = events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Final(final_event) => Some(final_event),
                _ => None,
            })
            .collect();
        assert_eq!(partials.len(), 1);
        assert_eq!(finals.len(), 1);
        assert_eq!(partials[0].local_id, finals[0].local_id);
        assert_eq!(finals[0].text, "hello world");
    }

    #[test]
    fn an_endpoint_finalizes_the_pending_partial() {
        let mut backend = ScriptedBackend::new(vec![result("hello", &[0.0])]);
        backend.endpoint_after = Some(1);
        let mut engine = StreamingEngine::with_backend(Box::new(backend));
        let mut events = Vec::new();
        engine
            .push(&[0.0; 16_000], 0, &mut |event| events.push(event))
            .expect("push succeeds");

        let finals: Vec<&AsrFinal> = events
            .iter()
            .filter_map(|event| match event {
                AsrEvent::Final(final_event) => Some(final_event),
                _ => None,
            })
            .collect();
        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0].text, "hello");
    }
}
