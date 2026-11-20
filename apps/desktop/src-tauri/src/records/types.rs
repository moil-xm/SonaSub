use serde::{Deserialize, Serialize};

use crate::{
    audio::{AudioDeviceRef, AudioSourceKind},
    config::AppConfig,
    subtitles::{SubtitlePatch, SubtitleSegment},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordState {
    NotStarted,
    Starting,
    Running,
    Paused,
    WaitingForDevice,
    Failed,
    Ended,
}

impl RecordState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "notStarted",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::WaitingForDevice => "waitingForDevice",
            Self::Failed => "failed",
            Self::Ended => "ended",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "starting" => Self::Starting,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "waitingForDevice" => Self::WaitingForDevice,
            "failed" => Self::Failed,
            "ended" => Self::Ended,
            _ => Self::NotStarted,
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running | Self::WaitingForDevice
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceRunState {
    Starting,
    Running,
    Disconnected,
    Failed,
    Ended,
}

impl SourceRunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
            Self::Ended => "ended",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "disconnected" => Self::Disconnected,
            "failed" => Self::Failed,
            "ended" => Self::Ended,
            _ => Self::Starting,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceEndReason {
    RecordPaused,
    RecordStopped,
    DeviceRemoved,
    DeviceLost,
    PipelineFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSummary {
    pub id: String,
    pub name: String,
    pub state: RecordState,
    pub devices: Vec<AudioDeviceRef>,
    pub active_devices: Vec<AudioDeviceRef>,
    pub has_active_source: bool,
    pub last_error: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordListItem {
    pub id: String,
    pub name: String,
    pub has_active_source: bool,
}

impl From<&RecordSummary> for RecordListItem {
    fn from(summary: &RecordSummary) -> Self {
        Self {
            id: summary.id.clone(),
            name: summary.name.clone(),
            has_active_source: summary.has_active_source,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSource {
    pub source_run_id: String,
    pub record_id: String,
    pub source_kind: AudioSourceKind,
    pub device_id: String,
    pub device_name: String,
    pub state: SourceRunState,
    pub elapsed_base_ms: u64,
    pub elapsed_ms: u64,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub error: Option<String>,
}

impl RecordSource {
    pub fn device_ref(&self) -> AudioDeviceRef {
        AudioDeviceRef::new(self.source_kind, self.device_id.clone())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordStats {
    pub started_at_ms: u64,
    pub active_sources: usize,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordNotice {
    pub level: String,
    pub message: String,
    pub at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSnapshot {
    pub summary: RecordSummary,
    pub config: AppConfig,
    pub sources: Vec<RecordSource>,
    pub segments: Vec<SubtitleSegment>,
    pub stats: RecordStats,
    pub notices: Vec<RecordNotice>,
    pub summary_revision: u64,
    pub subtitle_revision: u64,
    pub stats_revision: u64,
    pub total_subtitle_count: u64,
    pub history_limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordInput {
    pub name: String,
    pub config: AppConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSummaryEvent {
    pub record_id: String,
    pub revision: u64,
    pub summary: RecordSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSubtitleBatch {
    pub record_id: String,
    pub source_run_id: String,
    pub revision: u64,
    pub patches: Vec<SubtitlePatch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordStatsEvent {
    pub record_id: String,
    pub revision: u64,
    pub stats: RecordStats,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordNoticeEvent {
    pub record_id: String,
    pub notice: RecordNotice,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordExportSummary {
    pub directory: String,
    pub audio_files: usize,
    pub original_lrc_files: usize,
    pub translation_lrc_files: usize,
    pub audio_disabled: bool,
    pub missing_audio_runs: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDevice {
    pub kind: AudioSourceKind,
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub capture_supported: bool,
    pub capture_reason: Option<String>,
    pub is_available: bool,
    pub occupied_record_id: Option<String>,
    pub occupied_record_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredRecord {
    pub summary: RecordSummary,
    pub config: AppConfig,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
}
