use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub started_at_ms: u64,
    pub audio_frames: u64,
    pub dropped_frames: u64,
    pub partial_updates: u64,
    pub finalized_subtitles: u64,
    pub translation_requests: u64,
    pub translation_failures: u64,
    pub speaker_updates: u64,
    pub diarization_windows: u64,
    pub audio_peak: f32,
    pub audio_rms: f32,
    pub last_subtitle_latency_ms: u64,
    pub average_subtitle_latency_ms: u64,
    pub last_translation_latency_ms: u64,
    pub average_translation_latency_ms: u64,
    pub last_speaker_latency_ms: u64,
    pub last_diarization_latency_ms: u64,
    pub cpu_percent: f32,
    pub rss_mb: f64,
}

pub struct SessionMetrics {
    stats: SessionStats,
    subtitle_latency_total: u64,
    subtitle_latency_count: u64,
    translation_latency_total: u64,
    translation_latency_count: u64,
}

impl SessionMetrics {
    pub fn new(started_at_ms: u64) -> Self {
        Self {
            stats: SessionStats {
                started_at_ms,
                ..SessionStats::default()
            },
            subtitle_latency_total: 0,
            subtitle_latency_count: 0,
            translation_latency_total: 0,
            translation_latency_count: 0,
        }
    }

    pub fn snapshot(&self) -> SessionStats {
        self.stats.clone()
    }

    pub fn add_audio(&mut self, frames: u64, dropped: u64) {
        self.stats.audio_frames = self.stats.audio_frames.saturating_add(frames);
        self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(dropped);
    }

    pub fn set_level(&mut self, peak: f32, rms: f32) {
        self.stats.audio_peak = peak;
        self.stats.audio_rms = rms;
    }

    pub fn partial(&mut self) {
        self.stats.partial_updates = self.stats.partial_updates.saturating_add(1);
    }

    pub fn final_subtitle(&mut self, latency_ms: u64) {
        self.stats.finalized_subtitles = self.stats.finalized_subtitles.saturating_add(1);
        self.stats.last_subtitle_latency_ms = latency_ms;
        self.subtitle_latency_total = self.subtitle_latency_total.saturating_add(latency_ms);
        self.subtitle_latency_count = self.subtitle_latency_count.saturating_add(1);
        self.stats.average_subtitle_latency_ms =
            self.subtitle_latency_total / self.subtitle_latency_count;
    }

    pub fn translation(&mut self, latency_ms: u64, failed: bool) {
        self.stats.translation_requests = self.stats.translation_requests.saturating_add(1);
        if failed {
            self.stats.translation_failures = self.stats.translation_failures.saturating_add(1);
        }
        self.stats.last_translation_latency_ms = latency_ms;
        self.translation_latency_total = self.translation_latency_total.saturating_add(latency_ms);
        self.translation_latency_count = self.translation_latency_count.saturating_add(1);
        self.stats.average_translation_latency_ms =
            self.translation_latency_total / self.translation_latency_count;
    }

    pub fn speaker(&mut self, latency_ms: u64) {
        self.stats.speaker_updates = self.stats.speaker_updates.saturating_add(1);
        self.stats.last_speaker_latency_ms = latency_ms;
    }

    pub fn diarization(&mut self, latency_ms: u64) {
        self.stats.diarization_windows = self.stats.diarization_windows.saturating_add(1);
        self.stats.last_diarization_latency_ms = latency_ms;
    }

    pub fn set_resources(&mut self, cpu_percent: f32, rss_mb: f64) {
        self.stats.cpu_percent = cpu_percent;
        self.stats.rss_mb = rss_mb;
    }
}
