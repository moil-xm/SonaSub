use super::Runner;
use crate::{
    config::TranslationTrigger,
    models::{BERGAMOT_ENGINE_ID, TRANSLATION_MODELS},
    subtitles::StableCandidate,
    translation::{
        prompt_language, source_language, PushOutcome, TranslationRequest, TranslationResult,
    },
};

impl Runner {
    pub(super) fn handle_translation_result(&mut self, result: TranslationResult) {
        let stale = {
            let runtime = self.inner.runtime();
            runtime
                .store
                .is_stale(&result.request, self.config_revision)
        };
        if stale {
            return;
        }
        let failed = result.result.is_err();
        let patch = {
            let mut runtime = self.inner.runtime();
            runtime.metrics.translation(result.latency_ms, failed);
            runtime.store.set_translation(
                &result.request.segment_id,
                result.request.text_revision,
                result.result,
                &result.request.target_language,
                BERGAMOT_ENGINE_ID,
                &result.request.model_id,
            )
        };
        if let Some(patch) = patch {
            self.pending_patches.push(patch);
        }
    }

    pub(super) fn queue_translation(
        &mut self,
        segment_id: &str,
        text: String,
        language: &Option<String>,
    ) {
        let target = self.config.translation.target_language.clone();
        let needs = {
            let runtime = self.inner.runtime();
            runtime.store.needs_translation(segment_id, &target)
        };
        if !needs {
            return;
        }
        let segment_revision = {
            let runtime = self.inner.runtime();
            runtime
                .store
                .segment(segment_id)
                .map(|segment| segment.revision)
        };
        let Some(text_revision) = segment_revision else {
            return;
        };
        let configured_language = if self.config.recognition.source_language == "auto" {
            None
        } else {
            prompt_language(Some(&self.config.recognition.source_language))
        };
        let source_language = source_language(&text, configured_language.or(language.as_deref()));
        if source_language == target {
            let patch = {
                let mut runtime = self.inner.runtime();
                runtime
                    .store
                    .mark_same_language(segment_id, &target, BERGAMOT_ENGINE_ID)
            };
            if let Some(patch) = patch {
                self.pending_patches.push(patch);
            }
            return;
        }
        let model = TRANSLATION_MODELS.iter().find(|model| {
            model.target_language == target && model.source_language == source_language
        });
        let Some(model) = model else {
            let patch = {
                let mut runtime = self.inner.runtime();
                runtime.store.set_translation_unavailable(
                    segment_id,
                    format!(
                        "No local Bergamot direction for {source_language} to {target}; this segment remains source-only."
                    ),
                    &target,
                    BERGAMOT_ENGINE_ID,
                )
            };
            if let Some(patch) = patch {
                self.pending_patches.push(patch);
            }
            return;
        };
        let request = TranslationRequest {
            session_id: self.session_id.clone(),
            segment_id: segment_id.to_owned(),
            text_revision,
            config_revision: self.config_revision,
            text,
            source_language: source_language.to_owned(),
            target_language: target.clone(),
            model_id: model.id.to_owned(),
        };
        let patch = {
            let mut runtime = self.inner.runtime();
            runtime.store.mark_translation_pending(
                segment_id,
                &target,
                BERGAMOT_ENGINE_ID,
                "text",
                model.id,
            )
        };
        if let Some(patch) = patch {
            self.pending_patches.push(patch);
        }
        match self.workers.queue.push(request) {
            PushOutcome::Queued => {}
            PushOutcome::Displaced(evicted) => {
                let patch = {
                    let mut runtime = self.inner.runtime();
                    runtime.store.set_translation_error(
                        &evicted.segment_id,
                        evicted.text_revision,
                        &evicted.target_language,
                        BERGAMOT_ENGINE_ID,
                        &evicted.model_id,
                        "Translation queue was full; a newer subtitle superseded this request."
                            .to_owned(),
                    )
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
            PushOutcome::Closed(rejected) => {
                let patch = {
                    let mut runtime = self.inner.runtime();
                    runtime.store.set_translation_unavailable(
                        &rejected.segment_id,
                        "The translation worker already stopped, so this subtitle stays source-only."
                            .to_owned(),
                        &rejected.target_language,
                        BERGAMOT_ENGINE_ID,
                    )
                };
                if let Some(patch) = patch {
                    self.pending_patches.push(patch);
                }
            }
        }
    }

    pub(super) fn schedule_stable_translations(&mut self) {
        if self.config.translation.trigger != TranslationTrigger::StableSegment {
            return;
        }
        let candidates = {
            let runtime = self.inner.runtime();
            runtime
                .store
                .stable_candidates(self.config.recognition.stable_wait_ms)
        };
        for candidate in candidates {
            self.queue_stable_candidate(candidate);
        }
    }

    fn queue_stable_candidate(&mut self, candidate: StableCandidate) {
        if self.ending {
            return;
        }
        self.queue_translation(
            &candidate.segment_id,
            candidate.text,
            &candidate.source_language,
        );
    }
}
