use serde::Serialize;

use crate::subtitles::SubtitleSegment;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SessionStatus {
    Idle,
    Starting { session_id: String, stage: String },
    Running { info: Box<SessionInfo> },
    Stopping { session_id: String, stage: String },
    Failed { error: String },
}

impl SessionStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Starting { .. })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub started_at_ms: u64,
    pub audio_backend: String,
    pub audio_device: Option<String>,
    pub models_dir: String,
    pub asr: AsrInfo,
    pub translation: TranslationInfo,
    pub speakers: SpeakerInfoStatus,
    pub history_limit: usize,
    pub run_boundary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrInfo {
    pub model_id: String,
    pub model_label: String,
    pub mode: String,
    pub source_language: String,
    pub max_segment_secs: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationInfo {
    pub engine: String,
    pub runtime: String,
    pub model_family: String,
    pub model_id: String,
    pub model_label: String,
    pub mode: String,
    pub target_language: String,
    pub trigger: String,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerInfoStatus {
    pub enabled: bool,
    pub diarization: bool,
    pub embedding_model: String,
    pub diarization_window_secs: f32,
}

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotice {
    pub level: String,
    pub message: String,
    pub at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    /// Monotonic counter of every state change the backend applied.
    pub revision: u64,
    /// The session the current state belongs to. `None` before the first
    /// session started.
    pub session_id: Option<String>,
    pub status: SessionStatus,
    /// Revision of the status this snapshot carries.
    pub status_revision: u64,
    /// Revision of the subtitle list this snapshot carries.
    pub subtitle_revision: u64,
    /// Revision of the statistics this snapshot carries.
    pub stats_revision: u64,
    pub segments: Vec<SubtitleSegment>,
    pub stats: SessionStats,
    pub notices: Vec<SessionNotice>,
    pub history_limit: usize,
}

/// A status change published to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub revision: u64,
    pub status: SessionStatus,
}

/// A statistics update published to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsEvent {
    pub revision: u64,
    pub stats: SessionStats,
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
