use super::{DiarizationWindow, SpeakerInfo, SPEAKER_SOURCE_DIARIZATION};

impl DiarizationWindow {
    pub(crate) fn speaker_for_segment(
        &self,
        start_secs: f64,
        end_secs: Option<f64>,
    ) -> Option<SpeakerInfo> {
        let end = end_secs.unwrap_or(self.window_end);
        let mut durations: Vec<(String, f64, Option<f32>, bool)> = Vec::new();
        for interval in &self.intervals {
            let overlap = end.min(interval.end_secs) - start_secs.max(interval.start_secs);
            if overlap < 0.2 {
                continue;
            }
            let Some(id) = &interval.speaker_id else {
                continue;
            };
            if let Some(existing) = durations.iter_mut().find(|(speaker, ..)| speaker == id) {
                existing.1 += overlap;
                if interval.confidence.unwrap_or(0.0) > existing.2.unwrap_or(0.0) {
                    existing.2 = interval.confidence;
                }
                existing.3 |= interval.uncertain;
            } else {
                durations.push((id.clone(), overlap, interval.confidence, interval.uncertain));
            }
        }
        durations.sort_by(|left, right| right.1.total_cmp(&left.1));
        let (id, _, confidence, uncertain_cluster) = durations.first()?;
        let index = id
            .strip_prefix('S')
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let overlap = self
            .overlap_regions
            .iter()
            .any(|(start, finish)| end.min(*finish) - start_secs.max(*start) > 0.15);
        Some(SpeakerInfo {
            id: id.clone(),
            index,
            confidence: *confidence,
            uncertain: *uncertain_cluster || durations.len() > 1,
            overlap,
            source: SPEAKER_SOURCE_DIARIZATION.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speaker::DiarizationInterval;

    fn interval(id: &str, start_secs: f64, end_secs: f64, confidence: f32) -> DiarizationInterval {
        DiarizationInterval {
            start_secs,
            end_secs,
            local_speaker: 0,
            speaker_id: Some(id.to_owned()),
            confidence: Some(confidence),
            uncertain: false,
        }
    }

    #[test]
    fn attributes_by_total_duration_and_preserves_overlap() {
        let window = DiarizationWindow {
            window_start: 0.0,
            window_end: 2.0,
            intervals: vec![
                interval("S2", 0.0, 0.9, 0.95),
                interval("S1", 0.6, 1.2, 0.6),
                interval("S1", 1.2, 2.0, 0.8),
            ],
            overlap_regions: vec![(0.6, 0.9)],
            latency_ms: 0,
        };
        let speaker = window.speaker_for_segment(0.0, None).unwrap();
        assert_eq!(speaker.id, "S1");
        assert_eq!(speaker.index, 1);
        assert_eq!(speaker.confidence, Some(0.8));
        assert!(speaker.uncertain);
        assert!(speaker.overlap);
        assert_eq!(speaker.source, SPEAKER_SOURCE_DIARIZATION);

        let speaker = window.speaker_for_segment(0.0, Some(0.5)).unwrap();
        assert_eq!(speaker.id, "S2");
        assert!(!speaker.uncertain);
        assert!(!speaker.overlap);
        assert!(window.speaker_for_segment(1.9, None).is_none());
    }
}
