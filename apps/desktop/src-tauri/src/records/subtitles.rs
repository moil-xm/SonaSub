use std::collections::{BTreeMap, HashMap};

use crate::{
    config::MAX_HISTORY_LIMIT,
    subtitles::{SubtitlePatch, SubtitleSegment},
};

#[derive(Default)]
pub struct RecordSubtitleStore {
    segments: HashMap<String, SubtitleSegment>,
    order: BTreeMap<(u64, String), String>,
    history_limit: usize,
}

impl RecordSubtitleStore {
    pub fn new(history_limit: usize) -> Self {
        Self {
            history_limit: history_limit.clamp(50, MAX_HISTORY_LIMIT),
            ..Self::default()
        }
    }

    pub fn replace(&mut self, segments: Vec<SubtitleSegment>) {
        self.segments.clear();
        self.order.clear();
        for segment in segments {
            self.insert(segment);
        }
        self.trim();
    }

    pub fn apply(&mut self, patches: Vec<SubtitlePatch>) -> Vec<SubtitleSegment> {
        let mut changed = Vec::new();
        for patch in patches {
            match patch {
                SubtitlePatch::Upsert { segment } | SubtitlePatch::Finalize { segment } => {
                    let id = segment.id.clone();
                    let Some(existing) = self.segments.get(&id) else {
                        self.insert(segment.clone());
                        changed.push(segment);
                        continue;
                    };
                    if segment.revision < existing.revision {
                        continue;
                    }
                    let mut merged = segment;
                    if merged.speaker_revision < existing.speaker_revision {
                        merged.speaker = existing.speaker.clone();
                        merged.speaker_revision = existing.speaker_revision;
                    }
                    if merged.occurrence_ms != existing.occurrence_ms {
                        self.order
                            .remove(&(existing.occurrence_ms, existing.id.clone()));
                    }
                    self.order
                        .insert((merged.occurrence_ms, merged.id.clone()), merged.id.clone());
                    self.segments.insert(id, merged.clone());
                    changed.push(merged);
                }
                SubtitlePatch::Speaker {
                    id,
                    speaker,
                    revision,
                } => {
                    let Some(segment) = self.segments.get_mut(&id) else {
                        continue;
                    };
                    if revision <= segment.speaker_revision {
                        continue;
                    }
                    segment.speaker = speaker;
                    segment.speaker_revision = revision;
                    segment.updated_at_ms = now_ms();
                    changed.push(segment.clone());
                }
                SubtitlePatch::Translation { id, translation } => {
                    let Some(segment) = self.segments.get_mut(&id) else {
                        continue;
                    };
                    if let Some(translation) = &translation {
                        if translation.revision != segment.revision {
                            continue;
                        }
                    }
                    segment.translation = translation;
                    segment.updated_at_ms = now_ms();
                    changed.push(segment.clone());
                }
            }
        }
        self.trim();
        changed
    }

    pub fn snapshot(&self) -> Vec<SubtitleSegment> {
        self.order
            .values()
            .filter_map(|id| self.segments.get(id))
            .cloned()
            .collect()
    }

    pub fn set_history_limit(&mut self, history_limit: usize) {
        self.history_limit = history_limit.clamp(50, MAX_HISTORY_LIMIT);
        self.trim();
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    fn insert(&mut self, segment: SubtitleSegment) {
        if let Some(existing) = self.segments.get(&segment.id) {
            self.order
                .remove(&(existing.occurrence_ms, existing.id.clone()));
        }
        self.order.insert(
            (segment.occurrence_ms, segment.id.clone()),
            segment.id.clone(),
        );
        self.segments.insert(segment.id.clone(), segment);
    }

    fn trim(&mut self) {
        while self.segments.len() > self.history_limit {
            let Some((_, id)) = self.order.pop_first() else {
                break;
            };
            self.segments.remove(&id);
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::{TranslationState, TranslationStatus};

    fn segment(id: &str, occurrence_ms: u64, revision: u32) -> SubtitleSegment {
        SubtitleSegment {
            id: id.to_owned(),
            session_id: "source".to_owned(),
            record_id: "record".to_owned(),
            source_run_id: "source".to_owned(),
            source_kind: crate::audio::AudioSourceKind::SystemOutput,
            device_id: "device".to_owned(),
            device_name: "Device".to_owned(),
            local_id: 1,
            start_secs: 1.0,
            end_secs: Some(2.0),
            device_time_secs: 3.0,
            occurrence_ms,
            text: format!("caption {revision}"),
            revision,
            is_final: true,
            speaker: None,
            speaker_revision: 0,
            translation: None,
            source_language: Some("en".to_owned()),
            created_at_ms: 1,
            updated_at_ms: 1,
            last_change: std::time::Instant::now(),
        }
    }

    #[test]
    fn merges_sources_by_occurrence_time_instead_of_arrival_order() {
        let mut store = RecordSubtitleStore::new(50);
        store.apply(vec![
            SubtitlePatch::Finalize {
                segment: segment("source-b:1", 20_000, 1),
            },
            SubtitlePatch::Finalize {
                segment: segment("source-a:1", 10_000, 1),
            },
        ]);
        let snapshot = store.snapshot();
        assert_eq!(snapshot[0].id, "source-a:1");
        assert_eq!(snapshot[1].id, "source-b:1");
    }

    #[test]
    fn caps_the_live_list_at_one_thousand_segments() {
        let mut store = RecordSubtitleStore::new(5_000);
        for index in 0..1_005 {
            store.apply(vec![SubtitlePatch::Finalize {
                segment: segment(&format!("source:{index}"), index as u64, 1),
            }]);
        }

        let snapshot = store.snapshot();
        assert_eq!(snapshot.len(), MAX_HISTORY_LIMIT);
        assert_eq!(snapshot[0].id, "source:5");
        assert_eq!(
            snapshot.last().map(|segment| segment.id.as_str()),
            Some("source:1004")
        );
    }

    #[test]
    fn ignores_a_late_translation_for_an_older_text_revision() {
        let mut store = RecordSubtitleStore::new(50);
        store.apply(vec![SubtitlePatch::Finalize {
            segment: segment("source:1", 10_000, 2),
        }]);
        store.apply(vec![SubtitlePatch::Translation {
            id: "source:1".to_owned(),
            translation: Some(TranslationState {
                target_language: "zh-Hans".to_owned(),
                text: Some("旧".to_owned()),
                status: TranslationStatus::Ready,
                revision: 1,
                engine: "bergamot".to_owned(),
                mode: "text".to_owned(),
                model_id: "model".to_owned(),
                error: None,
                updated_at_ms: 2,
            }),
        }]);
        assert!(store.snapshot()[0].translation.is_none());
    }
}
