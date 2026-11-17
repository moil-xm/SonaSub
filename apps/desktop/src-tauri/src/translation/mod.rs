use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
    time::Instant,
};

use serde::Serialize;

use crate::models::{ModelPaths, TranslationModelSpec, BERGAMOT_ENGINE_ID, LANGUAGE_LABEL_ZH};

pub const MAX_PENDING_REQUESTS: usize = 32;
/// The native runtime that executes the translation models.
pub const TRANSLATION_RUNTIME: &str = "fxtranslate";
/// The model family the compiled adapter reads.
pub const TRANSLATION_MODEL_FAMILY: &str = "bergamot-firefox";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCapabilities {
    pub engine: String,
    pub runtime: String,
    pub model_family: String,
    pub mode: String,
    pub streaming: bool,
    pub reason: Option<String>,
}

pub trait TranslationEngine {
    fn capabilities(&self) -> TranslationCapabilities;
    fn translate(&mut self, text: &str) -> Result<String, String>;
}

/// The minimal factory entry point for translation adapters.
///
/// Only the compiled adapter exists today. The factory keeps the worker
/// independent from the concrete engine without introducing a plugin market
/// or a dynamic loader.
pub fn create_engine(
    engine_id: &str,
    paths: &ModelPaths,
    spec: &'static TranslationModelSpec,
) -> Result<Box<dyn TranslationEngine>, String> {
    match engine_id {
        BERGAMOT_ENGINE_ID => Ok(Box::new(BergamotEngine::load(paths, spec)?)),
        other => Err(format!(
            "Unknown translation engine `{other}`. The only compiled adapter is `{BERGAMOT_ENGINE_ID}`."
        )),
    }
}

/// Bergamot-format Firefox Translations model executed by the native Rust
/// `fxtranslate` engine. The adapter is intentionally complete-text only:
/// `fxtranslate` returns a finished string, so reporting incremental output
/// would be a fabricated streaming capability.
pub struct BergamotEngine {
    engine: fxtranslate::engine::Engine,
    spec: &'static TranslationModelSpec,
}

impl BergamotEngine {
    pub fn load(paths: &ModelPaths, spec: &'static TranslationModelSpec) -> Result<Self, String> {
        let model = paths.root.join(spec.model.relative);
        let source_vocab = paths.root.join(spec.source_vocab.relative);
        let target_vocab = match &spec.target_vocab {
            Some(file) => paths.root.join(file.relative),
            None => source_vocab.clone(),
        };
        let mut engine =
            fxtranslate::engine::Engine::load_mmapped(&model, &source_vocab, &target_vocab)
                .map_err(|error| format!("Failed to load Bergamot model `{}`: {error}", spec.id))?;
        if let Some(shortlist) = &spec.shortlist {
            let path = paths.root.join(shortlist.relative);
            let shortlist = fxtranslate::shortlist::Shortlist::load(&path).map_err(|error| {
                format!(
                    "Failed to load the Bergamot lexical shortlist {}: {error}",
                    path.display()
                )
            })?;
            engine = engine.with_shortlist(shortlist);
        }
        Ok(Self { engine, spec })
    }

    /// The capabilities of the adapter without loading a model.
    pub fn capabilities_spec() -> TranslationCapabilities {
        TranslationCapabilities {
            engine: BERGAMOT_ENGINE_ID.to_owned(),
            runtime: TRANSLATION_RUNTIME.to_owned(),
            model_family: TRANSLATION_MODEL_FAMILY.to_owned(),
            mode: "text".to_owned(),
            streaming: false,
            reason: Some(
                "The fxtranslate runtime returns complete translated text for one submitted source string."
                    .to_owned(),
            ),
        }
    }
}

impl TranslationEngine for BergamotEngine {
    fn capabilities(&self) -> TranslationCapabilities {
        Self::capabilities_spec()
    }

    fn translate(&mut self, text: &str) -> Result<String, String> {
        let translation = self.engine.translate(text);
        if translation.trim().is_empty() {
            return Err(format!(
                "Bergamot returned an empty translation for {} to {}.",
                self.spec.source_language, self.spec.target_language
            ));
        }
        Ok(translation.trim().to_owned())
    }
}

/// The capabilities of a translation adapter without loading its model.
pub fn adapter_capabilities(engine_id: &str) -> Result<TranslationCapabilities, String> {
    match engine_id {
        BERGAMOT_ENGINE_ID => Ok(BergamotEngine::capabilities_spec()),
        other => Err(format!(
            "Unknown translation engine `{other}`. The only compiled adapter is `{BERGAMOT_ENGINE_ID}`."
        )),
    }
}

#[derive(Debug, Clone)]
pub struct TranslationRequest {
    pub session_id: String,
    pub segment_id: String,
    pub text_revision: u32,
    pub config_revision: u64,
    pub text: String,
    pub source_language: String,
    pub target_language: String,
    pub model_id: String,
}

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub request: TranslationRequest,
    pub result: Result<String, String>,
    pub latency_ms: u64,
}

pub struct TranslationQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

struct QueueState {
    pending: VecDeque<TranslationRequest>,
    closed: bool,
}

/// The result of offering a request to the translation queue.
#[derive(Debug)]
pub enum PushOutcome {
    Queued,
    /// The queue was closed, so the request was never accepted.
    Closed(TranslationRequest),
    /// A newer request for another subtitle displaced this one.
    Displaced(TranslationRequest),
}

impl TranslationQueue {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                pending: VecDeque::new(),
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    /// Replaces a pending request for the same subtitle, so ASR revisions do
    /// not queue behind text that is already obsolete.
    /// The caller reports a request that could not be accepted as failed
    /// instead of leaving it pending forever.
    pub fn push(&self, request: TranslationRequest) -> PushOutcome {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return PushOutcome::Closed(request);
        }
        state
            .pending
            .retain(|pending| pending.segment_id != request.segment_id);
        let evicted = if state.pending.len() >= MAX_PENDING_REQUESTS {
            state.pending.pop_front()
        } else {
            None
        };
        state.pending.push_back(request);
        self.ready.notify_one();
        match evicted {
            Some(evicted) => PushOutcome::Displaced(evicted),
            None => PushOutcome::Queued,
        }
    }

    pub fn pop(&self) -> Option<TranslationRequest> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if let Some(request) = state.pending.pop_front() {
                return Some(request);
            }
            if state.closed {
                return None;
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    pub fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        self.ready.notify_all();
    }
}

impl Default for TranslationQueue {
    fn default() -> Self {
        Self::new()
    }
}

pub fn run_worker(
    mut engine: Box<dyn TranslationEngine>,
    queue: &TranslationQueue,
    mut on_result: impl FnMut(TranslationResult),
) {
    while let Some(request) = queue.pop() {
        let started = Instant::now();
        let result = engine.translate(&request.text);
        let latency_ms = started.elapsed().as_millis() as u64;
        on_result(TranslationResult {
            request,
            result,
            latency_ms,
        });
    }
}

pub fn target_label(target: &str) -> &str {
    if target == "zh-Hans" {
        LANGUAGE_LABEL_ZH
    } else {
        target
    }
}
