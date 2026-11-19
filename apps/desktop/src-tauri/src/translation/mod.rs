mod language;
mod openai_compatible;

pub use language::detect_translation_source;
pub(crate) use language::{prompt_language, source_language};
use openai_compatible::OpenAiCompatibleEngine;

use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
    time::Instant,
};

use crate::{
    config::{
        normalize_openai_base_url, normalize_target_language, TranslationConfig, TranslationMode,
    },
    models::{
        resolve_translation_mode, ModelPaths, TranslationModelSpec, BERGAMOT_ENGINE_ID,
        LANGUAGE_LABEL_ZH, OPENAI_COMPATIBLE_ENGINE_ID, SUPPORTED_LANGUAGES, TRANSLATION_MODELS,
    },
};

pub const MAX_PENDING_REQUESTS: usize = 32;
/// The native runtime that executes the translation models.
pub const BERGAMOT_RUNTIME: &str = "fxtranslate";
/// The model family the compiled adapter reads.
pub const BERGAMOT_MODEL_FAMILY: &str = "bergamot-firefox";
pub const OPENAI_COMPATIBLE_RUNTIME: &str = "adk-model";
pub const OPENAI_COMPATIBLE_MODEL_FAMILY: &str = "openai-compatible";

pub trait TranslationEngine {
    fn translate(&mut self, request: &TranslationRequest) -> Result<String, String>;
}

/// The minimal factory entry point for translation adapters.
///
/// The factory keeps the worker
/// independent from the concrete engine without introducing a plugin market
/// or a dynamic loader.
pub fn create_engine(
    config: &TranslationConfig,
    paths: &ModelPaths,
    bergamot_spec: Option<&'static TranslationModelSpec>,
) -> Result<Box<dyn TranslationEngine>, String> {
    match config.engine.as_str() {
        BERGAMOT_ENGINE_ID => {
            let spec = bergamot_spec.ok_or_else(|| {
                "The Bergamot translation engine requires a local model.".to_owned()
            })?;
            Ok(Box::new(BergamotEngine::load(paths, spec)?))
        }
        OPENAI_COMPATIBLE_ENGINE_ID => Ok(Box::new(OpenAiCompatibleEngine::new(
            &config.openai_compatible,
        )?)),
        other => Err(format!(
            "Unknown translation engine `{other}`. Available engines: `{BERGAMOT_ENGINE_ID}`, `{OPENAI_COMPATIBLE_ENGINE_ID}`."
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
}

impl TranslationEngine for BergamotEngine {
    fn translate(&mut self, request: &TranslationRequest) -> Result<String, String> {
        let translation = self.engine.translate(&request.text);
        if translation.trim().is_empty() {
            return Err(format!(
                "Bergamot returned an empty translation for {} to {}.",
                self.spec.source_language, self.spec.target_language
            ));
        }
        Ok(translation.trim().to_owned())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedTranslation {
    pub engine: &'static str,
    pub runtime: &'static str,
    pub model_family: &'static str,
    pub mode: TranslationMode,
    pub model_id: String,
    pub model_label: String,
    pub target_language: String,
    pub bergamot_spec: Option<&'static TranslationModelSpec>,
}

pub fn resolve_translation_config(
    config: &TranslationConfig,
) -> Result<ResolvedTranslation, String> {
    let target_language = normalize_target_language(&config.target_language);
    if !SUPPORTED_LANGUAGES
        .iter()
        .any(|(code, _)| *code == target_language)
    {
        return Err(format!(
            "Unsupported translation target `{}`.",
            config.target_language
        ));
    }
    let mode = resolve_translation_mode(config.mode)?;
    match config.engine.as_str() {
        BERGAMOT_ENGINE_ID => {
            let spec = TRANSLATION_MODELS
                .iter()
                .find(|model| model.id == config.model_id)
                .ok_or_else(|| {
                    format!("Unknown translation model `{}`.", config.model_id)
                })?;
            if spec.target_language != target_language {
                return Err(format!(
                    "Translation model `{}` targets `{}`, not `{}`.",
                    spec.label, spec.target_language, target_language
                ));
            }
            Ok(ResolvedTranslation {
                engine: BERGAMOT_ENGINE_ID,
                runtime: BERGAMOT_RUNTIME,
                model_family: BERGAMOT_MODEL_FAMILY,
                mode,
                model_id: spec.id.to_owned(),
                model_label: spec.label.to_owned(),
                target_language: target_language.to_owned(),
                bergamot_spec: Some(spec),
            })
        }
        OPENAI_COMPATIBLE_ENGINE_ID => {
            normalize_openai_base_url(&config.openai_compatible.base_url)?;
            let model = config.openai_compatible.model.trim();
            if model.is_empty() {
                return Err("OpenAI-compatible model must not be empty.".to_owned());
            }
            Ok(ResolvedTranslation {
                engine: OPENAI_COMPATIBLE_ENGINE_ID,
                runtime: OPENAI_COMPATIBLE_RUNTIME,
                model_family: OPENAI_COMPATIBLE_MODEL_FAMILY,
                mode,
                model_id: model.to_owned(),
                model_label: model.to_owned(),
                target_language: target_language.to_owned(),
                bergamot_spec: None,
            })
        }
        other => Err(format!(
            "Unknown translation engine `{other}`. Available engines: `{BERGAMOT_ENGINE_ID}`, `{OPENAI_COMPATIBLE_ENGINE_ID}`."
        )),
    }
}

#[derive(Debug, Clone)]
pub enum TranslationRoute {
    NotNeeded,
    Unavailable(String),
    Ready {
        engine: &'static str,
        model_id: String,
    },
}

pub fn resolve_translation_route(
    config: &TranslationConfig,
    source_language: &str,
) -> Result<TranslationRoute, String> {
    let resolved = resolve_translation_config(config)?;
    if source_language == resolved.target_language {
        return Ok(TranslationRoute::NotNeeded);
    }
    if let Some(spec) = resolved.bergamot_spec {
        if spec.source_language != source_language {
            return Ok(TranslationRoute::Unavailable(format!(
                "The selected Bergamot model only handles {} to {}; this segment remains source-only.",
                spec.source_language, spec.target_language
            )));
        }
    }
    Ok(TranslationRoute::Ready {
        engine: resolved.engine,
        model_id: resolved.model_id,
    })
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
    pub engine: String,
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
        let result = engine.translate(&request);
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

#[cfg(test)]
mod tests {
    use super::{
        resolve_translation_config, resolve_translation_route, TranslationRoute,
        BERGAMOT_MODEL_FAMILY, BERGAMOT_RUNTIME, OPENAI_COMPATIBLE_MODEL_FAMILY,
        OPENAI_COMPATIBLE_RUNTIME,
    };
    use crate::{
        config::{TranslationConfig, TranslationMode},
        models::{
            BERGAMOT_ENGINE_ID, OPENAI_COMPATIBLE_ENGINE_ID, SUPPORTED_LANGUAGES,
            TRANSLATION_MODELS,
        },
    };

    fn openai_config(target_language: &str) -> TranslationConfig {
        let mut config = TranslationConfig {
            engine: OPENAI_COMPATIBLE_ENGINE_ID.to_owned(),
            target_language: target_language.to_owned(),
            ..TranslationConfig::default()
        };
        config.openai_compatible.base_url = "https://example.test/v1/chat/completions/".to_owned();
        config.openai_compatible.model = "remote-model".to_owned();
        config
    }

    #[test]
    fn the_default_translation_is_the_local_bergamot_direction() {
        let resolved = resolve_translation_config(&TranslationConfig::default()).unwrap();
        assert_eq!(resolved.engine, BERGAMOT_ENGINE_ID);
        assert_eq!(resolved.runtime, BERGAMOT_RUNTIME);
        assert_eq!(resolved.model_family, BERGAMOT_MODEL_FAMILY);
        assert_eq!(resolved.mode, TranslationMode::Text);
        assert_eq!(resolved.target_language, "zh-Hans");
        assert_eq!(resolved.model_id, "bergamot-en-zh-Hans");
        assert!(resolved.bergamot_spec.is_some());
    }

    #[test]
    fn openai_compatible_resolves_remote_metadata_without_a_local_model() {
        let resolved = resolve_translation_config(&openai_config("ja")).unwrap();
        assert_eq!(resolved.engine, OPENAI_COMPATIBLE_ENGINE_ID);
        assert_eq!(resolved.runtime, OPENAI_COMPATIBLE_RUNTIME);
        assert_eq!(resolved.model_family, OPENAI_COMPATIBLE_MODEL_FAMILY);
        assert_eq!(resolved.model_id, "remote-model");
        assert_eq!(resolved.model_label, "remote-model");
        assert!(resolved.bergamot_spec.is_none());
    }

    #[test]
    fn bergamot_rejects_every_target_outside_its_two_local_models() {
        for (code, _) in SUPPORTED_LANGUAGES {
            if matches!(*code, "en" | "zh-Hans") {
                continue;
            }
            let config = TranslationConfig {
                target_language: (*code).to_owned(),
                ..TranslationConfig::default()
            };
            assert!(
                resolve_translation_config(&config).is_err(),
                "Bergamot must reject target {code}"
            );
        }
    }

    #[test]
    fn openai_compatible_accepts_all_registered_targets() {
        for (code, _) in SUPPORTED_LANGUAGES {
            let resolved = resolve_translation_config(&openai_config(code)).unwrap();
            assert_eq!(resolved.target_language, *code);
        }
        assert_eq!(SUPPORTED_LANGUAGES.len(), 30);
    }

    #[test]
    fn openai_compatible_requires_a_valid_absolute_base_url_and_model() {
        let mut config = openai_config("ja");
        config.openai_compatible.base_url = "ftp://example.test/v1".to_owned();
        assert!(resolve_translation_config(&config).is_err());

        config.openai_compatible.base_url = "https://example.test/v1".to_owned();
        config.openai_compatible.model = "  ".to_owned();
        assert!(resolve_translation_config(&config).is_err());
    }

    #[test]
    fn matching_source_and_target_skip_translation() {
        assert!(matches!(
            resolve_translation_route(&TranslationConfig::default(), "zh-Hans").unwrap(),
            TranslationRoute::NotNeeded
        ));
        assert!(matches!(
            resolve_translation_route(&openai_config("yue"), "yue").unwrap(),
            TranslationRoute::NotNeeded
        ));
    }

    #[test]
    fn the_chinese_alias_resolves_to_simplified_chinese_and_yue_stays_distinct() {
        let config = TranslationConfig {
            target_language: "zh".to_owned(),
            ..TranslationConfig::default()
        };
        let resolved = resolve_translation_config(&config).unwrap();
        assert_eq!(resolved.target_language, "zh-Hans");

        let resolved = resolve_translation_config(&openai_config("yue")).unwrap();
        assert_eq!(resolved.target_language, "yue");
    }

    #[test]
    fn bergamot_reports_a_direction_it_cannot_translate() {
        let route = resolve_translation_route(&TranslationConfig::default(), "ja").unwrap();
        assert!(
            matches!(route, TranslationRoute::Unavailable(reason) if reason.contains("en to zh-Hans"))
        );
    }

    #[test]
    fn bergamot_and_openai_ready_routes_carry_their_real_engine_and_model() {
        let bergamot = resolve_translation_route(&TranslationConfig::default(), "en").unwrap();
        assert!(matches!(
            bergamot,
            TranslationRoute::Ready { engine, model_id }
                if engine == BERGAMOT_ENGINE_ID && model_id == "bergamot-en-zh-Hans"
        ));

        let openai = resolve_translation_route(&openai_config("yue"), "en").unwrap();
        assert!(matches!(
            openai,
            TranslationRoute::Ready { engine, model_id }
                if engine == OPENAI_COMPATIBLE_ENGINE_ID && model_id == "remote-model"
        ));
    }

    #[test]
    fn the_registry_keeps_both_bergamot_directions_available() {
        assert!(TRANSLATION_MODELS
            .iter()
            .any(|model| model.target_language == "zh-Hans"));
        assert!(TRANSLATION_MODELS
            .iter()
            .any(|model| model.target_language == "en"));
    }
}
