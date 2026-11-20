use serde::Serialize;

use crate::audio::CaptureInfo;
pub use crate::pipeline::metrics::{SessionMetrics, SessionStats};
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
    pub audio: CaptureInfo,
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
