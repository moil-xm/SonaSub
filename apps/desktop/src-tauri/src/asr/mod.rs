pub mod segmented;
pub mod streaming;

use std::path::Path;

use crate::{
    config::{AsrMode, RecognitionConfig},
    models::{ModelPaths, ASR_MODELS, ASR_SEGMENTED_ID, ASR_STREAMING_ID},
};

use self::{
    segmented::{SegmentedConfig, SegmentedEngine},
    streaming::{StreamingConfig, StreamingEngine},
};

#[derive(Debug, Clone)]
pub enum AsrInput {
    Audio {
        samples: Vec<f32>,
        start_frame_48k: u64,
    },
    Discontinuity {
        at_secs: f64,
    },
}

/// Control signals for the recognition worker.
///
/// The stop signal travels on its own channel so it can never be blocked by a
/// full audio backlog. A session that stops while the worker is still busy
/// decoding therefore always reaches the flush path instead of waiting for
/// the queue to drain.
#[derive(Debug, Clone, Copy)]
pub enum AsrControl {
    Stop,
}

#[derive(Debug, Clone)]
pub struct AsrPartial {
    pub local_id: u64,
    pub revision: u32,
    pub text: String,
    pub start_secs: f64,
    pub end_secs: f64,
}

#[derive(Debug, Clone)]
pub struct AsrFinal {
    pub local_id: u64,
    pub text: String,
    pub start_secs: f64,
    pub end_secs: f64,
    pub language: Option<String>,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub enum AsrEvent {
    Partial(AsrPartial),
    Final(AsrFinal),
    Warning(String),
    Fatal(String),
}

pub enum AsrEngine {
    Streaming(StreamingEngine),
    Segmented(SegmentedEngine),
}

/// The recognition surface used by the recognition worker.
///
/// The trait keeps the worker protocol independent from the concrete engines
/// so the stop and flush behaviour can be verified without the acoustic
/// models.
pub trait Recognition: Send {
    fn push(
        &mut self,
        samples: &[f32],
        start_frame_48k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String>;
    fn discontinuity(&mut self, at_secs: f64, emit: &mut dyn FnMut(AsrEvent))
        -> Result<(), String>;
    fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String>;
}

impl Recognition for AsrEngine {
    fn push(
        &mut self,
        samples: &[f32],
        start_frame_48k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        match self {
            Self::Streaming(engine) => engine.push(samples, start_frame_48k, emit),
            Self::Segmented(engine) => engine.push(samples, start_frame_48k, emit),
        }
    }

    fn discontinuity(
        &mut self,
        at_secs: f64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        match self {
            Self::Streaming(engine) => engine.discontinuity(at_secs, emit),
            Self::Segmented(engine) => engine.discontinuity(at_secs, emit),
        }
    }

    fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
        match self {
            Self::Streaming(engine) => engine.flush(emit),
            Self::Segmented(engine) => engine.flush(emit),
        }
    }
}

impl AsrEngine {
    pub fn load(
        mode: AsrMode,
        paths: &ModelPaths,
        config: &RecognitionConfig,
        model_id: &str,
    ) -> Result<Self, String> {
        let spec = ASR_MODELS
            .iter()
            .find(|spec| spec.id == model_id)
            .ok_or_else(|| format!("Unknown ASR model id `{model_id}`."))?;
        if config.source_language != "auto"
            && !spec
                .source_languages
                .iter()
                .any(|language| *language == config.source_language)
        {
            return Err(format!(
                "The selected ASR model does not support source language `{}`. Supported languages: {}.",
                config.source_language,
                spec.source_languages.join(", ")
            ));
        }
        match mode {
            AsrMode::Streaming => {
                if model_id != ASR_STREAMING_ID {
                    return Err(format!(
                        "Streaming mode requires the `{ASR_STREAMING_ID}` model, got `{model_id}`."
                    ));
                }
                let root = &paths.root;
                let engine = StreamingEngine::new(StreamingConfig {
                    encoder: path_string(
                        root,
                        "sherpa/streaming-zipformer-bilingual-zh-en/encoder.onnx",
                    )?,
                    decoder: path_string(
                        root,
                        "sherpa/streaming-zipformer-bilingual-zh-en/decoder.onnx",
                    )?,
                    joiner: path_string(
                        root,
                        "sherpa/streaming-zipformer-bilingual-zh-en/joiner.onnx",
                    )?,
                    tokens: path_string(
                        root,
                        "sherpa/streaming-zipformer-bilingual-zh-en/tokens.txt",
                    )?,
                    max_segment_secs: config.max_segment_secs,
                    num_threads: 2,
                })?;
                Ok(Self::Streaming(engine))
            }
            AsrMode::Segmented => {
                if model_id != ASR_SEGMENTED_ID {
                    return Err(format!(
                        "Segmented mode requires the `{ASR_SEGMENTED_ID}` model, got `{model_id}`."
                    ));
                }
                let root = &paths.root;
                let engine = SegmentedEngine::new(SegmentedConfig {
                    vad_model: path_string(root, "sherpa/silero_vad.onnx")?,
                    asr_model: path_string(root, "sherpa/sense-voice-int8/model.onnx")?,
                    tokens: path_string(root, "sherpa/sense-voice-int8/tokens.txt")?,
                    language: recognition_language(&config.source_language).to_owned(),
                    max_segment_secs: config.max_segment_secs,
                    vad_threshold: config.vad_threshold,
                    min_silence_secs: config.min_silence_secs,
                    min_speech_secs: config.min_speech_secs,
                })?;
                Ok(Self::Segmented(engine))
            }
            AsrMode::Auto => {
                Err("ASR auto mode must be resolved before loading a model.".to_owned())
            }
        }
    }
}

fn path_string(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    if !path.is_file() {
        return Err(format!(
            "Missing model file: {}. Open the models directory and follow docs/models.md.",
            path.display()
        ));
    }
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("Model path is not valid UTF-8: {}", path.display()))
}

fn recognition_language(source_language: &str) -> &str {
    match source_language {
        "zh-Hans" | "zh" => "zh",
        language => language,
    }
}
