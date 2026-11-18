use std::{
    collections::{HashMap, VecDeque},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    asr::{AsrFinal, AsrPartial},
    speaker::{SpeakerInfo, SPEAKER_SOURCE_DIARIZATION},
    translation::TranslationRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationStatus {
    Pending,
    Ready,
    Error,
    Stale,
    NotNeeded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationState {
    pub target_language: String,
    pub text: Option<String>,
    pub status: TranslationStatus,
    pub revision: u32,
    pub engine: String,
    pub mode: String,
    pub model_id: String,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleSegment {
    pub id: String,
    pub session_id: String,
    pub record_id: String,
    pub source_run_id: String,
    pub device_id: String,
    pub device_name: String,
    pub local_id: u64,
    pub start_secs: f64,
    pub end_secs: Option<f64>,
    /// Time accumulated by this device inside the record, including earlier
    /// source runs and excluding offline time.
    pub device_time_secs: f64,
    /// Wall-clock occurrence time used to merge subtitles from every source.
    pub occurrence_ms: u64,
    pub text: String,
    pub revision: u32,
    pub is_final: bool,
    pub speaker: Option<SpeakerInfo>,
    /// Increments every time the speaker of this segment is assigned. The
    /// frontend uses it to reject a late speaker patch.
    pub speaker_revision: u32,
    pub translation: Option<TranslationState>,
    pub source_language: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(skip)]
    pub last_change: Instant,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SubtitlePatch {
    Upsert {
        segment: SubtitleSegment,
    },
    Finalize {
        segment: SubtitleSegment,
    },
    Speaker {
        id: String,
        speaker: Option<SpeakerInfo>,
        revision: u32,
    },
    Translation {
        id: String,
        translation: Option<TranslationState>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleBatch {
    pub revision: u64,
    pub session_id: String,
    pub patches: Vec<SubtitlePatch>,
}

#[derive(Debug, Clone)]
pub struct StableCandidate {
    pub segment_id: String,
    pub text: String,
    pub text_revision: u32,
    pub source_language: Option<String>,
}

pub struct SubtitleStore {
    session_id: String,
    record_id: String,
    source_run_id: String,
    device_id: String,
    device_name: String,
    elapsed_base_ms: u64,
    occurrence_base_ms: u64,
    segments: HashMap<String, SubtitleSegment>,
    order: VecDeque<String>,
    history_limit: usize,
}

impl SubtitleStore {
    pub fn new(session_id: String, history_limit: usize) -> Self {
        Self {
            session_id,
            record_id: String::new(),
            source_run_id: String::new(),
            device_id: String::new(),
            device_name: String::new(),
            elapsed_base_ms: 0,
            occurrence_base_ms: 0,
            segments: HashMap::new(),
            order: VecDeque::new(),
            history_limit: history_limit.max(50),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_source(
        record_id: String,
        source_run_id: String,
        device_id: String,
        device_name: String,
        elapsed_base_ms: u64,
        occurrence_base_ms: u64,
        history_limit: usize,
    ) -> Self {
        Self {
            session_id: source_run_id.clone(),
            record_id,
            source_run_id,
            device_id,
            device_name,
            elapsed_base_ms,
            occurrence_base_ms,
            segments: HashMap::new(),
            order: VecDeque::new(),
            history_limit: history_limit.max(50),
        }
    }

    pub fn apply_partial(&mut self, partial: AsrPartial) -> Vec<SubtitlePatch> {
        let id = self.segment_id(partial.local_id);
        let now = now_ms();
        let mut patches = Vec::new();
        if let Some(segment) = self.segments.get_mut(&id) {
            let changed = segment.text != partial.text;
            segment.start_secs = partial.start_secs;
            segment.end_secs = Some(partial.end_secs);
            segment.updated_at_ms = now;
            if changed {
                segment.text = partial.text;
                segment.revision = segment.revision.saturating_add(1);
                segment.last_change = Instant::now();
                if segment.translation.is_some() {
                    segment.translation = Some(stale_translation(segment));
                    patches.push(SubtitlePatch::Translation {
                        id: id.clone(),
                        translation: segment.translation.clone(),
                    });
                }
            }
            patches.push(SubtitlePatch::Upsert {
                segment: segment.clone(),
            });
        } else {
            let segment = SubtitleSegment {
                id: id.clone(),
                session_id: self.session_id.clone(),
                record_id: self.record_id.clone(),
                source_run_id: self.source_run_id.clone(),
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
                local_id: partial.local_id,
                start_secs: partial.start_secs,
                end_secs: Some(partial.end_secs),
                device_time_secs: self.device_time(partial.start_secs),
                occurrence_ms: self.occurrence_ms(partial.start_secs),
                text: partial.text,
                revision: 1,
                is_final: false,
                speaker: None,
                speaker_revision: 0,
                translation: None,
                source_language: None,
                created_at_ms: now,
                updated_at_ms: now,
                last_change: Instant::now(),
            };
            self.insert(id, segment.clone());
            patches.push(SubtitlePatch::Upsert { segment });
        }
        patches
    }

    pub fn apply_final(&mut self, final_event: AsrFinal) -> Vec<SubtitlePatch> {
        let id = self.segment_id(final_event.local_id);
        let now = now_ms();
        let mut patches = Vec::new();
        let segment = match self.segments.get_mut(&id) {
            Some(segment) => {
                let changed = segment.text != final_event.text;
                segment.text = final_event.text;
                segment.start_secs = final_event.start_secs;
                segment.end_secs = Some(final_event.end_secs);
                segment.is_final = true;
                segment.updated_at_ms = now;
                segment.source_language = final_event.language.clone();
                if changed {
                    segment.revision = segment.revision.saturating_add(1);
                    if segment.translation.is_some() {
                        segment.translation = Some(stale_translation(segment));
                        patches.push(SubtitlePatch::Translation {
                            id: id.clone(),
                            translation: segment.translation.clone(),
                        });
                    }
                }
                segment.last_change = Instant::now();
                segment.clone()
            }
            None => {
                let segment = SubtitleSegment {
                    id: id.clone(),
                    session_id: self.session_id.clone(),
                    record_id: self.record_id.clone(),
                    source_run_id: self.source_run_id.clone(),
                    device_id: self.device_id.clone(),
                    device_name: self.device_name.clone(),
                    local_id: final_event.local_id,
                    start_secs: final_event.start_secs,
                    end_secs: Some(final_event.end_secs),
                    device_time_secs: self.device_time(final_event.start_secs),
                    occurrence_ms: self.occurrence_ms(final_event.start_secs),
                    text: final_event.text,
                    revision: 1,
                    is_final: true,
                    speaker: None,
                    speaker_revision: 0,
                    translation: None,
                    source_language: final_event.language,
                    created_at_ms: now,
                    updated_at_ms: now,
                    last_change: Instant::now(),
                };
                self.insert(id, segment.clone());
                segment
            }
        };
        patches.push(SubtitlePatch::Finalize { segment });
        patches
    }

    /// Merges a speaker result into a segment.
    ///
    /// Two independent paths produce speaker labels: a per-utterance embedding
    /// and a windowed diarization. Diarization knows which voices overlapped
    /// and covers a whole window, so it corrects the preliminary embedding
    /// label. A late embedding must therefore not erase a diarization label or
    /// its overlap marker; the outcome may not depend on arrival order.
    pub fn set_speaker(&mut self, id: &str, speaker: SpeakerInfo) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        if let Some(existing) = &segment.speaker {
            if existing.source == SPEAKER_SOURCE_DIARIZATION
                && speaker.source != SPEAKER_SOURCE_DIARIZATION
            {
                return None;
            }
            if *existing == speaker {
                return None;
            }
        }
        segment.speaker_revision = segment.speaker_revision.saturating_add(1);
        segment.speaker = Some(speaker.clone());
        segment.updated_at_ms = now_ms();
        let revision = segment.speaker_revision;
        Some(SubtitlePatch::Speaker {
            id: id.to_owned(),
            speaker: Some(speaker),
            revision,
        })
    }

    pub fn mark_translation_pending(
        &mut self,
        id: &str,
        target_language: &str,
        engine: &str,
        mode: &str,
        model_id: &str,
    ) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        let revision = segment.revision;
        let state = TranslationState {
            target_language: target_language.to_owned(),
            text: None,
            status: TranslationStatus::Pending,
            revision,
            engine: engine.to_owned(),
            mode: mode.to_owned(),
            model_id: model_id.to_owned(),
            error: None,
            updated_at_ms: now_ms(),
        };
        segment.translation = Some(state.clone());
        Some(SubtitlePatch::Translation {
            id: id.to_owned(),
            translation: Some(state),
        })
    }

    pub fn set_translation(
        &mut self,
        id: &str,
        text_revision: u32,
        result: Result<String, String>,
        target_language: &str,
        engine: &str,
        model_id: &str,
    ) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        if segment.revision != text_revision {
            return None;
        }
        let state = match result {
            Ok(text) => TranslationState {
                target_language: target_language.to_owned(),
                text: Some(text),
                status: TranslationStatus::Ready,
                revision: text_revision,
                engine: engine.to_owned(),
                mode: "text".to_owned(),
                model_id: model_id.to_owned(),
                error: None,
                updated_at_ms: now_ms(),
            },
            Err(error) => TranslationState {
                target_language: target_language.to_owned(),
                text: None,
                status: TranslationStatus::Error,
                revision: text_revision,
                engine: engine.to_owned(),
                mode: "text".to_owned(),
                model_id: model_id.to_owned(),
                error: Some(error),
                updated_at_ms: now_ms(),
            },
        };
        segment.translation = Some(state.clone());
        Some(SubtitlePatch::Translation {
            id: id.to_owned(),
            translation: Some(state),
        })
    }

    pub fn set_translation_error(
        &mut self,
        id: &str,
        text_revision: u32,
        target_language: &str,
        engine: &str,
        model_id: &str,
        error: String,
    ) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        if segment.revision != text_revision {
            return None;
        }
        let state = TranslationState {
            target_language: target_language.to_owned(),
            text: None,
            status: TranslationStatus::Error,
            revision: text_revision,
            engine: engine.to_owned(),
            mode: "text".to_owned(),
            model_id: model_id.to_owned(),
            error: Some(error),
            updated_at_ms: now_ms(),
        };
        segment.translation = Some(state.clone());
        Some(SubtitlePatch::Translation {
            id: id.to_owned(),
            translation: Some(state),
        })
    }

    pub fn set_translation_unavailable(
        &mut self,
        id: &str,
        reason: String,
        target_language: &str,
        engine: &str,
    ) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        let revision = segment.revision;
        let state = TranslationState {
            target_language: target_language.to_owned(),
            text: None,
            status: TranslationStatus::Unavailable,
            revision,
            engine: engine.to_owned(),
            mode: "text".to_owned(),
            model_id: String::new(),
            error: Some(reason),
            updated_at_ms: now_ms(),
        };
        segment.translation = Some(state.clone());
        Some(SubtitlePatch::Translation {
            id: id.to_owned(),
            translation: Some(state),
        })
    }

    pub fn mark_same_language(
        &mut self,
        id: &str,
        language: &str,
        engine: &str,
    ) -> Option<SubtitlePatch> {
        let segment = self.segments.get_mut(id)?;
        let revision = segment.revision;
        let state = TranslationState {
            target_language: language.to_owned(),
            text: None,
            status: TranslationStatus::NotNeeded,
            revision,
            engine: engine.to_owned(),
            mode: "text".to_owned(),
            model_id: String::new(),
            error: None,
            updated_at_ms: now_ms(),
        };
        segment.translation = Some(state.clone());
        Some(SubtitlePatch::Translation {
            id: id.to_owned(),
            translation: Some(state),
        })
    }

    pub fn stable_candidates(&self, wait_ms: u64) -> Vec<StableCandidate> {
        let wait = std::time::Duration::from_millis(wait_ms);
        self.order
            .iter()
            .filter_map(|id| self.segments.get(id))
            .filter(|segment| !segment.is_final)
            .filter(|segment| !segment.text.trim().is_empty())
            .filter(|segment| segment.last_change.elapsed() >= wait)
            .filter(|segment| match &segment.translation {
                None => true,
                Some(translation) => translation.status == TranslationStatus::Stale,
            })
            .map(|segment| StableCandidate {
                segment_id: segment.id.clone(),
                text: segment.text.clone(),
                text_revision: segment.revision,
                source_language: segment.source_language.clone(),
            })
            .collect()
    }

    pub fn segment(&self, id: &str) -> Option<&SubtitleSegment> {
        self.segments.get(id)
    }

    pub fn needs_translation(&self, id: &str, target: &str) -> bool {
        let Some(segment) = self.segments.get(id) else {
            return false;
        };
        match &segment.translation {
            Some(translation) => {
                translation.target_language != target
                    || translation.revision != segment.revision
                    || matches!(
                        translation.status,
                        TranslationStatus::Stale | TranslationStatus::Error
                    )
            }
            None => true,
        }
    }

    pub fn snapshot(&self) -> Vec<SubtitleSegment> {
        self.order
            .iter()
            .filter_map(|id| self.segments.get(id))
            .filter(|segment| !segment.text.trim().is_empty())
            .cloned()
            .collect()
    }

    pub fn history_limit(&self) -> usize {
        self.history_limit
    }

    /// Applies a new history limit and drops the subtitles that no longer
    /// fit, so the backend store and the frontend cache agree.
    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit.max(50);
        self.trim();
    }

    pub fn is_stale(&self, request: &TranslationRequest, current_config_revision: u64) -> bool {
        if request.config_revision != current_config_revision {
            return true;
        }
        if request.session_id != self.session_id {
            return true;
        }
        match self.segments.get(&request.segment_id) {
            Some(segment) => segment.revision != request.text_revision,
            None => true,
        }
    }

    pub fn config_revision(&self, request: &TranslationRequest) -> u64 {
        request.config_revision
    }

    fn segment_id(&self, local_id: u64) -> String {
        let scope = if self.source_run_id.is_empty() {
            &self.session_id
        } else {
            &self.source_run_id
        };
        format!("{scope}:{local_id}")
    }

    fn device_time(&self, start_secs: f64) -> f64 {
        self.elapsed_base_ms as f64 / 1_000.0 + start_secs
    }

    fn occurrence_ms(&self, start_secs: f64) -> u64 {
        self.occurrence_base_ms
            .saturating_add((start_secs.max(0.0) * 1_000.0) as u64)
    }

    fn insert(&mut self, id: String, segment: SubtitleSegment) {
        self.order.push_back(id.clone());
        self.segments.insert(id, segment);
        self.trim();
    }

    fn trim(&mut self) {
        while self.order.len() > self.history_limit {
            if let Some(removed) = self.order.pop_front() {
                self.segments.remove(&removed);
            }
        }
    }
}

fn stale_translation(segment: &SubtitleSegment) -> TranslationState {
    match &segment.translation {
        Some(translation) => TranslationState {
            text: None,
            status: TranslationStatus::Stale,
            revision: segment.revision,
            updated_at_ms: now_ms(),
            ..translation.clone()
        },
        None => TranslationState {
            target_language: String::new(),
            text: None,
            status: TranslationStatus::Stale,
            revision: segment.revision,
            engine: String::new(),
            mode: "text".to_owned(),
            model_id: String::new(),
            error: None,
            updated_at_ms: now_ms(),
        },
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speaker::{SPEAKER_SOURCE_DIARIZATION, SPEAKER_SOURCE_EMBEDDING};

    fn partial(local_id: u64, text: &str, revision: u32) -> AsrPartial {
        AsrPartial {
            local_id,
            revision,
            text: text.to_owned(),
            start_secs: 1.0,
            end_secs: 2.0,
        }
    }

    #[test]
    fn partial_updates_reuse_the_same_subtitle_id() {
        let mut store = SubtitleStore::new("s".to_owned(), 100);
        let first = store.apply_partial(partial(1, "hello", 1));
        let second = store.apply_partial(partial(1, "hello world", 2));
        let SubtitlePatch::Upsert { segment } = &second[0] else {
            panic!("expected upsert");
        };
        assert_eq!(segment.id, "s:1");
        assert_eq!(segment.revision, 2);
        assert_eq!(store.snapshot().len(), 1);
        assert!(matches!(first[0], SubtitlePatch::Upsert { .. }));
    }

    #[test]
    fn a_stale_translation_is_rejected_after_a_text_revision() {
        let mut store = SubtitleStore::new("s".to_owned(), 100);
        store.apply_partial(partial(1, "hello", 1));
        let request = TranslationRequest {
            session_id: "s".to_owned(),
            segment_id: "s:1".to_owned(),
            text_revision: 1,
            config_revision: 1,
            text: "hello".to_owned(),
            source_language: "en".to_owned(),
            target_language: "zh-Hans".to_owned(),
            model_id: "model".to_owned(),
        };
        store.apply_partial(partial(1, "hello there", 2));
        assert!(store.is_stale(&request, 1));
        assert!(store
            .set_translation(
                "s:1",
                request.text_revision,
                Ok("你好".to_owned()),
                "zh-Hans",
                "bergamot",
                "model"
            )
            .is_none());
    }

    #[test]
    fn history_is_bounded() {
        let mut store = SubtitleStore::new("s".to_owned(), 50);
        for id in 0..60 {
            store.apply_partial(partial(id, "hello", 1));
        }
        assert_eq!(store.snapshot().len(), 50);
    }

    fn speaker(source: &str, overlap: bool) -> SpeakerInfo {
        SpeakerInfo {
            id: "S1".to_owned(),
            index: 1,
            confidence: Some(0.7),
            uncertain: false,
            overlap,
            source: source.to_owned(),
        }
    }

    #[test]
    fn a_late_embedding_does_not_erase_a_diarization_label() {
        let mut store = SubtitleStore::new("s".to_owned(), 50);
        store.apply_partial(partial(1, "hello", 1));
        store.set_speaker("s:1", speaker(SPEAKER_SOURCE_DIARIZATION, true));

        let patch = store.set_speaker("s:1", speaker(SPEAKER_SOURCE_EMBEDDING, false));
        assert!(patch.is_none());
        let segment = store.segment("s:1").expect("segment exists");
        assert_eq!(
            segment
                .speaker
                .as_ref()
                .map(|speaker| speaker.source.as_str()),
            Some(SPEAKER_SOURCE_DIARIZATION)
        );
        assert!(segment
            .speaker
            .as_ref()
            .is_some_and(|speaker| speaker.overlap));
    }

    #[test]
    fn a_diarization_label_replaces_an_earlier_embedding_label() {
        let mut store = SubtitleStore::new("s".to_owned(), 50);
        store.apply_partial(partial(1, "hello", 1));
        store.set_speaker("s:1", speaker(SPEAKER_SOURCE_EMBEDDING, false));

        let patch = store.set_speaker("s:1", speaker(SPEAKER_SOURCE_DIARIZATION, true));
        assert!(patch.is_some());
        let segment = store.segment("s:1").expect("segment exists");
        assert_eq!(
            segment
                .speaker
                .as_ref()
                .map(|speaker| speaker.source.as_str()),
            Some(SPEAKER_SOURCE_DIARIZATION)
        );
        assert_eq!(segment.speaker_revision, 2);
    }
}
