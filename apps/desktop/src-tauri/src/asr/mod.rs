mod qwen3;
pub mod segmented;
pub mod streaming;

use std::path::Path;

use crate::{
    config::{AsrMode, RecognitionConfig},
    models::{
        validate_asr_config, ModelPaths, ASR_QWEN3_06B_ID, ASR_QWEN3_17B_ID, ASR_STREAMING_ID,
    },
};

use self::{
    segmented::{SegmentedConfig, SegmentedEngine, SegmentedRecognizer},
    streaming::{StreamingConfig, StreamingEngine},
};

#[derive(Debug, Clone)]
pub enum AsrInput {
    Audio {
        samples: Vec<f32>,
        start_frame_16k: u64,
    },
    Discontinuity {
        at_secs: f64,
    },
}

/// Control signals for the recognition worker.
///
/// The stop signal travels on its own channel so it can never be blocked by a
/// full audio backlog. The resampler tail is carried with the stop signal so
/// it is consumed after the control message wins the worker's biased select,
/// before the engine flush.
#[derive(Debug, Clone)]
pub enum AsrControl {
    Stop { tail: Vec<AsrInput> },
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
        start_frame_16k: u64,
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
        start_frame_16k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        match self {
            Self::Streaming(engine) => engine.push(samples, start_frame_16k, emit),
            Self::Segmented(engine) => engine.push(samples, start_frame_16k, emit),
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
        validate_asr_config(model_id, mode, &config.source_language)?;
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
                let root = &paths.root;
                let is_qwen3 = model_id == ASR_QWEN3_06B_ID || model_id == ASR_QWEN3_17B_ID;
                let qwen3_root = is_qwen3.then(|| root.join(format!("sherpa/{model_id}")));
                let config = SegmentedConfig {
                    backend: if is_qwen3 {
                        SegmentedRecognizer::Qwen3
                    } else {
                        SegmentedRecognizer::SenseVoice
                    },
                    vad_model: path_string(root, "sherpa/silero_vad.onnx")?,
                    asr_model: if is_qwen3 {
                        String::new()
                    } else {
                        path_string(root, "sherpa/sense-voice-int8/model.onnx")?
                    },
                    tokens: if is_qwen3 {
                        String::new()
                    } else {
                        path_string(root, "sherpa/sense-voice-int8/tokens.txt")?
                    },
                    qwen3_conv_frontend: qwen3_root
                        .as_ref()
                        .map(|root| path_string(root, "conv_frontend.onnx"))
                        .transpose()?,
                    qwen3_encoder: qwen3_root
                        .as_ref()
                        .map(|root| path_string(root, "encoder.int8.onnx"))
                        .transpose()?,
                    qwen3_decoder: qwen3_root
                        .as_ref()
                        .map(|root| path_string(root, "decoder.int8.onnx"))
                        .transpose()?,
                    qwen3_tokenizer: qwen3_root
                        .as_ref()
                        .map(|root| path_dir_string(root, "tokenizer"))
                        .transpose()?,
                    language: recognition_language(&config.source_language).to_owned(),
                    max_segment_secs: config.max_segment_secs,
                    vad_threshold: config.vad_threshold,
                    min_silence_secs: config.min_silence_secs,
                    min_speech_secs: config.min_speech_secs,
                };
                let engine = SegmentedEngine::new(config)?;
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

fn path_dir_string(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    if !path.is_dir() {
        return Err(format!(
            "Missing model directory: {}. Open the models directory and follow docs/models.md.",
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
