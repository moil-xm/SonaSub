use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::Serialize;
use sherpa_rs::{
    diarize::{Diarize, DiarizeConfig},
    speaker_id::{EmbeddingExtractor, ExtractorConfig},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerInfo {
    pub id: String,
    pub index: usize,
    pub confidence: Option<f32>,
    pub uncertain: bool,
    pub overlap: bool,
    pub source: String,
}

#[derive(Debug, Clone)]
struct SpeakerProfile {
    id: String,
    index: usize,
    centroid: Vec<f32>,
    observations: u32,
}

pub struct SpeakerRegistry {
    profiles: Vec<SpeakerProfile>,
    max_speakers: usize,
    threshold: f32,
}

impl SpeakerRegistry {
    pub fn new(max_speakers: usize, threshold: f32) -> Self {
        Self {
            profiles: Vec::new(),
            max_speakers: max_speakers.max(1),
            threshold,
        }
    }

    pub fn identify(
        &mut self,
        embedding: &[f32],
        allow_create: bool,
    ) -> (Option<SpeakerInfo>, bool) {
        let embedding = normalize(embedding);
        let mut best: Option<(usize, f32)> = None;
        for (index, profile) in self.profiles.iter().enumerate() {
            let score = cosine(&embedding, &profile.centroid);
            if best.map(|(_, current)| score > current).unwrap_or(true) {
                best = Some((index, score));
            }
        }
        if let Some((index, score)) = best {
            if score >= self.threshold {
                let profile = &mut self.profiles[index];
                let observations = profile.observations as f32;
                for (centroid, value) in profile.centroid.iter_mut().zip(embedding.iter()) {
                    *centroid = (*centroid * observations + value) / (observations + 1.0);
                }
                profile.centroid = normalize(&profile.centroid);
                profile.observations = profile.observations.saturating_add(1);
                let id = profile.id.clone();
                let speaker_index = profile.index;
                return (
                    Some(SpeakerInfo {
                        id,
                        index: speaker_index,
                        confidence: Some(score.clamp(0.0, 1.0)),
                        uncertain: false,
                        overlap: false,
                        source: "embedding".to_owned(),
                    }),
                    false,
                );
            }
        }
        if allow_create && self.profiles.len() < self.max_speakers {
            let index = self.profiles.len() + 1;
            let id = format!("S{index}");
            self.profiles.push(SpeakerProfile {
                id: id.clone(),
                index,
                centroid: embedding,
                observations: 1,
            });
            return (
                Some(SpeakerInfo {
                    id,
                    index,
                    confidence: None,
                    uncertain: true,
                    overlap: false,
                    source: "embedding".to_owned(),
                }),
                true,
            );
        }
        (None, false)
    }

    pub fn best_match(&self, embedding: &[f32]) -> Option<SpeakerInfo> {
        let embedding = normalize(embedding);
        let mut best: Option<(usize, f32)> = None;
        for (index, profile) in self.profiles.iter().enumerate() {
            let score = cosine(&embedding, &profile.centroid);
            if best.map(|(_, current)| score > current).unwrap_or(true) {
                best = Some((index, score));
            }
        }
        if let Some((index, score)) = best {
            if score >= self.threshold {
                let profile = &self.profiles[index];
                return Some(SpeakerInfo {
                    id: profile.id.clone(),
                    index: profile.index,
                    confidence: Some(score.clamp(0.0, 1.0)),
                    uncertain: false,
                    overlap: false,
                    source: "diarization".to_owned(),
                });
            }
        }
        None
    }
}

pub struct SpeakerEngine {
    extractor: EmbeddingExtractor,
}

impl SpeakerEngine {
    pub fn new(model: &Path) -> Result<Self, String> {
        let extractor = EmbeddingExtractor::new(ExtractorConfig {
            model: model.display().to_string(),
            provider: Some("cpu".to_owned()),
            num_threads: Some(2),
            debug: false,
        })
        .map_err(|error| format!("Failed to load the speaker embedding model: {error}"))?;
        Ok(Self { extractor })
    }

    pub fn embedding(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        if samples.len() < (0.25 * 16_000.0) as usize {
            return Err("The speech segment is too short for speaker analysis.".to_owned());
        }
        self.extractor
            .compute_speaker_embedding(samples.to_vec(), 16_000)
            .map_err(|error| format!("Speaker embedding failed: {error}"))
    }
}

#[derive(Debug, Clone)]
pub struct DiarizationInterval {
    pub start_secs: f64,
    pub end_secs: f64,
    pub local_speaker: i32,
    pub speaker_id: Option<String>,
    pub confidence: Option<f32>,
    pub uncertain: bool,
}

#[derive(Debug, Clone)]
pub struct DiarizationWindow {
    pub window_start: f64,
    pub window_end: f64,
    pub intervals: Vec<DiarizationInterval>,
    pub overlap_regions: Vec<(f64, f64)>,
    pub latency_ms: u64,
}

pub struct DiarizationEngine {
    diarize: Diarize,
    extractor: EmbeddingExtractor,
}

impl DiarizationEngine {
    pub fn new(segmentation_model: &Path, embedding_model: &Path) -> Result<Self, String> {
        let diarize = Diarize::new(
            segmentation_model,
            embedding_model,
            DiarizeConfig {
                num_clusters: Some(-1),
                threshold: Some(0.5),
                min_duration_on: Some(0.25),
                min_duration_off: Some(0.25),
                provider: Some("cpu".to_owned()),
                debug: false,
            },
        )
        .map_err(|error| format!("Failed to load the diarization models: {error}"))?;
        let extractor = EmbeddingExtractor::new(ExtractorConfig {
            model: embedding_model.display().to_string(),
            provider: Some("cpu".to_owned()),
            num_threads: Some(1),
            debug: false,
        })
        .map_err(|error| format!("Failed to load the diarization embedding model: {error}"))?;
        Ok(Self { diarize, extractor })
    }

    pub fn analyze(
        &mut self,
        window_start: f64,
        samples: Arc<Vec<f32>>,
        registry: &Arc<Mutex<SpeakerRegistry>>,
    ) -> Result<DiarizationWindow, String> {
        let started = Instant::now();
        let segments = match self.diarize.compute((*samples).clone(), None) {
            Ok(segments) => segments,
            Err(error) if error.to_string().contains("No segments") => Vec::new(),
            Err(error) => return Err(format!("Speaker diarization failed: {error}")),
        };
        let mut intervals = Vec::with_capacity(segments.len());
        let mut clusters: BTreeMap<i32, (Vec<f32>, usize)> = BTreeMap::new();
        for segment in segments {
            let start_offset = (segment.start as f64 * 16_000.0).max(0.0) as usize;
            let end_offset = (segment.end as f64 * 16_000.0).max(0.0) as usize;
            let end_offset = end_offset.min(samples.len());
            if end_offset <= start_offset {
                continue;
            }
            let embedding = self
                .extractor
                .compute_speaker_embedding(samples[start_offset..end_offset].to_vec(), 16_000)
                .map_err(|error| format!("Diarization embedding failed: {error}"))?;
            let cluster = clusters
                .entry(segment.speaker)
                .or_insert_with(|| (vec![0.0; embedding.len()], 0));
            for (sum, value) in cluster.0.iter_mut().zip(embedding.iter()) {
                *sum += value;
            }
            cluster.1 += 1;
            intervals.push(DiarizationInterval {
                start_secs: window_start + segment.start as f64,
                end_secs: window_start + segment.end as f64,
                local_speaker: segment.speaker,
                speaker_id: None,
                confidence: None,
                uncertain: true,
            });
        }
        let mappings = {
            let mut registry = registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            clusters
                .into_iter()
                .map(|(local_speaker, (sum, count))| {
                    let centroid = if count == 0 {
                        sum
                    } else {
                        sum.into_iter().map(|value| value / count as f32).collect()
                    };
                    let (speaker, _) = registry.identify(&centroid, true);
                    (local_speaker, speaker)
                })
                .collect::<BTreeMap<_, _>>()
        };
        for interval in &mut intervals {
            if let Some(Some(speaker)) = mappings.get(&interval.local_speaker) {
                interval.speaker_id = Some(speaker.id.clone());
                interval.confidence = speaker.confidence;
                interval.uncertain = speaker.uncertain;
            }
        }
        let overlap_regions = find_overlaps(&intervals);
        let window_end = window_start + samples.len() as f64 / 16_000.0;
        Ok(DiarizationWindow {
            window_start,
            window_end,
            intervals,
            overlap_regions,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn find_overlaps(intervals: &[DiarizationInterval]) -> Vec<(f64, f64)> {
    let mut overlaps = Vec::new();
    for (index, left) in intervals.iter().enumerate() {
        for right in intervals.iter().skip(index + 1) {
            if left.local_speaker == right.local_speaker {
                continue;
            }
            let start = left.start_secs.max(right.start_secs);
            let end = left.end_secs.min(right.end_secs);
            if end - start > 0.2 {
                overlaps.push((start, end));
            }
        }
    }
    overlaps
}

fn normalize(embedding: &[f32]) -> Vec<f32> {
    let norm = embedding
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm <= f32::EPSILON {
        return embedding.to_vec();
    }
    embedding.iter().map(|value| value / norm).collect()
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_reuses_a_matching_profile() {
        let mut registry = SpeakerRegistry::new(4, 0.6);
        let (first, created) = registry.identify(&[1.0, 0.0, 0.0], true);
        assert!(created);
        let first_id = first.unwrap().id;
        let (second, created) = registry.identify(&[0.99, 0.01, 0.0], true);
        assert!(!created);
        assert_eq!(second.unwrap().id, first_id);
    }

    #[test]
    fn registry_creates_a_second_profile_for_a_different_voice() {
        let mut registry = SpeakerRegistry::new(4, 0.6);
        registry.identify(&[1.0, 0.0, 0.0], true);
        let (speaker, created) = registry.identify(&[0.0, 1.0, 0.0], true);
        assert!(created);
        assert_eq!(speaker.unwrap().index, 2);
    }

    #[test]
    fn overlap_regions_ignore_same_speaker() {
        let intervals = vec![
            DiarizationInterval {
                start_secs: 0.0,
                end_secs: 2.0,
                local_speaker: 0,
                speaker_id: Some("S1".to_owned()),
                confidence: None,
                uncertain: false,
            },
            DiarizationInterval {
                start_secs: 1.0,
                end_secs: 3.0,
                local_speaker: 0,
                speaker_id: Some("S1".to_owned()),
                confidence: None,
                uncertain: false,
            },
        ];
        assert!(find_overlaps(&intervals).is_empty());
    }
}
