use std::{ffi::CString, ptr, slice};

use sherpa_rs::sherpa_rs_sys as sys;

use crate::asr::{AsrEvent, AsrFinal, AsrPartial};

pub struct OnlineRecognizer {
    recognizer: *const sys::SherpaOnnxOnlineRecognizer,
}

pub struct OnlineStream {
    stream: *const sys::SherpaOnnxOnlineStream,
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
    pub fn new(config: &StreamingConfig) -> Result<Self, String> {
        let encoder = cstring(&config.encoder)?;
        let decoder = cstring(&config.decoder)?;
        let joiner = cstring(&config.joiner)?;
        let tokens = cstring(&config.tokens)?;
        let provider = cstring("cpu")?;
        let decoding = cstring("modified_beam_search")?;

        let mut model_config: sys::SherpaOnnxOnlineModelConfig = unsafe { std::mem::zeroed() };
        model_config.transducer = sys::SherpaOnnxOnlineTransducerModelConfig {
            encoder: encoder.as_ptr(),
            decoder: decoder.as_ptr(),
            joiner: joiner.as_ptr(),
        };
        model_config.tokens = tokens.as_ptr();
        model_config.num_threads = config.num_threads.max(1);
        model_config.provider = provider.as_ptr();
        model_config.debug = 0;

        let mut recognizer_config: sys::SherpaOnnxOnlineRecognizerConfig =
            unsafe { std::mem::zeroed() };
        recognizer_config.feat_config = sys::SherpaOnnxFeatureConfig {
            sample_rate: 16_000,
            feature_dim: 80,
        };
        recognizer_config.model_config = model_config;
        recognizer_config.decoding_method = decoding.as_ptr();
        recognizer_config.max_active_paths = 4;
        recognizer_config.enable_endpoint = 1;
        recognizer_config.rule1_min_trailing_silence = 0.4;
        recognizer_config.rule2_min_trailing_silence = 0.6;
        recognizer_config.rule3_min_utterance_length = config.max_segment_secs.max(2.0);

        let recognizer = unsafe { sys::SherpaOnnxCreateOnlineRecognizer(&recognizer_config) };
        if recognizer.is_null() {
            return Err(
                "sherpa-onnx could not create the streaming recognizer. Check the model files and architecture."
                    .to_owned(),
            );
        }
        Ok(Self { recognizer })
    }

    pub fn create_stream(&self) -> Result<OnlineStream, String> {
        let stream = unsafe { sys::SherpaOnnxCreateOnlineStream(self.recognizer) };
        if stream.is_null() {
            return Err("sherpa-onnx could not create a streaming input stream.".to_owned());
        }
        Ok(OnlineStream { stream })
    }
}

unsafe impl Send for OnlineRecognizer {}

impl Drop for OnlineRecognizer {
    fn drop(&mut self) {
        unsafe { sys::SherpaOnnxDestroyOnlineRecognizer(self.recognizer) };
    }
}

impl OnlineStream {
    pub fn accept(&mut self, samples: &[f32]) {
        unsafe {
            sys::SherpaOnnxOnlineStreamAcceptWaveform(
                self.stream,
                16_000,
                samples.as_ptr(),
                samples.len() as i32,
            );
        }
    }

    pub fn decode(&mut self, recognizer: &OnlineRecognizer) {
        while unsafe { sys::SherpaOnnxIsOnlineStreamReady(recognizer.recognizer, self.stream) } != 0
        {
            unsafe { sys::SherpaOnnxDecodeOnlineStream(recognizer.recognizer, self.stream) };
        }
    }

    pub fn result(&mut self, recognizer: &OnlineRecognizer) -> Option<StreamingResult> {
        unsafe {
            let pointer = sys::SherpaOnnxGetOnlineStreamResult(recognizer.recognizer, self.stream);
            if pointer.is_null() {
                return None;
            }
            let raw = &*pointer;
            let text = if raw.text.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(raw.text)
                    .to_string_lossy()
                    .trim()
                    .to_owned()
            };
            let timestamps = if raw.timestamps.is_null() || raw.count <= 0 {
                Vec::new()
            } else {
                slice::from_raw_parts(raw.timestamps, raw.count as usize).to_vec()
            };
            sys::SherpaOnnxDestroyOnlineRecognizerResult(pointer);
            Some(StreamingResult { text, timestamps })
        }
    }

    pub fn is_endpoint(&self, recognizer: &OnlineRecognizer) -> bool {
        unsafe { sys::SherpaOnnxOnlineStreamIsEndpoint(recognizer.recognizer, self.stream) != 0 }
    }

    pub fn reset(&mut self, recognizer: &OnlineRecognizer) {
        unsafe { sys::SherpaOnnxOnlineStreamReset(recognizer.recognizer, self.stream) };
    }

    pub fn finish(&mut self) {
        unsafe { sys::SherpaOnnxOnlineStreamInputFinished(self.stream) };
    }
}

impl Drop for OnlineStream {
    fn drop(&mut self) {
        unsafe { sys::SherpaOnnxDestroyOnlineStream(self.stream) };
    }
}

unsafe impl Send for OnlineStream {}

#[derive(Debug, Clone)]
pub struct StreamingResult {
    pub text: String,
    pub timestamps: Vec<f32>,
}

pub struct StreamingEngine {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
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
        let recognizer = OnlineRecognizer::new(&config)?;
        let stream = recognizer.create_stream()?;
        Ok(Self {
            recognizer,
            stream,
            segment_start_secs: 0.0,
            audio_position_secs: 0.0,
            segment_audio: Vec::new(),
            current: None,
            next_local_id: 1,
        })
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        start_frame_48k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        if self.segment_audio.is_empty() {
            self.segment_start_secs = start_frame_48k as f64 / 48_000.0;
            self.audio_position_secs = self.segment_start_secs;
        }
        self.stream.accept(samples);
        self.segment_audio.extend_from_slice(samples);
        self.audio_position_secs += samples.len() as f64 / 16_000.0;
        self.stream.decode(&self.recognizer);
        let result = self.stream.result(&self.recognizer);
        if let Some(result) = &result {
            self.emit_partial(result, emit);
        }
        if self.stream.is_endpoint(&self.recognizer) {
            if let Some(result) = result {
                self.finalize(&result, emit);
            }
            self.stream.reset(&self.recognizer);
            self.segment_start_secs = self.audio_position_secs;
            self.segment_audio.clear();
            self.current = None;
        }
        Ok(())
    }

    pub fn discontinuity(&mut self, at_secs: f64, emit: &mut dyn FnMut(AsrEvent)) {
        self.finalize_current(emit);
        self.stream.reset(&self.recognizer);
        self.segment_start_secs = at_secs;
        self.audio_position_secs = at_secs;
        self.segment_audio.clear();
        self.current = None;
    }

    pub fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
        self.stream.finish();
        self.stream.decode(&self.recognizer);
        if let Some(result) = self.stream.result(&self.recognizer) {
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
            return;
        }
        let (start_secs, end_secs) = self.result_times(result);
        let Some(current) = self.current.take() else {
            return;
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
            local_id: current.local_id,
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

fn cstring(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|error| format!("Invalid path for sherpa-onnx: {error}"))
}

#[allow(dead_code)]
fn _assert_ptr_size() {
    let _ = ptr::null::<sys::SherpaOnnxOnlineRecognizer>();
}
